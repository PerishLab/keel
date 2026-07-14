use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::{Attribute, Fields, Ident, ItemStruct, Meta, Path, Token, Type, parse_macro_input};

#[proc_macro_attribute]
pub fn resource(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemStruct);
    match expand(input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

fn expand(input: ItemStruct) -> syn::Result<proc_macro2::TokenStream> {
    let name = &input.ident;
    let vis = &input.vis;
    let Fields::Named(fields) = &input.fields else {
        return Err(syn::Error::new_spanned(
            &input,
            "resource must use named fields",
        ));
    };

    let mut rows = Vec::new();
    let mut links = Vec::new();
    let mut marks = Vec::new();

    for field in &fields.named {
        let ident = field
            .ident
            .as_ref()
            .ok_or_else(|| syn::Error::new_spanned(field, "field needs a name"))?;
        let label = ident.to_string();
        let ty = &field.ty;
        marks.push(quote! { #ty });
        if let Some(atom) = atom(&field.attrs)? {
            rows.push(quote! {
                .field(#label, ::keel::atom::Kind::#atom)
            });
            continue;
        }
        if let Some((card, target)) = link(&field.attrs, &field.ty)? {
            links.push(quote! {
                .bond(#label, ::keel::bond::Kind::#card, #target)
            });
            continue;
        }
        return Err(syn::Error::new_spanned(
            field,
            "resource field needs #[field(...)] or #[relation(...)]",
        ));
    }

    let shape = if marks.is_empty() {
        quote! { () }
    } else {
        quote! { (#(#marks),*) }
    };
    let mark = format_ident!("Mark{}", name);

    Ok(quote! {
        #[allow(non_camel_case_types)]
        #vis struct #name;

        type #mark = #shape;

        impl ::keel::spec::Resource for #name {
            fn name() -> &'static str {
                stringify!(#name)
            }

            fn spec() -> ::keel::spec::Spec {
                let _mark: ::core::option::Option<#mark> = ::core::option::Option::None;
                let _ = _mark;
                ::keel::spec::Spec::build(stringify!(#name))
                    #(#rows)*
                    #(#links)*
                    .seal()
            }
        }
    })
}

fn atom(attrs: &[Attribute]) -> syn::Result<Option<Ident>> {
    for attr in attrs {
        if !attr.path().is_ident("field") {
            continue;
        }
        let Meta::List(list) = &attr.meta else {
            return Err(syn::Error::new_spanned(
                attr,
                "use #[field(string)] or #[field(url)]",
            ));
        };
        let atoms = Punctuated::<Ident, Token![,]>::parse_terminated
            .parse2(list.tokens.clone())
            .map_err(|_| syn::Error::new_spanned(attr, "use #[field(string)] or #[field(url)]"))?;
        if atoms.len() != 1 {
            return Err(syn::Error::new_spanned(attr, "field takes one atom"));
        }
        let atom = atoms[0].clone();
        let kind = match atom.to_string().as_str() {
            "string" => Ident::new("Text", atom.span()),
            "url" => Ident::new("Link", atom.span()),
            other => {
                return Err(syn::Error::new(
                    atom.span(),
                    format!("unknown field atom: {other}"),
                ));
            }
        };
        return Ok(Some(kind));
    }
    Ok(None)
}

fn link(attrs: &[Attribute], ty: &Type) -> syn::Result<Option<(Ident, String)>> {
    for attr in attrs {
        if !attr.path().is_ident("relation") {
            continue;
        }
        let Meta::List(list) = &attr.meta else {
            return Err(syn::Error::new_spanned(attr, "use #[relation(Type, n2m)]"));
        };
        let parts = Punctuated::<Path, Token![,]>::parse_terminated
            .parse2(list.tokens.clone())
            .map_err(|_| syn::Error::new_spanned(attr, "use #[relation(Type, n2m)]"))?;
        if parts.len() != 2 {
            return Err(syn::Error::new_spanned(attr, "use #[relation(Type, n2m)]"));
        }
        let target = tail(&parts[0])?;
        let card = last(&parts[1])?;
        let kind = match card.to_string().as_str() {
            "n2m" => Ident::new("N2m", card.span()),
            other => {
                return Err(syn::Error::new(
                    card.span(),
                    format!("unknown relation kind: {other}"),
                ));
            }
        };
        let _ = ty;
        return Ok(Some((kind, target)));
    }
    Ok(None)
}

fn tail(path: &Path) -> syn::Result<String> {
    path.segments
        .last()
        .map(|seg| seg.ident.to_string())
        .ok_or_else(|| syn::Error::new_spanned(path, "empty path"))
}

fn last(path: &Path) -> syn::Result<Ident> {
    path.segments
        .last()
        .map(|seg| seg.ident.clone())
        .ok_or_else(|| syn::Error::new_spanned(path, "empty path"))
}

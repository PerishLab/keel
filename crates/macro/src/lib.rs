use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::{
    Attribute, Expr, ExprAssign, Fields, Ident, ItemStruct, Meta, Token, Type, parse_macro_input,
};

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
        let (mark, row, link) = one(field)?;
        marks.push(mark);
        if let Some(row) = row {
            rows.push(row);
        }
        if let Some(link) = link {
            links.push(link);
        }
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

fn one(
    field: &syn::Field,
) -> syn::Result<(
    proc_macro2::TokenStream,
    Option<proc_macro2::TokenStream>,
    Option<proc_macro2::TokenStream>,
)> {
    let ident = field
        .ident
        .as_ref()
        .ok_or_else(|| syn::Error::new_spanned(field, "field needs a name"))?;
    let label = ident.to_string();
    let ty = &field.ty;
    let mark = quote! { #ty };
    if let Some(atom) = atom(&field.attrs)? {
        let row = quote! { .field(#label, ::keel::atom::Kind::#atom) };
        return Ok((mark, Some(row), None));
    }
    if let Some((card, target, slots)) = link(&field.attrs, &field.ty)? {
        let pairs = slots.iter().map(|(n, k)| {
            quote! { (#n, ::keel::atom::Kind::#k) }
        });
        let row = quote! {
            .bond(#label, ::keel::bond::Kind::#card, #target, &[#(#pairs),*])
        };
        return Ok((mark, None, Some(row)));
    }
    Err(syn::Error::new_spanned(
        field,
        "resource field needs #[field(...)] or #[relation(...)]",
    ))
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
        return Ok(Some(kind_of(&atoms[0])?));
    }
    Ok(None)
}

type Link = (Ident, String, Vec<(String, Ident)>);

fn link(attrs: &[Attribute], ty: &Type) -> syn::Result<Option<Link>> {
    for attr in attrs {
        if !attr.path().is_ident("relation") {
            continue;
        }
        let Meta::List(list) = &attr.meta else {
            return Err(syn::Error::new_spanned(
                attr,
                "use #[relation(Type, n2m)] or #[relation(Type, n2m, field = atom)]",
            ));
        };
        let items = Punctuated::<Expr, Token![,]>::parse_terminated
            .parse2(list.tokens.clone())
            .map_err(|_| {
                syn::Error::new_spanned(
                    attr,
                    "use #[relation(Type, n2m)] or #[relation(Type, n2m, field = atom)]",
                )
            })?;
        if items.len() < 2 {
            return Err(syn::Error::new_spanned(attr, "use #[relation(Type, n2m)]"));
        }
        let target = path_tail(&items[0])?;
        let card = path_ident(&items[1])?;
        let kind = card_of(&card)?;
        let mut slots = Vec::new();
        for item in items.iter().skip(2) {
            slots.push(slot_of(item)?);
        }
        let _ = ty;
        return Ok(Some((kind, target, slots)));
    }
    Ok(None)
}

fn kind_of(atom: &Ident) -> syn::Result<Ident> {
    match atom.to_string().as_str() {
        "string" => Ok(Ident::new("Text", atom.span())),
        "url" => Ok(Ident::new("Link", atom.span())),
        other => Err(syn::Error::new(
            atom.span(),
            format!("unknown field atom: {other}"),
        )),
    }
}

fn card_of(card: &Ident) -> syn::Result<Ident> {
    match card.to_string().as_str() {
        "n2m" => Ok(Ident::new("N2m", card.span())),
        other => Err(syn::Error::new(
            card.span(),
            format!("unknown relation kind: {other}"),
        )),
    }
}

fn slot_of(item: &Expr) -> syn::Result<(String, Ident)> {
    let Expr::Assign(ExprAssign { left, right, .. }) = item else {
        return Err(syn::Error::new_spanned(
            item,
            "bond field form: name = atom",
        ));
    };
    let name = expr_ident(left)?;
    let atom = expr_ident(right)?;
    Ok((name.to_string(), kind_of(&atom)?))
}

fn path_tail(expr: &Expr) -> syn::Result<String> {
    let Expr::Path(path) = expr else {
        return Err(syn::Error::new_spanned(expr, "expected type path"));
    };
    path.path
        .segments
        .last()
        .map(|seg| seg.ident.to_string())
        .ok_or_else(|| syn::Error::new_spanned(expr, "empty path"))
}

fn path_ident(expr: &Expr) -> syn::Result<Ident> {
    let Expr::Path(path) = expr else {
        return Err(syn::Error::new_spanned(expr, "expected ident"));
    };
    path.path
        .segments
        .last()
        .map(|seg| seg.ident.clone())
        .ok_or_else(|| syn::Error::new_spanned(expr, "empty path"))
}

fn expr_ident(expr: &Expr) -> syn::Result<Ident> {
    match expr {
        Expr::Path(path) => path
            .path
            .get_ident()
            .cloned()
            .ok_or_else(|| syn::Error::new_spanned(expr, "expected ident")),
        _ => Err(syn::Error::new_spanned(expr, "expected ident")),
    }
}

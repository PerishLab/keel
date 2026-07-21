use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{Fields, Ident, ItemStruct, parse_macro_input};

#[proc_macro_attribute]
pub fn resource(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemStruct);
    let veil = attr
        .to_string()
        .split(',')
        .any(|word| word.trim() == "veil");
    match expand(input, veil) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

pub(crate) fn expand(input: ItemStruct, veil: bool) -> syn::Result<proc_macro2::TokenStream> {
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
    let veil = if veil {
        quote! { .veil() }
    } else {
        quote! {}
    };

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
                    #veil
                    .seal()
            }
        }
    })
}

pub(crate) fn one(
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
    if let Some(made) = atom(&field.attrs)? {
        let row = match made {
            Made::Atom(atom, only) => grow(&label, &atom, &only),
            Made::Serial(scope) => quote! { .serial(#label, #scope) },
        };
        return Ok((mark, Some(row), None));
    }
    if let Some(link) = link(&field.attrs, &field.ty)? {
        let Link {
            card,
            target,
            slots,
            need,
            root,
            crew,
        } = link;
        let pairs = slots.iter().map(|(n, k)| {
            quote! { (#n, ::keel::atom::Kind::#k) }
        });
        let row = if root {
            quote! {
                .root(#label, ::keel::bond::Kind::#card, #target)
            }
        } else if crew {
            quote! {
                .crew(#label, ::keel::bond::Kind::#card, #target)
            }
        } else if need {
            quote! {
                .bond(#label, ::keel::bond::Kind::#card, #target, &[#(#pairs),*])
            }
        } else {
            quote! {
                .free(#label, ::keel::bond::Kind::#card, #target)
            }
        };
        return Ok((mark, None, Some(row)));
    }
    Err(syn::Error::new_spanned(
        field,
        "resource field needs #[field(...)] or #[relation(...)]",
    ))
}

pub(crate) enum Only {
    Free,
    All,
    Per(Vec<String>),
}

pub(crate) enum Made {
    Atom(Ident, Only),
    Serial(String),
}

pub(crate) fn grow(label: &str, atom: &Ident, only: &Only) -> proc_macro2::TokenStream {
    match only {
        Only::Free => quote! { .field(#label, ::keel::atom::Kind::#atom) },
        Only::All => quote! { .sole(#label, ::keel::atom::Kind::#atom) },
        Only::Per(scope) => {
            let refs = scope.iter();
            quote! { .per(#label, ::keel::atom::Kind::#atom, &[#(#refs),*]) }
        }
    }
}

pub(crate) use parse::*;

mod parse;

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use std::collections::{BTreeMap, BTreeSet};
use syn::{Fields, Ident, ItemStruct, parse_macro_input};

#[proc_macro_attribute]
pub fn resource(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemStruct);
    let head = match head(attr.into()) {
        Ok(head) => head,
        Err(err) => return err.to_compile_error().into(),
    };
    match expand(input, head) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

pub(crate) fn expand(input: ItemStruct, head: Head) -> syn::Result<proc_macro2::TokenStream> {
    let name = &input.ident;
    let vis = &input.vis;
    let Fields::Named(fields) = &input.fields else {
        return Err(syn::Error::new_spanned(
            &input,
            "resource must use named fields",
        ));
    };

    let (parts, aliases) = collect(&fields.named)?;
    let (marks, rows, links) = emit::emit(parts, &aliases);
    let shape = if marks.is_empty() {
        quote! { () }
    } else {
        quote! { (#(#marks),*) }
    };
    let mark = format_ident!("Mark{}", name);
    let unit = match &head.name {
        Some(text) => quote! { #text },
        None => quote! { stringify!(#name) },
    };
    let veil = head.veil.then(|| quote! { .veil() });
    let frozen = head.frozen.then(|| quote! { .freeze() });

    Ok(quote! {
        #[allow(non_camel_case_types)]
        #vis struct #name;

        type #mark = #shape;

        impl ::keel::spec::Resource for #name {
            fn name() -> &'static str {
                #unit
            }

            fn spec() -> ::keel::spec::Spec {
                let _mark: ::core::option::Option<#mark> = ::core::option::Option::None;
                let _ = _mark;
                ::keel::spec::Spec::build(#unit)
                    #(#rows)*
                    #(#links)*
                    #veil
                    #frozen
                    .seal()
            }
        }
    })
}

fn collect(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::Token![,]>,
) -> syn::Result<(Vec<Part>, BTreeMap<String, String>)> {
    let mut parts = Vec::with_capacity(fields.len());
    let mut aliases = BTreeMap::new();
    let mut seen = BTreeSet::new();
    for field in fields {
        let part = one(field)?;
        let folded = part.label.to_ascii_lowercase();
        if !seen.insert(folded) {
            return Err(syn::Error::new_spanned(
                field,
                format!("duplicate field name: {}", part.label),
            ));
        }
        aliases.insert(part.ident.clone(), part.label.clone());
        parts.push(part);
    }
    Ok((parts, aliases))
}

pub(crate) struct Part {
    pub ident: String,
    pub label: String,
    pub mark: proc_macro2::TokenStream,
    pub body: Body,
}

pub(crate) enum Body {
    Atom(Bud),
    Serial(Tally),
    Link(Link),
}

pub(crate) fn one(field: &syn::Field) -> syn::Result<Part> {
    let ident = field
        .ident
        .as_ref()
        .ok_or_else(|| syn::Error::new_spanned(field, "field needs a name"))?;
    let raw = ident.to_string();
    let ty = &field.ty;
    let mark = quote! { #ty };
    let made = atom(&field.attrs)?;
    let bond = link(&field.attrs, &field.ty)?;
    match (made, bond) {
        (Some(_), Some(_)) => Err(syn::Error::new_spanned(
            field,
            "field cannot have both #[field(...)] and #[relation(...)]",
        )),
        (Some(Made::Atom(bud)), None) => Ok(Part {
            label: bud.name.clone().unwrap_or_else(|| raw.clone()),
            ident: raw,
            mark,
            body: Body::Atom(bud),
        }),
        (Some(Made::Serial(tally)), None) => Ok(Part {
            label: tally.name.clone().unwrap_or_else(|| raw.clone()),
            ident: raw,
            mark,
            body: Body::Serial(tally),
        }),
        (None, Some(link)) => Ok(Part {
            label: link.name.clone().unwrap_or_else(|| raw.clone()),
            ident: raw,
            mark,
            body: Body::Link(link),
        }),
        (None, None) => Err(syn::Error::new_spanned(
            field,
            "resource field needs #[field(...)] or #[relation(...)]",
        )),
    }
}

pub(crate) enum Scope {
    Field(String),
    Exact(String),
}

pub(crate) enum Only {
    Free,
    All,
    Per(Vec<Scope>),
}

pub(crate) struct Bud {
    pub name: Option<String>,
    pub kind: Ident,
    pub only: Only,
    pub need: bool,
    pub guard: Guard,
}

pub(crate) struct Tally {
    pub name: Option<String>,
    pub scope: Scope,
}

pub(crate) enum Made {
    Atom(Bud),
    Serial(Tally),
}

#[derive(Default, PartialEq)]
pub(crate) struct Guard {
    pub fallback: Option<String>,
    pub values: Vec<String>,
    pub min: Option<i64>,
    pub max: Option<i64>,
}

pub(crate) use head::*;
pub(crate) use link::*;
pub(crate) use parse::*;

mod emit;
mod head;
mod link;
mod parse;
mod rule;

#[cfg(test)]
mod test;

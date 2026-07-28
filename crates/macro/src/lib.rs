use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{Fields, Ident, ItemStruct, parse_macro_input};

#[proc_macro_attribute]
pub fn resource(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemStruct);
    let words: Vec<String> = attr
        .to_string()
        .split(',')
        .map(|word| word.trim().to_string())
        .filter(|word| !word.is_empty())
        .collect();
    if let Some(word) = words
        .iter()
        .find(|word| !matches!(word.as_str(), "veil" | "frozen"))
    {
        return syn::Error::new(
            proc_macro2::Span::call_site(),
            format!("unknown resource mode: {word}"),
        )
        .to_compile_error()
        .into();
    }
    let veil = words.iter().any(|word| word == "veil");
    let frozen = words.iter().any(|word| word == "frozen");
    match expand(input, veil, frozen) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

pub(crate) fn expand(
    input: ItemStruct,
    veil: bool,
    frozen: bool,
) -> syn::Result<proc_macro2::TokenStream> {
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
    let frozen = if frozen {
        quote! { .freeze() }
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
                    #frozen
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
            Made::Atom(atom, only, need, guard) => grow(&label, &atom, &only, need, &guard),
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
    Atom(Ident, Only, bool, Guard),
    Serial(String),
}

#[derive(Default)]
pub(crate) struct Guard {
    pub fallback: Option<String>,
    pub values: Vec<String>,
    pub min: Option<i64>,
    pub max: Option<i64>,
}

pub(crate) fn grow(
    label: &str,
    atom: &Ident,
    only: &Only,
    need: bool,
    guard: &Guard,
) -> proc_macro2::TokenStream {
    let field = match (only, need) {
        (Only::Free, true) => quote! { .field(#label, ::keel::atom::Kind::#atom) },
        (Only::Free, false) => {
            quote! { .optional(#label, ::keel::atom::Kind::#atom, ::keel::spec::Only::Free) }
        }
        (Only::All, true) => quote! { .sole(#label, ::keel::atom::Kind::#atom) },
        (Only::All, false) => {
            quote! { .optional(#label, ::keel::atom::Kind::#atom, ::keel::spec::Only::All) }
        }
        (Only::Per(scope), true) => {
            let refs = scope.iter();
            quote! { .per(#label, ::keel::atom::Kind::#atom, &[#(#refs),*]) }
        }
        (Only::Per(scope), false) => {
            let refs = scope.iter();
            quote! {
                .optional(
                    #label,
                    ::keel::atom::Kind::#atom,
                    ::keel::spec::Only::Per(vec![#(#refs.to_string()),*]),
                )
            }
        }
    };
    if guard.fallback.is_none()
        && guard.values.is_empty()
        && guard.min.is_none()
        && guard.max.is_none()
    {
        return field;
    }
    let fallback = guard
        .fallback
        .as_ref()
        .map(|value| quote! { .default(#value) });
    let values = (!guard.values.is_empty()).then(|| {
        let values = &guard.values;
        quote! { .values(&[#(#values),*]) }
    });
    let min = guard.min.map(|value| quote! { .min(#value) });
    let max = guard.max.map(|value| quote! { .max(#value) });
    quote! {
        #field
        .rule(
            #label,
            ::keel::spec::Rule::new()
                #fallback
                #values
                #min
                #max
        )
    }
}

pub(crate) use parse::*;

mod parse;
mod rule;

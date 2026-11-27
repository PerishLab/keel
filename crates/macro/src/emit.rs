use crate::head::Aim;
use crate::{Body, Bud, Guard, Link, Only, Part, Scope, Tally};
use quote::quote;
use std::collections::BTreeMap;

pub(crate) fn emit(
    parts: Vec<Part>,
    aliases: &BTreeMap<String, String>,
) -> (
    Vec<proc_macro2::TokenStream>,
    Vec<proc_macro2::TokenStream>,
    Vec<proc_macro2::TokenStream>,
) {
    let mut rows = Vec::new();
    let mut links = Vec::new();
    let mut marks = Vec::new();
    for part in parts {
        marks.push(part.mark);
        match part.body {
            Body::Atom(bud) => rows.push(grow(&part.label, &bud, aliases)),
            Body::Serial(tally) => rows.push(serial(&part.label, &tally, aliases)),
            Body::Link(link) => links.push(wire(&part.label, link)),
        }
    }
    (marks, rows, links)
}

fn resolve(scope: &Scope, aliases: &BTreeMap<String, String>) -> String {
    match scope {
        Scope::Field(name) => aliases.get(name).cloned().unwrap_or_else(|| name.clone()),
        Scope::Exact(name) => name.clone(),
    }
}

fn serial(
    label: &str,
    tally: &Tally,
    aliases: &BTreeMap<String, String>,
) -> proc_macro2::TokenStream {
    let scope = resolve(&tally.scope, aliases);
    quote! { .serial(#label, #scope) }
}

fn wire(label: &str, link: Link) -> proc_macro2::TokenStream {
    let Link {
        card,
        target,
        slots,
        need,
        root,
        crew,
        closure,
        ..
    } = link;
    let target = match target {
        Aim::Fixed(text) => quote! { #text },
        Aim::Type(ty) => quote! { <#ty as ::keel::spec::Resource>::name() },
    };
    let pairs = slots.iter().map(|(n, k)| {
        quote! { (#n, ::keel::atom::Kind::#k) }
    });
    if closure {
        quote! { .closure(#label, ::keel::bond::Kind::#card, #target, &[#(#pairs),*]) }
    } else if root {
        quote! { .root(#label, ::keel::bond::Kind::#card, #target) }
    } else if crew {
        quote! { .crew(#label, ::keel::bond::Kind::#card, #target) }
    } else if need {
        quote! { .bond(#label, ::keel::bond::Kind::#card, #target, &[#(#pairs),*]) }
    } else {
        quote! { .free(#label, ::keel::bond::Kind::#card, #target) }
    }
}

pub(crate) fn grow(
    label: &str,
    bud: &Bud,
    aliases: &BTreeMap<String, String>,
) -> proc_macro2::TokenStream {
    let Bud {
        kind: atom,
        only,
        need,
        guard,
        ..
    } = bud;
    let field = match (only, *need) {
        (Only::Free, true) => quote! { .field(#label, ::keel::atom::Kind::#atom) },
        (Only::Free, false) => {
            quote! { .optional(#label, ::keel::atom::Kind::#atom, ::keel::spec::Only::Free) }
        }
        (Only::All, true) => quote! { .sole(#label, ::keel::atom::Kind::#atom) },
        (Only::All, false) => {
            quote! { .optional(#label, ::keel::atom::Kind::#atom, ::keel::spec::Only::All) }
        }
        (Only::Per(scope), true) => {
            let refs: Vec<String> = scope.iter().map(|item| resolve(item, aliases)).collect();
            quote! { .per(#label, ::keel::atom::Kind::#atom, &[#(#refs),*]) }
        }
        (Only::Per(scope), false) => {
            let refs: Vec<String> = scope.iter().map(|item| resolve(item, aliases)).collect();
            quote! {
                .optional(
                    #label,
                    ::keel::atom::Kind::#atom,
                    ::keel::spec::Only::Per(vec![#(#refs.to_string()),*]),
                )
            }
        }
    };
    if *guard == Guard::default() {
        return field;
    }
    let fallback = guard.fallback.as_ref().map(|v| quote! { .default(#v) });
    let values = (!guard.values.is_empty()).then(|| {
        let values = &guard.values;
        quote! { .values(&[#(#values),*]) }
    });
    let min = guard.min.map(|v| quote! { .min(#v) });
    let max = guard.max.map(|v| quote! { .max(#v) });
    quote! { #field .rule(#label, ::keel::spec::Rule::new() #fallback #values #min #max) }
}

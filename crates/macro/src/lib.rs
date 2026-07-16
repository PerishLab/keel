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
    if let Some(made) = atom(&field.attrs)? {
        let row = match made {
            Made::Atom(atom, only) => grow(&label, &atom, &only),
            Made::Serial(scope) => quote! { .serial(#label, #scope) },
        };
        return Ok((mark, Some(row), None));
    }
    if let Some((card, target, slots, need, root)) = link(&field.attrs, &field.ty)? {
        let pairs = slots.iter().map(|(n, k)| {
            quote! { (#n, ::keel::atom::Kind::#k) }
        });
        let row = if root {
            quote! {
                .root(#label, ::keel::bond::Kind::#card, #target)
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

enum Only {
    Free,
    All,
    Per(String),
}

enum Made {
    Atom(Ident, Only),
    Serial(String),
}

fn grow(label: &str, atom: &Ident, only: &Only) -> proc_macro2::TokenStream {
    match only {
        Only::Free => quote! { .field(#label, ::keel::atom::Kind::#atom) },
        Only::All => quote! { .sole(#label, ::keel::atom::Kind::#atom) },
        Only::Per(scope) => quote! { .per(#label, ::keel::atom::Kind::#atom, #scope) },
    }
}

fn atom(attrs: &[Attribute]) -> syn::Result<Option<Made>> {
    for attr in attrs {
        if !attr.path().is_ident("field") {
            continue;
        }
        let items = told(attr)?;
        if items.is_empty() {
            return Err(syn::Error::new_spanned(attr, "field takes one atom"));
        }
        let first = expr_ident(&items[0])?;
        if first == "serial" {
            return Ok(Some(Made::Serial(tally(attr, &items)?)));
        }
        let kind = kind_of(&first)?;
        let mut only = Only::Free;
        for item in items.iter().skip(1) {
            only = only_of(attr, item)?;
        }
        return Ok(Some(Made::Atom(kind, only)));
    }
    Ok(None)
}

fn tally(attr: &Attribute, items: &Punctuated<Expr, Token![,]>) -> syn::Result<String> {
    if items.len() != 2 {
        return Err(syn::Error::new_spanned(attr, "serial needs scope = rel"));
    }
    let Expr::Assign(ExprAssign { left, right, .. }) = &items[1] else {
        return Err(syn::Error::new_spanned(attr, "serial needs scope = rel"));
    };
    if expr_ident(left)? != "scope" {
        return Err(syn::Error::new_spanned(attr, "serial needs scope = rel"));
    }
    Ok(expr_ident(right)?.to_string())
}

fn told(attr: &Attribute) -> syn::Result<Punctuated<Expr, Token![,]>> {
    let Meta::List(list) = &attr.meta else {
        return Err(syn::Error::new_spanned(
            attr,
            "use #[field(string)] or #[field(string, unique)]",
        ));
    };
    Punctuated::<Expr, Token![,]>::parse_terminated
        .parse2(list.tokens.clone())
        .map_err(|_| {
            syn::Error::new_spanned(
                attr,
                "use #[field(string)] or #[field(string, unique = rel)]",
            )
        })
}

fn only_of(attr: &Attribute, item: &Expr) -> syn::Result<Only> {
    if expr_ident(item).is_ok_and(|word| word == "unique") {
        return Ok(Only::All);
    }
    if let Expr::Assign(ExprAssign { left, right, .. }) = item {
        let name = expr_ident(left)?;
        if name == "unique" {
            return Ok(Only::Per(expr_ident(right)?.to_string()));
        }
    }
    Err(syn::Error::new_spanned(
        attr,
        "field extras: unique or unique = rel",
    ))
}

type Link = (Ident, String, Vec<(String, Ident)>, bool, bool);

fn link(attrs: &[Attribute], ty: &Type) -> syn::Result<Option<Link>> {
    for attr in attrs {
        if !attr.path().is_ident("relation") {
            continue;
        }
        let items = rest(attr)?;
        if items.len() < 2 {
            return Err(syn::Error::new_spanned(
                attr,
                "use #[relation(Type, many2many)]",
            ));
        }
        let target = path_tail(&items[0])?;
        let card = path_ident(&items[1])?;
        let kind = card_of(&card)?;
        let mut slots = Vec::new();
        let mut need = true;
        let mut root = false;
        for item in items.iter().skip(2) {
            if expr_ident(item).is_ok_and(|word| word == "opt") {
                need = false;
                continue;
            }
            if expr_ident(item).is_ok_and(|word| word == "root") {
                root = true;
                continue;
            }
            slots.push(slot_of(item)?);
        }
        shape(attr, &kind, &slots, need, root)?;
        let _ = ty;
        return Ok(Some((kind, target, slots, need, root)));
    }
    Ok(None)
}

fn rest(attr: &Attribute) -> syn::Result<Punctuated<Expr, Token![,]>> {
    let Meta::List(list) = &attr.meta else {
        return Err(syn::Error::new_spanned(
            attr,
            "use #[relation(Type, many2many)] or #[relation(Type, many2one)]",
        ));
    };
    Punctuated::<Expr, Token![,]>::parse_terminated
        .parse2(list.tokens.clone())
        .map_err(|_| {
            syn::Error::new_spanned(
                attr,
                "use #[relation(Type, many2many, field = atom)] or #[relation(Type, many2one, opt)]",
            )
        })
}

fn shape(
    attr: &Attribute,
    kind: &Ident,
    slots: &[(String, Ident)],
    need: bool,
    root: bool,
) -> syn::Result<()> {
    if kind != "Many2many" && !slots.is_empty() {
        return Err(syn::Error::new_spanned(attr, "only many2many takes fields"));
    }
    if kind == "Many2many" && (!need || root) {
        return Err(syn::Error::new_spanned(
            attr,
            "opt and root are for single refs only",
        ));
    }
    if root && !need {
        return Err(syn::Error::new_spanned(attr, "root is always required"));
    }
    Ok(())
}

fn kind_of(atom: &Ident) -> syn::Result<Ident> {
    match atom.to_string().as_str() {
        "string" => Ok(Ident::new("Text", atom.span())),
        "url" => Ok(Ident::new("Link", atom.span())),
        "int" => Ok(Ident::new("Int", atom.span())),
        "bool" => Ok(Ident::new("Bool", atom.span())),
        other => Err(syn::Error::new(
            atom.span(),
            format!("unknown field atom: {other}"),
        )),
    }
}

fn card_of(card: &Ident) -> syn::Result<Ident> {
    match card.to_string().as_str() {
        "many2many" => Ok(Ident::new("Many2many", card.span())),
        "many2one" => Ok(Ident::new("Many2one", card.span())),
        "one2one" => Ok(Ident::new("One2one", card.span())),
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

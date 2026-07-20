use crate::*;
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::{Attribute, Expr, ExprAssign, Ident, Meta, Token, Type};

pub(crate) fn atom(attrs: &[Attribute]) -> syn::Result<Option<Made>> {
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

pub(crate) fn tally(attr: &Attribute, items: &Punctuated<Expr, Token![,]>) -> syn::Result<String> {
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

pub(crate) fn told(attr: &Attribute) -> syn::Result<Punctuated<Expr, Token![,]>> {
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

pub(crate) fn only_of(attr: &Attribute, item: &Expr) -> syn::Result<Only> {
    if expr_ident(item).is_ok_and(|word| word == "unique") {
        return Ok(Only::All);
    }
    if let Expr::Assign(ExprAssign { left, right, .. }) = item {
        let name = expr_ident(left)?;
        if name == "unique" {
            return Ok(Only::Per(scopes(attr, right)?));
        }
    }
    Err(syn::Error::new_spanned(
        attr,
        "field extras: unique or unique = rel",
    ))
}

pub(crate) fn scopes(attr: &Attribute, expr: &Expr) -> syn::Result<Vec<String>> {
    if let Expr::Tuple(tuple) = expr {
        let mut out = Vec::new();
        for item in &tuple.elems {
            out.push(expr_ident(item)?.to_string());
        }
        if out.is_empty() {
            return Err(syn::Error::new_spanned(attr, "unique scope is empty"));
        }
        return Ok(out);
    }
    Ok(vec![expr_ident(expr)?.to_string()])
}

type Link = (Ident, String, Vec<(String, Ident)>, bool, bool, bool);

pub(crate) fn link(attrs: &[Attribute], ty: &Type) -> syn::Result<Option<Link>> {
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
        let mut crew = false;
        for item in items.iter().skip(2) {
            if expr_ident(item).is_ok_and(|word| word == "opt") {
                need = false;
                continue;
            }
            if expr_ident(item).is_ok_and(|word| word == "root") {
                root = true;
                continue;
            }
            if expr_ident(item).is_ok_and(|word| word == "crew") {
                crew = true;
                continue;
            }
            slots.push(slot_of(item)?);
        }
        shape(attr, &kind, &slots, need, root, crew)?;
        let _ = ty;
        return Ok(Some((kind, target, slots, need, root, crew)));
    }
    Ok(None)
}

pub(crate) fn rest(attr: &Attribute) -> syn::Result<Punctuated<Expr, Token![,]>> {
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

pub(crate) fn shape(
    attr: &Attribute,
    kind: &Ident,
    slots: &[(String, Ident)],
    need: bool,
    root: bool,
    crew: bool,
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
    if crew && kind != "Many2many" {
        return Err(syn::Error::new_spanned(attr, "crew is many2many only"));
    }
    if root && !need {
        return Err(syn::Error::new_spanned(attr, "root is always required"));
    }
    Ok(())
}

pub(crate) fn kind_of(atom: &Ident) -> syn::Result<Ident> {
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

pub(crate) fn card_of(card: &Ident) -> syn::Result<Ident> {
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

pub(crate) fn slot_of(item: &Expr) -> syn::Result<(String, Ident)> {
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

pub(crate) fn path_tail(expr: &Expr) -> syn::Result<String> {
    let Expr::Path(path) = expr else {
        return Err(syn::Error::new_spanned(expr, "expected type path"));
    };
    path.path
        .segments
        .last()
        .map(|seg| seg.ident.to_string())
        .ok_or_else(|| syn::Error::new_spanned(expr, "empty path"))
}

pub(crate) fn path_ident(expr: &Expr) -> syn::Result<Ident> {
    let Expr::Path(path) = expr else {
        return Err(syn::Error::new_spanned(expr, "expected ident"));
    };
    path.path
        .segments
        .last()
        .map(|seg| seg.ident.clone())
        .ok_or_else(|| syn::Error::new_spanned(expr, "empty path"))
}

pub(crate) fn expr_ident(expr: &Expr) -> syn::Result<Ident> {
    match expr {
        Expr::Path(path) => path
            .path
            .get_ident()
            .cloned()
            .ok_or_else(|| syn::Error::new_spanned(expr, "expected ident")),
        _ => Err(syn::Error::new_spanned(expr, "expected ident")),
    }
}

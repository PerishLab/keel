use crate::Scope;
use crate::parse::Spell;
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::{Expr, ExprAssign, ExprLit, Ident, Lit, Token, Type};

#[derive(Default)]
pub(crate) struct Head {
    pub name: Option<String>,
    pub veil: bool,
    pub frozen: bool,
}

pub(crate) enum Aim {
    Fixed(String),
    Type(Box<Type>, String),
}

pub(crate) fn head(tokens: proc_macro2::TokenStream) -> syn::Result<Head> {
    let hint =
        "use #[resource], #[resource(veil)], #[resource(frozen)], or #[resource(name = \"...\")]";
    let items = Punctuated::<Expr, Token![,]>::parse_terminated
        .parse2(tokens)
        .map_err(|err| syn::Error::new(err.span(), hint))?;
    let mut out = Head::default();
    for item in &items {
        step(&mut out, item, hint)?;
    }
    Ok(out)
}

fn step(out: &mut Head, item: &Expr, hint: &str) -> syn::Result<()> {
    if let Expr::Assign(assign) = item {
        return keyed(out, item, assign);
    }
    let Ok(word) = item.ident() else {
        return Err(syn::Error::new_spanned(item, hint));
    };
    match word.to_string().as_str() {
        "veil" => flag(&mut out.veil, item, "resource", "veil"),
        "frozen" => flag(&mut out.frozen, item, "resource", "frozen"),
        "name" => Err(syn::Error::new_spanned(
            item,
            "resource name needs name = \"...\"",
        )),
        other => Err(syn::Error::new_spanned(
            item,
            format!("unknown resource mode: {other}"),
        )),
    }
}

fn keyed(out: &mut Head, item: &Expr, assign: &ExprAssign) -> syn::Result<()> {
    match assign.left.ident()?.to_string().as_str() {
        "name" => put(&mut out.name, &assign.right, item, "resource name"),
        other => Err(syn::Error::new_spanned(
            item,
            format!("unknown resource option: {other}"),
        )),
    }
}

pub(crate) fn put(
    slot: &mut Option<String>,
    expr: &Expr,
    item: &Expr,
    role: &str,
) -> syn::Result<()> {
    if slot.is_some() {
        return Err(syn::Error::new_spanned(item, format!("duplicate {role}")));
    }
    *slot = Some(label(expr, role)?);
    Ok(())
}

pub(crate) fn flag(slot: &mut bool, item: &Expr, kind: &str, name: &str) -> syn::Result<()> {
    if *slot {
        return Err(syn::Error::new_spanned(
            item,
            format!("duplicate {kind} option: {name}"),
        ));
    }
    *slot = true;
    Ok(())
}

pub(crate) fn label(expr: &Expr, role: &str) -> syn::Result<String> {
    let Expr::Lit(ExprLit {
        lit: Lit::Str(text),
        ..
    }) = expr
    else {
        return Err(syn::Error::new_spanned(
            expr,
            format!("{role} needs a nonempty string literal"),
        ));
    };
    let value = text.value();
    if value.trim().is_empty() {
        return Err(syn::Error::new_spanned(
            expr,
            format!("{role} must not be empty"),
        ));
    }
    if value.chars().any(char::is_whitespace) {
        return Err(syn::Error::new_spanned(
            expr,
            format!("{role} must not contain whitespace"),
        ));
    }
    Ok(value)
}

pub(crate) fn place(expr: &Expr, role: &str) -> syn::Result<Scope> {
    if matches!(
        expr,
        Expr::Lit(ExprLit {
            lit: Lit::Str(_),
            ..
        })
    ) {
        return Ok(Scope::Exact(label(expr, role)?));
    }
    if let Ok(ident) = expr.ident() {
        return Ok(Scope::Field(ident.to_string()));
    }
    Err(syn::Error::new_spanned(
        expr,
        format!("{role} needs an ident or nonempty string literal"),
    ))
}

fn same(ty: &Type, head: &Ident) -> bool {
    let Type::Path(path) = ty else {
        return false;
    };
    path.qself.is_none()
        && path
            .path
            .segments
            .last()
            .is_some_and(|seg| seg.ident == *head && seg.arguments.is_empty())
}

pub(crate) fn aim(expr: &Expr, ty: &Type) -> syn::Result<Aim> {
    if matches!(
        expr,
        Expr::Lit(ExprLit {
            lit: Lit::Str(_),
            ..
        })
    ) {
        return Ok(Aim::Fixed(label(expr, "relation target")?));
    }
    let Expr::Path(path) = expr else {
        return Err(syn::Error::new_spanned(
            expr,
            "relation target needs a type path or nonempty string literal",
        ));
    };
    let segs = &path.path.segments;
    if segs.len() == 2 {
        let root = segs[0].ident.to_string().to_ascii_lowercase();
        let name = segs[1].ident.to_string().to_ascii_lowercase();
        return Ok(Aim::Fixed(format!("{root}:{name}")));
    }
    let head = expr.head()?;
    if same(ty, &head) {
        return Ok(Aim::Type(Box::new(ty.clone()), head.to_string()));
    }
    Ok(Aim::Fixed(head.to_string()))
}

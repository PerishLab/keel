use crate::parse::{Read, Spell};
use crate::{Guard, Only};
use syn::{Attribute, Expr, ExprAssign, ExprLit, ExprUnary, Lit, UnOp};

pub(crate) fn read(
    attr: &Attribute,
    item: &Expr,
    only: &mut Only,
    guard: &mut Guard,
) -> syn::Result<bool> {
    let Expr::Assign(ExprAssign { left, right, .. }) = item else {
        return Ok(false);
    };
    match left.ident()?.to_string().as_str() {
        "unique" => *only = Only::Per(attr.scopes(right)?),
        "default" => guard.fallback = Some(literal(right)?),
        "values" => guard.values = literals(right)?,
        "min" => guard.min = Some(integer(right)?),
        "max" => guard.max = Some(integer(right)?),
        other => {
            return Err(syn::Error::new_spanned(
                item,
                format!("unknown field rule: {other}"),
            ));
        }
    }
    Ok(true)
}

fn literals(expr: &Expr) -> syn::Result<Vec<String>> {
    let Expr::Tuple(tuple) = expr else {
        return Err(syn::Error::new_spanned(
            expr,
            "values needs a nonempty tuple",
        ));
    };
    if tuple.elems.is_empty() {
        return Err(syn::Error::new_spanned(
            expr,
            "values needs a nonempty tuple",
        ));
    }
    tuple.elems.iter().map(literal).collect()
}

fn literal(expr: &Expr) -> syn::Result<String> {
    match expr {
        Expr::Lit(ExprLit {
            lit: Lit::Str(value),
            ..
        }) => Ok(value.value()),
        Expr::Lit(ExprLit {
            lit: Lit::Int(value),
            ..
        }) => Ok(value.base10_digits().into()),
        Expr::Lit(ExprLit {
            lit: Lit::Bool(value),
            ..
        }) => Ok(value.value.to_string()),
        Expr::Unary(ExprUnary {
            op: UnOp::Neg(_),
            expr,
            ..
        }) => integer(expr).map(|value| (-value).to_string()),
        _ => Err(syn::Error::new_spanned(
            expr,
            "field rule needs a string, integer, or bool literal",
        )),
    }
}

fn integer(expr: &Expr) -> syn::Result<i64> {
    match expr {
        Expr::Lit(ExprLit {
            lit: Lit::Int(value),
            ..
        }) => value.base10_parse(),
        Expr::Unary(ExprUnary {
            op: UnOp::Neg(_),
            expr,
            ..
        }) => integer(expr).map(|value| -value),
        _ => Err(syn::Error::new_spanned(
            expr,
            "range bound needs integer literal",
        )),
    }
}

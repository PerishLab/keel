use crate::Made;
use crate::parse::atom;
use syn::{Attribute, parse_quote};

fn values(attr: Attribute) -> syn::Result<Vec<String>> {
    match atom(&[attr])? {
        Some(Made::Atom(bud)) => Ok(bud.guard.values),
        _ => panic!("expected an atom"),
    }
}

fn refusal(attr: Attribute) -> String {
    match values(attr) {
        Err(err) => err.to_string(),
        Ok(held) => panic!("expected a refusal, held {held:?}"),
    }
}

#[test]
fn array() {
    let held = values(parse_quote!(#[field(string, values = ["ready", "done"])]));
    assert_eq!(held.expect("array"), ["ready", "done"]);
}

#[test]
fn single() {
    let held = values(parse_quote!(#[field(string, values = ["ready"])]));
    assert_eq!(held.expect("single"), ["ready"]);
}

#[test]
fn literal() {
    let held = values(parse_quote!(#[field(int, values = [1, -2, 3, 4, 5])]));
    assert_eq!(held.expect("literal"), ["1", "-2", "3", "4", "5"]);
}

#[test]
fn legacy() {
    let held = values(parse_quote!(#[field(string, values = ("ready", "done"))]));
    assert_eq!(held.expect("tuple"), ["ready", "done"]);
}

#[test]
fn empty() {
    let note = refusal(parse_quote!(#[field(string, values = [])]));
    assert!(note.contains("nonempty array"), "{note}");
    let note = refusal(parse_quote!(#[field(string, values = ())]));
    assert!(note.contains("nonempty array"), "{note}");
}

#[test]
fn bare() {
    let note = refusal(parse_quote!(#[field(string, values = "ready")]));
    assert!(note.contains("nonempty array"), "{note}");
}

#[test]
fn foreign() {
    let note = refusal(parse_quote!(#[field(string, values = [ready])]));
    assert!(note.contains("literal"), "{note}");
}

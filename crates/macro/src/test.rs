use crate::head::head;
use crate::parse::atom;
use crate::{Made, expand};
use syn::{Attribute, ItemStruct, parse_quote};

mod refuse;

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

pub(super) fn emitted(attr: proc_macro2::TokenStream, item: ItemStruct) -> syn::Result<String> {
    let mode = head(attr)?;
    Ok(expand(item, mode)?.to_string())
}

pub(super) fn failed(attr: proc_macro2::TokenStream, item: ItemStruct) -> String {
    match emitted(attr, item) {
        Err(err) => err.to_string(),
        Ok(code) => panic!("expected a refusal, emitted {code}"),
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

#[test]
fn compat() {
    let code = emitted(
        parse_quote!(),
        parse_quote! {
            struct Member {
                #[field(string)]
                branch: string,
            }
        },
    )
    .expect("compat");
    assert!(code.contains("stringify ! (Member)"), "{code}");
    assert!(code.contains(". field (\"branch\""), "{code}");
}

#[test]
fn named() {
    let code = emitted(
        parse_quote!(name = "IssueMember"),
        parse_quote! {
            pub struct Member {
                #[field(string)]
                branch: string,
            }
        },
    )
    .expect("named");
    assert!(code.contains("pub struct Member ;"), "{code}");
    assert!(
        code.contains("fn name () -> & 'static str { \"IssueMember\" }"),
        "{code}"
    );
    assert!(code.contains("Spec :: build (\"IssueMember\")"), "{code}");
}

#[test]
fn modes() {
    let code = emitted(
        parse_quote!(veil, name = "IssueBoundary", frozen),
        parse_quote! {
            struct Boundary {
                #[field(string)]
                head: string,
            }
        },
    )
    .expect("modes");
    assert!(code.contains("Spec :: build (\"IssueBoundary\")"), "{code}");
    assert!(code.contains(". veil ()"), "{code}");
    assert!(code.contains(". freeze ()"), "{code}");

    let reversed = emitted(
        parse_quote!(frozen, veil, name = "IssueBoundary",),
        parse_quote! {
            struct Boundary {
                #[field(string)]
                head: string,
            }
        },
    )
    .expect("reversed");
    assert_eq!(code, reversed);
}

#[test]
fn renamed() {
    let code = emitted(
        parse_quote!(name = "MessageEvent", frozen),
        parse_quote! {
            struct Event {
                #[field(string, name = "actor_type", values = ["soul", "system"])]
                kind: string,
                #[field(int, opt, name = "base_version", min = 1)]
                version: int,
                #[field(string, name = "coalesce_key", opt, unique = (parent, kind))]
                key: string,
                #[relation(Order, many2one, root, name = "sales_order")]
                parent: Order,
                #[field(serial, scope = parent, name = "seq_no")]
                number: int,
            }
        },
    )
    .expect("renamed");
    assert!(code.contains(". field (\"actor_type\""), "{code}");
    assert!(code.contains(". rule (\"actor_type\""), "{code}");
    assert!(code.contains(". optional (\"base_version\""), "{code}");
    assert!(code.contains(". rule (\"base_version\""), "{code}");
    assert!(
        code.contains(
            "Only :: Per (vec ! [\"sales_order\" . to_string () , \"actor_type\" . to_string ()])"
        ),
        "{code}"
    );
    assert!(
        code.contains(
            ". root (\"sales_order\" , :: keel :: bond :: Kind :: Many2one , { # [allow (unused_imports)] use :: keel :: spec :: { Named as _ , Plain as _ } ; (& :: keel :: spec :: Probe :: < Order > :: NEW) . pick (\"Order\") })"
        ),
        "{code}"
    );
    assert!(
        code.contains(". serial (\"seq_no\" , \"sales_order\")"),
        "{code}"
    );
}

#[test]
fn scoped() {
    let code = emitted(
        parse_quote!(name = "OrderLine"),
        parse_quote! {
            struct Line {
                #[field(string, unique = "sales_order")]
                code: string,
                #[field(serial, name = "line_no", scope = "sales_order")]
                number: int,
                #[relation("SalesOrder", many2one, root, name = "sales_order")]
                order: Order,
            }
        },
    )
    .expect("scoped");
    assert!(
        code.contains(". per (\"code\" , :: keel :: atom :: Kind :: Text , & [\"sales_order\"])"),
        "{code}"
    );
    assert!(
        code.contains(". serial (\"line_no\" , \"sales_order\")"),
        "{code}"
    );
    assert!(
        code.contains(
            ". root (\"sales_order\" , :: keel :: bond :: Kind :: Many2one , \"SalesOrder\")"
        ),
        "{code}"
    );
}

#[test]
fn relations() {
    let code = emitted(
        parse_quote!(),
        parse_quote! {
            struct Note {
                #[relation(Team, many2one, root)]
                team: team::Team,
                #[relation(Shop::Tag, many2many, name = "shop_tags", grade = string)]
                tags: shop::Tag,
                #[relation(Ghost, many2many)]
                ghosts: string,
                #[relation(Tag, many2many, name = string)]
                labels: Tag,
            }
        },
    )
    .expect("relations");
    assert!(
        code.contains("(& :: keel :: spec :: Probe :: < team :: Team > :: NEW) . pick (\"Team\")"),
        "{code}"
    );
    assert!(
        code.contains(
            ". bond (\"shop_tags\" , :: keel :: bond :: Kind :: Many2many , \"shop:tag\""
        ),
        "{code}"
    );
    assert!(
        code.contains(". bond (\"ghosts\" , :: keel :: bond :: Kind :: Many2many , \"Ghost\""),
        "{code}"
    );
    assert!(
        code.contains("(\"name\" , :: keel :: atom :: Kind :: Text)"),
        "{code}"
    );
}

#[test]
fn targets() {
    let code = emitted(
        parse_quote!(),
        parse_quote! {
            struct Entry {
                #[relation(Marker, many2one, root)]
                marker: Marker,
                #[relation(Custom, many2one)]
                custom: Custom,
                #[relation(Leaf, many2one, opt)]
                leaf: sub::Leaf,
            }
        },
    )
    .expect("targets");
    assert!(
        code.contains("(& :: keel :: spec :: Probe :: < Marker > :: NEW) . pick (\"Marker\")"),
        "{code}"
    );
    assert!(
        code.contains("(& :: keel :: spec :: Probe :: < Custom > :: NEW) . pick (\"Custom\")"),
        "{code}"
    );
    assert!(
        code.contains("(& :: keel :: spec :: Probe :: < sub :: Leaf > :: NEW) . pick (\"Leaf\")"),
        "{code}"
    );
}

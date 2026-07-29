use keel::adapt::db::Sqlite;
use keel::spec::{Only, Rule, Spec};
use keel::{Graph, atom, bond};

fn shop() -> Spec {
    Spec::build("Shop")
        .field("name", atom::Kind::Text)
        .sole("code", atom::Kind::Text)
        .optional("note", atom::Kind::Text, Only::Free)
        .optional("slug", atom::Kind::Text, Only::All)
        .seal()
}

fn tag() -> Spec {
    Spec::build("Tag")
        .field("hue", atom::Kind::Text)
        .veil()
        .seal()
}

fn item() -> Spec {
    Spec::build("Item")
        .root("shop", bond::Kind::Many2one, "Shop")
        .per("sku", atom::Kind::Text, &["shop"])
        .serial("seq", "shop")
        .optional("alt", atom::Kind::Text, Only::Per(vec!["shop".to_string()]))
        .field("live", atom::Kind::Bool)
        .optional("site", atom::Kind::Link, Only::Free)
        .field("size", atom::Kind::Int)
        .rule("size", Rule::new().min(1).max(9))
        .field("grade", atom::Kind::Text)
        .rule("grade", Rule::new().default("b").values(&["a", "b"]))
        .bond(
            "tags",
            bond::Kind::Many2many,
            "Tag",
            &[("hue", atom::Kind::Text)],
        )
        .free("twin", bond::Kind::One2one, "Tag")
        .seal()
}

fn graph() -> Graph {
    let mut graph = Graph::new();
    graph.add(shop()).add(tag()).add(item());
    graph
}

#[tokio::test]
async fn built() {
    let core = crate::support::boot(graph(), Sqlite::memory().await.expect("db"))
        .await
        .expect("bind");
    let plan = core.plan();
    let item = plan
        .units()
        .values()
        .find(|unit| unit.name() == "Item")
        .expect("Item");
    assert_eq!(item.fields().len(), 7);
    assert_eq!(item.bonds().len(), 3);
    assert_eq!(item.key(), "shop:item");
}

#[tokio::test]
async fn served() {
    let core = crate::support::boot(graph(), Sqlite::memory().await.expect("db"))
        .await
        .expect("bind");
    let shop = core.put("Shop", &[("name", "one"), ("code", "S1")]).await;
    let shop = shop.expect("shop");
    let held = core
        .put(
            "Item",
            &[
                ("shop", &shop.to_string()),
                ("sku", "A1"),
                ("live", "true"),
                ("size", "3"),
                ("grade", "a"),
            ],
        )
        .await;
    assert!(held.is_ok(), "{held:?}");
}

use super::super::world::{clean, graph, spot};
use keel::adapt::db::Sqlite;
use keel::atom::Kind;
use keel::spec::{Resource, Spec};
use keel::{Cell, Row, bind};

struct Thin;
struct Wide;

impl Resource for Thin {
    fn name() -> &'static str {
        "Card"
    }

    fn spec() -> Spec {
        Spec::build(Self::name()).field("code", Kind::Text).seal()
    }
}

impl Resource for Wide {
    fn name() -> &'static str {
        "Card"
    }

    fn spec() -> Spec {
        Spec::build(Self::name())
            .field("code", Kind::Text)
            .sole("tag", Kind::Text)
            .seal()
    }
}

fn named(rows: &[Row]) -> Vec<String> {
    let mut out: Vec<String> = rows
        .iter()
        .map(|row| row.text("name").unwrap_or_default().to_string())
        .collect();
    out.sort();
    out
}

#[tokio::test]
async fn etched() {
    let path = spot("schema_etched");
    let core = crate::support::boot(graph::<Thin>(), Sqlite::file(&path).await.expect("first"))
        .await
        .expect("bind");
    let units = core.live("@unit").await.expect("units");
    assert_eq!(named(&units), vec!["Card".to_string()]);
    let held = units.first().expect("unit");
    assert_eq!(held.text("key"), Some("card"));
    assert_eq!(
        held.cells().get("veil").map(Cell::show),
        Some("false".into())
    );
    assert_eq!(
        held.cells().get("generation").map(Cell::show),
        Some("1".into())
    );
    let fields = core.live("@field").await.expect("fields");
    assert_eq!(named(&fields), vec!["code".to_string()]);
    let owner = fields.first().expect("field").cells().get("unit").cloned();
    assert_eq!(owner.map(|cell| cell.show()), Some(held.key().to_string()));
    drop(core);
    clean(&path);
}

#[tokio::test]
async fn tracked() {
    let path = spot("schema_tracked");
    let core = crate::support::boot(graph::<Thin>(), Sqlite::file(&path).await.expect("first"))
        .await
        .expect("bind");
    assert_eq!(
        named(&core.live("@field").await.expect("one")),
        vec!["code"]
    );
    drop(core);

    let core = bind(graph::<Wide>(), Sqlite::file(&path).await.expect("second"))
        .await
        .expect("evolve");
    let fields = core.live("@field").await.expect("two");
    assert_eq!(named(&fields), vec!["code".to_string(), "tag".to_string()]);
    let units = core.live("@unit").await.expect("units");
    assert_eq!(units.len(), 1);
    let at = units[0].cells().get("generation").map(Cell::show);
    assert_eq!(at, Some("2".into()));
    drop(core);
    clean(&path);
}

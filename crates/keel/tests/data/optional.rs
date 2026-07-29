use keel::adapt::db::Sqlite;
use keel::atom::string;
use keel::query::form;
use keel::{Cell, Graph, resource};

#[resource]
struct Note {
    #[field(string)]
    title: string,
    #[field(string, opt)]
    detail: string,
    #[field(string, unique, opt)]
    label: string,
}

#[tokio::test]
async fn scalar() {
    let mut graph = Graph::new();
    graph.plug::<Note>();
    let core = crate::support::boot(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind");

    let first = core.put("Note", &[("title", "first")]).await.expect("put");
    core.put("Note", &[("title", "second")])
        .await
        .expect("second null label");
    let row = core.live("Note").await.expect("live").remove(0);
    assert!(!row.cells().contains_key("detail"));
    assert!(!row.cells().contains_key("label"));
    assert_eq!(
        core.query("from Note where detail is null")
            .await
            .expect("query")
            .rows()
            .len(),
        2
    );
    assert_eq!(
        core.ask(&form("Note").missing("label"))
            .await
            .expect("builder")
            .rows()
            .len(),
        2
    );

    core.set("Note", first, &[("detail", ""), ("label", "held")])
        .await
        .expect("set values");
    let row = core
        .live("Note")
        .await
        .expect("live")
        .into_iter()
        .find(|row| row.key() == first)
        .expect("first");
    assert_eq!(row.cell("detail"), Some(&Cell::Text(String::new())));
    assert_eq!(row.text("label"), Some("held"));
    assert!(
        core.put("Note", &[("title", "duplicate"), ("label", "held")])
            .await
            .is_err()
    );

    core.unset("Note", first, &["detail", "label"])
        .await
        .expect("unset");
    let row = core
        .live("Note")
        .await
        .expect("live")
        .into_iter()
        .find(|row| row.key() == first)
        .expect("first");
    assert!(!row.cells().contains_key("detail"));
    assert!(!row.cells().contains_key("label"));
    assert!(core.unset("Note", first, &["title"]).await.is_err());
}

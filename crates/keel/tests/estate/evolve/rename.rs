use keel::adapt::db::Sqlite;
use keel::atom::Kind;
use keel::bond;
use keel::spec::{Only, Spec};
use keel::wire::{Val, Wire};
use keel::{Graph, bind};
use std::path::Path;

pub(super) fn graph(name: &str) -> Graph {
    let mut graph = Graph::new();
    graph.add(Spec::build("Root").field("name", Kind::Text).seal());
    graph.add(
        Spec::build(name)
            .field("value", Kind::Text)
            .optional("note", Kind::Text, Only::Free)
            .root("root", bond::Kind::Many2one, "Root")
            .seal(),
    );
    graph
}

pub(super) async fn rows(path: &Path, table: &str) -> Vec<Vec<Val>> {
    Sqlite::file(path).await.expect("wire").rows(
        &format!("SELECT id, value, note, root_id, expires_at, created_at, updated_at FROM \"{table}\" ORDER BY id"), &[]
    ).await.expect("rows")
}

pub(super) async fn boot(path: &Path) {
    let core = crate::support::boot(graph("Old"), Sqlite::file(path).await.expect("wire"))
        .await
        .expect("boot");
    let root = core.put("Root", &[("name", "owner")]).await.expect("root");
    for value in ["live", "retired", "collected"] {
        core.put("root:old", &[("value", value), ("root", &root.to_string())])
            .await
            .expect("row");
    }
    core.end("root:old", 2).await.expect("end");
    drop(core);
    let mut wire = Sqlite::file(path).await.expect("wire");
    wire.run("UPDATE \"root_old\" SET note = NULL", &[])
        .await
        .expect("nullable legacy value");
    wire.run("DELETE FROM \"root_old\" WHERE id = 3", &[])
        .await
        .expect("collect");
}

#[tokio::test]
async fn preserves() {
    let directory = tempfile::tempdir().expect("directory");
    let path = directory.path().join("estate");
    boot(&path).await;
    let before = rows(&path, "root_old").await;
    let core = bind(graph("New"), Sqlite::file(&path).await.expect("wire"))
        .rename("root:old", "root:new")
        .await
        .expect("rename");
    assert!(core.live("root:old").await.is_err());
    assert_eq!(core.live("root:new").await.expect("live").len(), 1);
    assert_eq!(rows(&path, "root_new").await, before);
    assert_eq!(rows(&path, "@g1:root_old").await, before);
    assert_eq!(
        core.put("root:new", &[("value", "after"), ("root", "1")])
            .await
            .expect("next"),
        4
    );
    drop(core);
    let core = bind(graph("New"), Sqlite::file(&path).await.expect("wire"))
        .rename("root:old", "root:new")
        .await
        .expect("reopen");
    assert_eq!(
        core.put("root:new", &[("value", "restart"), ("root", "1")])
            .await
            .expect("next"),
        5
    );
    drop(core);
    let mut wire = Sqlite::file(&path).await.expect("wire");
    assert_eq!(
        wire.rows("SELECT COUNT(*) FROM \"@generation\"", &[])
            .await
            .expect("generations"),
        vec![vec![Val::Int(2)]]
    );
}

#[tokio::test]
async fn fresh() {
    let directory = tempfile::tempdir().expect("directory");
    let path = directory.path().join("estate");
    drop(
        crate::support::boot(graph("New"), Sqlite::file(&path).await.expect("wire"))
            .await
            .expect("boot"),
    );
    bind(graph("New"), Sqlite::file(&path).await.expect("wire"))
        .rename("root:old", "root:new")
        .await
        .expect("already current");
}

#[tokio::test]
async fn routes() {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    let directory = tempfile::tempdir().expect("directory");
    let path = directory.path().join("estate");
    boot(&path).await;
    let core = bind(graph("New"), Sqlite::file(&path).await.expect("wire"))
        .rename("root:old", "root:new")
        .await
        .expect("rename");
    core.put(
        "@grant",
        &[
            ("who", "anon"),
            ("verb", "see"),
            ("unit", "*"),
            ("scope", "all"),
        ],
    )
    .await
    .expect("grant");
    let app = keel::app(core.share(), "");
    for (path, expected) in [
        ("/root:old", StatusCode::NOT_FOUND),
        ("/root:new", StatusCode::OK),
    ] {
        let request = Request::builder()
            .uri(path)
            .body(Body::empty())
            .expect("request");
        assert_eq!(
            app.clone()
                .oneshot(request)
                .await
                .expect("response")
                .status(),
            expected
        );
    }
}

#[path = "support/fault.rs"]
mod fault;
mod support;

use keel::adapt::db::Sqlite;
use keel::{Graph, bind};
use keel_document::{Error, Store};
use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use support::{core, document, model, set};

fn inventory(root: &Path) -> Vec<String> {
    let db = Database::open(root.join("store.redb")).unwrap();
    let read = db.begin_read().unwrap();
    let table = read
        .open_table(TableDefinition::<&str, &[u8]>::new("values"))
        .unwrap();
    table
        .iter()
        .unwrap()
        .map(|entry| entry.unwrap().0.value().to_string())
        .collect()
}

#[tokio::test]
async fn publication() {
    let home = tempfile::tempdir().unwrap();
    drop(core(home.path()).await);
    let armed = Arc::new(AtomicBool::new(false));
    let wire = fault::Fault {
        inner: Sqlite::file(&home.path().join("estate.sqlite3"))
            .await
            .unwrap(),
        armed: armed.clone(),
    };
    let mut graph = Graph::new();
    keel_document::stock(&mut graph);
    graph.add(
        keel::spec::Spec::build("Actor")
            .sole("login", keel::atom::Kind::Text)
            .seal(),
    );
    let core = Arc::new(bind(graph, wire).await.unwrap());
    let root = home.path().join("document");
    let store = Store::create(core.clone(), model(), &root).await.unwrap();
    let initial = store
        .sudo()
        .create("settings", "alice", document("{}"))
        .await
        .unwrap();
    armed.store(true, Ordering::SeqCst);
    assert!(matches!(
        store
            .sudo()
            .edit(&initial.head, &[set("/mode", "true")])
            .await,
        Err(Error::Store(keel::adapt::Store::Down(_)))
    ));
    assert_eq!(
        store
            .sudo()
            .read("settings", initial.head.id)
            .await
            .unwrap(),
        initial
    );
    drop(store);
    assert_eq!(inventory(&root).len(), 2);
    let store = Store::open(core.clone(), model(), &root).await.unwrap();
    assert_eq!(
        store
            .sudo()
            .read("settings", initial.head.id)
            .await
            .unwrap(),
        initial
    );
    drop(store);
    assert_eq!(
        inventory(&root),
        vec![format!("{}/{}", initial.head.id, initial.head.token)]
    );
}

#[tokio::test]
async fn retention() {
    let home = tempfile::tempdir().unwrap();
    let core = core(home.path()).await;
    let root = home.path().join("document");
    let store = Store::create(core.clone(), model(), &root).await.unwrap();
    let initial = store
        .sudo()
        .create("settings", "alice", document("{}"))
        .await
        .unwrap();
    let current = store
        .sudo()
        .edit(&initial.head, &[set("/mode", "null")])
        .await
        .unwrap();
    drop(store);
    assert_eq!(inventory(&root).len(), 2);
    let store = Store::open(core.clone(), model(), &root).await.unwrap();
    drop(store);
    assert_eq!(
        inventory(&root),
        vec![format!("{}/{}", current.head.id, current.head.token)]
    );
    assert_eq!(
        core.live("documentstore:document:revision")
            .await
            .unwrap()
            .len(),
        2
    );
}

#[tokio::test]
async fn unknown() {
    let home = tempfile::tempdir().unwrap();
    let core = core(home.path()).await;
    let root = home.path().join("document");
    let store = Store::create(core.clone(), model(), &root).await.unwrap();
    let initial = store
        .sudo()
        .create("settings", "alice", document("{}"))
        .await
        .unwrap();
    store
        .sudo()
        .edit(&initial.head, &[set("/mode", "null")])
        .await
        .unwrap();
    drop(store);
    let before = inventory(&root);
    core.set(
        "documentstore:document",
        initial.head.id,
        &[("head", &"f".repeat(64))],
    )
    .await
    .unwrap();
    assert!(Store::open(core, model(), &root).await.is_err());
    assert_eq!(inventory(&root), before);
}

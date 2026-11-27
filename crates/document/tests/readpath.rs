mod support;

use keel_document::{Query, Store};
use support::{counting, document, model};

#[tokio::test]
async fn reading() {
    let home = tempfile::tempdir().unwrap();
    let (core, counts) = counting(home.path()).await;
    let store = Store::create(core, model(), &home.path().join("document"))
        .await
        .unwrap();
    let record = store
        .sudo()
        .create("settings", "alice", document(r#"{"theme":"light"}"#))
        .await
        .unwrap();
    let begins = counts.begins();
    let rollbacks = counts.rollbacks();
    store.sudo().read("settings", record.head.id).await.unwrap();
    assert_eq!(counts.begins(), begins);
    assert_eq!(counts.rollbacks(), rollbacks);
}

#[tokio::test]
async fn scoped() {
    let home = tempfile::tempdir().unwrap();
    let (core, counts) = counting(home.path()).await;
    let store = Store::create(core.clone(), model(), &home.path().join("document"))
        .await
        .unwrap();
    let actor = core.put("Actor", &[("login", "alice")]).await.unwrap();
    let first = store
        .sudo()
        .create("settings", "a", document("{}"))
        .await
        .unwrap();
    store
        .sudo()
        .create("settings", "b", document("{}"))
        .await
        .unwrap();
    core.grant(keel::Grant::new(
        &actor.to_string(),
        "see",
        "documentstore:document",
        &format!("row {}", first.head.id),
    ))
    .await
    .unwrap();
    let query = Query {
        collection: "settings".into(),
        ..Query::default()
    };
    let begins = counts.begins();
    let rollbacks = counts.rollbacks();
    let (seen, stats) = store.of(actor).query(&query).await.unwrap();
    assert_eq!(seen.len(), 1);
    assert_eq!(stats.candidates, 2);
    assert_eq!(stats.admitted, 1);
    assert_eq!(counts.begins(), begins);
    assert_eq!(counts.rollbacks(), rollbacks);
}

#[tokio::test]
async fn warm() {
    let home = tempfile::tempdir().unwrap();
    let (core, counts) = counting(home.path()).await;
    let store = Store::create(core, model(), &home.path().join("document"))
        .await
        .unwrap();
    store
        .sudo()
        .create("settings", "a", document("{}"))
        .await
        .unwrap();
    let query = Query {
        collection: "settings".into(),
        ..Query::default()
    };
    store.sudo().query(&query).await.unwrap();
    assert!(
        store.sudo().read("settings", 999_999).await.is_err(),
        "a missing row refuses"
    );
    let reads = counts.reads();
    store.sudo().query(&query).await.unwrap();
    assert_eq!(counts.reads(), reads, "a failed read keeps the cache warm");
    assert_eq!(counts.rollbacks(), 0);
}

#[tokio::test]
async fn reopen() {
    let home = tempfile::tempdir().unwrap();
    let root = home.path().join("document");
    let (core, counts) = counting(home.path()).await;
    let store = Store::create(core, model(), &root).await.unwrap();
    for name in ["a", "b", "c"] {
        store
            .sudo()
            .create("settings", name, document("{}"))
            .await
            .unwrap();
    }
    drop(store);
    let (core, resumed) = counting(home.path()).await;
    let begins = resumed.begins();
    let rollbacks = resumed.rollbacks();
    let store = Store::open(core, model(), &root).await.unwrap();
    assert_eq!(resumed.begins(), begins);
    assert_eq!(resumed.rollbacks(), rollbacks);
    assert_eq!(store.sudo().read("settings", 1).await.unwrap().head.id, 1);
    let _ = counts.begins();
}

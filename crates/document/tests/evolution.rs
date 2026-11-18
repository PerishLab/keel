mod support;

use keel_document::{Collection, Error, Field, Model, Shape, Store};
use support::{core, document, set};

fn model(extra: Vec<Field>) -> Model {
    let mut fields = vec![Field::required("theme", Shape::Text)];
    fields.extend(extra);
    Model::new(vec![
        Collection::new("settings", Shape::Object(fields)).unwrap(),
    ])
    .unwrap()
}

#[tokio::test]
async fn append() {
    let home = tempfile::tempdir().unwrap();
    let core = core(home.path()).await;
    let root = home.path().join("document");
    let initial = model(vec![]);
    let store = Store::create(core.clone(), initial.clone(), &root)
        .await
        .unwrap();
    let record = store
        .sudo()
        .create("settings", "alice", document(r#"{"theme":"light"}"#))
        .await
        .unwrap();
    let new = model(vec![Field::optional(
        "choice",
        Shape::Nullable(Box::new(Shape::Text)),
    )]);
    drop(store);
    assert!(Store::open(core.clone(), new.clone(), &root).await.is_err());
    let bad = model(vec![Field::required("choice", Shape::Text)]);
    assert!(Store::evolve(core.clone(), bad, &root).await.is_err());
    let store = Store::evolve(core.clone(), new.clone(), &root)
        .await
        .unwrap();
    assert_eq!(
        store.sudo().read("settings", record.head.id).await.unwrap(),
        record
    );
    store
        .sudo()
        .edit(&record.head, &[set("/choice", "null")])
        .await
        .unwrap();
    drop(store);
    assert!(Store::open(core.clone(), initial, &root).await.is_err());
    assert!(Store::open(core, new, &root).await.is_ok());
}

#[tokio::test]
async fn structure() {
    let home = tempfile::tempdir().unwrap();
    let store = Store::create(
        core(home.path()).await,
        model(vec![]),
        &home.path().join("document"),
    )
    .await
    .unwrap();
    let record = store
        .sudo()
        .create("settings", "alice", document(r#"{"theme":"light"}"#))
        .await
        .unwrap();
    assert!(matches!(
        store
            .sudo()
            .edit(&record.head, &[set("/theme", "null")])
            .await,
        Err(Error::Value { .. })
    ));
    assert_eq!(
        store.sudo().read("settings", record.head.id).await.unwrap(),
        record
    );
}

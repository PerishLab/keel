mod support;

use keel_document::{Collection, Error, Model, Shape, Store};
use radicula_document::{Operation, Path};
use support::{core, document, model, set};

#[tokio::test]
async fn durable() {
    let home = tempfile::tempdir().unwrap();
    let root = home.path().join("document");
    let core = core(home.path()).await;
    let store = Store::create(core.clone(), model(), &root).await.unwrap();
    let initial = store
        .sudo()
        .create("settings", "alice", document(r#"{"theme":"light"}"#))
        .await
        .unwrap();
    let changed = store
        .sudo()
        .edit(&initial.head, &[set("/theme", "\"dark\"")])
        .await
        .unwrap();
    assert_ne!(initial.head.token, changed.head.token);
    assert_eq!(
        store
            .sudo()
            .read("settings", initial.head.id)
            .await
            .unwrap(),
        changed
    );
    drop(store);
    drop(core);
    let store = Store::open(self::core(home.path()).await, model(), &root)
        .await
        .unwrap();
    assert_eq!(
        store
            .sudo()
            .read("settings", changed.head.id)
            .await
            .unwrap(),
        changed
    );
}

#[tokio::test]
async fn conflict() {
    let home = tempfile::tempdir().unwrap();
    let store = Store::create(
        core(home.path()).await,
        model(),
        &home.path().join("document"),
    )
    .await
    .unwrap();
    let first = store
        .sudo()
        .create("settings", "alice", document("{}"))
        .await
        .unwrap();
    let second = store
        .sudo()
        .edit(&first.head, &[set("/mode", "null")])
        .await
        .unwrap();
    assert!(matches!(
        store
            .sudo()
            .edit(&first.head, &[set("/mode", "false")])
            .await,
        Err(Error::Conflict { .. })
    ));
    assert!(matches!(
        store.sudo().end(&first.head).await,
        Err(Error::Conflict { .. })
    ));
    assert_eq!(
        store.sudo().read("settings", first.head.id).await.unwrap(),
        second
    );
}

#[tokio::test]
async fn atomic() {
    let home = tempfile::tempdir().unwrap();
    let store = Store::create(
        core(home.path()).await,
        model(),
        &home.path().join("document"),
    )
    .await
    .unwrap();
    let first = store
        .sudo()
        .create("settings", "alice", document("{}"))
        .await
        .unwrap();
    let operations = [
        set("/mode", "null"),
        Operation::Remove {
            path: Path::parse("/absent").unwrap(),
        },
    ];
    assert!(matches!(
        store.sudo().edit(&first.head, &operations).await,
        Err(Error::Patch { index: 1, .. })
    ));
    assert_eq!(
        store.sudo().read("settings", first.head.id).await.unwrap(),
        first
    );
}

#[tokio::test]
async fn custody() {
    let home = tempfile::tempdir().unwrap();
    let core = core(home.path()).await;
    let store = Store::create(core.clone(), model(), &home.path().join("document"))
        .await
        .unwrap();
    let record = store
        .sudo()
        .create("settings", "alice", document("{}"))
        .await
        .unwrap();
    assert_eq!(
        store.anon().read("settings", record.head.id).await,
        Err(Error::Missing)
    );
    assert!(
        store
            .anon()
            .create("settings", "bob", document("{}"))
            .await
            .is_err()
    );
    assert!(
        store
            .anon()
            .edit(&record.head, &[set("/mode", "null")])
            .await
            .is_err()
    );
    store.sudo().end(&record.head).await.unwrap();
    assert_eq!(
        store.sudo().read("settings", record.head.id).await,
        Err(Error::Missing)
    );
    assert!(
        core.live("documentstore:document:revision")
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn binding() {
    let home = tempfile::tempdir().unwrap();
    let root = home.path().join("document");
    let core = core(home.path()).await;
    let store = Store::create(core.clone(), model(), &root).await.unwrap();
    let record = store
        .sudo()
        .create("settings", "alice", document("{}"))
        .await
        .unwrap();
    assert!(Store::open(core.clone(), model(), &root).await.is_err());
    drop(store);
    let other = Model::new(vec![Collection::new("settings", Shape::Text).unwrap()]).unwrap();
    assert!(matches!(
        Store::open(core.clone(), other, &root).await,
        Err(Error::Binding(_))
    ));
    let foreign = tempfile::tempdir().unwrap();
    assert!(matches!(
        Store::open(self::core(foreign.path()).await, model(), &root).await,
        Err(Error::Foreign(_))
    ));
    let store = Store::open(core, model(), &root).await.unwrap();
    assert_eq!(
        store.sudo().read("settings", record.head.id).await.unwrap(),
        record
    );
}

mod support;

use keel_document::{Error, Store};
use support::{core, document, model, set};

#[tokio::test]
async fn writers() {
    let home = tempfile::tempdir().unwrap();
    let store = Store::create(
        core(home.path()).await,
        model(),
        &home.path().join("document"),
    )
    .await
    .unwrap();
    let initial = store
        .sudo()
        .create("settings", "alice", document("{}"))
        .await
        .unwrap();
    let left = store.sudo();
    let right = store.sudo();
    let first = [set("/mode", "true")];
    let second = [set("/mode", "false")];
    let (left, right) = tokio::join!(
        left.edit(&initial.head, &first),
        right.edit(&initial.head, &second)
    );
    let (winner, loser) = match left {
        Ok(winner) => (winner, right),
        Err(error) => (right.unwrap(), Err(error)),
    };
    assert!(matches!(loser, Err(Error::Conflict { .. })));
    assert_eq!(
        store
            .sudo()
            .read("settings", initial.head.id)
            .await
            .unwrap(),
        winner
    );
}

#[tokio::test]
async fn retire() {
    let home = tempfile::tempdir().unwrap();
    let store = Store::create(
        core(home.path()).await,
        model(),
        &home.path().join("document"),
    )
    .await
    .unwrap();
    let initial = store
        .sudo()
        .create("settings", "alice", document("{}"))
        .await
        .unwrap();
    let ending = store.sudo();
    let editing = store.sudo();
    let change = [set("/mode", "true")];
    let (end, edit) = tokio::join!(
        ending.end(&initial.head),
        editing.edit(&initial.head, &change)
    );
    assert_ne!(end.is_ok(), edit.is_ok());
    match edit {
        Ok(record) => assert_eq!(
            store.sudo().read("settings", record.head.id).await.unwrap(),
            record
        ),
        Err(Error::Missing) => assert_eq!(
            store.sudo().read("settings", initial.head.id).await,
            Err(Error::Missing)
        ),
        other => panic!("unexpected edit: {other:?}"),
    }
}

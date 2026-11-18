mod support;

use keel_document::{Condition, Error, Projection, Query, Store};
use radicula_document::Path;
use support::{core, document, model, set};

fn projection(name: &str, path: &str) -> Projection {
    Projection {
        name: name.into(),
        path: Path::parse(path).unwrap(),
    }
}

#[tokio::test]
async fn paths() {
    let home = tempfile::tempdir().unwrap();
    let store = Store::create(
        core(home.path()).await,
        model(),
        &home.path().join("document"),
    )
    .await
    .unwrap();
    store
        .sudo()
        .create(
            "settings",
            "alice",
            document(r#"{"a.b":[{"a/b":null,"~":false}]}"#),
        )
        .await
        .unwrap();
    let mut query = Query {
        conditions: vec![],
        select: vec![
            projection("null", "/a.b/0/a~1b"),
            projection("tilde", "/a.b/0/~0"),
            projection("missing", "/absent"),
            projection("root", ""),
        ],
    };
    let result = store.sudo().query("settings", &query).await.unwrap();
    assert_eq!(result.len(), 1);
    assert!(result[0].values["null"].is_null());
    assert_eq!(result[0].values["tilde"], false);
    assert!(!result[0].values.contains_key("missing"));
    assert!(result[0].values["root"].is_object());
    query.conditions = vec![Condition::Missing(Path::parse("/a.b/0/a~1b").unwrap())];
    assert!(
        store
            .sudo()
            .query("settings", &query)
            .await
            .unwrap()
            .is_empty()
    );
    query.conditions = vec![Condition::Equal {
        path: Path::parse("/a.b/0/a~1b").unwrap(),
        value: document("null").take(),
    }];
    assert_eq!(
        store.sudo().query("settings", &query).await.unwrap().len(),
        1
    );
    query.select = vec![projection("kind", "/a.b/0/~0/nope")];
    assert!(matches!(
        store.sudo().query("settings", &query).await,
        Err(Error::Document(radicula_document::Error::Kind))
    ));
}

#[tokio::test]
async fn authority() {
    let home = tempfile::tempdir().unwrap();
    let core = core(home.path()).await;
    let store = Store::create(core.clone(), model(), &home.path().join("document"))
        .await
        .unwrap();
    let actor = core.put("Actor", &[("login", "alice")]).await.unwrap();
    let root = core.live("documentstore").await.unwrap()[0].key();
    let grant = core
        .put(
            "@grant",
            &[
                ("who", &actor.to_string()),
                ("verb", "*"),
                ("unit", "documentstore"),
                ("scope", &format!("row {root}")),
            ],
        )
        .await
        .unwrap();
    let record = store
        .of(actor)
        .create("settings", "alice", document("{}"))
        .await
        .unwrap();
    store
        .sudo()
        .create("settings", "bob", document("{}"))
        .await
        .unwrap();
    let query = Query {
        select: vec![projection("root", "")],
        ..Query::default()
    };
    assert_eq!(
        store
            .of(actor)
            .query("settings", &query)
            .await
            .unwrap()
            .len(),
        2
    );
    assert!(
        store
            .anon()
            .query("settings", &query)
            .await
            .unwrap()
            .is_empty()
    );
    store
        .of(actor)
        .edit(&record.head, &[set("/theme", "null")])
        .await
        .unwrap();
    core.end("@grant", grant).await.unwrap();
    assert!(
        store
            .of(actor)
            .create("settings", "carol", document("{}"))
            .await
            .is_err()
    );
    let bob = core.put("Actor", &[("login", "bob")]).await.unwrap();
    let see = core
        .put(
            "@grant",
            &[
                ("who", &bob.to_string()),
                ("verb", "see"),
                ("unit", "documentstore:document"),
                ("scope", &format!("row {}", record.head.id)),
            ],
        )
        .await
        .unwrap();
    assert_eq!(
        store.of(bob).query("settings", &query).await.unwrap().len(),
        1
    );
    core.end("@grant", see).await.unwrap();
    assert!(
        store
            .of(bob)
            .query("settings", &query)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn empty() {
    let home = tempfile::tempdir().unwrap();
    let store = Store::create(
        core(home.path()).await,
        model(),
        &home.path().join("document"),
    )
    .await
    .unwrap();
    let query = Query {
        select: vec![projection("same", ""), projection("same", "/x")],
        ..Query::default()
    };
    assert!(matches!(
        store.sudo().query("settings", &query).await,
        Err(Error::Model(_))
    ));
}

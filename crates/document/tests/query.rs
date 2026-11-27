mod support;

use keel_document::{Condition, Error, Limit, Projection, Query, Store};
use radicula_document::Path;
use support::{core, document, model, set};

fn projection(name: &str, path: &str) -> Projection {
    Projection {
        name: name.into(),
        path: Path::parse(path).unwrap(),
    }
}

fn base() -> Query {
    Query {
        collection: "settings".into(),
        ..Query::default()
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
        select: vec![
            projection("null", "/a.b/0/a~1b"),
            projection("tilde", "/a.b/0/~0"),
            projection("missing", "/absent"),
            projection("root", ""),
        ],
        ..base()
    };
    let (result, stats) = store.sudo().query(&query).await.unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(stats.candidates, 1);
    assert_eq!(stats.admitted, 1);
    assert_eq!(stats.matched, 1);
    assert!(stats.bytes > 0);
    assert!(result[0].values["null"].is_null());
    assert_eq!(result[0].values["tilde"], false);
    assert!(!result[0].values.contains_key("missing"));
    assert!(result[0].values["root"].is_object());
    query.conditions = vec![Condition::Missing(Path::parse("/a.b/0/a~1b").unwrap())];
    assert!(store.sudo().query(&query).await.unwrap().0.is_empty());
    query.conditions = vec![Condition::Equal {
        path: Path::parse("/a.b/0/a~1b").unwrap(),
        value: document("null").take(),
    }];
    assert_eq!(store.sudo().query(&query).await.unwrap().0.len(), 1);
    query.select = vec![projection("kind", "/a.b/0/~0/nope")];
    assert!(matches!(
        store.sudo().query(&query).await,
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
        .grant(keel::Grant::new(
            &actor.to_string(),
            "*",
            "documentstore",
            &format!("row {root}"),
        ))
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
        ..base()
    };
    let (seen, stats) = store.of(actor).query(&query).await.unwrap();
    assert_eq!(seen.len(), 2);
    assert_eq!(stats.candidates, 2);
    assert_eq!(stats.admitted, 2);
    assert!(store.anon().query(&query).await.unwrap().0.is_empty());
    store
        .of(actor)
        .edit(&record.head, &[set("/theme", "null")])
        .await
        .unwrap();
    core.revoke(grant).await.unwrap();
    assert!(
        store
            .of(actor)
            .create("settings", "carol", document("{}"))
            .await
            .is_err()
    );
    let bob = core.put("Actor", &[("login", "bob")]).await.unwrap();
    let see = core
        .grant(keel::Grant::new(
            &bob.to_string(),
            "see",
            "documentstore:document",
            &format!("row {}", record.head.id),
        ))
        .await
        .unwrap();
    let (seen, stats) = store.of(bob).query(&query).await.unwrap();
    assert_eq!(seen.len(), 1);
    assert_eq!(stats.candidates, 2);
    assert_eq!(stats.admitted, 1);
    core.revoke(see).await.unwrap();
    assert!(store.of(bob).query(&query).await.unwrap().0.is_empty());
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
        ..base()
    };
    assert!(matches!(
        store.sudo().query(&query).await,
        Err(Error::Model(_))
    ));
}

#[tokio::test]
async fn budgets() {
    let home = tempfile::tempdir().unwrap();
    let store = Store::create(
        core(home.path()).await,
        model(),
        &home.path().join("document"),
    )
    .await
    .unwrap();
    for name in ["a", "b"] {
        store
            .sudo()
            .create("settings", name, document("{}"))
            .await
            .unwrap();
    }
    for limit in [
        Limit {
            candidates: 1,
            rows: 10,
            bytes: 1024,
        },
        Limit {
            candidates: 10,
            rows: 1,
            bytes: 1024,
        },
        Limit {
            candidates: 10,
            rows: 10,
            bytes: 0,
        },
        Limit {
            candidates: 0,
            rows: 0,
            bytes: 0,
        },
    ] {
        let query = Query { limit, ..base() };
        assert!(matches!(
            store.sudo().query(&query).await,
            Err(Error::Query(_))
        ));
    }
}

#[tokio::test]
async fn collection() {
    let home = tempfile::tempdir().unwrap();
    let store = Store::create(
        core(home.path()).await,
        model(),
        &home.path().join("document"),
    )
    .await
    .unwrap();
    let query = Query {
        collection: "absent".into(),
        ..Query::default()
    };
    assert!(matches!(
        store.sudo().query(&query).await,
        Err(Error::Collection(_))
    ));
}

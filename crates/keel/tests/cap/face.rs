use crate::world::*;
use axum::http::{HeaderMap, StatusCode};
use keel::adapt::db::Sqlite;
use keel::serve::{Operator, admit};
use keel::{Graph, Who};

#[tokio::test]
async fn faces() {
    let mut graph = Graph::new();
    graph.plug::<Actor>();
    let core = crate::support::boot(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind");
    let ada = core.put("Actor", &[("login", "ada")]).await.expect("ada");

    let op = core.of(ada);
    assert_eq!(op.who(), Who::Op(ada));
    assert!(op.put("Actor", &[("login", "eve")]).await.is_err());
    assert_eq!(op.live("Actor").await.expect("live").len(), 0);
    assert_eq!(
        core.anon()
            .query("from Actor")
            .await
            .expect("q")
            .rows()
            .len(),
        0
    );
    assert_eq!(core.anon().who(), Who::Anon);

    let headers = HeaderMap::new();
    assert_eq!(
        admit(&core, &headers, Some(&Operator(ada)), "see")
            .await
            .expect("admit")
            .who(),
        Who::Op(ada)
    );
    let mut bad = HeaderMap::new();
    bad.insert("authorization", "sudo wrong".parse().expect("header"));
    assert_eq!(
        admit(&core, &bad, None, "see").await.err(),
        Some(StatusCode::UNAUTHORIZED)
    );
}

#[tokio::test]
async fn birth() {
    let mut graph = Graph::new();
    graph.plug::<Actor>();
    let core = crate::support::boot(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind")
        .identify("Actor")
        .expect("identify");
    let sudo = core.sudo();
    sudo.put(
        "@grant",
        &[
            ("who", "anon"),
            ("verb", "put"),
            ("unit", "Actor"),
            ("scope", "all"),
        ],
    )
    .await
    .expect("register seed");

    let eve = core
        .anon()
        .put("Actor", &[("login", "eve")])
        .await
        .expect("register");
    let own = core.of(eve);
    own.set("Actor", eve, &[("login", "eva")])
        .await
        .expect("newborn owns itself");
    assert_eq!(own.live("Actor").await.expect("live").len(), 1);
    assert!(
        core.of(eve + 1)
            .set("Actor", eve, &[("login", "x")])
            .await
            .is_err()
    );

    let root = sudo
        .batch(async |tx| tx.birth(&[("login", "root")]).await)
        .await
        .expect("sudo birth");
    core.of(root)
        .set("Actor", root, &[("login", "rooted")])
        .await
        .expect("born operator owns itself");
    assert!(
        core.of(root)
            .batch(async |tx| tx.birth(&[("login", "nested")]).await)
            .await
            .is_err()
    );
}

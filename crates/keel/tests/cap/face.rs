use crate::world::*;
use keel::adapt::db::Sqlite;
use keel::{Graph, Who, bind};

#[tokio::test]
async fn faces() {
    let mut graph = Graph::new();
    graph.plug::<Actor>();
    let core = bind(graph, Sqlite::memory().await.expect("db"))
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
}

#[tokio::test]
async fn birth() {
    let mut graph = Graph::new();
    graph.plug::<Actor>();
    let core = bind(graph, Sqlite::memory().await.expect("db"))
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
}

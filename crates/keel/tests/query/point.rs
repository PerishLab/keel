use crate::probe::{Org, Repo, Tally, stand};
use keel::{Graph, Op, form};
use std::sync::Arc;
use std::sync::atomic::Ordering;

#[tokio::test]
async fn exact() {
    let tally = Arc::new(Tally::default());
    let mut graph = Graph::new();
    graph.plug::<Org>();
    let core = stand(graph, tally.clone()).await;
    let mut wanted = 0;
    for number in 0..128 {
        wanted = core
            .put("Org", &[("name", &format!("org{number}"))])
            .await
            .expect("put");
    }
    let tree = form("Org").when("id", Op::Eq, &format!("+0{wanted}"));
    tally.materialized.store(0, Ordering::Relaxed);
    let row = core.one(&tree).await.expect("point").expect("row");
    assert_eq!(row.key(), wanted);
    assert_eq!(row.text("name"), Some("org127"));
    assert_eq!(tally.materialized.load(Ordering::Relaxed), 1);

    tally.materialized.store(0, Ordering::Relaxed);
    let denied = tree.clone().when("name", Op::Eq, "other");
    assert!(core.one(&denied).await.expect("mixed").is_none());
    assert_eq!(tally.materialized.load(Ordering::Relaxed), 1);
    let counted = core.ask(&tree.clone().count()).await.expect("count");
    assert_eq!(counted.count(), Some(1));
    assert!(
        core.ask(&tree.clone().top(0))
            .await
            .expect("page")
            .rows()
            .is_empty()
    );
    assert!(
        core.ask(&tree.clone().past(wanted))
            .await
            .expect("cursor")
            .rows()
            .is_empty()
    );

    tally.materialized.store(0, Ordering::Relaxed);
    assert!(
        core.one(&form("Org").when("id", Op::Eq, "9999"))
            .await
            .expect("missing")
            .is_none()
    );
    assert_eq!(tally.materialized.load(Ordering::Relaxed), 0);
    assert!(
        core.one(&form("Org").when("id", Op::Eq, "invalid"))
            .await
            .is_err()
    );
    assert_eq!(tally.materialized.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn expiry() {
    let tally = Arc::new(Tally::default());
    let mut graph = Graph::new();
    graph.plug::<Org>().plug::<Repo>();
    let core = stand(graph, tally.clone()).await;
    let owner = core.put("Org", &[("name", "owner")]).await.expect("owner");
    let repo = core
        .put("Repo", &[("name", "repo"), ("org", &owner.to_string())])
        .await
        .expect("repo");
    let tree = form("Repo").when("id", Op::Eq, &repo.to_string());
    assert_eq!(
        core.one(&tree).await.expect("live").expect("repo").key(),
        repo
    );
    core.end("Repo", repo).await.expect("expire");
    tally.materialized.store(0, Ordering::Relaxed);
    assert!(core.one(&tree).await.expect("expired").is_none());
    assert_eq!(tally.materialized.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn authority() {
    let tally = Arc::new(Tally::default());
    let mut graph = Graph::new();
    graph.plug::<Org>();
    let core = stand(graph, tally.clone())
        .await
        .identify("Org")
        .expect("identity");
    let visible = core
        .put("Org", &[("name", "visible")])
        .await
        .expect("visible");
    let hidden = core
        .put("Org", &[("name", "hidden")])
        .await
        .expect("hidden");
    let open = form("Org").when("id", Op::Eq, &visible.to_string());
    let closed = form("Org").when("id", Op::Eq, &hidden.to_string());
    assert!(core.one(&closed).await.expect("possession").is_some());
    assert!(core.of(1).one(&open).await.expect("no grant").is_none());
    let grant = core
        .put(
            "@grant",
            &[
                ("who", "all"),
                ("verb", "see"),
                ("unit", "Org"),
                ("scope", "pred name = \"visible\""),
            ],
        )
        .await
        .expect("grant");
    assert_eq!(
        core.of(1)
            .one(&open)
            .await
            .expect("permitted")
            .expect("row")
            .key(),
        visible
    );
    assert!(core.of(1).one(&closed).await.expect("other row").is_none());
    core.end("@grant", grant).await.expect("revoke");
    assert!(core.of(1).one(&open).await.expect("revoked").is_none());
}

use crate::probe::{Org, Tally, stand};
use keel::atom::string;
use keel::{Graph, Op, form, resource};
use std::sync::Arc;
use std::sync::atomic::Ordering;

#[resource]
pub struct Ticket {
    #[field(string, unique)]
    tag: string,
    #[field(string)]
    name: string,
}

#[resource]
pub struct Slip {
    #[field(string, unique, opt)]
    tag: string,
    #[field(string)]
    name: string,
}

#[tokio::test]
async fn optional() {
    let tally = Arc::new(Tally::default());
    let mut graph = Graph::new();
    graph.plug::<Slip>();
    let core = stand(graph, tally.clone()).await;
    core.put("Slip", &[("name", "absent")])
        .await
        .expect("absent");
    let empty = core
        .put("Slip", &[("tag", ""), ("name", "empty")])
        .await
        .expect("empty");
    tally.materialized.store(0, Ordering::Relaxed);
    assert_eq!(
        core.one(&form("Slip").when("tag", Op::Eq, ""))
            .await
            .expect("empty query")
            .expect("row")
            .key(),
        empty
    );
    assert_eq!(tally.materialized.load(Ordering::Relaxed), 1);
    let null = core
        .ask(&form("Slip").when("tag", Op::Null, ""))
        .await
        .expect("null query");
    assert_eq!(null.rows().len(), 1);
    assert_eq!(null.rows()[0].text("name"), Some("absent"));
}

#[tokio::test]
async fn exact() {
    let tally = Arc::new(Tally::default());
    let mut graph = Graph::new();
    graph.plug::<Ticket>();
    let core = stand(graph, tally.clone()).await;
    for number in 0..128 {
        core.put(
            "Ticket",
            &[("tag", &format!("ticket{number}")), ("name", "ordinary")],
        )
        .await
        .expect("put");
    }
    let value = "É' OR 1=1 --";
    let wanted = core
        .put("Ticket", &[("tag", value), ("name", "selected")])
        .await
        .expect("selected");
    let tree = form("Ticket").when("tag", Op::Eq, value);
    tally.materialized.store(0, Ordering::Relaxed);
    assert_eq!(
        core.one(&tree).await.expect("unique").expect("row").key(),
        wanted
    );
    assert_eq!(tally.materialized.load(Ordering::Relaxed), 1);

    tally.materialized.store(0, Ordering::Relaxed);
    assert!(
        core.one(&tree.clone().when("name", Op::Eq, "other"))
            .await
            .expect("mixed")
            .is_none()
    );
    assert_eq!(tally.materialized.load(Ordering::Relaxed), 0);
    assert_eq!(
        core.ask(&tree.clone().count())
            .await
            .expect("count")
            .count(),
        Some(1)
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
        core.one(&form("Ticket").when("tag", Op::Eq, "é' OR 1=1 --"))
            .await
            .expect("case")
            .is_none()
    );
    assert_eq!(tally.materialized.load(Ordering::Relaxed), 0);
    core.end("Ticket", wanted).await.expect("expire");
    tally.materialized.store(0, Ordering::Relaxed);
    assert!(core.one(&tree).await.expect("expired").is_none());
    assert_eq!(tally.materialized.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn authority() {
    let tally = Arc::new(Tally::default());
    let mut graph = Graph::new();
    graph.plug::<Org>().plug::<Ticket>();
    let core = stand(graph, tally).await.identify("Org").expect("identity");
    core.put("Org", &[("name", "operator")])
        .await
        .expect("operator");
    let visible = core
        .put("Ticket", &[("tag", "visible"), ("name", "shared")])
        .await
        .expect("visible");
    core.put("Ticket", &[("tag", "hidden"), ("name", "shared")])
        .await
        .expect("hidden");
    let open = form("Ticket").when("tag", Op::Eq, "visible");
    let closed = form("Ticket").when("tag", Op::Eq, "hidden");
    assert!(core.one(&closed).await.expect("possession").is_some());
    assert!(core.of(1).one(&open).await.expect("no grant").is_none());
    let grant = core
        .put(
            "@grant",
            &[
                ("who", "all"),
                ("verb", "see"),
                ("unit", "Ticket"),
                ("scope", "pred tag = \"visible\""),
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
    assert!(core.of(1).one(&closed).await.expect("hidden").is_none());
    core.end("@grant", grant).await.expect("revoke");
    assert!(core.of(1).one(&open).await.expect("revoked").is_none());
}

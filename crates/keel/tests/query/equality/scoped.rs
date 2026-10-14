use crate::probe::{Tally, stand};
use keel::atom::{int, string};
use keel::{Graph, Op, Rank, form, resource};
use std::sync::Arc;
use std::sync::atomic::Ordering;

#[resource]
pub struct Entry {
    #[field(string)]
    kind: string,
    #[field(string, unique = kind)]
    target: string,
    #[field(int)]
    weight: int,
}

#[tokio::test]
async fn scoped() {
    let tally = Arc::new(Tally::default());
    let mut graph = Graph::new();
    graph.plug::<Entry>();
    let core = stand(graph, tally.clone()).await;
    for n in 0..64 {
        core.put(
            "Entry",
            &[
                ("kind", "message"),
                ("target", &format!("other{n}")),
                ("weight", "0"),
            ],
        )
        .await
        .expect("distractor");
    }
    let value = "É' OR 1=1 --";
    let first = core
        .put(
            "Entry",
            &[("kind", "message"), ("target", value), ("weight", "1")],
        )
        .await
        .expect("first");
    let second = core
        .put(
            "Entry",
            &[("kind", "thinking"), ("target", value), ("weight", "2")],
        )
        .await
        .expect("second");
    let tree = form("Entry").when("target", Op::Eq, value);
    tally.materialized.store(0, Ordering::Relaxed);
    let all = core.ask(&tree).await.expect("non-global equality");
    assert_eq!(
        all.rows().iter().map(|r| r.key()).collect::<Vec<_>>(),
        vec![first, second]
    );
    assert_eq!(tally.materialized.load(Ordering::Relaxed), 2);

    for selected in [
        tree.clone().when("kind", Op::Eq, "message"),
        form("Entry")
            .when("kind", Op::Eq, "message")
            .when("target", Op::Eq, value),
    ] {
        tally.materialized.store(0, Ordering::Relaxed);
        assert_eq!(
            core.one(&selected)
                .await
                .expect("conjunction")
                .expect("selected")
                .key(),
            first
        );
        assert_eq!(tally.materialized.load(Ordering::Relaxed), 1);
    }
    tally.materialized.store(0, Ordering::Relaxed);
    let mixed = tree.clone().when("weight", Op::Gt, "1");
    assert_eq!(
        core.one(&mixed)
            .await
            .expect("mixed")
            .expect("second")
            .key(),
        second
    );
    assert_eq!(tally.materialized.load(Ordering::Relaxed), 2);
    assert_eq!(
        core.ask(&tree.clone().count())
            .await
            .expect("count")
            .count(),
        Some(2)
    );
    let page = core
        .ask(&tree.clone().order("weight", Rank::Desc).top(1))
        .await
        .expect("page");
    assert_eq!(page.rows()[0].key(), second);
    core.end("Entry", first).await.expect("expire");
    tally.materialized.store(0, Ordering::Relaxed);
    assert_eq!(
        core.one(&tree)
            .await
            .expect("live")
            .expect("remaining")
            .key(),
        second
    );
    assert_eq!(tally.materialized.load(Ordering::Relaxed), 1);
}

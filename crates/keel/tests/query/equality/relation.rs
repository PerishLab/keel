use crate::probe::{Org, Tally, stand};
use keel::atom::string;
use keel::{Graph, Op, Rank, form, resource};
use std::sync::Arc;
use std::sync::atomic::Ordering;

#[resource]
pub struct Note {
    #[field(string)]
    label: string,
    #[relation(Org, many2one)]
    owner: Org,
    #[relation(Org, many2one, opt)]
    peer: Org,
}

#[tokio::test]
async fn selected() {
    let tally = Arc::new(Tally::default());
    let mut graph = Graph::new();
    graph.plug::<Org>().plug::<Note>();
    let core = stand(graph, tally.clone()).await;
    let first = core.put("Org", &[("name", "first")]).await.unwrap();
    let second = core.put("Org", &[("name", "second")]).await.unwrap();
    for n in 0..32 {
        core.put(
            "Note",
            &[("label", &n.to_string()), ("owner", &second.to_string())],
        )
        .await
        .unwrap();
    }
    let wanted = core
        .put(
            "Note",
            &[
                ("label", "É' OR 1=1 --"),
                ("owner", &first.to_string()),
                ("peer", &second.to_string()),
            ],
        )
        .await
        .unwrap();
    let other = core
        .put("Note", &[("label", "other"), ("owner", &first.to_string())])
        .await
        .unwrap();
    let tree = form("Note").when("owner", Op::Eq, &first.to_string());
    tally.materialized.store(0, Ordering::Relaxed);
    assert_eq!(core.ask(&tree).await.unwrap().rows().len(), 2);
    assert_eq!(tally.materialized.load(Ordering::Relaxed), 2);
    for query in [
        tree.clone().when("peer", Op::Eq, &second.to_string()),
        form("Note").when("peer", Op::Eq, &second.to_string()).when(
            "owner",
            Op::Eq,
            &first.to_string(),
        ),
        tree.clone().when("label", Op::Eq, "É' OR 1=1 --"),
    ] {
        tally.materialized.store(0, Ordering::Relaxed);
        assert_eq!(core.one(&query).await.unwrap().unwrap().key(), wanted);
        assert_eq!(tally.materialized.load(Ordering::Relaxed), 1);
    }
    assert_eq!(
        core.ask(&tree.clone().count()).await.unwrap().count(),
        Some(2)
    );
    assert_eq!(
        core.ask(&tree.clone().order("label", Rank::Asc).top(1))
            .await
            .unwrap()
            .rows()[0]
            .key(),
        other
    );
    assert_eq!(
        core.ask(&tree.clone().past(wanted)).await.unwrap().rows()[0].key(),
        other
    );
    core.end("Note", wanted).await.unwrap();
    tally.materialized.store(0, Ordering::Relaxed);
    assert_eq!(core.one(&tree).await.unwrap().unwrap().key(), other);
    assert_eq!(tally.materialized.load(Ordering::Relaxed), 1);
    core.end("Note", other).await.unwrap();
    tally.materialized.store(0, Ordering::Relaxed);
    assert!(core.one(&tree).await.unwrap().is_none());
    assert_eq!(tally.materialized.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn literals() {
    let tally = Arc::new(Tally::default());
    let mut graph = Graph::new();
    graph.plug::<Org>().plug::<Note>();
    let core = stand(graph, tally.clone()).await;
    let owner = core.put("Org", &[("name", "owner")]).await.unwrap();
    let wanted = core
        .put("Note", &[("label", "set"), ("owner", &owner.to_string())])
        .await
        .unwrap();
    for value in [owner.to_string(), format!("+{owner}"), format!("00{owner}")] {
        tally.materialized.store(0, Ordering::Relaxed);
        assert_eq!(
            core.one(&form("Note").when("owner", Op::Eq, &value))
                .await
                .unwrap()
                .unwrap()
                .key(),
            wanted
        );
        assert_eq!(tally.materialized.load(Ordering::Relaxed), 1);
    }
    for value in ["", "junk", "9223372036854775808", "1 OR 1=1", " 1"] {
        tally.materialized.store(0, Ordering::Relaxed);
        assert_eq!(
            core.one(&form("Note").when("owner", Op::Eq, value))
                .await
                .unwrap_err()
                .to_string(),
            "adapt: id needs integer"
        );
        assert_eq!(tally.materialized.load(Ordering::Relaxed), 0);
    }
    for value in ["-1", "0", "9999"] {
        tally.materialized.store(0, Ordering::Relaxed);
        assert!(
            core.one(&form("Note").when("owner", Op::Eq, value))
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(tally.materialized.load(Ordering::Relaxed), 0);
    }
    assert_eq!(
        core.one(&form("Note").when("peer", Op::Null, ""))
            .await
            .unwrap()
            .unwrap()
            .key(),
        wanted
    );
    assert!(
        core.one(&form("Note").when("peer", Op::Eq, &owner.to_string()))
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        core.one(&form("Note").when("owner", Op::Ne, "9999"))
            .await
            .unwrap()
            .unwrap()
            .key(),
        wanted
    );
}

#[resource]
pub struct Seal {
    #[field(string)]
    label: string,
    #[relation(Org, one2one, root)]
    owner: Org,
}

#[tokio::test]
async fn rooted() {
    let tally = Arc::new(Tally::default());
    let mut graph = Graph::new();
    graph.plug::<Org>().plug::<Seal>();
    let core = stand(graph, tally.clone()).await;
    let owner = core.put("Org", &[("name", "owner")]).await.unwrap();
    let other = core.put("Org", &[("name", "other")]).await.unwrap();
    let wanted = core
        .put("Seal", &[("label", "set"), ("owner", &owner.to_string())])
        .await
        .unwrap();
    core.put("Seal", &[("label", "other"), ("owner", &other.to_string())])
        .await
        .unwrap();
    tally.materialized.store(0, Ordering::Relaxed);
    assert_eq!(
        core.one(&form("Seal").when("owner", Op::Eq, &owner.to_string()))
            .await
            .unwrap()
            .unwrap()
            .key(),
        wanted
    );
    assert_eq!(tally.materialized.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn authority() {
    let tally = Arc::new(Tally::default());
    let mut graph = Graph::new();
    graph.plug::<Org>().plug::<Note>();
    let core = stand(graph, tally).await.identify("Org").unwrap();
    let owner = core.put("Org", &[("name", "owner")]).await.unwrap();
    let wanted = core
        .put(
            "Note",
            &[("label", "visible"), ("owner", &owner.to_string())],
        )
        .await
        .unwrap();
    core.put(
        "Note",
        &[("label", "hidden"), ("owner", &owner.to_string())],
    )
    .await
    .unwrap();
    let tree = form("Note").when("owner", Op::Eq, &owner.to_string());
    assert!(core.of(owner).one(&tree).await.unwrap().is_none());
    let grant = core
        .put(
            "@grant",
            &[
                ("who", "all"),
                ("verb", "see"),
                ("unit", "Note"),
                ("scope", "pred label = \"visible\""),
            ],
        )
        .await
        .unwrap();
    assert_eq!(
        core.of(owner).one(&tree).await.unwrap().unwrap().key(),
        wanted
    );
    core.end("@grant", grant).await.unwrap();
    assert!(core.of(owner).one(&tree).await.unwrap().is_none());
}

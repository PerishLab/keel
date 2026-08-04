use super::*;
use keel::adapt::Error;
use keel::atom::Kind;
use keel::bond::Kind as Bond;
use keel::spec::{Only, Spec};
use keel::{bind, bootstrap};

async fn reject(spec: Spec, expected: &str) {
    let mut graph = Graph::new();
    graph.add(spec);
    let wire = Sqlite::memory().await.expect("db");
    match bootstrap(graph, wire) {
        Err(Error::Adapt(note)) => assert!(note.contains(expected), "{note}"),
        Err(err) => panic!("unexpected {err}"),
        Ok(_) => panic!("expected rejection"),
    }
}

fn spec(extra: u8, closure: bool) -> Spec {
    let held = Spec::build("Actor").field("name", Kind::Text);
    let held = if extra > 0 {
        held.optional("tag", Kind::Text, Only::Free)
    } else {
        held
    };
    let held = if extra > 1 {
        held.optional("note", Kind::Text, Only::Free)
    } else {
        held
    };
    if closure {
        held.closure("members", Bond::Many2many, "Actor", &[])
            .seal()
    } else {
        held.bond("members", Bond::Many2many, "Actor", &[]).seal()
    }
}

#[tokio::test]
async fn plan() {
    reject(
        Spec::build("Actor")
            .closure("members", Bond::Many2one, "Actor", &[])
            .seal(),
        "self many2many",
    )
    .await;
    reject(
        Spec::build("Actor")
            .closure("members", Bond::Many2many, "Other", &[])
            .seal(),
        "self many2many",
    )
    .await;
    reject(
        Spec::build("Actor")
            .field("MEMBERS_CLOSURE", Kind::Text)
            .closure("members", Bond::Many2many, "Actor", &[])
            .seal(),
        "duplicate field",
    )
    .await;
    reject(
        Spec::build("Actor")
            .bond("MEMBERS_CLOSURE", Bond::Many2many, "Actor", &[])
            .closure("members", Bond::Many2many, "Actor", &[])
            .seal(),
        "duplicate field",
    )
    .await;
}

#[tokio::test]
async fn ddl() {
    let core = boot().await;
    let actor = core.plan().units().get("actor").expect("actor plan");
    let source = actor
        .bonds()
        .iter()
        .find(|edge| edge.name() == "members")
        .expect("source");
    let closure = actor
        .bonds()
        .iter()
        .find(|edge| edge.name() == "members_closure")
        .expect("closure");
    assert!(source.closure());
    assert_eq!(closure.kind(), Bond::Many2many);
    assert_eq!(closure.target(), source.target());
    assert!(closure.fields().is_empty());
    assert!(core.has("actor_members_closure").await.expect("table"));
    assert_eq!(
        core.cols("actor_members_closure").await.expect("columns"),
        vec![
            "id",
            "actor_id",
            "members_closure_id",
            "expires_at",
            "created_at",
            "updated_at"
        ]
    );
}

#[tokio::test]
async fn evolve() {
    let path = std::env::temp_dir().join(format!(
        "keel-closure-{}-{}.db",
        std::process::id(),
        keel::life::tick()
    ));
    let mut first = Graph::new();
    first.add(spec(0, false));
    let core = crate::support::boot(first, Sqlite::file(&path).await.expect("first"))
        .await
        .expect("boot");
    let ids = seed(&core, &["ada", "bob", "cy"]).await;
    core.tie("Actor", "members", ends(ids[0], ids[1]), &[])
        .await
        .expect("ab");
    core.tie("Actor", "members", ends(ids[1], ids[2]), &[])
        .await
        .expect("bc");
    drop(core);

    let mut second = Graph::new();
    second.add(spec(1, true));
    let core = bind(second, Sqlite::file(&path).await.expect("second"))
        .await
        .expect("evolve");
    let ada = core
        .ties("Actor", "members_closure", ids[0])
        .await
        .expect("backfilled");
    let bob = core
        .ties("Actor", "members_closure", ids[1])
        .await
        .expect("backfilled");
    assert_eq!(
        ada.iter().map(keel::life::Tie::key).collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(bob[0].key(), 3);
    assert!(
        !core
            .has("@g1:actor_members_closure")
            .await
            .expect("old closure")
    );
    assert!(core.has("@g1:actor_members").await.expect("old direct"));
    drop(core);

    let mut third = Graph::new();
    third.add(spec(2, true));
    let core = bind(third, Sqlite::file(&path).await.expect("third"))
        .await
        .expect("preserve closure");
    assert_eq!(
        core.ties("Actor", "members_closure", ids[0])
            .await
            .expect("stable keys")
            .iter()
            .map(keel::life::Tie::key)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert!(
        core.has("@g2:actor_members_closure")
            .await
            .expect("old generated")
    );
    drop(core);

    let mut fourth = Graph::new();
    fourth.add(spec(2, false));
    let core = bind(fourth, Sqlite::file(&path).await.expect("fourth"))
        .await
        .expect("retire closure");
    assert!(!core.has("actor_members_closure").await.expect("retired"));
    assert!(
        core.has("@g3:actor_members_closure")
            .await
            .expect("retained closure")
    );
    drop(core);
    std::fs::remove_file(path).expect("clean");
}

use super::world::{clean, graph, spot};
use keel::adapt::Error;
use keel::adapt::db::Sqlite;
use keel::atom::Kind;
use keel::bind;
use keel::estate::{Check, Fault};
use keel::spec::{Only, Resource, Spec};

struct Sparse;
struct Dense;
struct Seed;
struct Seeded;
struct Words;
struct Integer;

impl Resource for Sparse {
    fn name() -> &'static str {
        "Presence"
    }

    fn spec() -> Spec {
        Spec::build(Self::name())
            .field("name", Kind::Text)
            .optional("note", Kind::Text, Only::Free)
            .seal()
    }
}

impl Resource for Dense {
    fn name() -> &'static str {
        "Presence"
    }

    fn spec() -> Spec {
        Spec::build(Self::name())
            .field("name", Kind::Text)
            .field("note", Kind::Text)
            .seal()
    }
}

impl Resource for Seed {
    fn name() -> &'static str {
        "Seed"
    }

    fn spec() -> Spec {
        Spec::build(Self::name()).field("name", Kind::Text).seal()
    }
}

impl Resource for Seeded {
    fn name() -> &'static str {
        "Seed"
    }

    fn spec() -> Spec {
        Spec::build(Self::name())
            .field("name", Kind::Text)
            .optional("note", Kind::Text, Only::Free)
            .seal()
    }
}

impl Resource for Words {
    fn name() -> &'static str {
        "MaybeCast"
    }

    fn spec() -> Spec {
        Spec::build(Self::name())
            .optional("value", Kind::Text, Only::Free)
            .seal()
    }
}

impl Resource for Integer {
    fn name() -> &'static str {
        "MaybeCast"
    }

    fn spec() -> Spec {
        Spec::build(Self::name())
            .optional("value", Kind::Int, Only::Free)
            .seal()
    }
}

#[tokio::test]
async fn presence() {
    let path = spot("scalar_presence");
    let core = bind(graph::<Sparse>(), Sqlite::file(&path).await.expect("first"))
        .await
        .expect("bind");
    core.put("Presence", &[("name", "empty")])
        .await
        .expect("put");
    drop(core);

    match bind(graph::<Dense>(), Sqlite::file(&path).await.expect("second")).await {
        Err(Error::Estate(Fault::Blocked { path, check })) => {
            assert_eq!(path, "presence.note");
            assert_eq!(check, Check::Presence);
        }
        Err(err) => panic!("unexpected {err}"),
        Ok(_) => panic!("expected presence block"),
    }
    clean(&path);
}

#[tokio::test]
async fn relax() {
    let path = spot("scalar_relax");
    let core = bind(graph::<Dense>(), Sqlite::file(&path).await.expect("first"))
        .await
        .expect("bind");
    let key = core
        .put("Presence", &[("name", "held"), ("note", "ready")])
        .await
        .expect("put");
    drop(core);

    let core = bind(
        graph::<Sparse>(),
        Sqlite::file(&path).await.expect("second"),
    )
    .await
    .expect("evolve");
    assert_eq!(
        core.live("Presence").await.expect("live")[0].text("note"),
        Some("ready")
    );
    core.unset("Presence", key, &["note"]).await.expect("unset");
    drop(core);
    clean(&path);
}

#[tokio::test]
async fn add() {
    let path = spot("optional_add");
    let core = bind(graph::<Seed>(), Sqlite::file(&path).await.expect("first"))
        .await
        .expect("bind");
    core.put("Seed", &[("name", "held")]).await.expect("put");
    drop(core);

    let core = bind(
        graph::<Seeded>(),
        Sqlite::file(&path).await.expect("second"),
    )
    .await
    .expect("evolve");
    let row = core.live("Seed").await.expect("live").remove(0);
    assert!(!row.cells().contains_key("note"));
    drop(core);
    clean(&path);
}

#[tokio::test]
async fn cast() {
    let path = spot("nullable_cast");
    let core = bind(graph::<Words>(), Sqlite::file(&path).await.expect("first"))
        .await
        .expect("bind");
    core.put("MaybeCast", &[]).await.expect("put null");
    drop(core);

    let core = bind(
        graph::<Integer>(),
        Sqlite::file(&path).await.expect("second"),
    )
    .await
    .expect("evolve");
    let row = core.live("MaybeCast").await.expect("live").remove(0);
    assert!(!row.cells().contains_key("value"));
    drop(core);
    clean(&path);
}

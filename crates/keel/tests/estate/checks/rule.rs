use super::super::world::{clean, graph, spot};
use keel::adapt::Error;
use keel::adapt::db::Sqlite;
use keel::atom::Kind;
use keel::estate::{Check, Fault};
use keel::spec::{Only, Resource, Rule, Spec};
use keel::{Cell, bind};

struct Seed;
struct Filled;
struct Open;
struct Narrow;
struct Sparse;
struct Dense;
struct Zero;
struct One;

impl Resource for Seed {
    fn name() -> &'static str {
        "Seed"
    }

    fn spec() -> Spec {
        Spec::build(Self::name()).field("name", Kind::Text).seal()
    }
}

impl Resource for Filled {
    fn name() -> &'static str {
        "Seed"
    }

    fn spec() -> Spec {
        Spec::build(Self::name())
            .field("name", Kind::Text)
            .field("state", Kind::Text)
            .rule("state", Rule::new().default("ready").values(&["ready"]))
            .seal()
    }
}

impl Resource for Open {
    fn name() -> &'static str {
        "State"
    }

    fn spec() -> Spec {
        Spec::build(Self::name()).field("value", Kind::Text).seal()
    }
}

impl Resource for Narrow {
    fn name() -> &'static str {
        "State"
    }

    fn spec() -> Spec {
        Spec::build(Self::name())
            .field("value", Kind::Text)
            .rule("value", Rule::new().values(&["done", "ready"]))
            .seal()
    }
}

impl Resource for Sparse {
    fn name() -> &'static str {
        "Presence"
    }

    fn spec() -> Spec {
        Spec::build(Self::name())
            .optional("count", Kind::Int, Only::Free)
            .seal()
    }
}

impl Resource for Dense {
    fn name() -> &'static str {
        "Presence"
    }

    fn spec() -> Spec {
        Spec::build(Self::name())
            .field("count", Kind::Int)
            .rule("count", Rule::new().default("1").min(1))
            .seal()
    }
}

fn counter(default: &str) -> Spec {
    Spec::build("Counter")
        .field("count", Kind::Int)
        .rule("count", Rule::new().default(default).min(0))
        .seal()
}

impl Resource for Zero {
    fn name() -> &'static str {
        "Counter"
    }

    fn spec() -> Spec {
        counter("0")
    }
}

impl Resource for One {
    fn name() -> &'static str {
        "Counter"
    }

    fn spec() -> Spec {
        counter("1")
    }
}

#[tokio::test]
async fn add_default() {
    let path = spot("rule_add_default");
    let core = crate::support::boot(graph::<Seed>(), Sqlite::file(&path).await.expect("first"))
        .await
        .expect("bind");
    core.put("Seed", &[("name", "held")]).await.expect("put");
    drop(core);

    let core = bind(
        graph::<Filled>(),
        Sqlite::file(&path).await.expect("second"),
    )
    .await
    .expect("evolve");
    assert_eq!(
        core.live("Seed").await.expect("live")[0].text("state"),
        Some("ready")
    );
    drop(core);
    clean(&path);
}

#[tokio::test]
async fn narrow() {
    let path = spot("rule_narrow");
    let core = crate::support::boot(graph::<Open>(), Sqlite::file(&path).await.expect("first"))
        .await
        .expect("bind");
    let key = core
        .put("State", &[("value", "unknown")])
        .await
        .expect("put");
    drop(core);

    match bind(
        graph::<Narrow>(),
        Sqlite::file(&path).await.expect("blocked"),
    )
    .await
    {
        Err(Error::Estate(Fault::Blocked { path, check })) => {
            assert_eq!(path, "state.value");
            assert_eq!(check, Check::Values);
        }
        Err(err) => panic!("unexpected {err}"),
        Ok(_) => panic!("expected values block"),
    }

    let core = bind(graph::<Open>(), Sqlite::file(&path).await.expect("repair"))
        .await
        .expect("reopen");
    core.set("State", key, &[("value", "ready")])
        .await
        .expect("repair");
    drop(core);
    let core = bind(
        graph::<Narrow>(),
        Sqlite::file(&path).await.expect("narrow"),
    )
    .await
    .expect("evolve");
    drop(core);
    let core = bind(graph::<Open>(), Sqlite::file(&path).await.expect("widen"))
        .await
        .expect("widen");
    core.put("State", &[("value", "anything")])
        .await
        .expect("open");
    drop(core);
    clean(&path);
}

#[tokio::test]
async fn fill_null() {
    let path = spot("rule_fill_null");
    let core = crate::support::boot(graph::<Sparse>(), Sqlite::file(&path).await.expect("first"))
        .await
        .expect("bind");
    core.put("Presence", &[]).await.expect("put");
    drop(core);

    let core = bind(graph::<Dense>(), Sqlite::file(&path).await.expect("second"))
        .await
        .expect("evolve");
    assert_eq!(
        core.live("Presence").await.expect("live")[0].cell("count"),
        Some(&Cell::Int(1))
    );
    drop(core);
    clean(&path);
}

#[tokio::test]
async fn change_default() {
    let path = spot("rule_change_default");
    let core = crate::support::boot(graph::<Zero>(), Sqlite::file(&path).await.expect("first"))
        .await
        .expect("bind");
    core.put("Counter", &[]).await.expect("zero");
    drop(core);

    let core = bind(graph::<One>(), Sqlite::file(&path).await.expect("second"))
        .await
        .expect("evolve");
    core.put("Counter", &[]).await.expect("one");
    let rows = core.live("Counter").await.expect("live");
    assert_eq!(rows[0].cell("count"), Some(&Cell::Int(0)));
    assert_eq!(rows[1].cell("count"), Some(&Cell::Int(1)));
    drop(core);
    clean(&path);
}

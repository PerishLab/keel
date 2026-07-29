use super::super::world::{clean, graph, spot};
use keel::adapt::Error;
use keel::adapt::db::Sqlite;
use keel::atom::Kind;
use keel::bind;
use keel::estate::{Check, Fault};
use keel::spec::{Resource, Rule, Spec};

struct Open;
struct Bounded;
struct Any;
struct False;

impl Resource for Open {
    fn name() -> &'static str {
        "Count"
    }

    fn spec() -> Spec {
        Spec::build(Self::name()).field("value", Kind::Int).seal()
    }
}

impl Resource for Bounded {
    fn name() -> &'static str {
        "Count"
    }

    fn spec() -> Spec {
        Spec::build(Self::name())
            .field("value", Kind::Int)
            .rule("value", Rule::new().min(0).max(2))
            .seal()
    }
}

impl Resource for Any {
    fn name() -> &'static str {
        "Flag"
    }

    fn spec() -> Spec {
        Spec::build(Self::name()).field("value", Kind::Bool).seal()
    }
}

impl Resource for False {
    fn name() -> &'static str {
        "Flag"
    }

    fn spec() -> Spec {
        Spec::build(Self::name())
            .field("value", Kind::Bool)
            .rule("value", Rule::new().values(&["false"]))
            .seal()
    }
}

async fn blocked<T: Resource>(path: &std::path::Path) {
    match bind(graph::<T>(), Sqlite::file(path).await.expect("blocked")).await {
        Err(Error::Estate(Fault::Blocked { path, check })) => {
            assert!(path.ends_with(".value"));
            assert_eq!(check, Check::Values);
        }
        Err(err) => panic!("unexpected {err}"),
        Ok(_) => panic!("expected values block"),
    }
}

#[tokio::test]
async fn range() {
    let path = spot("rule_range");
    let core = crate::support::boot(graph::<Open>(), Sqlite::file(&path).await.expect("first"))
        .await
        .expect("bind");
    let key = core.put("Count", &[("value", "3")]).await.expect("put");
    drop(core);
    blocked::<Bounded>(&path).await;

    let core = bind(graph::<Open>(), Sqlite::file(&path).await.expect("repair"))
        .await
        .expect("reopen");
    core.set("Count", key, &[("value", "2")])
        .await
        .expect("repair");
    drop(core);
    let core = bind(
        graph::<Bounded>(),
        Sqlite::file(&path).await.expect("narrow"),
    )
    .await
    .expect("narrow");
    drop(core);
    let core = bind(graph::<Open>(), Sqlite::file(&path).await.expect("widen"))
        .await
        .expect("widen");
    drop(core);
    clean(&path);
}

#[tokio::test]
async fn boolean() {
    let path = spot("rule_bool");
    let core = crate::support::boot(graph::<Any>(), Sqlite::file(&path).await.expect("first"))
        .await
        .expect("bind");
    core.put("Flag", &[("value", "true")]).await.expect("put");
    drop(core);
    blocked::<False>(&path).await;
    clean(&path);
}

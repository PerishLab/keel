use super::super::world::{clean, graph, spot};
use keel::adapt::Error;
use keel::adapt::db::Sqlite;
use keel::atom::Kind;
use keel::bind;
use keel::estate::Fault;
use keel::spec::{Resource, Spec};

struct Mutable;
struct Frozen;

impl Resource for Mutable {
    fn name() -> &'static str {
        "Fact"
    }

    fn spec() -> Spec {
        Spec::build(Self::name()).field("tag", Kind::Text).seal()
    }
}

impl Resource for Frozen {
    fn name() -> &'static str {
        "Fact"
    }

    fn spec() -> Spec {
        Spec::build(Self::name())
            .field("tag", Kind::Text)
            .freeze()
            .seal()
    }
}

#[tokio::test]
async fn mode() {
    let path = spot("frozen_mode");
    let core = crate::support::boot(
        graph::<Mutable>(),
        Sqlite::file(&path).await.expect("first"),
    )
    .await
    .expect("bind");
    drop(core);
    match bind(
        graph::<Frozen>(),
        Sqlite::file(&path).await.expect("second"),
    )
    .await
    {
        Err(Error::Estate(Fault::Denied { path, note })) => {
            assert_eq!(path, "fact");
            assert!(note.contains("mode"));
        }
        Err(err) => panic!("unexpected {err}"),
        Ok(_) => panic!("expected mode refusal"),
    }
    clean(&path);
}

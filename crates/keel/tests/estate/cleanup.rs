use super::world::*;
use keel::adapt::Error;
use keel::adapt::db::Sqlite;
use keel::config::{Estate, Retain};
use keel::ddl::Grain;
use keel::wire::{Val, Wire};
use keel::{bind, life};

struct Fail<W> {
    wire: W,
}

impl<W: Wire> Wire for Fail<W> {
    fn grain(&self) -> Grain {
        self.wire.grain()
    }

    async fn run(&mut self, sql: &str, args: &[Val]) -> Result<u64, Error> {
        self.wire.run(sql, args).await
    }

    async fn plant(&mut self, sql: &str, args: &[Val]) -> Result<i64, Error> {
        self.wire.plant(sql, args).await
    }

    async fn rows(&mut self, sql: &str, args: &[Val]) -> Result<Vec<Vec<Val>>, Error> {
        self.wire.rows(sql, args).await
    }

    async fn script(&mut self, sql: &str) -> Result<(), Error> {
        if sql.starts_with("DROP TABLE") {
            return Err(Error::Adapt("injected cleanup failure".into()));
        }
        self.wire.script(sql).await
    }
}

#[tokio::test]
async fn expired() {
    let path = spot("cleanup-expired");
    let core = crate::support::boot(graph::<Alpha>(), Sqlite::file(&path).await.expect("first"))
        .await
        .expect("bind");
    drop(core);
    let core = bind(
        pair::<Alpha, Beta>(),
        Sqlite::file(&path).await.expect("second"),
    )
    .await
    .expect("first evolution");
    core.put("Beta", &[("name", "held")])
        .await
        .expect("put beta");
    drop(core);
    let core = bind(graph::<Beta>(), Sqlite::file(&path).await.expect("third"))
        .await
        .expect("second evolution");
    drop(core);

    let mut wire = Sqlite::file(&path).await.expect("age");
    wire.run(
        "UPDATE \"@generation\" SET created = 1, retired = ?1 WHERE state = ?2",
        &[
            Val::Int(life::tick() - 30 * 24 * 60 * 60),
            Val::Text("cleanup".into()),
        ],
    )
    .await
    .expect("age cleanup");
    drop(wire);

    let core = bind(graph::<Beta>(), Sqlite::file(&path).await.expect("collect"))
        .await
        .expect("collect");
    assert!(!core.has("@g1:alpha").await.expect("first gone"));
    assert!(!core.has("@g2:alpha").await.expect("second alpha gone"));
    assert!(!core.has("@g2:beta").await.expect("second beta gone"));
    assert!(core.has("beta").await.expect("active held"));
    assert_eq!(core.live("Beta").await.expect("beta").len(), 1);
    drop(core);

    let mut wire = Sqlite::file(&path).await.expect("inspect");
    let rows = wire
        .rows(
            "SELECT id FROM \"@generation\" WHERE state = ?1",
            &[Val::Text("cleanup".into())],
        )
        .await
        .expect("generations");
    assert!(rows.is_empty());
    drop(wire);
    clean(&path);
}

#[tokio::test]
async fn immediate() {
    let path = spot("cleanup-immediate");
    let core = crate::support::boot(graph::<Alpha>(), Sqlite::file(&path).await.expect("first"))
        .await
        .expect("bind");
    drop(core);

    let mut estate = Estate::default();
    estate.generation.cleanup.retain = Retain::Span(0);
    let core = bind(graph::<Grow>(), Sqlite::file(&path).await.expect("evolve"))
        .estate(&estate)
        .await
        .expect("evolve and collect");
    assert!(!core.has("@g1:alpha").await.expect("cleanup gone"));
    assert!(core.has("alpha").await.expect("active held"));
    drop(core);
    clean(&path);
}

#[tokio::test]
async fn forever() {
    let path = spot("cleanup-forever");
    let core = crate::support::boot(graph::<Alpha>(), Sqlite::file(&path).await.expect("first"))
        .await
        .expect("bind");
    drop(core);
    let core = bind(graph::<Grow>(), Sqlite::file(&path).await.expect("evolve"))
        .await
        .expect("evolve");
    drop(core);
    let mut wire = Sqlite::file(&path).await.expect("age");
    wire.run(
        "UPDATE \"@generation\" SET created = 1, retired = 1 WHERE id = 1",
        &[],
    )
    .await
    .expect("age cleanup");
    drop(wire);

    let mut estate = Estate::default();
    estate.generation.cleanup.retain = Retain::Forever;
    let core = bind(graph::<Grow>(), Sqlite::file(&path).await.expect("retain"))
        .estate(&estate)
        .await
        .expect("retain");
    assert!(core.has("@g1:alpha").await.expect("cleanup held"));
    drop(core);
    clean(&path);
}

#[tokio::test]
async fn rollback() {
    let path = spot("cleanup-rollback");
    let core = crate::support::boot(graph::<Alpha>(), Sqlite::file(&path).await.expect("first"))
        .await
        .expect("bind");
    drop(core);
    let core = bind(graph::<Grow>(), Sqlite::file(&path).await.expect("evolve"))
        .await
        .expect("evolve");
    drop(core);
    let mut wire = Sqlite::file(&path).await.expect("age");
    wire.run(
        "UPDATE \"@generation\" SET created = 1, retired = 1 WHERE id = 1",
        &[],
    )
    .await
    .expect("age cleanup");
    drop(wire);

    let failed = Fail {
        wire: Sqlite::file(&path).await.expect("failed"),
    };
    assert!(bind(graph::<Grow>(), failed).await.is_err());

    let mut estate = Estate::default();
    estate.generation.cleanup.retain = Retain::Forever;
    let core = bind(graph::<Grow>(), Sqlite::file(&path).await.expect("verify"))
        .estate(&estate)
        .await
        .expect("seal remains exact");
    assert!(core.has("@g1:alpha").await.expect("table restored"));
    drop(core);
    let mut wire = Sqlite::file(&path).await.expect("inspect");
    let rows = wire
        .rows(
            "SELECT id FROM \"@generation\" WHERE id = 1 AND state = ?1",
            &[Val::Text("cleanup".into())],
        )
        .await
        .expect("catalog");
    assert_eq!(rows.len(), 1);
    drop(wire);
    clean(&path);
}

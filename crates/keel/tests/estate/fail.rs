use super::world::*;
use keel::adapt::Error;
use keel::adapt::db::Sqlite;
use keel::bind;
use keel::ddl::Grain;
use keel::wire::{Val, Wire};

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
        if sql.contains("@g2:") {
            return Err(Error::Adapt("injected evolution failure".into()));
        }
        self.wire.script(sql).await
    }
}

pub(super) async fn tables(path: &std::path::Path) -> Vec<Vec<Val>> {
    let mut wire = Sqlite::file(path).await.expect("inspect");
    wire.rows(
        "SELECT name FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'",
        &[],
    )
    .await
    .expect("tables")
}

#[tokio::test]
async fn rollback() {
    let path = spot("rollback");
    let core = crate::support::boot(graph::<Alpha>(), Sqlite::file(&path).await.expect("first"))
        .await
        .expect("bind");
    drop(core);

    let wire = Fail {
        wire: Sqlite::file(&path).await.expect("failed"),
    };
    assert!(bind(graph::<Grow>(), wire).await.is_err());

    let core = bind(graph::<Alpha>(), Sqlite::file(&path).await.expect("reopen"))
        .await
        .expect("old active");
    assert!(!core.has("@g2:alpha").await.expect("candidate"));
    drop(core);
    let mut wire = Sqlite::file(&path).await.expect("inspect");
    let rows = wire
        .rows(
            "SELECT value FROM \"@clock\" WHERE name = ?1",
            &[Val::Text("generation".into())],
        )
        .await
        .expect("clock");
    assert_eq!(rows[0][0].int(), 1);
    drop(wire);
    clean(&path);
}

mod custody {
    use super::{Alpha, Grow, graph, tables};
    use keel::adapt::{Error, Sqlite};
    use keel::estate::Fault;
    use keel::{Wire, bind, bootstrap};
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn race() {
        let dir = tempfile::tempdir().expect("directory");
        let path = dir.path().join("estate.sqlite");
        let left = Sqlite::file(&path).await.expect("left");
        let right = Sqlite::file(&path).await.expect("right");
        let calls = AtomicUsize::new(0);
        let mut token = String::new();
        let mut other = String::new();
        let (first, second) = tokio::join!(
            bootstrap(graph::<Alpha>(), left)
                .expect("left bootstrap")
                .custody(|minted| {
                    calls.fetch_add(1, Ordering::Relaxed);
                    token = minted.into();
                    Ok(())
                }),
            bootstrap(graph::<Alpha>(), right)
                .expect("right bootstrap")
                .custody(|minted| {
                    calls.fetch_add(1, Ordering::Relaxed);
                    other = minted.into();
                    Ok(())
                }),
        );
        let (core, token) = match (first.expect("left"), second.expect("right")) {
            (Some(core), None) => (core, token),
            (None, Some(core)) => (core, other),
            _ => panic!("expected one seal and one occupied result"),
        };
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        assert!(core.seal(&token).await.expect("sudo"));

        let wire = Sqlite::file(&path).await.expect("reopen");
        let out = bootstrap(graph::<Grow>(), wire)
            .expect("changed bootstrap")
            .custody(|_| panic!("changed model must skip custody"))
            .await;
        assert!(matches!(out, Err(Error::Estate(Fault::Occupied))));
    }

    #[tokio::test]
    async fn partial() {
        let mut wire = Sqlite::memory().await.expect("db");
        wire.script("CREATE TABLE stray (id INTEGER PRIMARY KEY NOT NULL)")
            .await
            .expect("stray");
        let out = bootstrap(graph::<Alpha>(), wire)
            .expect("bootstrap")
            .custody(|_| panic!("partial estate must skip custody"))
            .await;
        assert!(matches!(out, Err(Error::Estate(Fault::Unsealed))));
    }

    #[tokio::test]
    async fn rollback() {
        let dir = tempfile::tempdir().expect("directory");
        let path = dir.path().join("estate.sqlite");
        let wire = Sqlite::file(&path).await.expect("db");
        let failure = Error::Adapt("custody refused".into());
        let out = bootstrap(graph::<Alpha>(), wire)
            .expect("bootstrap")
            .custody(|_| Err(failure.clone()))
            .await;
        assert!(matches!(out, Err(error) if error == failure));

        let wire = Sqlite::file(&path).await.expect("reopen");
        let bound = bind(graph::<Alpha>(), wire).await;
        assert!(matches!(bound, Err(Error::Estate(Fault::Vacant))));
        assert!(tables(&path).await.is_empty());
    }
}

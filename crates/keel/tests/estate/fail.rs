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

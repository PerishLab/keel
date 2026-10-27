use super::rename::{boot, graph, rows};
use keel::adapt::Error;
use keel::adapt::db::Sqlite;
use keel::bind;
use keel::ddl::Grain;
use keel::wire::{Val, Wire};

struct Fail(Sqlite);

impl Wire for Fail {
    fn grain(&self) -> Grain {
        self.0.grain()
    }

    async fn run(&mut self, sql: &str, args: &[Val]) -> Result<u64, Error> {
        self.0.run(sql, args).await
    }

    async fn plant(&mut self, sql: &str, args: &[Val]) -> Result<i64, Error> {
        self.0.plant(sql, args).await
    }

    async fn rows(&mut self, sql: &str, args: &[Val]) -> Result<Vec<Vec<Val>>, Error> {
        self.0.rows(sql, args).await
    }

    async fn script(&mut self, sql: &str) -> Result<(), Error> {
        if sql.starts_with("ALTER TABLE") && sql.contains("root_old") {
            return Err(Error::Adapt("injected activation failure".into()));
        }
        self.0.script(sql).await
    }
}

#[tokio::test]
async fn atomic() {
    let directory = tempfile::tempdir().expect("directory");
    let path = directory.path().join("estate");
    boot(&path).await;
    let before = rows(&path, "root_old").await;
    assert!(
        bind(graph("New"), Fail(Sqlite::file(&path).await.expect("wire")))
            .rename("root:old", "root:new")
            .await
            .is_err()
    );
    bind(graph("Old"), Sqlite::file(&path).await.expect("wire"))
        .await
        .expect("old usable");
    assert_eq!(rows(&path, "root_old").await, before);
    let mut wire = Sqlite::file(&path).await.expect("wire");
    assert_eq!(
        wire.rows(
            "SELECT name, value FROM \"@clock\" WHERE name LIKE 'unit:root:%' ORDER BY name",
            &[]
        )
        .await
        .expect("clocks"),
        vec![vec![Val::Text("unit:root:old".into()), Val::Int(3)]]
    );
    assert_eq!(
        wire.rows("SELECT COUNT(*) FROM \"@generation\"", &[])
            .await
            .expect("generations"),
        vec![vec![Val::Int(1)]]
    );
    drop(wire);
    let core = bind(graph("New"), Sqlite::file(&path).await.expect("wire"))
        .rename("root:old", "root:new")
        .await
        .expect("retry");
    assert_eq!(
        core.put("root:new", &[("value", "next"), ("root", "1")])
            .await
            .expect("next"),
        4
    );
}

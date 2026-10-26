use keel::adapt::Error;
use keel::adapt::db::Sqlite;
use keel::ddl::Grain;
use keel::estate::Fault;
use keel::graph::Graph;
use keel::spec::Spec;
use keel::wire::{Val, Wire};
use keel::{atom, bind, bond};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(1);

const REF: &str = "CREATE INDEX \"ref_";
const LOCK: &str = "UPDATE \"@estate\" SET shape = shape WHERE id = 1";

pub const NAMES: [&str; 8] = [
    "ref_@field_@scope_field_id",
    "ref_@field_@value_field_id",
    "ref_@unit_@bond_unit_id",
    "ref_@unit_@field_bond_id",
    "ref_@unit_@field_unit_id",
    "ref_shop_item_shop_id",
    "ref_shop_item_tags_item_id",
    "ref_shop_item_tags_tag_id",
];

pub struct Old<W> {
    pub wire: W,
    pub also: Option<&'static str>,
    pub skip: Option<&'static str>,
}

impl<W: Wire> Wire for Old<W> {
    fn grain(&self) -> Grain {
        self.wire.grain()
    }

    async fn run(&mut self, sql: &str, args: &[Val]) -> Result<u64, Error> {
        if sql == LOCK {
            return Err(Error::Adapt("no in-place step".into()));
        }
        self.wire.run(sql, args).await
    }

    async fn plant(&mut self, sql: &str, args: &[Val]) -> Result<i64, Error> {
        self.wire.plant(sql, args).await
    }

    async fn rows(&mut self, sql: &str, args: &[Val]) -> Result<Vec<Vec<Val>>, Error> {
        self.wire.rows(sql, args).await
    }

    async fn script(&mut self, sql: &str) -> Result<(), Error> {
        if sql.starts_with(REF) {
            return match self.also.take() {
                Some(extra) => self.wire.script(extra).await,
                None => Ok(()),
            };
        }
        if self.skip.is_some_and(|skip| sql.starts_with(skip)) {
            return Ok(());
        }
        self.wire.script(sql).await
    }
}

pub struct Refuse<W> {
    pub wire: W,
}

impl<W: Wire> Wire for Refuse<W> {
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
        if sql.starts_with("CREATE INDEX \"ref_shop_item_shop_id\"") {
            return Err(Error::Adapt("injected index failure".into()));
        }
        self.wire.script(sql).await
    }
}

pub fn shop() -> Graph {
    let mut graph = Graph::new();
    graph.add(Spec::build("Shop").field("name", atom::Kind::Text).seal());
    graph.add(Spec::build("Tag").field("hue", atom::Kind::Text).seal());
    graph.add(
        Spec::build("Item")
            .root("shop", bond::Kind::Many2one, "Shop")
            .field("size", atom::Kind::Int)
            .bond("tags", bond::Kind::Many2many, "Tag", &[])
            .seal(),
    );
    graph
}

pub fn grown() -> Graph {
    let mut graph = shop();
    graph.add(Spec::build("Note").field("text", atom::Kind::Text).seal());
    graph
}

pub struct Spot {
    pub path: PathBuf,
}

impl Spot {
    pub fn new(name: &str) -> Self {
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "keel-index-{}-{name}-{id}.sqlite",
            std::process::id()
        ));
        Self { path }
    }

    pub async fn wire(&self) -> Sqlite {
        Sqlite::file(&self.path).await.expect("open")
    }

    pub async fn old(&self) -> Old<Sqlite> {
        Old {
            wire: self.wire().await,
            also: None,
            skip: None,
        }
    }

    pub async fn aged(&self) {
        let core = crate::support::boot(shop(), self.old().await)
            .await
            .expect("old estate");
        drop(core);
        assert!(self.names().await.is_empty());
    }

    pub async fn names(&self) -> Vec<String> {
        self.wire()
            .await
            .rows(
                "SELECT name FROM sqlite_master WHERE type = 'index' AND name LIKE 'ref_%' ORDER BY name",
                &[],
            )
            .await
            .expect("indexes")
            .iter()
            .map(|row| row[0].text())
            .collect()
    }

    pub async fn held(&self) -> Vec<Vec<Val>> {
        let mut wire = self.wire().await;
        let mut out = wire
            .rows("SELECT active, shape FROM \"@estate\"", &[])
            .await
            .expect("estate");
        out.extend(
            wire.rows("SELECT id, digest FROM \"@generation\" ORDER BY id", &[])
                .await
                .expect("generation"),
        );
        out
    }

    pub async fn plan(&self, sql: &str) -> String {
        self.wire()
            .await
            .rows(
                &format!("EXPLAIN QUERY PLAN {sql}"),
                &[Val::Int(1), Val::Int(0)],
            )
            .await
            .expect("plan")
            .iter()
            .map(|row| row[3].text())
            .collect::<Vec<_>>()
            .join("; ")
    }

    pub async fn drifts(&self) {
        let before = self.held().await;
        match bind(shop(), self.wire().await).await {
            Err(Error::Estate(Fault::Drift { expected, found })) => assert_ne!(expected, found),
            Err(err) => panic!("unexpected {err}"),
            Ok(_) => panic!("expected drift"),
        }
        assert_eq!(self.held().await, before);
        assert!(self.names().await.is_empty());
    }
}

impl Drop for Spot {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

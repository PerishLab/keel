use keel::adapt::db::Sqlite;
use keel::atom::string;
use keel::config::{Collection, Estate};
use keel::wire::{Val, Wire};
use keel::{Core, Graph, bootstrap, resource};
use std::time::Duration;

#[resource]
struct Org {
    #[field(string)]
    name: string,
}

#[resource]
struct Note {
    #[field(string)]
    name: string,
    #[relation(Org, many2one)]
    org: Org,
}

#[resource]
struct Team {
    #[field(string)]
    name: string,
    #[relation(Org, many2many)]
    orgs: Org,
}

pub(super) fn graph() -> Graph {
    let mut graph = Graph::new();
    graph.plug::<Org>().plug::<Note>().plug::<Team>();
    graph
}

pub(super) struct Lab {
    pub(super) core: Core<Sqlite>,
    pub(super) wire: Sqlite,
    pub(super) dir: tempfile::TempDir,
}

pub(super) fn policy() -> Collection {
    Collection {
        period: 1,
        batch: 2,
        ..Default::default()
    }
}

impl Lab {
    pub(super) async fn new(collection: Collection) -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("estate.sqlite");
        let wire = Sqlite::file(&path).await.expect("wire");
        let mut boot = bootstrap(graph(), wire)
            .expect("bootstrap")
            .estate(&Estate {
                collection,
                ..Default::default()
            });
        let token = boot.mint().await.expect("mint");
        let core = boot.seal(&token).await.expect("seal");
        let wire = Sqlite::file(path).await.expect("inspect");
        Self { core, wire, dir }
    }

    pub(super) async fn org(&self, name: &str) -> i64 {
        self.core.put("Org", &[("name", name)]).await.expect("put")
    }

    pub(super) async fn end(&mut self, unit: &str, key: i64, age: i64) {
        self.core.end(unit, key).await.expect("end");
        self.age(unit, key, age).await;
    }

    pub(super) async fn age(&mut self, unit: &str, key: i64, age: i64) {
        let seat = self
            .core
            .plan()
            .units()
            .values()
            .find(|u| u.name() == unit)
            .expect("unit");
        self.wire
            .run(
                &format!(
                    "UPDATE {} SET expires_at = ?1 WHERE id = ?2",
                    keel::ddl::seat(seat)
                ),
                &[Val::Int(keel::life::tick() - age), Val::Int(key)],
            )
            .await
            .expect("age");
    }

    pub(super) async fn count(&mut self, table: &str) -> usize {
        self.wire
            .rows(&format!("SELECT COUNT(*) FROM \"{table}\""), &[])
            .await
            .expect("count")[0][0]
            .int() as usize
    }

    pub(super) async fn until(&mut self, table: &str, expected: usize) {
        tokio::time::timeout(Duration::from_secs(10), async {
            while self.count(table).await != expected {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("collection made progress");
    }
}

struct Grow;

impl keel::spec::Resource for Grow {
    fn name() -> &'static str {
        "Org"
    }
    fn spec() -> keel::spec::Spec {
        keel::spec::Spec::build("Org")
            .field("name", keel::atom::Kind::Text)
            .field("extra", keel::atom::Kind::Text)
            .seal()
    }
}

pub(super) fn grown() -> Graph {
    let mut graph = Graph::new();
    graph.plug::<Grow>().plug::<Note>().plug::<Team>();
    graph
}

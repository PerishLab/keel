use crate::adapt::{Error, Sqlite};
use crate::atom::Kind;
use crate::bond;
use crate::ddl::Grain;
use crate::model::manifest::Manifest;
use crate::plan::Plan;
use crate::spec::Spec;
use crate::wire::{Val, Wire};
use crate::{Core, Graph};
use std::cell::Cell;

struct Step {
    wire: Sqlite,
    needle: &'static str,
}

impl Wire for Step {
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
        let rows = self.wire.rows(sql, args).await?;
        if sql.contains(self.needle) {
            advance();
        }
        Ok(rows)
    }

    async fn script(&mut self, sql: &str) -> Result<(), Error> {
        self.wire.script(sql).await
    }
}

fn graph() -> Graph {
    let mut graph = Graph::new();
    graph.add(Spec::build("Shelf").field("name", Kind::Text).seal());
    graph.add(
        Spec::build("Box")
            .field("name", Kind::Text)
            .root("shelf", bond::Kind::Many2one, "Shelf")
            .seal(),
    );
    graph
}

async fn core() -> Core<Step> {
    let wire = Step {
        wire: Sqlite::memory().await.expect("memory"),
        needle: "WHERE \"shelf_id\" = ?1",
    };
    let mut boot = crate::bootstrap(graph(), wire).expect("bootstrap");
    let token = boot.mint().await.expect("mint");
    boot.seal(&token).await.expect("seal")
}

async fn family(core: &Core<Step>) -> (i64, i64) {
    let shelf = core
        .put("Shelf", &[("name", "parent")])
        .await
        .expect("shelf");
    let child = core
        .put("Box", &[("name", "child"), ("shelf", &shelf.to_string())])
        .await
        .expect("child");
    (shelf, child)
}

#[tokio::test(flavor = "current_thread")]
async fn immediate() {
    let _time = hold(100);
    let core = core().await;
    let (shelf, _) = family(&core).await;
    core.batch(async |tx| tx.end("Shelf", shelf).await)
        .await
        .expect("end crosses tick");
    assert!(read().expect("clock") > 100);
    assert!(core.live("Shelf").await.expect("parent").is_empty());
    assert!(core.live("Box").await.expect("child").is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn past() {
    let _time = hold(100);
    let core = core().await;
    let (shelf, _) = family(&core).await;
    let out = core
        .batch(async |tx| tx.lease("Shelf", shelf, 99).await)
        .await;
    assert_eq!(out, Err(Error::Adapt("lease is not the past".into())));
    assert_eq!(core.live("Shelf").await.expect("parent").len(), 1);
    assert_eq!(core.live("Box").await.expect("child").len(), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn future() {
    let _time = hold(100);
    let core = core().await;
    let (shelf, _) = family(&core).await;
    core.lease("Shelf", shelf, 200).await.expect("future lease");
    assert!(read().expect("clock") > 100);
    assert_eq!(
        core.live("Shelf").await.expect("parent")[0].expires(),
        Some(200)
    );
    assert_eq!(
        core.live("Box").await.expect("child")[0].expires(),
        Some(200)
    );
}

#[tokio::test(flavor = "current_thread")]
async fn estate() {
    let _time = hold(100);
    let mut wire = Step {
        wire: Sqlite::memory().await.expect("memory"),
        needle: "WHERE \"unit_id\" = ?1",
    };
    let graph = graph();
    let plan = Plan::lift(&graph).expect("plan");
    let manifest = Manifest::lift(&plan);
    let token = crate::cap::wild().expect("token");
    crate::estate::bootstrap(&plan, &manifest, &token, &mut wire)
        .await
        .expect("seal");
    let mut changed = Graph::new();
    changed.add(
        Spec::build("Shelf")
            .field("name", Kind::Text)
            .field("size", Kind::Int)
            .seal(),
    );
    for spec in graph.nodes().values().filter(|spec| spec.name() != "Shelf") {
        changed.add(spec.clone());
    }
    let plan = Plan::lift(&changed).expect("changed");
    let manifest = Manifest::lift(&plan);
    let cleanup = crate::config::Cleanup::default();
    crate::estate::attach(
        &plan,
        &manifest,
        crate::estate::adopt::Policy {
            cleanup: &cleanup,
            adopt: false,
        },
        &mut wire,
    )
    .await
    .expect("estate evolution crosses ticks");
    assert!(read().expect("clock") > 100);
    let rows = wire
        .rows("SELECT active FROM \"@estate\"", &[])
        .await
        .expect("active generation");
    assert_eq!(rows[0][0].int(), 2);
}

thread_local! {
    static TIME: Cell<Option<i64>> = const { Cell::new(None) };
}

pub(super) struct Hold(Option<i64>);

pub(super) fn hold(tick: i64) -> Hold {
    Hold(TIME.replace(Some(tick)))
}

pub(super) fn read() -> Option<i64> {
    TIME.get()
}

pub(super) fn advance() {
    TIME.set(TIME.get().map(|tick| tick + 1));
}

impl Drop for Hold {
    fn drop(&mut self) {
        TIME.set(self.0);
    }
}

#![allow(dead_code)]

use keel::adapt::db::Sqlite;
use keel::ddl::Grain;
use keel::{Core, Graph, Val, Wire, bind, bootstrap};
use keel_document::{Collection, Model, Shape};
use radicula_document::{Document, Operation, Path};
use std::path::Path as Directory;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

pub fn model() -> Model {
    Model::new(vec![Collection::new("settings", Shape::Any).unwrap()]).unwrap()
}

pub async fn core(home: &Directory) -> Arc<Core<Sqlite>> {
    let file = home.join("estate.sqlite3");
    let fresh = !file.exists();
    let wire = Sqlite::file(&file).await.unwrap();
    let mut graph = Graph::new();
    keel_document::stock(&mut graph);
    graph.add(
        keel::spec::Spec::build("Actor")
            .sole("login", keel::atom::Kind::Text)
            .seal(),
    );
    let core = if fresh {
        let mut boot = bootstrap(graph, wire).unwrap();
        let token = boot.mint().await.unwrap();
        boot.seal(&token).await.unwrap()
    } else {
        bind(graph, wire).await.unwrap()
    };
    Arc::new(core.identify("Actor").unwrap())
}

#[derive(Default)]
pub struct Counts {
    pub begins: Arc<AtomicU64>,
    pub rollbacks: Arc<AtomicU64>,
    pub reads: Arc<AtomicU64>,
}

impl Counts {
    pub fn begins(&self) -> u64 {
        self.begins.load(Ordering::SeqCst)
    }

    pub fn rollbacks(&self) -> u64 {
        self.rollbacks.load(Ordering::SeqCst)
    }

    pub fn reads(&self) -> u64 {
        self.reads.load(Ordering::SeqCst)
    }
}

pub struct Counting {
    inner: Sqlite,
    counts: Arc<Counts>,
}

impl Wire for Counting {
    fn grain(&self) -> Grain {
        self.inner.grain()
    }

    async fn run(&mut self, sql: &str, args: &[Val]) -> Result<u64, keel::adapt::Error> {
        self.inner.run(sql, args).await
    }

    async fn plant(&mut self, sql: &str, args: &[Val]) -> Result<i64, keel::adapt::Error> {
        self.inner.plant(sql, args).await
    }

    async fn rows(&mut self, sql: &str, args: &[Val]) -> Result<Vec<Vec<Val>>, keel::adapt::Error> {
        self.counts.reads.fetch_add(1, Ordering::SeqCst);
        self.inner.rows(sql, args).await
    }

    async fn script(&mut self, sql: &str) -> Result<(), keel::adapt::Error> {
        match sql {
            "BEGIN" => {
                self.counts.begins.fetch_add(1, Ordering::SeqCst);
            }
            "ROLLBACK" => {
                self.counts.rollbacks.fetch_add(1, Ordering::SeqCst);
            }
            _ => {}
        }
        self.inner.script(sql).await
    }

    async fn revive(&mut self) -> Result<(), keel::adapt::Error> {
        self.inner.revive().await
    }

    async fn epoch(&mut self) -> Result<Option<String>, keel::adapt::Error> {
        self.inner.epoch().await
    }
}

pub async fn counting(home: &Directory) -> (Arc<Core<Counting>>, Arc<Counts>) {
    let file = home.join("estate.sqlite3");
    let fresh = !file.exists();
    let wire = Sqlite::file(&file).await.unwrap();
    let counts = Arc::new(Counts::default());
    let wire = Counting {
        inner: wire,
        counts: counts.clone(),
    };
    let mut graph = Graph::new();
    keel_document::stock(&mut graph);
    graph.add(
        keel::spec::Spec::build("Actor")
            .sole("login", keel::atom::Kind::Text)
            .seal(),
    );
    let core = if fresh {
        let mut boot = bootstrap(graph, wire).unwrap();
        let token = boot.mint().await.unwrap();
        boot.seal(&token).await.unwrap()
    } else {
        bind(graph, wire).await.unwrap()
    };
    (Arc::new(core.identify("Actor").unwrap()), counts)
}

pub fn document(text: &str) -> Document {
    Document::parse(text).unwrap()
}

pub fn set(path: &str, text: &str) -> Operation {
    Operation::Set {
        path: Path::parse(path).unwrap(),
        value: document(text).take(),
    }
}

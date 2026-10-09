#![allow(dead_code)]

use keel::adapt::db::Sqlite;
use keel::{Core, Graph, bind, bootstrap};
use keel_column::{Record, Store, Table};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

pub const WIDTH: u64 = 1_000_000;

static SERIAL: AtomicUsize = AtomicUsize::new(0);

pub fn table() -> Table {
    Table::new("at", &["producer", "trace"])
        .and_then(|table| table.sort(&["trace"]))
        .and_then(|table| table.grain(Some("producer"), WIDTH))
        .expect("table")
}

pub fn temp(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "keel-column-{label}-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&path).expect("temp");
    path
}

pub async fn estate(home: &Path) -> Arc<Core<Sqlite>> {
    let path = home.join("estate.sqlite3");
    let fresh = !path.exists();
    let wire = Sqlite::file(&path).await.expect("sqlite");
    let core = if fresh {
        let mut boot = bootstrap(graph(), wire).expect("bootstrap");
        let token = boot.mint().await.expect("mint");
        boot.seal(&token).await.expect("seal")
    } else {
        bind(graph(), wire).await.expect("bind")
    };
    Arc::new(core)
}

pub async fn store(home: &Path) -> (Arc<Core<Sqlite>>, Store<Sqlite>) {
    let core = estate(home).await;
    let store = Store::open(core.clone(), table(), &home.join("column"))
        .await
        .expect("store");
    (core, store)
}

pub fn batch(index: u64, slot: u64) -> Vec<Record> {
    (0..3)
        .map(|part| Record {
            raw: format!("r{index}-{part}").into_bytes(),
            time: slot * WIDTH + index % WIDTH,
            keys: vec!["concord".to_string(), format!("trace-{}", index % 4)],
        })
        .collect()
}

pub async fn everything(store: &Store<Sqlite>) -> Vec<String> {
    let mut found = Vec::new();
    store
        .scan(&keel_column::Filter::default(), &mut |raw| {
            found.push(String::from_utf8_lossy(raw).into_owned());
            Ok(())
        })
        .await
        .expect("scan");
    found.sort();
    found
}

fn graph() -> Graph {
    let mut graph = Graph::new();
    keel_column::stock(&mut graph);
    graph
}

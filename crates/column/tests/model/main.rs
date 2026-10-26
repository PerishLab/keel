#[path = "../support/mod.rs"]
mod support;

use keel_column::{Store, Table};
use support::*;

#[tokio::test]
async fn drift() {
    let home = tempfile::tempdir().unwrap();
    let (core, store) = store(home.path()).await;
    store.append("one", batch(1, 1)).await.unwrap();
    drop(store);
    let changed = Table::new("at", &["trace", "producer"]).unwrap();
    let opened = Store::open(core, changed, &home.path().join("column")).await;
    assert!(
        opened.is_err(),
        "a changed key order reopened positional records"
    );
}

mod layout;
mod legacy;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut result = BTreeMap::new();
    for entry in std::fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            result.extend(snapshot(&path));
        } else if path.file_name().unwrap() != "lock" {
            result.insert(path.clone(), std::fs::read(path).unwrap());
        }
    }
    result
}

fn file(root: &Path, area: &str) -> PathBuf {
    let scope = std::fs::read_dir(root.join(area))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    std::fs::read_dir(scope)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path()
}

fn append(path: &Path, bytes: &[u8]) {
    use std::io::Write;
    std::fs::OpenOptions::new()
        .append(true)
        .open(path)
        .unwrap()
        .write_all(bytes)
        .unwrap();
}

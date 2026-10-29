mod cost;
mod flow;
mod recover;
mod refuse;
#[path = "../support/mod.rs"]
mod support;
#[path = "../support/typed.rs"]
mod typed;

use keel_column::{Filter, Kind, Query, Store, Table, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use support::*;
use typed::*;

fn files(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut result = BTreeMap::new();
    for entry in std::fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            result.extend(files(&path));
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

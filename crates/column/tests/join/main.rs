mod reference;
mod refusal;
#[path = "../support/mod.rs"]
mod support;
mod view;

use arrow_array::{Array, UInt64Array};
use keel::adapt::db::Sqlite;
use keel_column::{Filter, Kind, Record, Row, Store, Table, Value, join};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use support::*;

type Pair = (u64, Option<u64>, bool);

fn model(kind: Kind) -> Table {
    table()
        .field("left", kind)
        .unwrap()
        .field("right", kind)
        .unwrap()
        .field("value", Kind::Text)
        .unwrap()
}

fn row(id: u64, side: &str, key: Value) -> Row {
    let mut values = vec![Value::Null; 3];
    values[usize::from(side == "right")] = key;
    Record {
        raw: vec![255, id as u8],
        time: id,
        keys: vec![side.into(), format!("trace-{}", id % 2)],
    }
    .values(values)
}

fn query() -> join::Query {
    let left = join::Side::new(
        Filter {
            keys: vec![("producer".into(), "left".into())],
            ..Filter::default()
        },
        "left",
        &["at"],
    );
    let right = join::Side::new(
        Filter {
            keys: vec![("producer".into(), "right".into())],
            ..Filter::default()
        },
        "right",
        &["at"],
    );
    join::Query::new(left, right)
}

async fn read(store: &Store<Sqlite>, query: &join::Query) -> (Vec<Pair>, join::Stats) {
    let mut pairs = Vec::new();
    let stats = store
        .join(query, &mut |batch| {
            assert_eq!(batch.left.num_rows(), batch.right.num_rows());
            assert_eq!(batch.present.len(), batch.left.num_rows());
            let left = batch
                .left
                .column(0)
                .as_any()
                .downcast_ref::<UInt64Array>()
                .unwrap();
            let right = batch
                .right
                .column(0)
                .as_any()
                .downcast_ref::<UInt64Array>()
                .unwrap();
            for row in 0..batch.left.num_rows() {
                let present = batch.present.value(row);
                assert_eq!(present, !right.is_null(row));
                pairs.push((
                    left.value(row),
                    if present {
                        Some(right.value(row))
                    } else {
                        None
                    },
                    present,
                ));
            }
            Ok(())
        })
        .await
        .unwrap();
    pairs.sort();
    (pairs, stats)
}

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

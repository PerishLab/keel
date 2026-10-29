use crate::support::table;
use arrow_array::{
    Array, BinaryArray, BooleanArray, Int64Array, RecordBatch, StringArray, UInt64Array,
};
use keel::adapt::db::Sqlite;
use keel_column::{Filter, Kind, Query, Record, Store, Table, Value};
use std::collections::BTreeMap;

pub(crate) fn model() -> Table {
    table()
        .field("ok", Kind::Bool)
        .unwrap()
        .field("code", Kind::Int)
        .unwrap()
        .field("size", Kind::Uint)
        .unwrap()
        .field("name", Kind::Text)
        .unwrap()
        .field("data", Kind::Bytes)
        .unwrap()
}

pub(crate) fn record(time: u64) -> Record {
    Record {
        raw: vec![0, 255, time as u8],
        time,
        keys: vec!["concord".into(), format!("trace-{}", time % 4)],
    }
}

pub(crate) fn values(time: u64) -> Vec<Value> {
    vec![
        Value::Bool(time.is_multiple_of(2)),
        Value::Int(i64::MIN + time as i64),
        Value::Uint(u64::MAX - time),
        Value::Text(format!("值-{time}")),
        Value::Bytes(vec![255, 0, time as u8]),
    ]
}

pub(crate) async fn read(store: &Store<Sqlite>) -> BTreeMap<u64, (Vec<u8>, Vec<Value>)> {
    let mut found = BTreeMap::new();
    store
        .query(
            &Query::select(
                Filter::default(),
                &["at", "raw", "ok", "code", "size", "name", "data"],
            ),
            &mut |batch| {
                for row in 0..batch.num_rows() {
                    let time = batch
                        .column(0)
                        .as_any()
                        .downcast_ref::<UInt64Array>()
                        .unwrap()
                        .value(row);
                    let raw = batch
                        .column(1)
                        .as_any()
                        .downcast_ref::<BinaryArray>()
                        .unwrap()
                        .value(row)
                        .to_vec();
                    let values = (2..7).map(|column| value(&batch, column, row)).collect();
                    assert!(found.insert(time, (raw, values)).is_none());
                }
                Ok(())
            },
        )
        .await
        .unwrap();
    found
}

pub(crate) fn value(batch: &RecordBatch, column: usize, row: usize) -> Value {
    let array = batch.column(column);
    if array.is_null(row) {
        return Value::Null;
    }
    match column {
        2 => Value::Bool(
            array
                .as_any()
                .downcast_ref::<BooleanArray>()
                .unwrap()
                .value(row),
        ),
        3 => Value::Int(
            array
                .as_any()
                .downcast_ref::<Int64Array>()
                .unwrap()
                .value(row),
        ),
        4 => Value::Uint(
            array
                .as_any()
                .downcast_ref::<UInt64Array>()
                .unwrap()
                .value(row),
        ),
        5 => Value::Text(
            array
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap()
                .value(row)
                .into(),
        ),
        _ => Value::Bytes(
            array
                .as_any()
                .downcast_ref::<BinaryArray>()
                .unwrap()
                .value(row)
                .to_vec(),
        ),
    }
}

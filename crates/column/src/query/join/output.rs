use super::cell::Cell;
use super::{Batch, Output, Query, Stats};
use crate::{Error, Value};
use arrow_array::{
    ArrayRef, BinaryArray, BooleanArray, Int64Array, RecordBatch, StringArray, UInt64Array,
};
use arrow_schema::{DataType, Schema};
use std::sync::Arc;

pub(super) struct Buffer {
    schema: (Arc<Schema>, Arc<Schema>),
    left: Vec<Vec<Value>>,
    right: Vec<Vec<Value>>,
    present: Vec<bool>,
    bytes: u64,
}

impl Buffer {
    pub fn new(schema: (Arc<Schema>, Arc<Schema>)) -> Self {
        Self {
            schema,
            left: Vec::new(),
            right: Vec::new(),
            present: Vec::new(),
            bytes: 0,
        }
    }

    pub fn push(
        &mut self,
        left: (&RecordBatch, usize),
        right: Option<&[Value]>,
        state: (&Query, &mut Stats, &mut Output<'_>),
    ) -> Result<(), Error> {
        let (query, stats, output) = state;
        let (batch, at) = left;
        let width = (self.schema.0.fields().len(), self.schema.1.fields().len());
        let fixed = 2 * std::mem::size_of::<Vec<Value>>()
            + (width.0 + width.1) * std::mem::size_of::<Value>()
            + 1;
        let payload = (0..width.0)
            .map(|column| Cell::get(batch, column, at).size())
            .sum::<u64>()
            + right.map_or(0, |values| values.iter().map(Value::size).sum());
        let charge = fixed as u64 + payload;
        let rows = super::super::bounded(stats.rows, 1, query.limit.rows, "join result rows")?;
        let bytes = super::super::bounded(
            stats.bytes,
            charge,
            query.limit.bytes,
            "join result charged bytes",
        )?;
        if charge > query.limit.batch {
            return Err(Error::new("join pair exceeds batch charged-byte limit"));
        }
        let full = self.left.len() >= query.batch || self.bytes + charge > query.limit.batch;
        if full {
            self.flush(stats, output)?;
        }
        self.left.push(
            (0..width.0)
                .map(|column| Cell::get(batch, column, at).owned())
                .collect(),
        );
        self.right
            .push(right.map_or_else(|| vec![Value::Null; width.1], <[Value]>::to_vec));
        self.present.push(right.is_some());
        self.bytes += charge;
        stats.rows = rows;
        stats.bytes = bytes;
        Ok(())
    }

    pub fn flush(&mut self, stats: &mut Stats, output: &mut Output<'_>) -> Result<(), Error> {
        if self.left.is_empty() {
            return Ok(());
        }
        let batch = Batch {
            left: make(self.schema.0.clone(), &self.left)?,
            right: make(self.schema.1.clone(), &self.right)?,
            present: BooleanArray::from(self.present.clone()),
        };
        output(batch)?;
        stats.batches += 1;
        self.left.clear();
        self.right.clear();
        self.present.clear();
        self.bytes = 0;
        Ok(())
    }
}

fn make(schema: Arc<Schema>, rows: &[Vec<Value>]) -> Result<RecordBatch, Error> {
    let columns = schema
        .fields()
        .iter()
        .enumerate()
        .map(|(index, field)| array(rows, index, field.data_type()))
        .collect();
    Ok(RecordBatch::try_new(schema, columns)?)
}

fn array(rows: &[Vec<Value>], column: usize, kind: &DataType) -> ArrayRef {
    let values = rows.iter().map(|row| &row[column]);
    match kind {
        DataType::Boolean => Arc::new(BooleanArray::from_iter(values.map(|value| match value {
            Value::Bool(value) => Some(*value),
            _ => None,
        }))),
        DataType::Int64 => Arc::new(Int64Array::from_iter(values.map(|value| match value {
            Value::Int(value) => Some(*value),
            _ => None,
        }))),
        DataType::UInt64 => Arc::new(UInt64Array::from_iter(values.map(|value| match value {
            Value::Uint(value) => Some(*value),
            _ => None,
        }))),
        DataType::Utf8 => Arc::new(StringArray::from_iter(values.map(|value| match value {
            Value::Text(value) => Some(value.as_str()),
            _ => None,
        }))),
        DataType::Binary => Arc::new(BinaryArray::from_iter(values.map(|value| match value {
            Value::Bytes(value) => Some(value.as_slice()),
            _ => None,
        }))),
        _ => unreachable!(),
    }
}

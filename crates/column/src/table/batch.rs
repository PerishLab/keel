use super::{Kind, Row, Table, Value};
use crate::Error;
use arrow_array::{
    ArrayRef, BinaryArray, BooleanArray, Int64Array, RecordBatch, StringArray, UInt64Array,
};
use std::sync::Arc;

impl Table {
    pub(crate) fn batch(&self, rows: &[Row], columns: &[usize]) -> Result<RecordBatch, Error> {
        let schema = Arc::new(self.schema().project(columns)?);
        let arrays = columns
            .iter()
            .map(|index| self.values(rows, *index))
            .collect();
        Ok(RecordBatch::try_new(schema, arrays)?)
    }

    fn values(&self, rows: &[Row], index: usize) -> ArrayRef {
        if index == 0 {
            return Arc::new(UInt64Array::from_iter_values(
                rows.iter().map(|row| row.time),
            ));
        }
        if index <= self.keys.len() {
            return Arc::new(StringArray::from_iter_values(
                rows.iter().map(|row| row.keys[index - 1].as_str()),
            ));
        }
        if index == self.keys.len() + 1 {
            return Arc::new(BinaryArray::from_iter_values(
                rows.iter().map(|row| row.raw.as_slice()),
            ));
        }
        let field = index - self.keys.len() - 2;
        scalar(rows, field, self.fields[field].kind)
    }
}

fn scalar(rows: &[Row], index: usize, kind: Kind) -> ArrayRef {
    let values = rows
        .iter()
        .map(|row| row.values.get(index).unwrap_or(&Value::Null));
    match kind {
        Kind::Bool => Arc::new(BooleanArray::from_iter(values.map(|value| match value {
            Value::Bool(value) => Some(*value),
            _ => None,
        }))),
        Kind::Int => Arc::new(Int64Array::from_iter(values.map(|value| match value {
            Value::Int(value) => Some(*value),
            _ => None,
        }))),
        Kind::Uint => Arc::new(UInt64Array::from_iter(values.map(|value| match value {
            Value::Uint(value) => Some(*value),
            _ => None,
        }))),
        Kind::Text => Arc::new(StringArray::from_iter(values.map(|value| match value {
            Value::Text(value) => Some(value.as_str()),
            _ => None,
        }))),
        Kind::Bytes => Arc::new(BinaryArray::from_iter(values.map(|value| match value {
            Value::Bytes(value) => Some(value.as_slice()),
            _ => None,
        }))),
    }
}

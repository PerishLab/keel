use crate::Value;
use arrow_array::{BinaryArray, BooleanArray, Int64Array, RecordBatch, StringArray, UInt64Array};
use arrow_schema::DataType;

#[derive(Clone, Copy, Hash)]
pub(super) enum Cell<'a> {
    Null,
    Bool(bool),
    Int(i64),
    Uint(u64),
    Text(&'a str),
    Bytes(&'a [u8]),
}

impl<'a> Cell<'a> {
    pub fn get(batch: &'a RecordBatch, column: usize, row: usize) -> Self {
        let array = batch.column(column);
        if array.is_null(row) {
            return Self::Null;
        }
        match array.data_type() {
            DataType::Boolean => Self::Bool(
                array
                    .as_any()
                    .downcast_ref::<BooleanArray>()
                    .unwrap()
                    .value(row),
            ),
            DataType::Int64 => Self::Int(
                array
                    .as_any()
                    .downcast_ref::<Int64Array>()
                    .unwrap()
                    .value(row),
            ),
            DataType::UInt64 => Self::Uint(
                array
                    .as_any()
                    .downcast_ref::<UInt64Array>()
                    .unwrap()
                    .value(row),
            ),
            DataType::Utf8 => Self::Text(
                array
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .unwrap()
                    .value(row),
            ),
            DataType::Binary => Self::Bytes(
                array
                    .as_any()
                    .downcast_ref::<BinaryArray>()
                    .unwrap()
                    .value(row),
            ),
            _ => unreachable!(),
        }
    }

    pub fn owned(self) -> Value {
        match self {
            Self::Null => Value::Null,
            Self::Bool(value) => Value::Bool(value),
            Self::Int(value) => Value::Int(value),
            Self::Uint(value) => Value::Uint(value),
            Self::Text(value) => Value::Text(value.into()),
            Self::Bytes(value) => Value::Bytes(value.into()),
        }
    }

    pub fn equals(self, value: &Value) -> bool {
        match (self, value) {
            (Self::Bool(left), Value::Bool(right)) => left == *right,
            (Self::Int(left), Value::Int(right)) => left == *right,
            (Self::Uint(left), Value::Uint(right)) => left == *right,
            (Self::Text(left), Value::Text(right)) => left == right,
            (Self::Bytes(left), Value::Bytes(right)) => left == right,
            _ => false,
        }
    }

    pub fn size(self) -> u64 {
        match self {
            Self::Null => 0,
            Self::Bool(_) => 1,
            Self::Int(_) | Self::Uint(_) => 8,
            Self::Text(value) => value.len() as u64,
            Self::Bytes(value) => value.len() as u64,
        }
    }
}

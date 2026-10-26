use super::{Record, Table};
use crate::Error;
use arrow_schema::DataType;
use serde::{Deserialize, Serialize};
use std::ops::Deref;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Bool,
    Int,
    Uint,
    Text,
    Bytes,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Column {
    pub name: String,
    pub kind: Kind,
    pub nullable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Uint(u64),
    Text(String),
    Bytes(Vec<u8>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub record: Record,
    pub values: Vec<Value>,
}

impl From<Record> for Row {
    fn from(record: Record) -> Self {
        Self {
            record,
            values: Vec::new(),
        }
    }
}

impl Deref for Row {
    type Target = Record;

    fn deref(&self) -> &Record {
        &self.record
    }
}

impl Record {
    pub fn values(self, values: Vec<Value>) -> Row {
        Row {
            record: self,
            values,
        }
    }
}

impl Kind {
    pub(crate) fn arrow(self) -> DataType {
        match self {
            Self::Bool => DataType::Boolean,
            Self::Int => DataType::Int64,
            Self::Uint => DataType::UInt64,
            Self::Text => DataType::Utf8,
            Self::Bytes => DataType::Binary,
        }
    }
}

impl Value {
    pub(crate) fn fits(&self, kind: Kind) -> bool {
        matches!(
            (self, kind),
            (Self::Null, _)
                | (Self::Bool(_), Kind::Bool)
                | (Self::Int(_), Kind::Int)
                | (Self::Uint(_), Kind::Uint)
                | (Self::Text(_), Kind::Text)
                | (Self::Bytes(_), Kind::Bytes)
        )
    }

    pub(crate) fn stored(&self) -> u64 {
        self.size()
            + if matches!(self, Self::Text(_) | Self::Bytes(_)) {
                5
            } else {
                1
            }
    }

    pub(crate) fn size(&self) -> u64 {
        match self {
            Self::Null => 0,
            Self::Bool(_) => 1,
            Self::Int(_) | Self::Uint(_) => 8,
            Self::Text(value) => value.len() as u64,
            Self::Bytes(value) => value.len() as u64,
        }
    }
}

impl Table {
    pub fn field(mut self, name: &str, kind: Kind) -> Result<Self, Error> {
        if name.is_empty() || self.column(name).is_ok() {
            return Err(Error::new(
                "column field names must be non-empty and distinct",
            ));
        }
        self.fields.push(Column {
            name: name.into(),
            kind,
            nullable: true,
        });
        self.describe()?;
        Ok(self)
    }

    pub fn fields(&self) -> &[Column] {
        &self.fields
    }

    pub(crate) fn column(&self, name: &str) -> Result<usize, Error> {
        if name == self.time() {
            return Ok(0);
        }
        if name == "raw" {
            return Ok(self.keys.len() + 1);
        }
        if let Ok(index) = self.index(name) {
            return Ok(index + 1);
        }
        self.fields
            .iter()
            .position(|field| field.name == name)
            .map(|index| self.keys.len() + 2 + index)
            .ok_or_else(|| Error::new(format!("{name:?} is not a declared column")))
    }

    pub(crate) fn accept(&self, row: &Row) -> Result<(), Error> {
        self.check(row)?;
        if row.values.len() > self.fields.len() {
            return Err(Error::new(
                "row has more values than declared nullable fields",
            ));
        }
        for (value, field) in row.values.iter().zip(&self.fields) {
            if !value.fits(field.kind) {
                return Err(Error::new(format!(
                    "value does not match declared type of {:?}",
                    field.name
                )));
            }
        }
        let keys = row.keys.iter().map(|key| 4 + key.len() as u64);
        let values = row.values.iter().map(Value::stored);
        let size = 17
            + row.raw.len() as u64
            + keys.chain(values).sum::<u64>()
            + if row.values.is_empty() { 0 } else { 4 };
        if size > u32::MAX as u64 {
            return Err(Error::new("encoded row exceeds 4 GiB"));
        }
        Ok(())
    }
}

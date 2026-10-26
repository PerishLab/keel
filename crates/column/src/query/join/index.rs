use super::cell::Cell;
use super::{Limit, Stats};
use crate::{Error, Value};
use arrow_array::RecordBatch;
use std::collections::{HashMap, hash_map::RandomState};
use std::hash::BuildHasher;

pub(super) struct Entry {
    pub key: Value,
    pub values: Vec<Value>,
}

pub(super) struct Index {
    pub rows: HashMap<u64, Vec<Entry>>,
    hash: RandomState,
}

impl Index {
    pub fn new() -> Self {
        Self {
            rows: HashMap::new(),
            hash: RandomState::new(),
        }
    }

    pub fn hash(&self, key: Cell<'_>) -> u64 {
        self.hash.hash_one(key)
    }

    pub fn insert(
        &mut self,
        key: Cell<'_>,
        row: (&RecordBatch, usize, usize),
        state: (&Limit, &mut Stats),
    ) -> Result<(), Error> {
        let (batch, at, columns) = row;
        let (limit, stats) = state;
        let rows = super::super::bounded(stats.indexed, 1, limit.build, "join build rows")?;
        let fixed =
            std::mem::size_of::<Vec<Value>>() + (columns + 1) * std::mem::size_of::<Value>();
        let payload = (0..columns)
            .map(|column| Cell::get(batch, column, at).size())
            .sum::<u64>();
        let memory = super::super::bounded(
            stats.memory,
            fixed as u64 + payload + key.size(),
            limit.memory,
            "join index charged bytes",
        )?;
        let hash = self.hash(key);
        let values = (0..columns)
            .map(|column| Cell::get(batch, column, at).owned())
            .collect();
        self.rows.entry(hash).or_default().push(Entry {
            key: key.owned(),
            values,
        });
        stats.memory = memory;
        stats.indexed = rows;
        Ok(())
    }
}

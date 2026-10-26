mod cell;
mod engine;
mod index;
mod output;
mod plan;
mod store;

use super::Filter;
use crate::{Error, Value};
use arrow_array::{BooleanArray, RecordBatch};

pub type Output<'a> = dyn FnMut(Batch) -> Result<(), Error> + Send + 'a;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Condition {
    Equal(String, Value),
    Missing(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Side {
    pub filter: Filter,
    pub key: String,
    pub columns: Vec<String>,
    pub when: Vec<Condition>,
}

impl Side {
    pub fn new(filter: Filter, key: &str, columns: &[&str]) -> Self {
        Self {
            filter,
            key: key.into(),
            columns: columns.iter().map(|name| (*name).into()).collect(),
            when: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Inner,
    Left,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Limit {
    pub scan: super::Limit,
    pub build: u64,
    pub memory: u64,
    pub work: u64,
    pub rows: u64,
    pub bytes: u64,
    pub batch: u64,
}

impl Default for Limit {
    fn default() -> Self {
        Self {
            scan: super::Limit::default(),
            build: 100_000,
            memory: 64 * 1024 * 1024,
            work: 10_000_000,
            rows: 1_000_000,
            bytes: 64 * 1024 * 1024,
            batch: 8 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Query {
    pub left: Side,
    pub right: Side,
    pub mode: Mode,
    pub batch: usize,
    pub limit: Limit,
}

impl Query {
    pub fn new(left: Side, right: Side) -> Self {
        Self {
            left,
            right,
            mode: Mode::Left,
            batch: 1024,
            limit: Limit::default(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Batch {
    pub left: RecordBatch,
    pub right: RecordBatch,
    pub present: BooleanArray,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Input {
    pub rows: u64,
    pub nulls: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub scan: super::Stats,
    pub left: Input,
    pub right: Input,
    pub indexed: u64,
    pub pairs: u64,
    pub missing: u64,
    pub rows: u64,
    pub batches: u64,
    pub memory: u64,
    pub bytes: u64,
    pub work: u64,
}

impl Stats {
    fn step(&mut self, limit: &Limit) -> Result<(), Error> {
        self.work = super::bounded(self.work, 1, limit.work, "join work")?;
        Ok(())
    }
}

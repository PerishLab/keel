mod meter;
mod plan;
mod reader;
pub(crate) mod scan;
mod store;

use crate::Error;
use arrow_array::RecordBatch;
pub use scan::Filter;

pub type Output<'a> = dyn FnMut(RecordBatch) -> Result<(), Error> + Send + 'a;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Projection {
    Columns(Vec<String>),
    Count,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Limit {
    pub candidates: u64,
    pub bytes: u64,
    pub rows: u64,
    pub memory: u64,
}

impl Default for Limit {
    fn default() -> Self {
        Self {
            candidates: 10_000_000,
            bytes: 256 * 1024 * 1024,
            rows: 1_000_000,
            memory: 256 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Query {
    pub filter: Filter,
    pub projection: Projection,
    pub batch: usize,
    pub limit: Limit,
}

impl Query {
    pub fn select(filter: Filter, columns: &[&str]) -> Self {
        Self {
            filter,
            projection: Projection::Columns(columns.iter().map(|name| (*name).into()).collect()),
            batch: 1024,
            limit: Limit::default(),
        }
    }

    pub fn count(filter: Filter) -> Self {
        Self {
            projection: Projection::Count,
            ..Self::select(filter, &[])
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub files: u64,
    pub groups: u64,
    pub candidates: u64,
    pub matched: u64,
    pub rows: u64,
    pub batches: u64,
    pub bytes: u64,
    pub memory: u64,
}

impl Stats {
    fn admit(&mut self, rows: u64, limit: &Limit) -> Result<(), Error> {
        self.candidates = bounded(self.candidates, rows, limit.candidates, "candidate rows")?;
        Ok(())
    }

    fn emit(&mut self, batch: RecordBatch, output: &mut Output<'_>) -> Result<(), Error> {
        let rows = batch.num_rows() as u64;
        if rows != 0 {
            output(batch)?;
            self.rows += rows;
            self.batches += 1;
        }
        Ok(())
    }
}

fn bounded(current: u64, added: u64, limit: u64, name: &str) -> Result<u64, Error> {
    current
        .checked_add(added)
        .filter(|total| *total <= limit)
        .ok_or_else(|| Error::new(format!("query {name} limit exceeded")))
}

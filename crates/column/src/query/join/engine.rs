use super::cell::Cell;
use super::index::Index;
use super::output::Buffer;
use super::plan::Plan;
use super::{Mode, Output, Query, Stats};
use crate::Error;
use arrow_array::RecordBatch;

pub(super) struct Engine<'a, 'b> {
    plan: &'a Plan<'b>,
    query: &'b Query,
    index: Index,
    buffer: Buffer,
    stats: Stats,
}

impl<'a, 'b> Engine<'a, 'b> {
    pub fn new(plan: &'a Plan<'b>, query: &'b Query) -> Self {
        Self {
            plan,
            query,
            index: Index::new(),
            buffer: Buffer::new(plan.schema.clone()),
            stats: Stats::default(),
        }
    }

    pub fn build(&mut self, batch: RecordBatch) -> Result<(), Error> {
        let input = &self.plan.right;
        for row in 0..batch.num_rows() {
            self.stats.step(&self.query.limit)?;
            if !input.admits(&batch, row, (&self.query.limit, &mut self.stats))? {
                continue;
            }
            self.stats.right.rows += 1;
            let key = Cell::get(&batch, input.key, row);
            if matches!(key, Cell::Null) {
                self.stats.right.nulls += 1;
                continue;
            }
            self.index.insert(
                key,
                (&batch, row, input.columns),
                (&self.query.limit, &mut self.stats),
            )?;
        }
        Ok(())
    }

    pub fn probe(&mut self, batch: RecordBatch, output: &mut Output<'_>) -> Result<(), Error> {
        let plan = self.plan;
        let input = &plan.left;
        for row in 0..batch.num_rows() {
            self.stats.step(&self.query.limit)?;
            if !input.admits(&batch, row, (&self.query.limit, &mut self.stats))? {
                continue;
            }
            self.stats.left.rows += 1;
            let key = Cell::get(&batch, input.key, row);
            if matches!(key, Cell::Null) {
                self.stats.left.nulls += 1;
            }
            let matched = self.matches(key, (&batch, row), output)?;
            if !matched {
                self.stats.missing += 1;
                if self.query.mode == Mode::Left {
                    self.buffer
                        .push((&batch, row), None, (self.query, &mut self.stats, output))?;
                }
            }
        }
        Ok(())
    }

    fn matches(
        &mut self,
        key: Cell<'_>,
        row: (&RecordBatch, usize),
        output: &mut Output<'_>,
    ) -> Result<bool, Error> {
        if matches!(key, Cell::Null) {
            return Ok(false);
        }
        let mut matched = false;
        if let Some(entries) = self.index.rows.get(&self.index.hash(key)) {
            for entry in entries {
                self.stats.step(&self.query.limit)?;
                if key.equals(&entry.key) {
                    self.buffer.push(
                        row,
                        Some(&entry.values),
                        (self.query, &mut self.stats, output),
                    )?;
                    self.stats.pairs += 1;
                    matched = true;
                }
            }
        }
        Ok(matched)
    }

    pub fn finish(
        mut self,
        scan: super::super::Stats,
        output: &mut Output<'_>,
    ) -> Result<Stats, Error> {
        self.buffer.flush(&mut self.stats, output)?;
        self.stats.scan = scan;
        Ok(self.stats)
    }
}

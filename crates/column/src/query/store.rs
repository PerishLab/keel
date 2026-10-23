use super::meter::Meter;
use super::plan::Plan;
use super::{Output, Query, Stats, bounded};
use crate::{Error, Record, Store, Table};
use arrow_array::{ArrayRef, BinaryArray, RecordBatch, StringArray, UInt64Array};
use keel::Wire;
use std::path::PathBuf;
use std::sync::Arc;

impl<W: Wire> Store<W> {
    pub async fn query(&self, query: &Query, output: &mut Output<'_>) -> Result<Stats, Error> {
        let plan = self.table.plan(query)?;
        let mut stats = Stats::default();
        let (paths, mut records) = self.view(&plan, query, &mut stats).await?;
        let meter = Meter::new(query.limit.bytes);
        for path in paths {
            self.table
                .read(&path, &plan, query, (&meter, &mut stats, output))?;
        }
        records.sort_by(|left, right| self.table.order(left, right));
        for chunk in records.chunks(query.batch) {
            stats.emit(self.table.project(chunk, &plan)?, output)?;
        }
        stats.bytes = meter.bytes();
        Ok(stats)
    }

    async fn view(
        &self,
        plan: &Plan,
        query: &Query,
        stats: &mut Stats,
    ) -> Result<(Vec<PathBuf>, Vec<Record>), Error> {
        let state = self.state.lock().await;
        let filter = &query.filter;
        let paths = state
            .parts
            .iter()
            .filter(|entry| {
                filter.to.is_none_or(|to| entry.since < to)
                    && filter.from.is_none_or(|from| entry.until > from)
            })
            .map(|entry| self.part(&entry.scope, entry.source))
            .collect();
        let mut records = Vec::new();
        for record in state.open.values().flat_map(|segment| &segment.records) {
            stats.admit(1, &query.limit)?;
            if self.table.admits(filter, record)? {
                stats.matched = bounded(stats.matched, 1, query.limit.rows, "matched rows")?;
                if !plan.columns.is_empty() {
                    stats.memory = bounded(
                        stats.memory,
                        copied(record, plan),
                        query.limit.memory,
                        "open snapshot bytes",
                    )?;
                    records.push(snapshot(record, plan));
                }
            }
        }
        Ok((paths, records))
    }
}

impl Table {
    fn project(&self, records: &[Record], plan: &Plan) -> Result<RecordBatch, Error> {
        let schema = Arc::new(self.schema().project(&plan.columns)?);
        let columns = plan
            .columns
            .iter()
            .map(|index| self.values(records, *index))
            .collect();
        Ok(RecordBatch::try_new(schema, columns)?.project(&plan.order)?)
    }

    fn values(&self, records: &[Record], index: usize) -> ArrayRef {
        if index == 0 {
            Arc::new(UInt64Array::from_iter_values(
                records.iter().map(|record| record.time),
            ))
        } else if index == self.keys().len() + 1 {
            Arc::new(BinaryArray::from_iter_values(
                records.iter().map(|record| record.raw.as_slice()),
            ))
        } else {
            Arc::new(StringArray::from_iter_values(
                records.iter().map(|record| record.keys[index - 1].as_str()),
            ))
        }
    }
}

fn copied(record: &Record, plan: &Plan) -> u64 {
    8 + record.keys.iter().map(|key| key.len() as u64).sum::<u64>()
        + if plan.raw { record.raw.len() as u64 } else { 0 }
}

fn snapshot(record: &Record, plan: &Plan) -> Record {
    Record {
        time: record.time,
        keys: record.keys.clone(),
        raw: if plan.raw {
            record.raw.clone()
        } else {
            Vec::new()
        },
    }
}

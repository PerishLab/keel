use super::meter::Meter;
use super::plan::Plan;
use super::{Output, Query, Stats, bounded};
use crate::{Error, Record, Row, Store, Value};
use keel::Wire;
use std::path::PathBuf;

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
            stats.emit(
                self.table
                    .batch(chunk, &plan.columns)?
                    .project(&plan.order)?,
                output,
            )?;
        }
        stats.bytes = meter.bytes();
        Ok(stats)
    }

    async fn view(
        &self,
        plan: &Plan,
        query: &Query,
        stats: &mut Stats,
    ) -> Result<(Vec<PathBuf>, Vec<Row>), Error> {
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

fn copied(record: &Row, plan: &Plan) -> u64 {
    8 + record.keys.iter().map(|key| key.len() as u64).sum::<u64>()
        + if plan.raw { record.raw.len() as u64 } else { 0 }
        + plan
            .fields
            .iter()
            .filter_map(|index| record.values.get(*index))
            .map(Value::size)
            .sum::<u64>()
}

fn snapshot(record: &Row, plan: &Plan) -> Row {
    Row {
        values: record
            .values
            .iter()
            .take(plan.fields.last().map_or(0, |index| index + 1))
            .enumerate()
            .map(|(index, value)| {
                if plan.fields.binary_search(&index).is_ok() {
                    value.clone()
                } else {
                    Value::Null
                }
            })
            .collect(),
        record: Record {
            time: record.time,
            keys: record.keys.clone(),
            raw: if plan.raw {
                record.raw.clone()
            } else {
                Vec::new()
            },
        },
    }
}

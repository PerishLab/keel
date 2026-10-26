use super::meter::Meter;
use super::plan::Plan;
use super::{Output, Query, Stats, bounded};
use crate::{Error, Record, Row, Store, Value};
use keel::Wire;
use std::path::PathBuf;

pub(super) struct View {
    paths: Vec<PathBuf>,
    records: Vec<Row>,
}

impl<W: Wire> Store<W> {
    pub async fn query(&self, query: &Query, output: &mut Output<'_>) -> Result<Stats, Error> {
        let plan = self.table.plan(query)?;
        let mut stats = Stats::default();
        let view = {
            let state = self.state.lock().await;
            self.capture(&state, (&plan, query), &mut stats)?
        };
        let meter = Meter::new(query.limit.bytes);
        self.execute(view, (&plan, query), (&meter, &mut stats, output))?;
        stats.bytes = meter.bytes();
        Ok(stats)
    }

    pub(super) fn execute(
        &self,
        mut view: View,
        request: (&Plan, &Query),
        state: (&Meter, &mut Stats, &mut Output<'_>),
    ) -> Result<(), Error> {
        let (plan, query) = request;
        let (meter, stats, output) = state;
        for path in view.paths {
            self.table
                .read(&path, plan, query, (meter, stats, output))?;
        }
        view.records
            .sort_by(|left, right| self.table.order(left, right));
        for chunk in view.records.chunks(query.batch) {
            stats.emit(
                self.table
                    .batch(chunk, &plan.columns)?
                    .project(&plan.order)?,
                output,
            )?;
        }
        Ok(())
    }

    pub(super) fn capture(
        &self,
        state: &crate::store::State,
        request: (&Plan, &Query),
        stats: &mut Stats,
    ) -> Result<View, Error> {
        let (plan, query) = request;
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
        Ok(View { paths, records })
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

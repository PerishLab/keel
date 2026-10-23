use super::meter::{Meter, Source};
use super::plan::Plan;
use super::{Output, Query, Stats, bounded, scan};
use crate::{Error, Table};
use arrow_array::{BooleanArray, RecordBatch, StringArray, UInt64Array};
use parquet::arrow::ProjectionMask;
use parquet::arrow::arrow_reader::{
    ArrowPredicate, ArrowPredicateFn, ParquetRecordBatchReaderBuilder, RowFilter,
};
use std::fs::File;
use std::path::Path;

impl Table {
    pub fn query(
        &self,
        path: &Path,
        query: &Query,
        output: &mut Output<'_>,
    ) -> Result<Stats, Error> {
        let plan = self.plan(query)?;
        let meter = Meter::new(query.limit.bytes);
        let mut stats = Stats::default();
        self.read(path, &plan, query, (&meter, &mut stats, output))?;
        stats.bytes = meter.bytes();
        Ok(stats)
    }

    pub(super) fn read(
        &self,
        path: &Path,
        plan: &Plan,
        query: &Query,
        state: (&Meter, &mut Stats, &mut Output<'_>),
    ) -> Result<(), Error> {
        let (meter, stats, output) = state;
        let source = Source::new(File::open(path)?, meter.clone())?;
        stats.files += 1;
        let mut builder = ParquetRecordBatchReaderBuilder::try_new(source)?;
        if builder.schema().as_ref() != &self.schema() {
            return Err(Error::new("query part schema differs from table"));
        }
        let groups = scan::admitted(&mut builder, &query.filter, &plan.keys)?;
        for group in &groups {
            stats.admit(
                builder.metadata().row_group(*group).num_rows() as u64,
                &query.limit,
            )?;
        }
        stats.groups += groups.len() as u64;
        let columns = builder.parquet_schema().clone();
        let reader = builder
            .with_batch_size(query.batch)
            .with_row_groups(groups)
            .with_projection(ProjectionMask::leaves(&columns, plan.columns.clone()))
            .with_row_filter(predicates(&columns, plan, query))
            .build()?;
        for batch in reader {
            let batch = batch?;
            stats.matched = bounded(
                stats.matched,
                batch.num_rows() as u64,
                query.limit.rows,
                "matched rows",
            )?;
            if !plan.columns.is_empty() {
                stats.emit(batch.project(&plan.order)?, output)?;
            }
        }
        Ok(())
    }
}

fn predicates(
    schema: &parquet::schema::types::SchemaDescriptor,
    plan: &Plan,
    query: &Query,
) -> RowFilter {
    let mut result: Vec<Box<dyn ArrowPredicate>> = Vec::new();
    let (from, to) = (query.filter.from, query.filter.to);
    if from.is_some() || to.is_some() {
        result.push(Box::new(ArrowPredicateFn::new(
            ProjectionMask::leaves(schema, [0]),
            move |batch: RecordBatch| {
                let times = batch
                    .column(0)
                    .as_any()
                    .downcast_ref::<UInt64Array>()
                    .unwrap();
                Ok(times
                    .iter()
                    .map(|time| time.map(|time| inside(time, from, to)))
                    .collect())
            },
        )));
    }
    for (index, value) in &plan.keys {
        let value = value.clone();
        result.push(Box::new(ArrowPredicateFn::new(
            ProjectionMask::leaves(schema, [*index]),
            move |batch: RecordBatch| {
                let values = batch
                    .column(0)
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .unwrap();
                Ok(values
                    .iter()
                    .map(|found| found.map(|found| found == value))
                    .collect::<BooleanArray>())
            },
        )));
    }
    RowFilter::new(result)
}

fn inside(time: u64, from: Option<u64>, to: Option<u64>) -> bool {
    from.is_none_or(|from| time >= from) && to.is_none_or(|to| time < to)
}

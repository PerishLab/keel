use crate::{Error, Record, Table};
use arrow_array::{Array, BinaryArray, BooleanArray, RecordBatch, StringArray, UInt64Array};
use parquet::arrow::ProjectionMask;
use parquet::arrow::arrow_reader::{
    ArrowPredicate, ArrowPredicateFn, ParquetRecordBatchReaderBuilder, RowFilter,
};
use parquet::file::statistics::Statistics;
use std::fs::File;
use std::path::Path;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Filter {
    pub from: Option<u64>,
    pub to: Option<u64>,
    pub keys: Vec<(String, String)>,
}

pub(crate) type Sink<'a> = dyn FnMut(&[u8]) -> Result<(), Error> + Send + 'a;

impl Table {
    pub(crate) fn admits(&self, filter: &Filter, record: &Record) -> Result<bool, Error> {
        if filter.from.is_some_and(|from| record.time < from)
            || filter.to.is_some_and(|to| record.time >= to)
        {
            return Ok(false);
        }
        for (key, value) in &filter.keys {
            if &record.keys[self.index(key)?] != value {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub fn scan(&self, path: &Path, filter: &Filter, sink: &mut Sink<'_>) -> Result<(), Error> {
        let keys = filter
            .keys
            .iter()
            .map(|(key, value)| Ok((self.index(key)? + 1, value.clone())))
            .collect::<Result<Vec<_>, Error>>()?;
        let mut builder = ParquetRecordBatchReaderBuilder::try_new(File::open(path)?)?;
        let groups = admitted(&mut builder, filter, &keys)?;
        if groups.is_empty() {
            return Ok(());
        }
        let columns = builder.parquet_schema().clone();
        let mut predicates: Vec<Box<dyn ArrowPredicate>> = Vec::new();
        if filter.from.is_some() || filter.to.is_some() {
            let (from, to) = (filter.from.unwrap_or(0), filter.to.unwrap_or(u64::MAX));
            predicates.push(Box::new(ArrowPredicateFn::new(
                ProjectionMask::leaves(&columns, [0]),
                move |batch: RecordBatch| {
                    let times = column::<UInt64Array>(&batch)?;
                    Ok(times
                        .iter()
                        .map(|time| time.map(|time| time >= from && time < to))
                        .collect())
                },
            )));
        }
        for (index, value) in keys {
            predicates.push(Box::new(ArrowPredicateFn::new(
                ProjectionMask::leaves(&columns, [index]),
                move |batch: RecordBatch| {
                    let values = column::<StringArray>(&batch)?;
                    Ok(values
                        .iter()
                        .map(|found| found.map(|found| found == value))
                        .collect::<BooleanArray>())
                },
            )));
        }
        let raw = self.keys().len() + 1;
        let reader = builder
            .with_row_groups(groups)
            .with_projection(ProjectionMask::leaves(&columns, [raw]))
            .with_row_filter(RowFilter::new(predicates))
            .build()?;
        for batch in reader {
            let batch = batch?;
            let values = column::<BinaryArray>(&batch).map_err(Error::from)?;
            for value in values.iter().flatten() {
                sink(value)?;
            }
        }
        Ok(())
    }
}

fn admitted(
    builder: &mut ParquetRecordBatchReaderBuilder<File>,
    filter: &Filter,
    keys: &[(usize, String)],
) -> Result<Vec<usize>, Error> {
    let mut groups = Vec::new();
    for group in 0..builder.metadata().num_row_groups() {
        let statistics = builder.metadata().row_group(group).column(0).statistics();
        if let Some(Statistics::Int64(times)) = statistics {
            let (low, high) = (times.min_opt(), times.max_opt());
            let (low, high) = (low.map(|low| *low as u64), high.map(|high| *high as u64));
            if filter.to.is_some_and(|to| low.is_some_and(|low| low >= to))
                || filter
                    .from
                    .is_some_and(|from| high.is_some_and(|high| high < from))
            {
                continue;
            }
        }
        let mut present = true;
        for (index, value) in keys {
            if let Some(bloom) = builder.get_row_group_column_bloom_filter(group, *index)? {
                present &= bloom.check(&value.as_str());
            }
        }
        if present {
            groups.push(group);
        }
    }
    Ok(groups)
}

fn column<T: Array + Clone + 'static>(batch: &RecordBatch) -> Result<T, arrow_schema::ArrowError> {
    batch
        .column(0)
        .as_any()
        .downcast_ref::<T>()
        .cloned()
        .ok_or_else(|| arrow_schema::ArrowError::SchemaError("unexpected column type".into()))
}

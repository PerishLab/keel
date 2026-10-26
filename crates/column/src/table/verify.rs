use super::Table;
use crate::manifest::Entry;
use crate::segment::Segment;
use crate::{Error, Record, recover};
use arrow_array::{StringArray, UInt64Array};
use parquet::arrow::ProjectionMask;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use std::collections::BTreeSet;
use std::fs::File;
use std::path::Path;

impl Table {
    pub(crate) fn inspect(&self, root: &Path, parts: &[Entry]) -> Result<(), Error> {
        for entry in parts {
            let path = crate::store::located(root, recover::PARTS, &entry.scope, entry.source)
                .with_extension("parquet");
            self.part(&path, &entry.scope)?;
        }
        let published: BTreeSet<_> = parts
            .iter()
            .map(|part| (part.scope.as_str(), part.source))
            .collect();
        let mut open = BTreeSet::new();
        for (scope, seq, path) in recover::listed(&root.join(recover::OPEN))? {
            let source = seq
                .ok_or_else(|| Error::new(format!("unknown legacy segment {}", path.display())))?;
            if path != crate::store::located(root, recover::OPEN, &scope, source) {
                return Err(Error::new(format!(
                    "unknown legacy segment {}",
                    path.display()
                )));
            }
            let present = Segment::inspect(&path, self, &scope)?;
            if present
                && !published.contains(&(scope.as_str(), source))
                && !open.insert(scope.clone())
            {
                return Err(Error::new("two legacy open segments in one partition"));
            }
        }
        Ok(())
    }

    fn part(&self, path: &Path, scope: &str) -> Result<(), Error> {
        let builder = ParquetRecordBatchReaderBuilder::try_new(File::open(path)?)?;
        if builder.schema().as_ref() != &self.schema() {
            return Err(Error::new("legacy part schema differs from column model"));
        }
        let projection = ProjectionMask::leaves(builder.parquet_schema(), 0..=self.keys.len());
        let reader = builder.with_projection(projection).build()?;
        let mut previous = None;
        for batch in reader {
            let batch = batch?;
            let times = batch
                .column(0)
                .as_any()
                .downcast_ref::<UInt64Array>()
                .unwrap();
            let keys: Vec<_> = batch.columns()[1..]
                .iter()
                .map(|column| column.as_any().downcast_ref::<StringArray>().unwrap())
                .collect();
            for index in 0..batch.num_rows() {
                let record = Record {
                    raw: Vec::new(),
                    time: times.value(index),
                    keys: keys.iter().map(|key| key.value(index).into()).collect(),
                };
                self.scope(&record, scope)?;
                if previous
                    .as_ref()
                    .is_some_and(|previous| self.order(previous, &record).is_gt())
                {
                    return Err(Error::new("legacy part order differs from column model"));
                }
                previous = Some(record);
            }
        }
        Ok(())
    }

    pub(crate) fn scope(&self, record: &Record, scope: &str) -> Result<(), Error> {
        self.check(record)?;
        if self.partition(record).scope() != scope {
            return Err(Error::new(
                "legacy record partition differs from column model",
            ));
        }
        Ok(())
    }
}

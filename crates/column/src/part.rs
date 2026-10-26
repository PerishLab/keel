use crate::table::RAW;
use crate::{Error, Partition, Record, Row, Table};
use arrow_schema::{DataType, Field, Schema};
use parquet::arrow::ArrowWriter;
use parquet::basic::{Compression, Encoding, ZstdLevel};
use parquet::file::properties::{EnabledStatistics, WriterProperties};
use parquet::schema::types::ColumnPath;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const LEVEL: i32 = 9;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Summary {
    pub partition: Partition,
    pub from: u64,
    pub until: u64,
    pub rows: u64,
}

impl Table {
    pub fn write(&self, path: &Path, records: Vec<Record>) -> Result<Summary, Error> {
        self.rows(path, records.into_iter().map(Row::from).collect())
    }

    pub fn rows(&self, path: &Path, mut records: Vec<Row>) -> Result<Summary, Error> {
        let summary = self.summarize(&records)?;
        records.sort_by(|left, right| self.order(left, right));
        let staging = staged(path);
        let written = self.encode(&staging, &records);
        let published = written.and_then(|()| {
            fs::hard_link(&staging, path).map_err(|error| {
                Error::new(format!("cannot publish part {}: {error}", path.display()))
            })
        });
        let _ = fs::remove_file(&staging);
        published.map(|()| summary)
    }

    pub fn schema(&self) -> Schema {
        let mut fields = vec![Field::new(self.time(), DataType::UInt64, false)];
        fields.extend(
            self.keys()
                .iter()
                .map(|key| Field::new(key, DataType::Utf8, false)),
        );
        fields.push(Field::new(RAW, DataType::Binary, false));
        fields.extend(
            self.fields()
                .iter()
                .map(|field| Field::new(&field.name, field.kind.arrow(), true)),
        );
        Schema::new(fields)
    }

    fn summarize(&self, records: &[Row]) -> Result<Summary, Error> {
        for record in records {
            self.accept(record)?;
        }
        let first = records
            .first()
            .ok_or_else(|| Error::new("a part needs at least one record"))?;
        let partition = self.partition(first);
        let mut summary = Summary {
            partition: partition.clone(),
            from: first.time,
            until: first.time,
            rows: 0,
        };
        for record in records {
            if self.partition(record) != partition {
                return Err(Error::new("a part holds records of one partition only"));
            }
            summary.from = summary.from.min(record.time);
            summary.until = summary.until.max(record.time);
            summary.rows += 1;
        }
        summary.until += 1;
        Ok(summary)
    }

    fn encode(&self, path: &Path, records: &[Row]) -> Result<(), Error> {
        let schema = Arc::new(self.schema());
        let columns: Vec<_> = (0..schema.fields().len()).collect();
        let batch = self.batch(records, &columns)?;
        let file = File::create_new(path)?;
        let mut writer = ArrowWriter::try_new(file, schema, Some(self.properties()?))?;
        writer.write(&batch)?;
        writer.into_inner()?.sync_all()?;
        Ok(())
    }

    fn properties(&self) -> Result<WriterProperties, Error> {
        let time = ColumnPath::from(self.time());
        let mut builder = WriterProperties::builder()
            .set_compression(Compression::ZSTD(ZstdLevel::try_new(LEVEL)?))
            .set_statistics_enabled(EnabledStatistics::Page)
            .set_column_dictionary_enabled(time.clone(), false)
            .set_column_encoding(time, Encoding::DELTA_BINARY_PACKED)
            .set_column_dictionary_enabled(ColumnPath::from(RAW), false);
        for key in self.keys() {
            builder = builder.set_column_bloom_filter_enabled(ColumnPath::from(key.as_str()), true);
        }
        Ok(builder.build())
    }
}

fn staged(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".staging");
    PathBuf::from(name)
}

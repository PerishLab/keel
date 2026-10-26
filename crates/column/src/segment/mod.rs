mod frame;
mod scalar;

use crate::{Error, Row, Table};
use frame::Frame;
use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

pub(crate) struct Segment {
    pub(crate) seq: u64,
    pub(crate) records: Vec<Row>,
    pub(crate) tokens: BTreeSet<String>,
    pub(crate) bytes: u64,
    pub(crate) born: u64,
    path: PathBuf,
    file: File,
}

impl Segment {
    pub(crate) fn create(path: PathBuf, seq: u64, born: u64) -> Result<Self, Error> {
        let directory = path
            .parent()
            .ok_or_else(|| Error::new("segment has no directory"))?;
        fs::create_dir_all(directory)?;
        let file = OpenOptions::new()
            .create_new(true)
            .append(true)
            .open(&path)?;
        File::open(directory)?.sync_all()?;
        Ok(Self {
            seq,
            records: Vec::new(),
            tokens: BTreeSet::new(),
            bytes: 0,
            born,
            path,
            file,
        })
    }

    pub(crate) fn load(path: PathBuf, seq: u64, born: u64, table: &Table) -> Result<Self, Error> {
        let bytes = fs::read(&path)?;
        let mut segment = Self {
            seq,
            records: Vec::new(),
            tokens: BTreeSet::new(),
            bytes: 0,
            born,
            file: OpenOptions::new().append(true).open(&path)?,
            path,
        };
        let mut pending = Vec::new();
        let mut committed = 0;
        for (frame, end) in frame::decode(&bytes) {
            match frame {
                Frame::Entry(record) => {
                    table.accept(&record)?;
                    pending.push(record);
                }
                Frame::Token(token) => {
                    segment.absorb(token, std::mem::take(&mut pending));
                    committed = end;
                }
            }
        }
        if committed < bytes.len() {
            segment.file.set_len(committed as u64)?;
            segment.file.sync_all()?;
        }
        Ok(segment)
    }

    pub(crate) fn inspect(path: &Path, table: &crate::Table, scope: &str) -> Result<bool, Error> {
        let bytes = fs::read(path)?;
        let frames = frame::decode(&bytes);
        if !bytes.is_empty()
            && !matches!(frames.last(), Some((Frame::Token(_), end)) if *end == bytes.len())
        {
            return Err(Error::new(
                "legacy adoption requires complete acknowledged segments; recover torn data with the original model first",
            ));
        }
        let mut present = false;
        for (frame, _) in frames {
            match frame {
                Frame::Entry(record) => {
                    table.scope(&record, scope)?;
                    table.accept(&record)?;
                    present = true;
                }
                Frame::Token(token) if token.is_empty() || token.contains('\n') => {
                    return Err(Error::new("legacy segment carries an invalid batch token"));
                }
                _ => {}
            }
        }
        Ok(present)
    }

    pub(crate) fn commit(&mut self, token: &str, records: Vec<Row>) -> Result<(), Error> {
        let mut buffer = Vec::new();
        for record in &records {
            buffer.extend(frame::encode(&Frame::Entry(record.clone()))?);
        }
        buffer.extend(frame::encode(&Frame::Token(token.to_string()))?);
        self.file.write_all(&buffer)?;
        self.file.sync_data()?;
        self.absorb(token.to_string(), records);
        Ok(())
    }

    pub(crate) fn remove(self) -> Result<(), Error> {
        fs::remove_file(&self.path)?;
        Ok(())
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    fn absorb(&mut self, token: String, records: Vec<Row>) {
        self.bytes += records
            .iter()
            .map(|record| {
                record.raw.len() as u64
                    + record.values.iter().map(crate::Value::stored).sum::<u64>()
                    + if record.values.is_empty() { 0 } else { 4 }
            })
            .sum::<u64>();
        self.records.extend(records);
        self.tokens.insert(token);
    }
}

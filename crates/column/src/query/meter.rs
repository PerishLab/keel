use bytes::Bytes;
use parquet::errors::{ParquetError, Result};
use parquet::file::reader::{ChunkReader, Length};
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub(super) struct Meter {
    value: Arc<Mutex<u64>>,
    limit: u64,
}

pub(super) struct Source {
    file: File,
    size: u64,
    meter: Meter,
}

pub(super) struct Stream {
    file: File,
    meter: Meter,
}

impl Meter {
    pub fn new(limit: u64) -> Self {
        Self {
            value: Arc::new(Mutex::new(0)),
            limit,
        }
    }

    pub fn bytes(&self) -> u64 {
        *self.value.lock().unwrap()
    }
}

impl Source {
    pub fn new(file: File, meter: Meter) -> io::Result<Self> {
        Ok(Self {
            size: file.metadata()?.len(),
            file,
            meter,
        })
    }
}

impl Length for Source {
    fn len(&self) -> u64 {
        self.size
    }
}

impl ChunkReader for Source {
    type T = Stream;

    fn get_read(&self, start: u64) -> Result<Self::T> {
        let mut file = self.file.try_clone()?;
        file.seek(SeekFrom::Start(start))?;
        Ok(Stream {
            file,
            meter: self.meter.clone(),
        })
    }

    fn get_bytes(&self, start: u64, length: usize) -> Result<Bytes> {
        if length as u64 > self.meter.limit.saturating_sub(self.meter.bytes()) {
            return Err(ParquetError::General(
                "query read bytes limit exceeded".into(),
            ));
        }
        let mut stream = self.get_read(start)?;
        let mut bytes = vec![0; length];
        stream.read_exact(&mut bytes)?;
        Ok(Bytes::from(bytes))
    }
}

impl Read for Stream {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        let mut value = self.meter.value.lock().unwrap();
        let remaining = self.meter.limit.saturating_sub(*value);
        if remaining == 0 {
            return Err(io::Error::other("query read bytes limit exceeded"));
        }
        let size = buffer
            .len()
            .min(usize::try_from(remaining).unwrap_or(usize::MAX));
        let size = self.file.read(&mut buffer[..size])?;
        *value += size as u64;
        Ok(size)
    }
}

use crate::Error;
use std::collections::BTreeSet;

pub(crate) const RAW: &str = "raw";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Table {
    time: String,
    keys: Vec<String>,
    sort: Vec<usize>,
    grain: Grain,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Grain {
    key: Option<usize>,
    width: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub raw: Vec<u8>,
    pub time: u64,
    pub keys: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Partition {
    pub key: Option<String>,
    pub slot: u64,
}

impl Table {
    pub fn new(time: &str, keys: &[&str]) -> Result<Self, Error> {
        let names: Vec<&str> = std::iter::once(time).chain(keys.iter().copied()).collect();
        let distinct: BTreeSet<&str> = names.iter().copied().collect();
        if names.iter().any(|name| name.is_empty() || *name == RAW) || distinct.len() != names.len()
        {
            return Err(Error::new(format!(
                "columns must be distinct, non-empty and not {RAW:?}: {names:?}"
            )));
        }
        Ok(Self {
            time: time.to_string(),
            keys: keys.iter().map(|key| (*key).to_string()).collect(),
            sort: Vec::new(),
            grain: Grain {
                key: None,
                width: u64::MAX,
            },
        })
    }

    pub fn sort(mut self, keys: &[&str]) -> Result<Self, Error> {
        self.sort = keys
            .iter()
            .map(|key| self.index(key))
            .collect::<Result<_, _>>()?;
        Ok(self)
    }

    pub fn grain(mut self, key: Option<&str>, width: u64) -> Result<Self, Error> {
        if width == 0 {
            return Err(Error::new("partition width must be positive"));
        }
        self.grain = Grain {
            key: key.map(|key| self.index(key)).transpose()?,
            width,
        };
        Ok(self)
    }

    pub fn partition(&self, record: &Record) -> Partition {
        Partition {
            key: self.grain.key.map(|index| record.keys[index].clone()),
            slot: record.time / self.grain.width,
        }
    }

    pub(crate) fn index(&self, key: &str) -> Result<usize, Error> {
        self.keys
            .iter()
            .position(|name| name == key)
            .ok_or_else(|| Error::new(format!("{key:?} is not a declared key column")))
    }

    pub(crate) fn time(&self) -> &str {
        &self.time
    }

    pub(crate) fn keys(&self) -> &[String] {
        &self.keys
    }

    pub(crate) fn order(&self, left: &Record, right: &Record) -> std::cmp::Ordering {
        self.sort
            .iter()
            .map(|index| left.keys[*index].cmp(&right.keys[*index]))
            .find(|ordering| ordering.is_ne())
            .unwrap_or_else(|| left.time.cmp(&right.time))
    }
}

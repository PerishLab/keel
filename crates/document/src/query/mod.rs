mod description;
mod execute;

use crate::{Error, Head};
use radicula_document::{Document, Path, Presence, Value};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

const DECLARATIONS: usize = 256;

#[derive(Clone, Debug, PartialEq)]
pub enum Condition {
    Equal { path: Path, value: Value },
    Missing(Path),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Projection {
    pub name: String,
    pub path: Path,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limit {
    pub candidates: u64,
    pub rows: u64,
    pub bytes: u64,
}

impl Default for Limit {
    fn default() -> Self {
        Self {
            candidates: 100_000,
            rows: 10_000,
            bytes: 64 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Query {
    pub collection: String,
    pub conditions: Vec<Condition>,
    pub select: Vec<Projection>,
    pub limit: Limit,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Stats {
    pub candidates: u64,
    pub admitted: u64,
    pub matched: u64,
    pub bytes: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Selection {
    pub head: Head,
    pub values: BTreeMap<String, Value>,
}

impl Query {
    pub(crate) fn validate(&self) -> Result<(), Error> {
        if self.collection.is_empty() {
            return Err(Error::Query("query needs a collection".into()));
        }
        if self.conditions.len() > DECLARATIONS || self.select.len() > DECLARATIONS {
            return Err(Error::Query("query exceeds declaration limit".into()));
        }
        let mut names = BTreeSet::new();
        for projection in &self.select {
            if projection.name.is_empty() || !names.insert(&projection.name) {
                return Err(Error::Model("duplicate projection name".into()));
            }
        }
        let mut seen = BTreeSet::new();
        for condition in &self.conditions {
            if !seen.insert(same(condition)?) {
                return Err(Error::Query("duplicate condition".into()));
            }
        }
        Ok(())
    }

    pub(crate) fn accepts(&self, document: &Document) -> Result<bool, Error> {
        for condition in &self.conditions {
            let accepted = match condition {
                Condition::Equal { path, value } => {
                    document.read(path).map_err(Error::Document)? == Presence::Present(value)
                }
                Condition::Missing(path) => {
                    document.read(path).map_err(Error::Document)? == Presence::Missing
                }
            };
            if !accepted {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub(crate) fn project(&self, document: &Document) -> Result<BTreeMap<String, Value>, Error> {
        let mut values = BTreeMap::new();
        for projection in &self.select {
            if let Presence::Present(value) =
                document.read(&projection.path).map_err(Error::Document)?
            {
                values.insert(projection.name.clone(), value.clone());
            }
        }
        Ok(values)
    }
}

fn same(condition: &Condition) -> Result<String, Error> {
    match condition {
        Condition::Equal { path, value } => {
            Document::new(value.clone()).map_err(Error::Document)?;
            Ok(format!("equal:{}\u{0}{}", text(path), value))
        }
        Condition::Missing(path) => Ok(format!("missing:{}", text(path))),
    }
}

pub(crate) fn text(path: &Path) -> String {
    let mut out = String::new();
    for part in path.parts() {
        out.push('/');
        for mark in part.chars() {
            match mark {
                '~' => out.push_str("~0"),
                '/' => out.push_str("~1"),
                _ => out.push(mark),
            }
        }
    }
    out
}

pub(crate) fn path(text: &str) -> Result<Path, Error> {
    Path::parse(text).map_err(Error::Document)
}

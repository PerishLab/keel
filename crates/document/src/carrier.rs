use crate::{Error, Model};
use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::path::Path;

const CONTROL: TableDefinition<&str, &str> = TableDefinition::new("control");
const VALUES: TableDefinition<&str, &[u8]> = TableDefinition::new("values");

pub(crate) struct Carrier {
    db: Database,
    _lock: File,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Binding {
    pub(crate) format: u32,
    pub(crate) root: i64,
    pub(crate) nonce: String,
    pub(crate) model: Model,
}

impl Carrier {
    pub(crate) fn create(root: &Path) -> Result<Self, Error> {
        fs::create_dir_all(root)?;
        if fs::read_dir(root)?.next().is_some() {
            return Err(Error::Binding("create requires a vacant directory".into()));
        }
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(root.join("lock"))?;
        hold(&lock)?;
        let db = Database::create(root.join("store.redb")).map_err(fault)?;
        Ok(Self { db, _lock: lock })
    }

    pub(crate) fn open(root: &Path) -> Result<Self, Error> {
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .open(root.join("lock"))?;
        hold(&lock)?;
        let db = Database::open(root.join("store.redb")).map_err(fault)?;
        Ok(Self { db, _lock: lock })
    }

    pub(crate) fn bind(&self, binding: &Binding) -> Result<(), Error> {
        binding.model.describe()?;
        let text = serde_json::to_string(binding).map_err(fault)?;
        let write = self.db.begin_write().map_err(fault)?;
        {
            let mut table = write.open_table(CONTROL).map_err(fault)?;
            table.insert("binding", text.as_str()).map_err(fault)?;
            write.open_table(VALUES).map_err(fault)?;
        }
        write.commit().map_err(fault)
    }

    pub(crate) fn binding(&self) -> Result<Binding, Error> {
        let read = self.db.begin_read().map_err(fault)?;
        let table = read.open_table(CONTROL).map_err(fault)?;
        let held = table
            .get("binding")
            .map_err(fault)?
            .ok_or_else(|| Error::Binding("carrier has no binding".into()))?;
        let binding: Binding = serde_json::from_str(held.value()).map_err(fault)?;
        if binding.format != 1 || binding.root <= 0 || !token(&binding.nonce) {
            return Err(Error::Binding("invalid carrier binding".into()));
        }
        binding.model.describe()?;
        Ok(binding)
    }

    pub(crate) fn insert(&self, id: i64, nonce: &str, value: &[u8]) -> Result<(), Error> {
        let key = key(id, nonce)?;
        let write = self.db.begin_write().map_err(fault)?;
        {
            let mut table = write.open_table(VALUES).map_err(fault)?;
            if table.get(key.as_str()).map_err(fault)?.is_some() {
                return Err(Error::Binding("payload key is already occupied".into()));
            }
            table.insert(key.as_str(), value).map_err(fault)?;
        }
        write.commit().map_err(fault)
    }

    pub(crate) fn get(&self, id: i64, nonce: &str) -> Result<Vec<u8>, Error> {
        let key = key(id, nonce)?;
        let read = self.db.begin_read().map_err(fault)?;
        let table = read.open_table(VALUES).map_err(fault)?;
        table
            .get(key.as_str())
            .map_err(fault)?
            .map(|held| held.value().to_vec())
            .ok_or_else(|| Error::Binding("published payload is absent".into()))
    }

    pub(crate) fn inventory(&self) -> Result<Vec<(i64, String)>, Error> {
        let read = self.db.begin_read().map_err(fault)?;
        let table = read.open_table(VALUES).map_err(fault)?;
        table
            .iter()
            .map_err(fault)?
            .map(|entry| {
                let (held, _) = entry.map_err(fault)?;
                let (id, token) = held
                    .value()
                    .split_once('/')
                    .ok_or_else(|| Error::Binding("invalid payload key".into()))?;
                let id = id.parse().map_err(fault)?;
                key(id, token)?;
                Ok((id, token.to_string()))
            })
            .collect()
    }

    pub(crate) fn prune(&self, keep: &BTreeSet<String>) -> Result<(), Error> {
        let write = self.db.begin_write().map_err(fault)?;
        {
            let mut table = write.open_table(VALUES).map_err(fault)?;
            table.retain(|key, _| keep.contains(key)).map_err(fault)?;
        }
        write.commit().map_err(fault)
    }
}

fn key(id: i64, nonce: &str) -> Result<String, Error> {
    if id <= 0 || !token(nonce) {
        return Err(Error::Binding("invalid payload identity".into()));
    }
    Ok(format!("{id}/{nonce}"))
}

fn token(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn hold(file: &File) -> Result<(), Error> {
    radicula_lock::attempt(file, radicula_lock::Mode::Exclusive)
        .map_err(|_| Error::Binding("document root is held by another writer".into()))
}

fn fault(error: impl std::fmt::Display) -> Error {
    Error::carrier(error.to_string())
}

pub(crate) fn nonce() -> Result<String, Error> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(fault)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

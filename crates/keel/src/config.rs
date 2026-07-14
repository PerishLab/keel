use crate::adapt::Error;
use crate::adapt::db::Sqlite;
use serde::Deserialize;
use std::path::{Path, PathBuf};

const NAME: &str = "keel.toml";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub listen: Listen,
    pub store: Store,
    pub root: PathBuf,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Listen {
    pub host: String,
    pub port: u16,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Store {
    pub kind: Kind,
    pub path: String,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    #[default]
    Memory,
    File,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct File {
    listen: Listen,
    store: Store,
}

impl Default for Listen {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 3000,
        }
    }
}

impl Default for Store {
    fn default() -> Self {
        Self {
            kind: Kind::Memory,
            path: String::new(),
        }
    }
}

pub fn load(root: impl AsRef<Path>) -> Config {
    let root = root.as_ref().to_path_buf();
    let path = root.join(NAME);
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let file: File = toml::from_str(&text).unwrap_or_default();
    Config {
        listen: file.listen,
        store: file.store,
        root,
    }
}

impl Config {
    pub fn open(&self) -> Result<Sqlite, Error> {
        match self.store.kind {
            Kind::Memory => Ok(Sqlite::memory()),
            Kind::File => {
                if self.store.path.trim().is_empty() {
                    return Err(Error::Adapt("store.kind=file requires store.path".into()));
                }
                let path = if Path::new(&self.store.path).is_absolute() {
                    PathBuf::from(&self.store.path)
                } else {
                    self.root.join(&self.store.path)
                };
                if let Some(parent) = path.parent()
                    && !parent.as_os_str().is_empty()
                {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| Error::Adapt(format!("store path: {e}")))?;
                }
                Ok(Sqlite::file(path.to_string_lossy()))
            }
        }
    }
}

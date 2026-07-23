use crate::adapt::Error;
use crate::adapt::db::Sqlite;
pub use plumb_lib::config::{Kind, Listen, Store};
use serde::Deserialize;
use std::path::{Path, PathBuf};

const NAME: &str = "keel.toml";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub listen: Listen,
    pub store: Store,
    pub identity: Identity,
    pub cache: Cache,
    pub root: PathBuf,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Cache {
    pub kind: Hold,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Hold {
    #[default]
    Memory,
    None,
}

impl Default for Cache {
    fn default() -> Self {
        Self { kind: Hold::Memory }
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Identity {
    pub unit: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct File {
    listen: Listen,
    store: Store,
    identity: Identity,
    cache: Cache,
}

pub fn load(root: impl AsRef<Path>) -> Config {
    let root = root.as_ref().to_path_buf();
    let path = root.join(NAME);
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let file: File = toml::from_str(&text).unwrap_or_default();
    Config {
        listen: file.listen,
        store: file.store,
        identity: file.identity,
        cache: file.cache,
        root,
    }
}

impl Config {
    pub async fn open(&self) -> Result<Sqlite, Error> {
        match self.store.kind {
            Kind::Memory => Sqlite::memory().await,
            Kind::File => {
                if self.store.path.trim().is_empty() {
                    return Err(Error::Adapt("store.kind=file requires store.path".into()));
                }
                let path = self.store.rebased(&self.root);
                if let Some(parent) = path.parent()
                    && !parent.as_os_str().is_empty()
                {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| Error::Adapt(format!("store path: {e}")))?;
                }
                Sqlite::file(path).await
            }
        }
    }
}

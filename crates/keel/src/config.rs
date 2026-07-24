use crate::adapt::Error;
use crate::adapt::db::Sqlite;
use plumb::config::{Env, Kind, Listen, Store};
use serde::Deserialize;
use std::path::{Path, PathBuf};

const NAME: &str = "keel.toml";

#[derive(Clone, Debug, Default, PartialEq, Eq, plumb::config::Cascade)]
pub struct Config {
    #[cascade(section)]
    pub listen: Listen,
    #[cascade(section)]
    pub store: Store,
    #[cascade(section)]
    pub identity: Identity,
    #[cascade(section)]
    pub cache: Cache,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, plumb::config::Cascade)]
#[cascade(section)]
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

impl Env for Hold {
    fn read(value: &str) -> Result<Self, String> {
        match value {
            "memory" => Ok(Hold::Memory),
            "none" => Ok(Hold::None),
            _ => Err("neither memory nor none".to_string()),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, plumb::config::Cascade)]
#[cascade(section)]
#[serde(default)]
pub struct Identity {
    pub unit: String,
}

pub fn load(start: impl AsRef<Path>) -> Result<(Config, PathBuf), plumb::config::Error> {
    let start = start.as_ref();
    match plumb::config::discover(start, NAME) {
        Ok(found) => {
            let root = found.parent().unwrap_or(start).to_path_buf();
            Ok((Config::resolve(Some(&found))?, root))
        }
        Err(_) => Ok((Config::resolve(None)?, start.to_path_buf())),
    }
}

impl Config {
    pub async fn open(&self, root: &Path) -> Result<Sqlite, Error> {
        match self.store.kind {
            Kind::Memory => Sqlite::memory().await,
            Kind::File => {
                if self.store.path.trim().is_empty() {
                    return Err(Error::Adapt("store.kind=file requires store.path".into()));
                }
                let path = self.store.rebased(root);
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

use plumb::config::Env;
use serde::Deserialize;
use std::path::{Path, PathBuf};

pub const NAME: &str = "keel.toml";

#[derive(Clone, Debug, Default, PartialEq, Eq, plumb::config::Cascade)]
pub struct Config {
    #[cascade(section)]
    pub listen: Listen,
    #[cascade(section)]
    pub identity: Identity,
    #[cascade(section)]
    pub cache: Cache,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, plumb::config::Cascade)]
#[cascade(section)]
#[serde(default)]
pub struct Listen {
    pub host: String,
    pub port: u16,
    pub prefix: String,
}

impl Default for Listen {
    fn default() -> Self {
        Listen {
            host: "127.0.0.1".to_string(),
            port: 3000,
            prefix: String::new(),
        }
    }
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

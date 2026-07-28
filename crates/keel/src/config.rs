use plumb::config::Env;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::str::FromStr;

pub const NAME: &str = "keel.toml";

#[derive(Clone, Debug, Default, PartialEq, Eq, plumb::config::Cascade)]
pub struct Config {
    #[cascade(section)]
    pub listen: Listen,
    #[cascade(section)]
    pub identity: Identity,
    #[cascade(section)]
    pub cache: Cache,
    #[cascade(section)]
    pub estate: Estate,
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

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, plumb::config::Cascade)]
#[cascade(section)]
#[serde(default)]
pub struct Estate {
    #[cascade(section)]
    pub generation: Generation,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, plumb::config::Cascade)]
#[cascade(section)]
#[serde(default)]
pub struct Generation {
    #[cascade(section)]
    pub cleanup: Cleanup,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, plumb::config::Cascade)]
#[cascade(section)]
#[serde(default)]
pub struct Cleanup {
    pub retain: Retain,
}

impl Default for Cleanup {
    fn default() -> Self {
        Self {
            retain: Retain::Span(30 * 24 * 60 * 60),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(try_from = "String")]
pub enum Retain {
    Span(u64),
    Forever,
}

impl Retain {
    pub(crate) fn elapsed(self, retired: i64, now: i64) -> bool {
        match self {
            Self::Forever => false,
            Self::Span(seconds) => now
                .checked_sub(retired)
                .and_then(|age| u64::try_from(age).ok())
                .is_some_and(|age| age >= seconds),
        }
    }
}

impl FromStr for Retain {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value == "forever" {
            return Ok(Self::Forever);
        }
        let (count, unit) = value.split_at(value.len().saturating_sub(1));
        if count.is_empty() || !count.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err("needs <digits><s|m|h|d> or forever".into());
        }
        let count = count
            .parse::<u64>()
            .map_err(|_| "duration is too large".to_string())?;
        let scale = match unit {
            "s" => 1,
            "m" => 60,
            "h" => 60 * 60,
            "d" => 24 * 60 * 60,
            _ => return Err("needs <digits><s|m|h|d> or forever".into()),
        };
        count
            .checked_mul(scale)
            .map(Self::Span)
            .ok_or_else(|| "duration is too large".to_string())
    }
}

impl TryFrom<String> for Retain {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl Env for Retain {
    fn read(value: &str) -> Result<Self, String> {
        value.parse()
    }
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

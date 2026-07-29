use serde::Deserialize;
use std::str::FromStr;

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Estate {
    pub generation: Generation,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Generation {
    pub cleanup: Cleanup,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
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

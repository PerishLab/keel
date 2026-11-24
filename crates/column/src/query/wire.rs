use super::Limit;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Scan {
    pub(super) format: u32,
    pub(super) kind: String,
    pub(super) filter: Filter,
    pub(super) projection: Projection,
    pub(super) batch: usize,
    pub(super) limit: Limit,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Filter {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) from: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) to: Option<u64>,
    #[serde(default)]
    pub(super) keys: Vec<Key>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Key {
    pub(super) name: String,
    pub(super) value: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Projection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) columns: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) count: Option<bool>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Join {
    pub(super) format: u32,
    pub(super) kind: String,
    pub(super) left: Side,
    pub(super) right: Side,
    pub(super) mode: super::join::Mode,
    pub(super) batch: usize,
    pub(super) limit: super::join::Limit,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Side {
    pub(super) filter: Filter,
    pub(super) key: String,
    pub(super) columns: Vec<String>,
    pub(super) when: Vec<Condition>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Condition {
    pub(super) kind: String,
    pub(super) column: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) value: Option<Value>,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(super) struct Value {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) null: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) bool: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) int: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) uint: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) bytes: Option<Vec<u8>>,
}

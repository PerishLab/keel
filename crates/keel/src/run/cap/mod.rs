use crate::life::Cell;
use std::collections::BTreeMap;

pub const GRANT: &str = "@grant";
pub const SEAL: &str = "@seal";
pub const PULSE: &str = "@pulse";
pub const WINDOW: usize = 4096;
pub const VERBS: [&str; 6] = ["see", "put", "set", "end", "tie", "cut"];
pub const DEPTH: usize = 16;

pub struct Mark<'a> {
    pub key: Option<i64>,
    pub cells: &'a BTreeMap<String, Cell>,
}

static BARE: BTreeMap<String, Cell> = BTreeMap::new();

impl Mark<'_> {
    pub fn none() -> Mark<'static> {
        Mark {
            key: None,
            cells: &BARE,
        }
    }
}

pub struct Plea<'a> {
    pub who: crate::face::Who,
    pub verb: &'a str,
    pub unit: &'a str,
    pub mark: &'a Mark<'a>,
}

mod check;
mod grant;
mod seal;
mod shape;

pub(crate) use check::*;
pub(crate) use grant::*;
pub(crate) use seal::*;
pub(crate) use shape::*;

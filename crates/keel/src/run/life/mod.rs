use crate::wire::Wire;
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Cell {
    Bool(bool),
    Int(i64),
    Text(String),
}

impl Cell {
    pub fn text(&self) -> &str {
        match self {
            Cell::Text(value) => value,
            _ => "",
        }
    }

    pub fn show(&self) -> String {
        match self {
            Cell::Text(value) => value.clone(),
            Cell::Int(value) => value.to_string(),
            Cell::Bool(value) => value.to_string(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Row {
    key: i64,
    cells: BTreeMap<String, Cell>,
    expires: Option<i64>,
    created: i64,
    updated: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Ends {
    pub left: i64,
    pub right: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Tie {
    key: i64,
    left: i64,
    right: i64,
    cells: BTreeMap<String, Cell>,
    expires: Option<i64>,
    created: i64,
    updated: i64,
}

impl Row {
    pub fn key(&self) -> i64 {
        self.key
    }

    pub fn cells(&self) -> &BTreeMap<String, Cell> {
        &self.cells
    }

    pub fn expires(&self) -> Option<i64> {
        self.expires
    }

    pub fn created(&self) -> i64 {
        self.created
    }

    pub fn updated(&self) -> i64 {
        self.updated
    }
}

impl Tie {
    pub fn key(&self) -> i64 {
        self.key
    }

    pub fn left(&self) -> i64 {
        self.left
    }

    pub fn right(&self) -> i64 {
        self.right
    }

    pub fn cells(&self) -> &BTreeMap<String, Cell> {
        &self.cells
    }

    pub fn expires(&self) -> Option<i64> {
        self.expires
    }

    pub fn created(&self) -> i64 {
        self.created
    }

    pub fn updated(&self) -> i64 {
        self.updated
    }
}

pub struct Work<'a, W: Wire> {
    wire: &'a mut W,
}

pub fn tick() -> i64 {
    now()
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

mod edit;
mod link;
mod make;
mod util;

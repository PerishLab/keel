use crate::life::{Row, Tie};
use std::collections::BTreeMap;

pub const CAP: usize = 10_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Slice {
    Live,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Op {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    In,
    Like,
    Null,
    Has,
    Some,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Rank {
    Asc,
    Desc,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Pred {
    field: String,
    op: Op,
    values: Vec<String>,
    nest: Option<Box<Pred>>,
}

impl Pred {
    pub fn field(&self) -> &str {
        &self.field
    }

    pub fn op(&self) -> Op {
        self.op
    }

    pub fn value(&self) -> &str {
        self.values.first().map(String::as_str).unwrap_or("")
    }

    pub fn values(&self) -> &[String] {
        &self.values
    }

    pub fn nest(&self) -> Option<&Pred> {
        self.nest.as_deref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Sort {
    field: String,
    rank: Rank,
}

impl Sort {
    pub fn field(&self) -> &str {
        &self.field
    }

    pub fn rank(&self) -> Rank {
        self.rank
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Tree {
    from: String,
    slice: Slice,
    preds: Vec<Pred>,
    links: Vec<String>,
    sorts: Vec<Sort>,
    limit: Option<usize>,
    after: Option<i64>,
    tally: bool,
}

impl Tree {
    pub fn when(mut self, field: &str, op: Op, value: &str) -> Self {
        self.preds.push(Pred {
            field: field.to_string(),
            op,
            values: vec![value.to_string()],
            nest: None,
        });
        self
    }

    pub fn any(mut self, field: &str, values: &[&str]) -> Self {
        self.preds.push(Pred {
            field: field.to_string(),
            op: Op::In,
            values: values.iter().map(|value| value.to_string()).collect(),
            nest: None,
        });
        self
    }

    pub fn missing(mut self, field: &str) -> Self {
        self.preds.push(Pred {
            field: field.to_string(),
            op: Op::Null,
            values: Vec::new(),
            nest: None,
        });
        self
    }

    pub fn link(mut self, bond: &str) -> Self {
        self.links.push(bond.to_string());
        self
    }

    pub fn order(mut self, field: &str, rank: Rank) -> Self {
        self.sorts.push(Sort {
            field: field.to_string(),
            rank,
        });
        self
    }

    pub fn top(mut self, n: usize) -> Self {
        self.limit = Some(n);
        self
    }

    pub fn past(mut self, key: i64) -> Self {
        self.after = Some(key);
        self
    }

    pub fn count(mut self) -> Self {
        self.tally = true;
        self
    }

    pub fn from(&self) -> &str {
        &self.from
    }

    pub fn slice(&self) -> Slice {
        self.slice
    }

    pub fn preds(&self) -> &[Pred] {
        &self.preds
    }

    pub fn links(&self) -> &[String] {
        &self.links
    }

    pub fn sort(&self) -> Option<&Sort> {
        self.sorts.first()
    }

    pub fn sorts(&self) -> &[Sort] {
        &self.sorts
    }

    pub fn limit(&self) -> Option<usize> {
        self.limit
    }

    pub fn after(&self) -> Option<i64> {
        self.after
    }

    pub fn tally(&self) -> bool {
        self.tally
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Bag {
    Unit(Vec<Row>),
    Bond(Vec<Tie>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pack {
    root: String,
    bags: BTreeMap<String, Bag>,
    count: Option<usize>,
}

impl Pack {
    pub fn root(&self) -> &str {
        &self.root
    }

    pub fn bags(&self) -> &BTreeMap<String, Bag> {
        &self.bags
    }

    pub fn unit(&self, key: &str) -> Option<&[Row]> {
        match self.bags.get(key) {
            Some(Bag::Unit(rows)) => Some(rows.as_slice()),
            _ => None,
        }
    }

    pub fn bond(&self, key: &str) -> Option<&[Tie]> {
        match self.bags.get(key) {
            Some(Bag::Bond(ties)) => Some(ties.as_slice()),
            _ => None,
        }
    }

    pub fn rows(&self) -> &[Row] {
        self.unit(&self.root).unwrap_or(&[])
    }

    pub fn count(&self) -> Option<usize> {
        self.count
    }

    pub(crate) fn amend(&mut self) -> &mut BTreeMap<String, Bag> {
        &mut self.bags
    }

    pub(crate) fn tallied(root: String, n: usize) -> Pack {
        Pack {
            root,
            bags: BTreeMap::new(),
            count: Some(n),
        }
    }
}

pub type Ask = Tree;

mod check;
mod parse;
mod run;
mod scan;
mod sift;

pub use check::*;
pub use parse::*;
pub use run::*;
pub(crate) use scan::*;
pub use sift::*;

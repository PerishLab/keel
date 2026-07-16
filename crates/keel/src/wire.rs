use crate::adapt::Error;

#[derive(Clone, Debug, PartialEq)]
pub enum Val {
    Null,
    Int(i64),
    Text(String),
}

impl Val {
    pub fn int(&self) -> i64 {
        match self {
            Val::Int(value) => *value,
            _ => 0,
        }
    }

    pub fn opt(&self) -> Option<i64> {
        match self {
            Val::Int(value) => Some(*value),
            _ => None,
        }
    }

    pub fn text(&self) -> String {
        match self {
            Val::Text(value) => value.clone(),
            Val::Int(value) => value.to_string(),
            Val::Null => String::new(),
        }
    }
}

pub trait Wire {
    fn run(&self, sql: &str, args: &[Val]) -> Result<u64, Error>;
    fn plant(&self, sql: &str, args: &[Val]) -> Result<i64, Error>;
    fn rows(&self, sql: &str, args: &[Val]) -> Result<Vec<Vec<Val>>, Error>;
    fn script(&self, sql: &str) -> Result<(), Error>;
}

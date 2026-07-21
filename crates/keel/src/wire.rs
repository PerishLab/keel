use crate::adapt::Error;
use crate::ddl::Grain;
use std::future::Future;

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

pub trait Wire: Send {
    fn grain(&self) -> Grain;
    fn run(&mut self, sql: &str, args: &[Val]) -> impl Future<Output = Result<u64, Error>> + Send;
    fn plant(&mut self, sql: &str, args: &[Val])
    -> impl Future<Output = Result<i64, Error>> + Send;
    fn rows(
        &mut self,
        sql: &str,
        args: &[Val],
    ) -> impl Future<Output = Result<Vec<Vec<Val>>, Error>> + Send;
    fn script(&mut self, sql: &str) -> impl Future<Output = Result<(), Error>> + Send;
    fn revive(&mut self) -> impl Future<Output = Result<(), Error>> + Send {
        async { Ok(()) }
    }
}

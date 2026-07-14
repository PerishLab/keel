use crate::adapt::Error;
use crate::life::{Ends, Row, Tie};
use crate::plan::Plan;

pub trait Store: Send + Sync {
    fn wire(&self, plan: &Plan) -> Result<(), Error>;
    fn put(&self, plan: &Plan, name: &str, fields: &[(&str, &str)]) -> Result<i64, Error>;
    fn live(&self, plan: &Plan, name: &str) -> Result<Vec<Row>, Error>;
    fn end(&self, plan: &Plan, name: &str, key: i64) -> Result<(), Error>;
    fn tie(&self, plan: &Plan, owner: &str, bond: &str, ends: Ends) -> Result<i64, Error>;
    fn ties(&self, plan: &Plan, owner: &str, bond: &str, left: i64) -> Result<Vec<Tie>, Error>;
    fn cut(&self, plan: &Plan, owner: &str, bond: &str, key: i64) -> Result<(), Error>;
    fn has(&self, name: &str) -> Result<bool, Error>;
    fn cols(&self, name: &str) -> Result<Vec<String>, Error>;
}

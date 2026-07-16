use crate::adapt::Error;
use crate::life::{Ends, Row, Tie};
use crate::plan::Plan;

pub trait Store: Send + Sync {
    fn wire(&self, plan: &Plan) -> Result<(), Error>;
    fn put(&self, plan: &Plan, name: &str, fields: &[(&str, &str)]) -> Result<i64, Error>;
    fn set(&self, plan: &Plan, name: &str, key: i64, fields: &[(&str, &str)]) -> Result<(), Error>;
    fn live(&self, plan: &Plan, name: &str) -> Result<Vec<Row>, Error>;
    fn one(&self, plan: &Plan, name: &str, key: i64) -> Result<Option<Row>, Error>;
    fn end(&self, plan: &Plan, name: &str, key: i64) -> Result<(), Error>;
    fn lease(&self, plan: &Plan, name: &str, key: i64, at: i64) -> Result<(), Error>;
    fn tie(
        &self,
        plan: &Plan,
        owner: &str,
        bond: &str,
        ends: Ends,
        fields: &[(&str, &str)],
    ) -> Result<i64, Error>;
    fn set_tie(
        &self,
        plan: &Plan,
        owner: &str,
        bond: &str,
        key: i64,
        fields: &[(&str, &str)],
    ) -> Result<(), Error>;
    fn ties(&self, plan: &Plan, owner: &str, bond: &str, left: i64) -> Result<Vec<Tie>, Error>;
    fn cut(&self, plan: &Plan, owner: &str, bond: &str, key: i64) -> Result<(), Error>;
    fn live_has(&self, plan: &Plan, name: &str, key: i64) -> Result<bool, Error>;
    fn has(&self, name: &str) -> Result<bool, Error>;
    fn cols(&self, name: &str) -> Result<Vec<String>, Error>;
}

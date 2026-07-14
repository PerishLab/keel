use crate::adapt::Error;
use crate::plan::Plan;

pub trait Db {
    fn wire(&self, plan: &Plan) -> Result<(), Error>;
}

pub struct Sqlite;

impl Db for Sqlite {
    fn wire(&self, plan: &Plan) -> Result<(), Error> {
        if plan.units().is_empty() {
            return Err(Error::Adapt("db plan is empty".into()));
        }
        for unit in plan.units().values() {
            let reign = unit.reign();
            if !reign.expires() || !reign.created() || !reign.updated() {
                return Err(Error::Adapt("reign incomplete".into()));
            }
        }
        Ok(())
    }
}

use crate::adapt::Error;
use crate::plan::Plan;

pub trait Http {
    fn wire(&self, plan: &Plan) -> Result<(), Error>;
}

pub struct Utopia;

impl Http for Utopia {
    fn wire(&self, plan: &Plan) -> Result<(), Error> {
        if plan.units().is_empty() {
            return Err(Error::Adapt("http plan is empty".into()));
        }
        Ok(())
    }
}

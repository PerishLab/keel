use crate::adapt::Error;
use crate::ddl;
use crate::plan::Plan;
use std::sync::Mutex;

pub trait Http {
    fn wire(&self, plan: &Plan) -> Result<(), Error>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Path {
    unit: String,
    route: String,
}

impl Path {
    pub fn unit(&self) -> &str {
        &self.unit
    }

    pub fn route(&self) -> &str {
        &self.route
    }
}

pub struct Utopia {
    paths: Mutex<Vec<Path>>,
}

impl Utopia {
    pub fn new() -> Self {
        Self {
            paths: Mutex::new(Vec::new()),
        }
    }

    pub fn paths(&self) -> Result<Vec<Path>, Error> {
        let guard = self
            .paths
            .lock()
            .map_err(|_| Error::Adapt("http lock".into()))?;
        Ok(guard.clone())
    }
}

impl Default for Utopia {
    fn default() -> Self {
        Self::new()
    }
}

impl Http for Utopia {
    fn wire(&self, plan: &Plan) -> Result<(), Error> {
        if plan.units().is_empty() {
            return Err(Error::Adapt("http plan is empty".into()));
        }
        let mut paths = Vec::new();
        for unit in plan.units().values() {
            let name = unit.name().to_string();
            let route = format!("/{}", ddl::table(unit.name()));
            paths.push(Path { unit: name, route });
        }
        paths.sort_by(|a, b| a.route.cmp(&b.route));
        let mut guard = self
            .paths
            .lock()
            .map_err(|_| Error::Adapt("http lock".into()))?;
        *guard = paths;
        Ok(())
    }
}

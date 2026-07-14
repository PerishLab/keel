use crate::adapt::Error;
use crate::life::{Ends, Row, Tie};
use crate::plan::Plan;
use crate::store::Store;
use std::sync::Arc;

pub struct Core<S: Store> {
    plan: Plan,
    store: S,
}

impl<S: Store> Core<S> {
    pub(crate) fn new(plan: Plan, store: S) -> Self {
        Self { plan, store }
    }

    pub fn plan(&self) -> &Plan {
        &self.plan
    }

    pub fn store(&self) -> &S {
        &self.store
    }

    pub fn put(&self, name: &str, fields: &[(&str, &str)]) -> Result<i64, Error> {
        self.store.put(&self.plan, name, fields)
    }

    pub fn live(&self, name: &str) -> Result<Vec<Row>, Error> {
        self.store.live(&self.plan, name)
    }

    pub fn end(&self, name: &str, key: i64) -> Result<(), Error> {
        self.store.end(&self.plan, name, key)
    }

    pub fn tie(&self, owner: &str, bond: &str, ends: Ends) -> Result<i64, Error> {
        self.store.tie(&self.plan, owner, bond, ends)
    }

    pub fn ties(&self, owner: &str, bond: &str, left: i64) -> Result<Vec<Tie>, Error> {
        self.store.ties(&self.plan, owner, bond, left)
    }

    pub fn cut(&self, owner: &str, bond: &str, key: i64) -> Result<(), Error> {
        self.store.cut(&self.plan, owner, bond, key)
    }

    pub fn has(&self, name: &str) -> Result<bool, Error> {
        self.store.has(name)
    }

    pub fn cols(&self, name: &str) -> Result<Vec<String>, Error> {
        self.store.cols(name)
    }

    pub fn share(self) -> Arc<Self> {
        Arc::new(self)
    }
}

use crate::adapt::Error;
use crate::life::{Ends, Row, Tie};
use crate::plan::Plan;
use crate::query::{self, Pack, Tree};
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

    pub fn set(&self, name: &str, key: i64, fields: &[(&str, &str)]) -> Result<(), Error> {
        self.store.set(&self.plan, name, key, fields)
    }

    pub fn live(&self, name: &str) -> Result<Vec<Row>, Error> {
        let pack = self.ask(&query::form(name))?;
        Ok(pack.rows().to_vec())
    }

    pub fn query(&self, text: &str) -> Result<Pack, Error> {
        let tree = query::parse(text)?;
        self.ask(&tree)
    }

    pub fn ask(&self, tree: &Tree) -> Result<Pack, Error> {
        query::run(&self.plan, &self.store, tree)
    }

    pub fn end(&self, name: &str, key: i64) -> Result<(), Error> {
        self.store.end(&self.plan, name, key)
    }

    pub fn tie(
        &self,
        owner: &str,
        bond: &str,
        ends: Ends,
        fields: &[(&str, &str)],
    ) -> Result<i64, Error> {
        self.store.tie(&self.plan, owner, bond, ends, fields)
    }

    pub fn set_tie(
        &self,
        owner: &str,
        bond: &str,
        key: i64,
        fields: &[(&str, &str)],
    ) -> Result<(), Error> {
        self.store.set_tie(&self.plan, owner, bond, key, fields)
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

    pub fn sudo(&self) -> Face<'_, S> {
        Face {
            core: self,
            who: Who::Sudo,
        }
    }

    pub fn of(&self, operator: i64) -> Face<'_, S> {
        Face {
            core: self,
            who: Who::Op(operator),
        }
    }

    pub fn anon(&self) -> Face<'_, S> {
        Face {
            core: self,
            who: Who::Anon,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Who {
    Sudo,
    Op(i64),
    Anon,
}

pub struct Face<'a, S: Store> {
    core: &'a Core<S>,
    who: Who,
}

impl<S: Store> Face<'_, S> {
    pub fn who(&self) -> Who {
        self.who
    }

    pub fn put(&self, name: &str, fields: &[(&str, &str)]) -> Result<i64, Error> {
        self.allow()?;
        self.core.put(name, fields)
    }

    pub fn set(&self, name: &str, key: i64, fields: &[(&str, &str)]) -> Result<(), Error> {
        self.allow()?;
        self.core.set(name, key, fields)
    }

    pub fn end(&self, name: &str, key: i64) -> Result<(), Error> {
        self.allow()?;
        self.core.end(name, key)
    }

    pub fn live(&self, name: &str) -> Result<Vec<Row>, Error> {
        self.allow()?;
        self.core.live(name)
    }

    pub fn query(&self, text: &str) -> Result<Pack, Error> {
        self.allow()?;
        self.core.query(text)
    }

    pub fn ask(&self, tree: &Tree) -> Result<Pack, Error> {
        self.allow()?;
        self.core.ask(tree)
    }

    pub fn tie(
        &self,
        owner: &str,
        bond: &str,
        ends: Ends,
        fields: &[(&str, &str)],
    ) -> Result<i64, Error> {
        self.allow()?;
        self.core.tie(owner, bond, ends, fields)
    }

    pub fn set_tie(
        &self,
        owner: &str,
        bond: &str,
        key: i64,
        fields: &[(&str, &str)],
    ) -> Result<(), Error> {
        self.allow()?;
        self.core.set_tie(owner, bond, key, fields)
    }

    pub fn ties(&self, owner: &str, bond: &str, left: i64) -> Result<Vec<Tie>, Error> {
        self.allow()?;
        self.core.ties(owner, bond, left)
    }

    pub fn cut(&self, owner: &str, bond: &str, key: i64) -> Result<(), Error> {
        self.allow()?;
        self.core.cut(owner, bond, key)
    }

    fn allow(&self) -> Result<(), Error> {
        match self.who {
            Who::Sudo => Ok(()),
            _ => Err(Error::Adapt("refused".into())),
        }
    }
}

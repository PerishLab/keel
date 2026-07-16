pub mod db;
pub mod http;

use crate::store::Store;

pub use db::Sqlite;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    Missing(String),
    Adapt(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing(name) => write!(f, "missing resource: {name}"),
            Self::Adapt(note) => write!(f, "adapt: {note}"),
        }
    }
}

impl std::error::Error for Error {}

pub fn bind<S: Store>(graph: crate::graph::Graph, store: S) -> Result<crate::face::Core<S>, Error> {
    let plan = crate::plan::Plan::lift(&graph)?;
    store.wire(&plan)?;
    if let Some(token) = crate::cap::genesis(&plan, &store)? {
        eprintln!("keel: sudo token {token}");
    }
    Ok(crate::face::Core::new(plan, store))
}

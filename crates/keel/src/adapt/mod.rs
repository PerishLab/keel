pub mod db;
pub mod http;
#[cfg(feature = "pg")]
pub mod pg;

use crate::wire::Wire;

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

pub async fn bind<W: Wire>(
    graph: crate::graph::Graph,
    mut wire: W,
) -> Result<crate::face::Core<W>, Error> {
    let plan = crate::plan::Plan::lift(&graph)?;
    if plan.units().is_empty() {
        return Err(Error::Adapt("db plan is empty".into()));
    }
    for unit in plan.units().values() {
        let reign = unit.reign();
        if !reign.expires() || !reign.created() || !reign.updated() {
            return Err(Error::Adapt("reign incomplete".into()));
        }
    }
    for stmt in crate::ddl::script(&plan, wire.grain()) {
        wire.script(&stmt).await?;
    }
    if let Some(token) = crate::cap::genesis(&plan, &mut wire).await? {
        eprintln!("keel: sudo token {token}");
    }
    Ok(crate::face::Core::new(plan, wire))
}

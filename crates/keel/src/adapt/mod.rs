pub mod db;
pub mod http;
#[cfg(feature = "pg")]
pub mod pg;

use crate::wire::Wire;
use std::future::{Future, IntoFuture as Await};
use std::pin::Pin;

pub use db::Sqlite;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    Missing(String),
    Adapt(String),
    Estate(crate::estate::Fault),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing(name) => write!(f, "missing resource: {name}"),
            Self::Adapt(note) => write!(f, "adapt: {note}"),
            Self::Estate(fault) => write!(f, "{fault}"),
        }
    }
}

impl std::error::Error for Error {}

pub struct Bind<W, H = ()> {
    graph: crate::graph::Graph,
    wire: W,
    estate: crate::config::Estate,
    adopt: bool,
    hook: Option<H>,
}

pub fn bind<W: Wire>(graph: crate::graph::Graph, wire: W) -> Bind<W> {
    Bind {
        graph,
        wire,
        estate: crate::config::Estate::default(),
        adopt: false,
        hook: None,
    }
}

impl<W, H> Bind<W, H> {
    pub fn estate(mut self, estate: &crate::config::Estate) -> Self {
        self.estate = estate.clone();
        self
    }

    pub fn adopt(mut self) -> Self {
        self.adopt = true;
        self
    }

    pub fn hook<N: crate::estate::Hook>(self, hook: N) -> Bind<W, N> {
        let Bind {
            graph,
            wire,
            estate,
            adopt,
            ..
        } = self;
        Bind {
            graph,
            wire,
            estate,
            adopt,
            hook: Some(hook),
        }
    }
}

impl<W: Wire + 'static, H: crate::estate::Hook + 'static> Await for Bind<W, H> {
    type Output = Result<crate::face::Core<W>, Error>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(ready(self))
    }
}

async fn ready<W: Wire, H: crate::estate::Hook>(
    bind: Bind<W, H>,
) -> Result<crate::face::Core<W>, Error> {
    let Bind {
        graph,
        mut wire,
        estate,
        adopt,
        mut hook,
    } = bind;
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
    let manifest = crate::model::manifest::Manifest::lift(&plan);
    if let Some(token) = crate::estate::attach(
        &plan,
        &manifest,
        crate::estate::adopt::Policy {
            cleanup: &estate.generation.cleanup,
            adopt,
        },
        &mut wire,
    )
    .await?
    {
        eprintln!("keel: sudo token {token}");
    }
    crate::estate::cleanup::deliver(hook.as_mut(), &mut wire).await?;
    Ok(crate::face::Core::new(plan, wire))
}

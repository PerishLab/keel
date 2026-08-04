use keel::adapt::db::Sqlite;
use keel::atom::string;
use keel::resource;
use keel::{Core, Ends, Graph};

#[resource]
pub(super) struct Actor {
    #[field(string)]
    name: string,
    #[relation(Actor, many2many, closure)]
    members: Actor,
}

pub(super) fn graph() -> Graph {
    let mut graph = Graph::new();
    graph.plug::<Actor>();
    graph
}

pub(super) async fn boot() -> Core<Sqlite> {
    crate::support::boot(graph(), Sqlite::memory().await.expect("db"))
        .await
        .expect("bind")
}

pub(super) async fn seed<W: keel::wire::Wire>(core: &Core<W>, names: &[&str]) -> Vec<i64> {
    let mut out = Vec::new();
    for name in names {
        out.push(core.put("Actor", &[("name", name)]).await.expect("actor"));
    }
    out
}

pub(super) fn ends(left: i64, right: i64) -> Ends {
    Ends { left, right }
}

mod fault;
mod life;
mod shape;

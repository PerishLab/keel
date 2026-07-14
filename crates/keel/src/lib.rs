pub mod adapt;
pub mod atom;
pub mod bond;
pub mod ddl;
pub mod graph;
pub mod plan;
pub mod spec;

pub use atom::{string, url};
pub use graph::Graph;
pub use keel_macro::resource;
pub use plan::Core;
pub use spec::Resource;

pub fn bind(
    graph: Graph,
    http: impl adapt::Http,
    db: &impl adapt::Db,
) -> Result<Core, adapt::Error> {
    let plan = plan::Plan::lift(&graph)?;
    http.wire(&plan)?;
    db.wire(&plan)?;
    Ok(Core::lift(plan))
}

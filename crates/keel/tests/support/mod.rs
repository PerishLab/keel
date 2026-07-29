use keel::adapt::Error;
use keel::{Core, Graph, Wire, bootstrap};

pub async fn boot<W: Wire>(graph: Graph, wire: W) -> Result<Core<W>, Error> {
    let mut boot = bootstrap(graph, wire)?;
    let token = boot.mint().await?;
    boot.seal(&token).await
}

use axum::Router;
use keel::adapt::Error;
use keel::adapt::db::Sqlite;
use keel::adapt::pg::Postgres;
use keel::estate::Fault;
use keel::{Graph, Wire};
use std::io::Write as _;
use std::path::Path;
use std::sync::Arc;

pub enum Store {
    Lite(Sqlite),
    Pg(Box<Postgres>),
}

pub async fn open(at: &str) -> Result<Store, Error> {
    match at.strip_prefix("sqlite:") {
        Some(path) => Ok(Store::Lite(Sqlite::file(path).await?)),
        None => Ok(Store::Pg(Box::new(Postgres::at(at).await?))),
    }
}

pub async fn mount<W: Wire + 'static>(wire: W, prefix: &str) -> Result<Router, Error> {
    configured(wire, prefix, &keel::config::Estate::default()).await
}

pub async fn configured<W: Wire + 'static>(
    mut wire: W,
    prefix: &str,
    estate: &keel::config::Estate,
) -> Result<Router, Error> {
    let graph = keel::estate::read(&mut wire).await?;
    let core = keel::bind(graph, wire).estate(estate).await?;
    Ok(keel::app(Arc::new(core), prefix))
}

pub async fn seal<W: Wire + 'static>(wire: W, graph: Graph, custody: &Path) -> Result<(), Error> {
    let mut boot = keel::bootstrap(graph, wire)?;
    let token = boot.mint().await?;
    keep(custody, &token)?;
    if let Err(error) = boot.seal(&token).await {
        let _ = std::fs::remove_file(custody);
        return Err(error);
    }
    Ok(())
}

pub async fn status<W: Wire>(mut wire: W) -> Result<&'static str, Error> {
    Ok(match keel::estate::read(&mut wire).await {
        Ok(_) => "sealed",
        Err(Error::Estate(Fault::Vacant)) => "vacant",
        Err(Error::Estate(_)) => "unknown",
        Err(error) => return Err(error),
    })
}

pub fn model(path: &Path) -> Result<Graph, Error> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| Error::Adapt(format!("model {}: {e}", path.display())))?;
    Graph::read(text.trim_end())
}

fn keep(custody: &Path, token: &str) -> Result<(), Error> {
    let fault = |e: std::io::Error| Error::Adapt(format!("custody {}: {e}", custody.display()));
    let mut file = create(custody).map_err(fault)?;
    if let Err(e) = file
        .write_all(token.as_bytes())
        .and_then(|()| file.sync_all())
    {
        let _ = std::fs::remove_file(custody);
        return Err(fault(e));
    }
    Ok(())
}

fn create(path: &Path) -> std::io::Result<std::fs::File> {
    let mut open = std::fs::OpenOptions::new();
    open.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut open, 0o600);
    open.open(path)
}

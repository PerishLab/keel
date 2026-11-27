mod bucket;
mod route;

use super::{Shed, Vault};
use axum::http::StatusCode;
use bucket::Bucket;
use keel::adapt::db::Sqlite;
use keel::{Core, Graph, bind, bootstrap};
use std::path::Path;
use std::sync::Arc;

#[keel::resource]
struct Actor {
    #[field(string)]
    name: keel::atom::string,
}

crate::blob!(Actor);

type Estate = Arc<Core<Sqlite>>;

async fn open(path: &Path) -> Estate {
    let mut graph = Graph::new();
    graph.plug::<Actor>();
    stock(&mut graph);
    let fresh = !path.exists();
    let wire = Sqlite::file(path).await.unwrap();
    let core = if fresh {
        let mut start = bootstrap(graph, wire).unwrap();
        let token = start.mint().await.unwrap();
        start.seal(&token).await.unwrap()
    } else {
        bind(graph, wire).await.unwrap()
    };
    core.identify("Actor").unwrap().share()
}

async fn operator(core: &Core<Sqlite>, name: &str) -> i64 {
    let id = core.put("Actor", &[("name", name)]).await.unwrap();
    core.grant(keel::Grant::new(&id.to_string(), "put", "Asset", "all"))
        .await
        .unwrap();
    id
}

#[tokio::test]
async fn reset() {
    let bucket = Bucket::open().await;
    let home = tempfile::tempdir().unwrap();
    let old = home.path().join("old");
    let core = open(&old).await;
    let owner = operator(&core, "owner").await;
    let vault = Vault::open(core.clone(), bucket.shed()).await.unwrap();
    let saved = route::stow(&vault, owner, b"old bytes").await;
    assert_eq!(saved.upload(b"old bytes").await, StatusCode::OK);
    drop(vault);
    drop(core);

    let core = open(&home.path().join("reset")).await;
    let other = operator(&core, "other").await;
    let vault = Vault::open(core, bucket.shed()).await.unwrap();
    let fresh = route::stow(&vault, other, b"new bytes").await;
    assert_eq!(saved.id, fresh.id);
    assert_ne!(saved.url.path(), fresh.url.path());
    let url = route::fetch(&vault, other, fresh.id).await.unwrap();
    assert_eq!(bucket.read(url.clone()).await.0, StatusCode::NOT_FOUND);
    assert_eq!(fresh.upload(b"new bytes").await, StatusCode::OK);
    assert_eq!(bucket.read(url).await.1, b"new bytes");
    vault.sweep(None).await.unwrap();

    let core = open(&old).await;
    let vault = Vault::open(core, bucket.shed()).await.unwrap();
    let url = route::fetch(&vault, owner, saved.id).await.unwrap();
    assert_eq!(bucket.read(url).await.1, b"old bytes");
}

#[tokio::test]
async fn restore() {
    let bucket = Bucket::open().await;
    let home = tempfile::tempdir().unwrap();
    let original = home.path().join("estate");
    let snapshot = home.path().join("snapshot");
    let core = open(&original).await;
    let owner = operator(&core, "owner").await;
    drop(Vault::open(core.clone(), bucket.shed()).await.unwrap());
    drop(core);
    std::fs::copy(&original, &snapshot).unwrap();

    let core = open(&original).await;
    let vault = Vault::open(core, bucket.shed()).await.unwrap();
    let first = route::stow(&vault, owner, b"old bytes").await;
    assert_eq!(first.upload(b"old bytes").await, StatusCode::OK);
    drop(vault);

    let core = open(&snapshot).await;
    let vault = Vault::open(core, bucket.shed()).await.unwrap();
    let second = route::stow(&vault, owner, b"new bytes").await;
    assert_eq!(first.id, second.id);
    assert_ne!(first.url.path(), second.url.path());
    assert!(
        first
            .url
            .path()
            .starts_with(&format!("/shed/{}", vault.prefix))
    );
    let fork = route::fetch(&vault, owner, second.id).await.unwrap();
    assert_eq!(bucket.read(fork).await.0, StatusCode::NOT_FOUND);
    vault.sweep(None).await.unwrap();
    assert!(bucket.has(first.url.path()));
}

#[tokio::test]
async fn integrity() {
    let bucket = Bucket::open().await;
    let home = tempfile::tempdir().unwrap();
    let core = open(&home.path().join("estate")).await;
    let owner = operator(&core, "owner").await;
    let other = operator(&core, "other").await;
    let vault = Vault::open(core.clone(), bucket.shed()).await.unwrap();
    let bytes = b"sample";
    let saved = route::stow(&vault, owner, bytes).await;
    assert_eq!(saved.upload(b"change").await, StatusCode::BAD_REQUEST);
    assert_eq!(saved.resize(b"short").await, StatusCode::FORBIDDEN);
    assert!(!bucket.has(saved.url.path()));
    assert_eq!(saved.upload(bytes).await, StatusCode::OK);
    assert!(route::fetch(&vault, other, saved.id).await.is_none());
    assert!(route::fetch(&vault, owner, saved.id).await.is_some());
    core.set("Asset", saved.id, &[("size", "4")]).await.unwrap();
    assert!(route::fetch(&vault, owner, saved.id).await.is_none());
}

#[tokio::test]
async fn collect() {
    let bucket = Bucket::open().await;
    let home = tempfile::tempdir().unwrap();
    let core = open(&home.path().join("estate")).await;
    let owner = operator(&core, "owner").await;
    let vault = Vault::open(core.clone(), bucket.shed()).await.unwrap();
    let saved = route::stow(&vault, owner, b"gone").await;
    let live = route::stow(&vault, owner, b"keep").await;
    assert_eq!(saved.upload(b"gone").await, StatusCode::OK);
    assert_eq!(live.upload(b"keep").await, StatusCode::OK);
    vault.sweep(None).await.unwrap();
    assert!(bucket.has(saved.url.path()));
    core.of(owner).end("Asset", saved.id).await.unwrap();
    assert!(route::fetch(&vault, owner, saved.id).await.is_none());
    drop(vault);

    let vault = Vault::open(core, bucket.shed()).await.unwrap();
    bucket.refuse(true);
    vault.sweep(None).await.unwrap();
    assert!(bucket.has(saved.url.path()));
    bucket.refuse(false);
    vault.sweep(None).await.unwrap();
    assert!(!bucket.has(saved.url.path()));
    assert!(bucket.has(live.url.path()));
    assert_eq!(saved.upload(b"gone").await, StatusCode::OK);
    vault.sweep(None).await.unwrap();
    assert!(!bucket.has(saved.url.path()));
}

#[tokio::test]
async fn reach() {
    let bucket = Bucket::open().await;
    let home = tempfile::tempdir().unwrap();
    let core = open(&home.path().join("estate")).await;
    let shed = Shed {
        name: "missing",
        ..bucket.shed()
    };
    assert!(Vault::open(core.clone(), shed).await.is_err());
    assert!(core.live("Silo").await.unwrap().is_empty());
    let vault = Vault::open(core.clone(), bucket.shed()).await.unwrap();
    assert!(vault.prefix.starts_with("asset/"));
    let foreign = Bucket::open().await;
    assert!(Vault::open(core, foreign.shed()).await.is_err());
}

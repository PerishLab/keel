use keel::adapt::Error;
use keel::adapt::db::Sqlite;
use keel::bind;
use keel::estate::Fault;
use keel::wire::{Val, Wire};

mod adopt;
mod checks;
mod cleanup;
mod cycles;
mod evolve;
mod fail;
mod hook;
mod optional;
mod world;
use world::*;

#[tokio::test]
async fn fresh() {
    let core = bind(graph::<Alpha>(), Sqlite::memory().await.expect("db"))
        .await
        .expect("bind");
    assert!(core.has("@estate").await.expect("estate"));
    assert!(core.has("@generation").await.expect("generation"));
    assert!(core.has("@clock").await.expect("clock"));
    assert!(core.has("@derivative").await.expect("derivative"));
}

#[tokio::test]
async fn exact() {
    let path = spot("exact");
    let core = bind(graph::<Alpha>(), Sqlite::file(&path).await.expect("first"))
        .await
        .expect("bind");
    core.put("Alpha", &[("zeta", "held"), ("alpha", "7")])
        .await
        .expect("put");
    drop(core);

    let core = bind(graph::<Swap>(), Sqlite::file(&path).await.expect("second"))
        .await
        .expect("exact");
    assert_eq!(core.live("Alpha").await.expect("live").len(), 1);
    drop(core);
    clean(&path);
}

#[tokio::test]
async fn unsealed() {
    let path = spot("unsealed");
    let mut wire = Sqlite::file(&path).await.expect("db");
    wire.script("CREATE TABLE stray (id INTEGER PRIMARY KEY NOT NULL)")
        .await
        .expect("stray");
    drop(wire);

    match bind(graph::<Alpha>(), Sqlite::file(&path).await.expect("open")).await {
        Err(Error::Estate(Fault::Unsealed)) => {}
        Err(err) => panic!("unexpected {err}"),
        Ok(_) => panic!("expected unsealed"),
    }
    let mut wire = Sqlite::file(&path).await.expect("inspect");
    let rows = wire
        .rows("SELECT name FROM sqlite_master WHERE name = '@estate'", &[])
        .await
        .expect("catalog");
    assert!(rows.is_empty());
    drop(wire);
    clean(&path);
}

#[tokio::test]
async fn drift() {
    let path = spot("drift");
    let core = bind(graph::<Alpha>(), Sqlite::file(&path).await.expect("first"))
        .await
        .expect("bind");
    drop(core);
    let mut wire = Sqlite::file(&path).await.expect("mutate");
    wire.script("CREATE TABLE stray (id INTEGER PRIMARY KEY NOT NULL)")
        .await
        .expect("stray");
    drop(wire);

    match bind(graph::<Alpha>(), Sqlite::file(&path).await.expect("second")).await {
        Err(Error::Estate(Fault::Drift { expected, found })) => {
            assert_ne!(expected, found);
        }
        Err(err) => panic!("unexpected {err}"),
        Ok(_) => panic!("expected drift"),
    }
    clean(&path);
}

#[tokio::test]
async fn unknown() {
    let path = spot("unknown");
    let core = bind(graph::<Alpha>(), Sqlite::file(&path).await.expect("first"))
        .await
        .expect("bind");
    drop(core);
    let mut wire = Sqlite::file(&path).await.expect("mutate");
    wire.run(
        "UPDATE \"@generation\" SET manifest = ?1 WHERE id = ?2",
        &[Val::Text(r#"{"version":4,"units":[]}"#.into()), Val::Int(1)],
    )
    .await
    .expect("mutate");
    drop(wire);

    match bind(graph::<Alpha>(), Sqlite::file(&path).await.expect("second")).await {
        Err(Error::Estate(Fault::Unknown(note))) => {
            assert!(note.contains("digest"));
        }
        Err(err) => panic!("unexpected {err}"),
        Ok(_) => panic!("expected unknown"),
    }
    clean(&path);
}

#[tokio::test]
async fn preflight() {
    let path = spot("preflight");
    let mut wire = Sqlite::file(&path).await.expect("db");
    wire.script("CREATE TABLE stray (id INTEGER PRIMARY KEY NOT NULL)")
        .await
        .expect("stray");
    drop(wire);

    match bind(graph::<Dup>(), Sqlite::file(&path).await.expect("open")).await {
        Err(Error::Adapt(note)) => assert!(note.contains("duplicate field")),
        Err(err) => panic!("unexpected {err}"),
        Ok(_) => panic!("expected preflight"),
    }
    clean(&path);
}

#[tokio::test]
async fn monotonic() {
    let path = spot("monotonic");
    let core = bind(graph::<Alpha>(), Sqlite::file(&path).await.expect("first"))
        .await
        .expect("bind");
    assert_eq!(
        core.put("Alpha", &[("zeta", "one"), ("alpha", "1")])
            .await
            .expect("one"),
        1
    );
    assert_eq!(
        core.put("Alpha", &[("zeta", "two"), ("alpha", "2")])
            .await
            .expect("two"),
        2
    );
    drop(core);

    let mut wire = Sqlite::file(&path).await.expect("clean");
    wire.run("DELETE FROM alpha", &[]).await.expect("delete");
    drop(wire);
    let core = bind(graph::<Alpha>(), Sqlite::file(&path).await.expect("open"))
        .await
        .expect("reopen");
    assert_eq!(
        core.put("Alpha", &[("zeta", "three"), ("alpha", "3")])
            .await
            .expect("three"),
        3
    );
    drop(core);
    clean(&path);
}

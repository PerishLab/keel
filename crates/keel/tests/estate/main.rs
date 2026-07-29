use keel::adapt::Error;
use keel::adapt::db::Sqlite;
use keel::estate::Fault;
use keel::wire::{Val, Wire};
use keel::{bind, bootstrap};

#[path = "../support/mod.rs"]
mod support;

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
    let wire = Sqlite::memory().await.expect("db");
    let mut boot = bootstrap(graph::<Alpha>(), wire).expect("bootstrap");
    assert_eq!(boot.status().await.expect("status"), keel::Status::Vacant);
    let token = boot.mint().await.expect("mint");
    assert_eq!(token.len(), 64);
    let core = boot.seal(&token).await.expect("seal");
    assert!(core.seal(&token).await.expect("sudo"));
    assert!(core.has("@estate").await.expect("estate"));
    assert!(core.has("@generation").await.expect("generation"));
    assert!(core.has("@clock").await.expect("clock"));
    assert!(core.has("@derivative").await.expect("derivative"));
}

#[tokio::test]
async fn vacant() {
    let path = spot("vacant");
    match bind(graph::<Alpha>(), Sqlite::file(&path).await.expect("bind")).await {
        Err(Error::Estate(Fault::Vacant)) => {}
        Err(err) => panic!("unexpected {err}"),
        Ok(_) => panic!("expected vacant"),
    }
    assert!(fail::tables(&path).await.is_empty());
    clean(&path);
}

#[tokio::test]
async fn token() {
    let path = spot("token");
    let boot =
        bootstrap(graph::<Alpha>(), Sqlite::file(&path).await.expect("boot")).expect("bootstrap");
    match boot.seal("bad").await {
        Err(Error::Estate(Fault::Token)) => {}
        Err(err) => panic!("unexpected {err}"),
        Ok(_) => panic!("expected token refusal"),
    }
    assert!(fail::tables(&path).await.is_empty());
    clean(&path);
}

#[tokio::test]
async fn replay() {
    let path = spot("replay");
    let sudo = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let boot =
        bootstrap(graph::<Alpha>(), Sqlite::file(&path).await.expect("first")).expect("bootstrap");
    let core = boot.seal(sudo).await.expect("seal");
    drop(core);

    let mut boot =
        bootstrap(graph::<Alpha>(), Sqlite::file(&path).await.expect("replay")).expect("bootstrap");
    assert_eq!(boot.status().await.expect("status"), keel::Status::Occupied);
    match boot.mint().await {
        Err(Error::Estate(Fault::Occupied)) => {}
        Err(err) => panic!("unexpected {err}"),
        Ok(_) => panic!("expected occupied"),
    }
    let core = boot.seal(sudo).await.expect("replay");
    drop(core);

    let boot =
        bootstrap(graph::<Grow>(), Sqlite::file(&path).await.expect("changed")).expect("bootstrap");
    match boot.seal(sudo).await {
        Err(Error::Estate(Fault::Occupied)) => {}
        Err(err) => panic!("unexpected {err}"),
        Ok(_) => panic!("expected occupied"),
    }

    let other = "1123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let boot = bootstrap(
        graph::<Alpha>(),
        Sqlite::file(&path).await.expect("conflict"),
    )
    .expect("bootstrap");
    match boot.seal(other).await {
        Err(Error::Estate(Fault::Occupied)) => {}
        Err(err) => panic!("unexpected {err}"),
        Ok(_) => panic!("expected occupied"),
    }
    let core = bind(graph::<Alpha>(), Sqlite::file(&path).await.expect("bind"))
        .await
        .expect("bind");
    assert!(core.seal(sudo).await.expect("sudo"));
    drop(core);
    clean(&path);
}

#[tokio::test]
async fn race() {
    let path = spot("race");
    let left = path.clone();
    let right = path.clone();
    let sudo = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let first = tokio::spawn(async move {
        let wire = Sqlite::file(left).await.expect("left");
        bootstrap(graph::<Alpha>(), wire)
            .expect("bootstrap")
            .seal(sudo)
            .await
    });
    let second = tokio::spawn(async move {
        let wire = Sqlite::file(right).await.expect("right");
        bootstrap(graph::<Alpha>(), wire)
            .expect("bootstrap")
            .seal(sudo)
            .await
    });
    let first = first.await.expect("first");
    let second = second.await.expect("second");
    assert!(first.is_ok() || second.is_ok());
    drop(first);
    drop(second);

    let wire = Sqlite::file(&path).await.expect("retry");
    let core = bootstrap(graph::<Alpha>(), wire)
        .expect("bootstrap")
        .seal(sudo)
        .await
        .expect("replay");
    assert!(core.seal(sudo).await.expect("sudo"));
    drop(core);
    let mut wire = Sqlite::file(&path).await.expect("inspect");
    let rows = wire
        .rows("SELECT COUNT(*) FROM \"@generation\"", &[])
        .await
        .expect("generation");
    assert_eq!(rows[0][0], Val::Int(1));
    drop(wire);
    clean(&path);
}

#[tokio::test]
async fn exact() {
    let path = spot("exact");
    let core = crate::support::boot(graph::<Alpha>(), Sqlite::file(&path).await.expect("first"))
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
    let core = crate::support::boot(graph::<Alpha>(), Sqlite::file(&path).await.expect("first"))
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
    let core = crate::support::boot(graph::<Alpha>(), Sqlite::file(&path).await.expect("first"))
        .await
        .expect("bind");
    drop(core);
    let mut wire = Sqlite::file(&path).await.expect("mutate");
    wire.run(
        "UPDATE \"@unit\" SET name = ?1",
        &[Val::Text("Other".into())],
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
    let core = crate::support::boot(graph::<Alpha>(), Sqlite::file(&path).await.expect("first"))
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

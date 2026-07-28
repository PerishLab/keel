use super::world::*;
use keel::adapt::Error;
use keel::adapt::db::Sqlite;
use keel::estate::{Check, Fault};
use keel::wire::{Val, Wire};
use keel::{Cell, Ends, bind};

#[tokio::test]
async fn additive() {
    let path = spot("additive");
    let core = bind(graph::<Alpha>(), Sqlite::file(&path).await.expect("first"))
        .await
        .expect("bind");
    drop(core);

    let core = bind(graph::<Grow>(), Sqlite::file(&path).await.expect("second"))
        .await
        .expect("evolve");
    assert!(core.has("@g1:alpha").await.expect("cleanup"));
    let key = core
        .put(
            "Alpha",
            &[("alpha", "1"), ("zeta", "after"), ("beta", "true")],
        )
        .await
        .expect("put");
    assert_eq!(key, 1);
    drop(core);
    let core = bind(graph::<Grow>(), Sqlite::file(&path).await.expect("third"))
        .await
        .expect("exact");
    assert_eq!(core.live("Alpha").await.expect("live").len(), 1);
    drop(core);
    clean(&path);
}

#[tokio::test]
async fn blocked() {
    let path = spot("blocked");
    let core = bind(graph::<Alpha>(), Sqlite::file(&path).await.expect("first"))
        .await
        .expect("bind");
    core.put("Alpha", &[("alpha", "1"), ("zeta", "held")])
        .await
        .expect("put");
    drop(core);

    match bind(graph::<Grow>(), Sqlite::file(&path).await.expect("second")).await {
        Err(Error::Estate(Fault::Blocked { path, check })) => {
            assert_eq!(path, "alpha.beta");
            assert_eq!(check, Check::Empty);
        }
        Err(err) => panic!("unexpected {err}"),
        Ok(_) => panic!("expected block"),
    }
    clean(&path);
}

#[tokio::test]
async fn contract() {
    let path = spot("contract");
    let core = bind(graph::<Alpha>(), Sqlite::file(&path).await.expect("first"))
        .await
        .expect("bind");
    let key = core
        .put("Alpha", &[("alpha", "7"), ("zeta", "held")])
        .await
        .expect("put");
    drop(core);

    let core = bind(
        graph::<Shrink>(),
        Sqlite::file(&path).await.expect("second"),
    )
    .await
    .expect("evolve");
    let row = core
        .live("Alpha")
        .await
        .expect("live")
        .into_iter()
        .next()
        .expect("row");
    assert_eq!(row.key(), key);
    assert_eq!(row.cells().get("zeta"), Some(&Cell::Text("held".into())));
    assert!(!row.cells().contains_key("alpha"));
    assert_eq!(
        core.put("Alpha", &[("zeta", "next")]).await.expect("next"),
        2
    );
    drop(core);
    clean(&path);
}

#[tokio::test]
async fn cast() {
    let path = spot("cast");
    let core = bind(graph::<Words>(), Sqlite::file(&path).await.expect("first"))
        .await
        .expect("bind");
    let key = core.put("Cast", &[("value", "7")]).await.expect("put");
    drop(core);

    let core = bind(
        graph::<Integer>(),
        Sqlite::file(&path).await.expect("second"),
    )
    .await
    .expect("evolve");
    let row = core.live("Cast").await.expect("live").remove(0);
    assert_eq!(row.key(), key);
    assert_eq!(row.cells().get("value"), Some(&Cell::Int(7)));
    drop(core);
    clean(&path);
}

#[tokio::test]
async fn denied() {
    let path = spot("denied");
    let core = bind(graph::<Linker>(), Sqlite::file(&path).await.expect("first"))
        .await
        .expect("bind");
    drop(core);

    match bind(
        graph::<Integer>(),
        Sqlite::file(&path).await.expect("second"),
    )
    .await
    {
        Err(Error::Estate(Fault::Denied { path, note })) => {
            assert_eq!(path, "cast.value");
            assert!(note.contains("cast"));
        }
        Err(err) => panic!("unexpected {err}"),
        Ok(_) => panic!("expected denial"),
    }
    let mut wire = Sqlite::file(&path).await.expect("inspect");
    let rows = wire
        .rows(
            "SELECT value FROM \"@clock\" WHERE name = ?1",
            &[Val::Text("generation".into())],
        )
        .await
        .expect("clock");
    assert_eq!(rows[0][0].int(), 1);
    drop(wire);
    clean(&path);
}

#[tokio::test]
async fn bonded() {
    let path = spot("bonded");
    let core = bind(
        pair::<Club, Member>(),
        Sqlite::file(&path).await.expect("first"),
    )
    .await
    .expect("bind");
    let club = core.put("Club", &[("name", "lab")]).await.expect("club");
    let member = core
        .put("Member", &[("name", "ada"), ("age", "42")])
        .await
        .expect("member");
    let tie = core
        .tie(
            "Member",
            "clubs",
            Ends {
                left: member,
                right: club,
            },
            &[("role", "owner")],
        )
        .await
        .expect("tie");
    drop(core);

    let core = bind(
        pair::<Club, Lean>(),
        Sqlite::file(&path).await.expect("second"),
    )
    .await
    .expect("evolve");
    let row = core.live("Member").await.expect("live").remove(0);
    assert_eq!(row.key(), member);
    assert!(!row.cells().contains_key("age"));
    let held = core.ties("Member", "clubs", member).await.expect("ties");
    assert_eq!(held[0].key(), tie);
    assert_eq!(held[0].right(), club);
    assert_eq!(
        held[0].cells().get("role"),
        Some(&Cell::Text("owner".into()))
    );
    let other = core.put("Club", &[("name", "other")]).await.expect("other");
    let next = core
        .tie(
            "Member",
            "clubs",
            Ends {
                left: member,
                right: other,
            },
            &[("role", "guest")],
        )
        .await
        .expect("next");
    assert_eq!(next, 2);
    drop(core);
    clean(&path);
}

#[tokio::test]
async fn required() {
    let path = spot("required");
    let core = bind(
        pair::<Org, Optional>(),
        Sqlite::file(&path).await.expect("first"),
    )
    .await
    .expect("bind");
    let org = core.put("Org", &[("name", "lab")]).await.expect("org");
    let doc = core
        .put("Doc", &[("title", "keel"), ("owner", &org.to_string())])
        .await
        .expect("doc");
    drop(core);

    let core = bind(
        pair::<Org, Required>(),
        Sqlite::file(&path).await.expect("second"),
    )
    .await
    .expect("evolve");
    let row = core.live("Doc").await.expect("live").remove(0);
    assert_eq!(row.key(), doc);
    assert_eq!(row.cells().get("owner"), Some(&Cell::Int(org)));
    drop(core);
    clean(&path);
}

#[tokio::test]
async fn absent() {
    let path = spot("absent");
    let core = bind(
        pair::<Org, Optional>(),
        Sqlite::file(&path).await.expect("first"),
    )
    .await
    .expect("bind");
    core.put("Doc", &[("title", "loose")]).await.expect("doc");
    drop(core);

    match bind(
        pair::<Org, Required>(),
        Sqlite::file(&path).await.expect("second"),
    )
    .await
    {
        Err(Error::Estate(Fault::Blocked { path, check })) => {
            assert_eq!(path, "doc.owner");
            assert_eq!(check, Check::Presence);
        }
        Err(err) => panic!("unexpected {err}"),
        Ok(_) => panic!("expected presence block"),
    }
    clean(&path);
}

use super::world::*;
use keel::adapt::Error;
use keel::adapt::db::Sqlite;
use keel::bind;
use keel::estate::{Check, Fault};

#[tokio::test]
async fn cycle() {
    let path = spot("cycle");
    let core = bind(graph::<Alpha>(), Sqlite::file(&path).await.expect("first"))
        .await
        .expect("bind");
    let alpha = core
        .put("Alpha", &[("alpha", "1"), ("zeta", "held")])
        .await
        .expect("alpha");
    drop(core);

    let core = bind(
        pair::<Alpha, Beta>(),
        Sqlite::file(&path).await.expect("second"),
    )
    .await
    .expect("add");
    assert_eq!(core.live("Alpha").await.expect("live")[0].key(), alpha);
    core.put("Beta", &[("name", "kept")]).await.expect("beta");
    core.end("Alpha", alpha).await.expect("end");
    drop(core);

    let core = bind(graph::<Beta>(), Sqlite::file(&path).await.expect("third"))
        .await
        .expect("drop");
    assert_eq!(core.live("Beta").await.expect("beta").len(), 1);
    assert!(core.live("Alpha").await.is_err());
    assert!(core.has("@g1:alpha").await.expect("first cleanup"));
    assert!(core.has("@g2:alpha").await.expect("second cleanup"));
    assert!(core.flow(0).await.is_err());
    drop(core);
    clean(&path);
}

#[tokio::test]
async fn authority() {
    let path = spot("authority");
    let core = bind(
        pair::<Alpha, Beta>(),
        Sqlite::file(&path).await.expect("first"),
    )
    .await
    .expect("bind");
    let grant = core
        .put(
            "@grant",
            &[
                ("who", "all"),
                ("verb", "see"),
                ("unit", "Alpha"),
                ("scope", "all"),
            ],
        )
        .await
        .expect("grant");
    drop(core);

    match bind(graph::<Beta>(), Sqlite::file(&path).await.expect("blocked")).await {
        Err(Error::Estate(Fault::Blocked { path, check })) => {
            assert_eq!(path, "@grant");
            assert_eq!(check, Check::Authority);
        }
        Err(err) => panic!("unexpected {err}"),
        Ok(_) => panic!("expected authority block"),
    }

    let core = bind(
        pair::<Alpha, Beta>(),
        Sqlite::file(&path).await.expect("repair"),
    )
    .await
    .expect("reopen");
    core.end("@grant", grant).await.expect("end grant");
    drop(core);
    let core = bind(graph::<Beta>(), Sqlite::file(&path).await.expect("evolve"))
        .await
        .expect("evolve");
    assert!(core.live("Alpha").await.is_err());
    drop(core);
    clean(&path);
}

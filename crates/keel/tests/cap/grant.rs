use crate::world::*;
use keel::adapt::db::Sqlite;
use keel::{Graph, Who, bind};

#[tokio::test]
async fn grant() {
    let mut graph = Graph::new();
    graph.plug::<Actor>();
    let core = bind(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind");
    let sudo = core.sudo();
    assert_eq!(sudo.who(), Who::Sudo);

    let row = sudo
        .put(
            "@grant",
            &[
                ("who", "anon"),
                ("verb", "see"),
                ("unit", "Actor"),
                ("scope", "all"),
            ],
        )
        .await
        .expect("seed");
    let rows = sudo.live("@grant").await.expect("live");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].key(), row);

    let pack = sudo.query("from @grant count").await.expect("audit");
    assert_eq!(pack.count(), Some(1));

    sudo.end("@grant", row).await.expect("revoke");
    assert_eq!(sudo.live("@grant").await.expect("live").len(), 0);

    assert!(sudo.set("@grant", row, &[("verb", "put")]).await.is_err());
}

#[tokio::test]
async fn vet() {
    let mut graph = Graph::new();
    graph.plug::<Actor>();
    let core = bind(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind");
    let sudo = core.sudo();
    let seed = async |who: &str, verb: &str, unit: &str, scope: &str| {
        sudo.put(
            "@grant",
            &[
                ("who", who),
                ("verb", verb),
                ("unit", unit),
                ("scope", scope),
            ],
        )
        .await
    };
    assert!(seed("anon", "grow", "Actor", "all").await.is_err());
    assert!(seed("someone", "see", "Actor", "all").await.is_err());
    assert!(seed("anon", "see", "Ghost", "all").await.is_err());
    assert!(seed("anon", "see", "Actor", "sometimes").await.is_err());
    assert!(
        seed("anon", "see", "*", r#"pred login = "a""#)
            .await
            .is_err()
    );
    assert!(seed("anon", "see", "Actor", "row x").await.is_err());
    seed("all", "*", "*", "all").await.expect("wildcard");
    seed("7", "put", "Actor", "row 3").await.expect("row scope");
    seed("7", "set", "Actor", r#"pred login = "@me""#)
        .await
        .expect("pred scope");
}

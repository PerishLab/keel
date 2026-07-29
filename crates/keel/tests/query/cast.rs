use crate::world::*;
use keel::adapt::db::Sqlite;
use keel::life::Cell;
use keel::{Graph, Row};

#[tokio::test]
async fn cast() {
    let mut graph = Graph::new();
    graph.plug::<Score>();
    let core = crate::support::boot(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind");
    let low = core
        .put(
            "Score",
            &[("name", "low"), ("points", "2"), ("passed", "false")],
        )
        .await
        .expect("put low");
    let mid = core
        .put(
            "Score",
            &[("name", "mid"), ("points", "10"), ("passed", "true")],
        )
        .await
        .expect("put mid");
    let top = core
        .put(
            "Score",
            &[("name", "top"), ("points", "42"), ("passed", "true")],
        )
        .await
        .expect("put top");

    let pack = core
        .query(r#"from Score where points > "5""#)
        .await
        .expect("gt");
    assert_eq!(pack.rows().len(), 2);

    let pack = core
        .query("from Score order by points desc")
        .await
        .expect("ord");
    assert_eq!(
        pack.rows().iter().map(Row::key).collect::<Vec<_>>(),
        vec![top, mid, low]
    );

    let pack = core
        .query(r#"from Score where passed = "true""#)
        .await
        .expect("flag");
    assert_eq!(pack.rows().len(), 2);
    assert_eq!(pack.rows()[0].cells().get("points"), Some(&Cell::Int(10)));
    assert_eq!(
        pack.rows()[0].cells().get("passed"),
        Some(&Cell::Bool(true))
    );

    core.set("Score", low, &[("points", "77")])
        .await
        .expect("set");
    let pack = core
        .query(r#"from Score where points >= "77""#)
        .await
        .expect("ge");
    assert_eq!(pack.rows().len(), 1);

    assert!(
        core.put(
            "Score",
            &[("name", "x"), ("points", "x"), ("passed", "true")]
        )
        .await
        .is_err()
    );
    assert!(
        core.put(
            "Score",
            &[("name", "x"), ("points", "1"), ("passed", "yep")]
        )
        .await
        .is_err()
    );
    assert!(core.set("Score", low, &[("passed", "1")]).await.is_err());
    assert!(
        core.query(r#"from Score where points = "x""#)
            .await
            .is_err()
    );
    assert!(
        core.query(r#"from Score where passed = "x""#)
            .await
            .is_err()
    );
}

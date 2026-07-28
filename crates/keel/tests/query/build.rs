use crate::world::*;
use keel::adapt::db::Sqlite;
use keel::{Graph, Op, Rank, bind, form};

#[tokio::test]
async fn build() {
    let mut graph = Graph::new();
    graph.plug::<Score>();
    let core = bind(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind");
    for (name, points, passed) in [
        ("algo", "90", "true"),
        ("net", "40", "false"),
        ("db", "75", "true"),
    ] {
        core.put(
            "Score",
            &[("name", name), ("points", points), ("passed", passed)],
        )
        .await
        .expect("put");
    }

    let quoted = core
        .ask(&form("Score").when("name", Op::Eq, r#"a"b\c"#))
        .await
        .expect("quoted");
    assert_eq!(quoted.rows().len(), 0);

    let passed = core
        .ask(
            &form("Score")
                .when("passed", Op::Eq, "true")
                .order("points", Rank::Desc),
        )
        .await
        .expect("passed");
    let names: Vec<_> = passed
        .rows()
        .iter()
        .filter_map(|row| row.text("name"))
        .collect();
    assert_eq!(names, ["algo", "db"]);

    let stable = core
        .ask(
            &form("Score")
                .order("passed", Rank::Asc)
                .order("points", Rank::Desc),
        )
        .await
        .expect("stable");
    let names: Vec<_> = stable
        .rows()
        .iter()
        .filter_map(|row| row.text("name"))
        .collect();
    assert_eq!(names, ["net", "algo", "db"]);

    let some = core
        .ask(&form("Score").any("name", &["net", "db"]))
        .await
        .expect("any");
    assert_eq!(some.rows().len(), 2);

    let tally = core
        .ask(&form("Score").when("points", Op::Ge, "50").count())
        .await
        .expect("tally");
    assert_eq!(tally.count(), Some(2));

    let top = core
        .one(&form("Score").order("points", Rank::Desc).top(1))
        .await
        .expect("one")
        .expect("row");
    assert_eq!(top.text("name"), Some("algo"));
    assert_eq!(top.int("points"), Some(90));
    assert_eq!(top.flag("passed"), Some(true));
    assert_eq!(top.text("points"), None);
    assert!(top.cell("name").is_some());

    let none = core
        .one(&form("Score").when("name", Op::Eq, "ghost"))
        .await
        .expect("none");
    assert!(none.is_none());

    let bad = core.ask(&form("Score").when("ghost", Op::Eq, "x")).await;
    assert!(bad.is_err());
}

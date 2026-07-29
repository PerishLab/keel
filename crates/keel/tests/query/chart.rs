use crate::world::*;
use keel::Graph;
use keel::adapt::db::Sqlite;
use keel::query;

fn shape(text: &str) -> String {
    query::shape(&query::parse(text).expect("parse"))
}

#[test]
fn keys() {
    let one = r#"from Student where nickname = "ada" limit 3"#;
    let two = r#"from Student where nickname = "bob" limit 9 after "4""#;
    assert_eq!(shape(one), shape(two));
    assert_ne!(
        query::digest(&query::parse(one).expect("one")),
        query::digest(&query::parse(two).expect("two"))
    );

    let base = r#"from Student where nickname = "ada""#;
    for apart in [
        r#"from Student where avatar = "ada""#,
        r#"from Student where nickname != "ada""#,
        r#"from Student where nickname = "ada" link classes"#,
        r#"from Student where nickname = "ada" order by avatar"#,
        r#"from Class where nickname = "ada""#,
    ] {
        assert_ne!(shape(base), shape(apart), "{apart}");
    }
    assert_ne!(
        shape(r#"from Student where classes some (id = "1")"#),
        shape(r#"from Student where classes some (title = "1")"#)
    );
}

#[tokio::test]
async fn cells() {
    let mut graph = Graph::new();
    graph.plug::<Score>();
    let core = crate::support::boot(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind");
    core.put(
        "Score",
        &[("name", "ada"), ("points", "40"), ("passed", "true")],
    )
    .await
    .expect("ada");

    let warm = core
        .query(r#"from Score where points > "10""#)
        .await
        .expect("warm");
    assert_eq!(warm.rows().len(), 1);

    assert!(
        core.query(r#"from Score where points > "abc""#)
            .await
            .is_err()
    );
    assert!(
        core.query(r#"from Score where passed = "maybe""#)
            .await
            .is_err()
    );

    let again = core
        .query(r#"from Score where points > "10" limit 1"#)
        .await
        .expect("again");
    assert_eq!(again.rows().len(), 1);
}

#[tokio::test]
async fn sorts() {
    let mut graph = Graph::new();
    graph.plug::<Score>();
    let core = crate::support::boot(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind");
    core.query("from Score order by points")
        .await
        .expect("warm");
    assert!(core.query("from Score order by nowhere").await.is_err());
}

#[tokio::test]
async fn bonds() {
    let mut graph = Graph::new();
    graph.plug::<Class>().plug::<Student>();
    let core = crate::support::boot(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind");

    core.query(r#"from Student where classes has "1""#)
        .await
        .expect("warm has");
    assert!(
        core.query(r#"from Student where classes has "abc""#)
            .await
            .is_err()
    );

    core.query(r#"from Student where classes some (id = "1")"#)
        .await
        .expect("warm some");
    assert!(
        core.query(r#"from Student where classes some (id = "abc")"#)
            .await
            .is_err()
    );
}

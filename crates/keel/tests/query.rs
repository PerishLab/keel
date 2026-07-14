use keel::adapt::db::Sqlite;
use keel::atom::{string, url};
use keel::query;
use keel::resource;
use keel::{Graph, bind};

#[resource]
struct Class {
    #[field(string)]
    title: string,
}

#[resource]
struct Student {
    #[field(string)]
    nickname: string,
    #[field(url)]
    avatar: url,
    #[relation(Class, n2m)]
    classes: Class,
}

#[test]
fn parse() {
    let ask = query::parse("from Student").expect("parse");
    assert_eq!(ask.unit(), "Student");
    let ask = query::parse("  FROM   student  ").expect("case");
    assert_eq!(ask.unit(), "student");
    assert!(query::parse("select *").is_err());
    assert!(query::parse("from").is_err());
    assert!(query::parse("from Student where x").is_err());
}

#[test]
fn run() {
    let mut graph = Graph::new();
    graph.plug::<Class>().plug::<Student>();
    let core = bind(graph, Sqlite::memory()).expect("bind");
    core.put(
        "Student",
        &[("nickname", "ada"), ("avatar", "https://a.example/a")],
    )
    .expect("put");
    let rows = core.query("from Student").expect("query");
    assert_eq!(rows.len(), 1);
    let rows = core.query("from student").expect("lower");
    assert_eq!(rows.len(), 1);
    let via = core.live("Student").expect("live sugar");
    assert_eq!(via.len(), 1);
    assert!(core.query("from Ghost").is_err());
}

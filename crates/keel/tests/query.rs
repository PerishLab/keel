use keel::adapt::db::Sqlite;
use keel::atom::{string, url};
use keel::query::{self, Op, Slice};
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
    let tree = query::parse("from Student").expect("parse");
    assert_eq!(tree.from(), "Student");
    assert_eq!(tree.slice(), Slice::Live);
    assert!(tree.preds().is_empty());
    assert_eq!(query::digest(&tree), "from student slice live");

    let tree = query::parse(r#"from Student where nickname = "ada""#).expect("where");
    assert_eq!(tree.preds().len(), 1);
    assert_eq!(tree.preds()[0].field(), "nickname");
    assert_eq!(tree.preds()[0].op(), Op::Eq);
    assert_eq!(tree.preds()[0].value(), "ada");
    assert_eq!(
        query::digest(&tree),
        r#"from student slice live where nickname = "ada""#
    );

    let tree =
        query::parse(r#"from Student where nickname = "ada" and avatar = "https://a.example/a""#)
            .expect("and");
    assert_eq!(tree.preds().len(), 2);

    let tree = query::parse(r#"FROM student WHERE nickname = "x""#).expect("case");
    assert_eq!(tree.from(), "student");

    assert!(query::parse("select *").is_err());
    assert!(query::parse("from").is_err());
    assert!(query::parse("from Student where").is_err());
    assert!(query::parse("from Student where nickname").is_err());
    assert!(query::parse(r#"from Student where nickname = ada"#).is_err());
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
    .expect("put ada");
    core.put(
        "Student",
        &[("nickname", "bob"), ("avatar", "https://b.example/b")],
    )
    .expect("put bob");

    let rows = core.query("from Student").expect("all");
    assert_eq!(rows.len(), 2);

    let rows = core
        .query(r#"from Student where nickname = "ada""#)
        .expect("filter");
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].cells().get("nickname").map(String::as_str),
        Some("ada")
    );

    let rows = core
        .query(r#"from Student where nickname = "ada" and avatar = "https://a.example/a""#)
        .expect("and");
    assert_eq!(rows.len(), 1);

    let rows = core
        .query(r#"from Student where nickname = "zoe""#)
        .expect("miss");
    assert!(rows.is_empty());

    assert!(core.query(r#"from Student where missing = "x""#).is_err());
    assert!(core.query("from Ghost").is_err());
}

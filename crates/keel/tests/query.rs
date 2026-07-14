use keel::adapt::db::Sqlite;
use keel::atom::{string, url};
use keel::query::{self, Op, Rank, Slice};
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
    assert!(tree.sort().is_none());
    assert!(tree.limit().is_none());
    assert!(tree.after().is_none());
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

    let tree = query::parse(r#"from Student where nickname != "ada""#).expect("ne");
    assert_eq!(tree.preds()[0].op(), Op::Ne);
    assert_eq!(
        query::digest(&tree),
        r#"from student slice live where nickname != "ada""#
    );

    let tree = query::parse(r#"from Student where nickname < "m""#).expect("lt");
    assert_eq!(tree.preds()[0].op(), Op::Lt);
    let tree = query::parse(r#"from Student where nickname <= "m""#).expect("le");
    assert_eq!(tree.preds()[0].op(), Op::Le);
    let tree = query::parse(r#"from Student where nickname > "m""#).expect("gt");
    assert_eq!(tree.preds()[0].op(), Op::Gt);
    let tree = query::parse(r#"from Student where nickname >= "m""#).expect("ge");
    assert_eq!(tree.preds()[0].op(), Op::Ge);
    assert_eq!(
        query::digest(&tree),
        r#"from student slice live where nickname >= "m""#
    );

    let tree = query::parse(r#"from Student where nickname in ("ada", "bob")"#).expect("in");
    assert_eq!(tree.preds()[0].op(), Op::In);
    assert_eq!(
        tree.preds()[0].values(),
        &["ada".to_string(), "bob".to_string()]
    );
    assert_eq!(
        query::digest(&tree),
        r#"from student slice live where nickname in ("ada", "bob")"#
    );

    let tree =
        query::parse(r#"from Student where nickname = "ada" and avatar = "https://a.example/a""#)
            .expect("and");
    assert_eq!(tree.preds().len(), 2);

    let tree = query::parse("from Student order by nickname").expect("order");
    assert_eq!(tree.sort().expect("sort").field(), "nickname");
    assert_eq!(tree.sort().expect("sort").rank(), Rank::Asc);
    assert_eq!(
        query::digest(&tree),
        "from student slice live order by nickname asc"
    );

    let tree =
        query::parse("from Student order by nickname desc limit 2 after \"3\"").expect("page");
    assert_eq!(tree.sort().expect("sort").rank(), Rank::Desc);
    assert_eq!(tree.limit(), Some(2));
    assert_eq!(tree.after(), Some(3));
    assert_eq!(
        query::digest(&tree),
        r#"from student slice live order by nickname desc limit 2 after "3""#
    );

    let tree = query::parse(r#"FROM student WHERE nickname = "x" LIMIT 1"#).expect("case");
    assert_eq!(tree.from(), "student");
    assert_eq!(tree.limit(), Some(1));

    assert!(query::parse("select *").is_err());
    assert!(query::parse("from").is_err());
    assert!(query::parse("from Student where").is_err());
    assert!(query::parse("from Student where nickname").is_err());
    assert!(query::parse(r#"from Student where nickname = ada"#).is_err());
    assert!(query::parse(r#"from Student where nickname in ()"#).is_err());
    assert!(query::parse(r#"from Student where nickname ~ "x""#).is_err());
    assert!(query::parse("from Student limit 0").is_err());
    assert!(query::parse("from Student limit").is_err());
    assert!(query::parse(r#"from Student after "x""#).is_err());
    assert!(query::parse("from Student limit 1 order by nickname").is_err());
}

#[test]
fn run() {
    let mut graph = Graph::new();
    graph.plug::<Class>().plug::<Student>();
    let core = bind(graph, Sqlite::memory()).expect("bind");
    let ada = core
        .put(
            "Student",
            &[("nickname", "ada"), ("avatar", "https://a.example/a")],
        )
        .expect("put ada");
    let bob = core
        .put(
            "Student",
            &[("nickname", "bob"), ("avatar", "https://b.example/b")],
        )
        .expect("put bob");
    let cy = core
        .put(
            "Student",
            &[("nickname", "cy"), ("avatar", "https://c.example/c")],
        )
        .expect("put cy");

    let rows = core.query("from Student").expect("all");
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].key(), ada);
    assert_eq!(rows[1].key(), bob);
    assert_eq!(rows[2].key(), cy);

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

    let rows = core
        .query(r#"from Student where nickname != "ada""#)
        .expect("ne");
    assert_eq!(rows.len(), 2);

    let rows = core
        .query(r#"from Student where nickname < "b""#)
        .expect("lt");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].key(), ada);

    let rows = core
        .query(r#"from Student where nickname >= "bob""#)
        .expect("ge");
    assert_eq!(rows.len(), 2);

    let rows = core
        .query(r#"from Student where nickname in ("ada", "cy")"#)
        .expect("in");
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().any(|row| row.key() == ada));
    assert!(rows.iter().any(|row| row.key() == cy));

    let rows = core
        .query(r#"from Student where nickname != "ada" and nickname < "d""#)
        .expect("mix");
    assert_eq!(rows.len(), 2);

    let rows = core
        .query("from Student order by nickname desc")
        .expect("desc");
    assert_eq!(
        rows.iter()
            .map(|row| row.cells().get("nickname").cloned().unwrap())
            .collect::<Vec<_>>(),
        vec!["cy", "bob", "ada"]
    );

    let rows = core.query("from Student limit 2").expect("limit");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].key(), ada);
    assert_eq!(rows[1].key(), bob);

    let rows = core
        .query(&format!(r#"from Student limit 1 after "{ada}""#))
        .expect("after");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].key(), bob);

    let rows = core
        .query(&format!(
            r#"from Student order by nickname desc limit 1 after "{bob}""#
        ))
        .expect("cursor");
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].cells().get("nickname").map(String::as_str),
        Some("ada")
    );

    let rows = core.query(r#"from Student after "999""#).expect("gone");
    assert!(rows.is_empty());

    assert!(core.query(r#"from Student where missing = "x""#).is_err());
    assert!(core.query("from Student order by missing").is_err());
    assert!(core.query("from Ghost").is_err());
}

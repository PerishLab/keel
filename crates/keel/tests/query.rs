use keel::adapt::db::Sqlite;
use keel::atom::{int, string, url};
use keel::life::{Cell, Ends};
use keel::query::{self, Op, Rank, Slice};
use keel::resource;
use keel::{Graph, Row, bind};

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
    #[relation(Class, many2many)]
    classes: Class,
}

#[test]
fn parse() {
    let tree = query::parse("from Student").expect("parse");
    assert_eq!(tree.from(), "Student");
    assert_eq!(tree.slice(), Slice::Live);
    assert!(tree.preds().is_empty());
    assert!(tree.links().is_empty());
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

    let tree = query::parse(r#"from Student where nickname in ("ada", "bob")"#).expect("in");
    assert_eq!(tree.preds()[0].op(), Op::In);

    let tree = query::parse("from Student link classes").expect("link");
    assert_eq!(tree.links(), &["classes".to_string()]);
    assert_eq!(query::digest(&tree), "from student slice live link classes");

    let tree = query::parse(r#"from Student where courses some (grade = "A")"#).expect("some");
    assert_eq!(tree.preds()[0].op(), Op::Some);
    assert_eq!(tree.preds()[0].field(), "courses");
    assert_eq!(tree.preds()[0].nest().expect("nest").field(), "grade");
    assert_eq!(
        query::digest(&tree),
        r#"from student slice live where courses some (grade = "A")"#
    );

    let tree = query::parse(
        r#"from Student where nickname = "ada" link classes order by nickname limit 2 after "1""#,
    )
    .expect("full");
    assert_eq!(tree.links().len(), 1);
    assert_eq!(tree.limit(), Some(2));
    assert_eq!(
        query::digest(&tree),
        r#"from student slice live where nickname = "ada" link classes order by nickname asc limit 2 after "1""#
    );

    let tree = query::parse("from Student order by nickname").expect("order");
    assert_eq!(tree.sort().expect("sort").field(), "nickname");
    assert_eq!(tree.sort().expect("sort").rank(), Rank::Asc);

    let tree =
        query::parse("from Student order by nickname desc limit 2 after \"3\"").expect("page");
    assert_eq!(tree.after(), Some(3));

    assert!(query::parse("from Student link classes link classes").is_err());
    assert!(query::parse("select *").is_err());
    assert!(query::parse("from Student limit 0").is_err());
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
    let math = core.put("Class", &[("title", "math")]).expect("put math");
    let tie = core
        .tie(
            "Student",
            "classes",
            Ends {
                left: ada,
                right: math,
            },
            &[],
        )
        .expect("tie");

    let pack = core.query("from Student").expect("all");
    assert_eq!(pack.root(), "student");
    assert_eq!(pack.rows().len(), 3);
    assert_eq!(pack.rows()[0].key(), ada);
    assert_eq!(pack.bags().len(), 1);

    let pack = core
        .query(r#"from Student where nickname = "ada""#)
        .expect("filter");
    assert_eq!(pack.rows().len(), 1);
    assert_eq!(
        pack.rows()[0].cells().get("nickname").map(Cell::text),
        Some("ada")
    );

    let pack = core
        .query(r#"from Student where nickname != "ada""#)
        .expect("ne");
    assert_eq!(pack.rows().len(), 2);

    let pack = core
        .query(r#"from Student where nickname in ("ada", "cy")"#)
        .expect("in");
    assert_eq!(pack.rows().len(), 2);

    let pack = core
        .query("from Student order by nickname desc")
        .expect("desc");
    assert_eq!(
        pack.rows()
            .iter()
            .map(|row| row.cells().get("nickname").expect("cell").show())
            .collect::<Vec<_>>(),
        vec!["cy", "bob", "ada"]
    );

    let pack = core.query("from Student limit 2").expect("limit");
    assert_eq!(pack.rows().len(), 2);
    assert_eq!(pack.rows()[0].key(), ada);

    let pack = core
        .query(&format!(r#"from Student limit 1 after "{ada}""#))
        .expect("after");
    assert_eq!(pack.rows().len(), 1);
    assert_eq!(pack.rows()[0].key(), bob);

    let pack = core.query("from Student link classes").expect("link");
    assert_eq!(pack.root(), "student");
    assert_eq!(pack.rows().len(), 3);
    let bonds = pack.bond("student.classes").expect("bond bag");
    assert_eq!(bonds.len(), 1);
    assert_eq!(bonds[0].key(), tie);
    assert_eq!(bonds[0].left(), ada);
    assert_eq!(bonds[0].right(), math);
    assert!(pack.unit("class").is_none());

    let pack = core
        .query(r#"from Student where nickname = "bob" link classes"#)
        .expect("empty bond");
    assert_eq!(pack.rows().len(), 1);
    assert_eq!(pack.bond("student.classes").expect("bag").len(), 0);

    let pack = core
        .query(r#"from Student where nickname = "ada" link classes"#)
        .expect("one");
    assert_eq!(pack.rows().len(), 1);
    assert_eq!(pack.bond("student.classes").expect("bag").len(), 1);

    let pack = core
        .query(&format!(r#"from Student where id = "{ada}""#))
        .expect("id eq");
    assert_eq!(pack.rows().len(), 1);
    assert_eq!(pack.rows()[0].key(), ada);

    let pack = core
        .query(&format!(r#"from Student where id in ("{ada}", "{cy}")"#))
        .expect("id in");
    assert_eq!(pack.rows().len(), 2);

    let pack = core
        .query(&format!(r#"from Class where id = "{math}""#))
        .expect("target");
    assert_eq!(pack.rows().len(), 1);
    assert_eq!(
        pack.rows()[0].cells().get("title").map(Cell::text),
        Some("math")
    );

    let pack = core
        .query("from Student order by id desc")
        .expect("id order");
    assert_eq!(pack.rows()[0].key(), cy);

    let pack = core.query("from Student LINK Classes").expect("case");
    assert!(pack.bond("student.classes").is_some());

    assert!(core.query(r#"from Student where id = "x""#).is_err());
    assert!(core.query("from Student link missing").is_err());
    assert!(core.query(r#"from Student where missing = "x""#).is_err());
    assert!(core.query("from Ghost").is_err());
    let _ = bob;
}

#[resource]
struct Score {
    #[field(string)]
    name: string,
    #[field(int)]
    points: int,
    #[field(bool)]
    passed: bool,
}

#[test]
fn cast() {
    let mut graph = Graph::new();
    graph.plug::<Score>();
    let core = bind(graph, Sqlite::memory()).expect("bind");
    let low = core
        .put(
            "Score",
            &[("name", "low"), ("points", "2"), ("passed", "false")],
        )
        .expect("put low");
    let mid = core
        .put(
            "Score",
            &[("name", "mid"), ("points", "10"), ("passed", "true")],
        )
        .expect("put mid");
    let top = core
        .put(
            "Score",
            &[("name", "top"), ("points", "42"), ("passed", "true")],
        )
        .expect("put top");

    let pack = core.query(r#"from Score where points > "5""#).expect("gt");
    assert_eq!(pack.rows().len(), 2);

    let pack = core.query("from Score order by points desc").expect("ord");
    assert_eq!(
        pack.rows().iter().map(Row::key).collect::<Vec<_>>(),
        vec![top, mid, low]
    );

    let pack = core
        .query(r#"from Score where passed = "true""#)
        .expect("flag");
    assert_eq!(pack.rows().len(), 2);
    assert_eq!(pack.rows()[0].cells().get("points"), Some(&Cell::Int(10)));
    assert_eq!(
        pack.rows()[0].cells().get("passed"),
        Some(&Cell::Bool(true))
    );

    core.set("Score", low, &[("points", "77")]).expect("set");
    let pack = core
        .query(r#"from Score where points >= "77""#)
        .expect("ge");
    assert_eq!(pack.rows().len(), 1);

    assert!(
        core.put(
            "Score",
            &[("name", "x"), ("points", "x"), ("passed", "true")]
        )
        .is_err()
    );
    assert!(
        core.put(
            "Score",
            &[("name", "x"), ("points", "1"), ("passed", "yep")]
        )
        .is_err()
    );
    assert!(core.set("Score", low, &[("passed", "1")]).is_err());
    assert!(core.query(r#"from Score where points = "x""#).is_err());
    assert!(core.query(r#"from Score where passed = "x""#).is_err());
}

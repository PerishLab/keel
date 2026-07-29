use crate::world::*;
use keel::Graph;
use keel::adapt::db::Sqlite;
use keel::life::{Cell, Ends};

#[tokio::test]
async fn run() {
    let mut graph = Graph::new();
    graph.plug::<Class>().plug::<Student>();
    let core = crate::support::boot(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind");
    let ada = core
        .put(
            "Student",
            &[("nickname", "ada"), ("avatar", "https://a.example/a")],
        )
        .await
        .expect("put ada");
    let bob = core
        .put(
            "Student",
            &[("nickname", "bob"), ("avatar", "https://b.example/b")],
        )
        .await
        .expect("put bob");
    let cy = core
        .put(
            "Student",
            &[("nickname", "cy"), ("avatar", "https://c.example/c")],
        )
        .await
        .expect("put cy");
    let math = core
        .put("Class", &[("title", "math")])
        .await
        .expect("put math");
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
        .await
        .expect("tie");

    let pack = core.query("from Student").await.expect("all");
    assert_eq!(pack.root(), "student");
    assert_eq!(pack.rows().len(), 3);
    assert_eq!(pack.rows()[0].key(), ada);
    assert_eq!(pack.bags().len(), 1);
    assert_eq!(pack.count(), None);

    let pack = core.query("from Student count").await.expect("count");
    assert_eq!(pack.count(), Some(3));
    assert!(pack.bags().is_empty());
    let pack = core
        .query(r#"from Student where nickname != "ada" count"#)
        .await
        .expect("count pred");
    assert_eq!(pack.count(), Some(2));
    assert!(core.query("from Student count limit 1").await.is_err());
    assert!(core.query("from Student count link classes").await.is_err());

    let pack = core
        .query(r#"from Student where nickname = "ada""#)
        .await
        .expect("filter");
    assert_eq!(pack.rows().len(), 1);
    assert_eq!(
        pack.rows()[0].cells().get("nickname").map(Cell::text),
        Some("ada")
    );

    let pack = core
        .query(r#"from Student where nickname != "ada""#)
        .await
        .expect("ne");
    assert_eq!(pack.rows().len(), 2);

    let pack = core
        .query(r#"from Student where nickname in ("ada", "cy")"#)
        .await
        .expect("in");
    assert_eq!(pack.rows().len(), 2);

    let pack = core
        .query(r#"from Student where nickname like "A""#)
        .await
        .expect("like");
    assert_eq!(pack.rows().len(), 1);
    assert_eq!(pack.rows()[0].key(), ada);

    let pack = core
        .query("from Student order by nickname desc")
        .await
        .expect("desc");
    assert_eq!(
        pack.rows()
            .iter()
            .map(|row| row.cells().get("nickname").expect("cell").show())
            .collect::<Vec<_>>(),
        vec!["cy", "bob", "ada"]
    );

    let pack = core
        .query("from Student order by nickname desc, id desc")
        .await
        .expect("compound order");
    assert_eq!(
        pack.rows()
            .iter()
            .map(|row| row.cells().get("nickname").expect("cell").show())
            .collect::<Vec<_>>(),
        vec!["cy", "bob", "ada"]
    );

    let pack = core.query("from Student limit 2").await.expect("limit");
    assert_eq!(pack.rows().len(), 2);
    assert_eq!(pack.rows()[0].key(), ada);

    let pack = core
        .query(&format!(r#"from Student limit 1 after "{ada}""#))
        .await
        .expect("after");
    assert_eq!(pack.rows().len(), 1);
    assert_eq!(pack.rows()[0].key(), bob);

    let pack = core.query("from Student link classes").await.expect("link");
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
        .await
        .expect("empty bond");
    assert_eq!(pack.rows().len(), 1);
    assert_eq!(pack.bond("student.classes").expect("bag").len(), 0);

    let pack = core
        .query(r#"from Student where nickname = "ada" link classes"#)
        .await
        .expect("one");
    assert_eq!(pack.rows().len(), 1);
    assert_eq!(pack.bond("student.classes").expect("bag").len(), 1);

    let pack = core
        .query(&format!(r#"from Student where id = "{ada}""#))
        .await
        .expect("id eq");
    assert_eq!(pack.rows().len(), 1);
    assert_eq!(pack.rows()[0].key(), ada);

    let pack = core
        .query(&format!(r#"from Student where id in ("{ada}", "{cy}")"#))
        .await
        .expect("id in");
    assert_eq!(pack.rows().len(), 2);

    let pack = core
        .query(&format!(r#"from Class where id = "{math}""#))
        .await
        .expect("target");
    assert_eq!(pack.rows().len(), 1);
    assert_eq!(
        pack.rows()[0].cells().get("title").map(Cell::text),
        Some("math")
    );

    let pack = core
        .query("from Student order by id desc")
        .await
        .expect("id order");
    assert_eq!(pack.rows()[0].key(), cy);

    let pack = core.query("from Student LINK Classes").await.expect("case");
    assert!(pack.bond("student.classes").is_some());

    assert!(core.query(r#"from Student where id = "x""#).await.is_err());
    assert!(core.query("from Student link missing").await.is_err());
    assert!(
        core.query(r#"from Student where missing = "x""#)
            .await
            .is_err()
    );
    assert!(core.query("from Ghost").await.is_err());
    let _ = bob;
}

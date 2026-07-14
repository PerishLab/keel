use keel::adapt::db::Sqlite;
use keel::atom::string;
use keel::life::Ends;
use keel::resource;
use keel::{Graph, bind};

#[resource]
struct Course {
    #[field(string)]
    code: string,
    #[field(string)]
    title: string,
}

#[resource]
struct Student {
    #[field(string)]
    no: string,
    #[field(string)]
    name: string,
    #[relation(Course, n2m)]
    courses: Course,
}

#[test]
fn select() {
    let mut graph = Graph::new();
    graph.plug::<Course>().plug::<Student>();
    let core = bind(graph, Sqlite::memory()).expect("bind");

    let algo = core
        .put("Course", &[("code", "CS101"), ("title", "algo")])
        .expect("algo");
    let db = core
        .put("Course", &[("code", "CS102"), ("title", "db")])
        .expect("db");
    let ada = core
        .put("Student", &[("no", "S01"), ("name", "ada")])
        .expect("ada");
    let bob = core
        .put("Student", &[("no", "S02"), ("name", "bob")])
        .expect("bob");

    core.tie(
        "Student",
        "courses",
        Ends {
            left: ada,
            right: algo,
        },
    )
    .expect("ada algo");
    core.tie(
        "Student",
        "courses",
        Ends {
            left: ada,
            right: db,
        },
    )
    .expect("ada db");
    core.tie(
        "Student",
        "courses",
        Ends {
            left: bob,
            right: algo,
        },
    )
    .expect("bob algo");

    let pack = core
        .query(r#"from Student where no = "S01" link courses"#)
        .expect("ada pack");
    assert_eq!(pack.rows().len(), 1);
    assert_eq!(pack.rows()[0].key(), ada);
    let ties = pack.bond("student.courses").expect("ties");
    assert_eq!(ties.len(), 2);
    let rights: Vec<i64> = ties.iter().map(|t| t.right()).collect();
    assert!(rights.contains(&algo));
    assert!(rights.contains(&db));

    let pack = core
        .query(&format!(
            r#"from Course where id in ("{algo}", "{db}") order by code"#
        ))
        .expect("hydrate");
    assert_eq!(pack.rows().len(), 2);
    assert_eq!(
        pack.rows()[0].cells().get("code").map(String::as_str),
        Some("CS101")
    );

    let tie = ties.iter().find(|t| t.right() == db).expect("db tie");
    core.cut("Student", "courses", tie.key()).expect("drop db");
    let pack = core
        .query(r#"from Student where no = "S01" link courses"#)
        .expect("after drop");
    assert_eq!(pack.bond("student.courses").expect("ties").len(), 1);
    assert_eq!(pack.bond("student.courses").expect("ties")[0].right(), algo);

    core.set("Student", bob, &[("name", "bobby")]).expect("set");
    let pack = core
        .query(r#"from Student where name = "bobby" link courses"#)
        .expect("bob");
    assert_eq!(pack.rows().len(), 1);
    assert_eq!(pack.bond("student.courses").expect("ties").len(), 1);

    let pack = core
        .query("from Student link courses order by no")
        .expect("all");
    assert_eq!(pack.rows().len(), 2);
    let all = pack.bond("student.courses").expect("all ties");
    assert_eq!(all.len(), 2);

    core.end("Course", db).expect("end db");
    let pack = core.query("from Course").expect("live courses");
    assert_eq!(pack.rows().len(), 1);
    assert_eq!(pack.rows()[0].key(), algo);
}

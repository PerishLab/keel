use keel::adapt::db::Sqlite;
use keel::atom::string;
use keel::life::{Cell, Ends};
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
    #[relation(Course, many2many, grade = string)]
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
        &[("grade", "A")],
    )
    .expect("ada algo");
    core.tie(
        "Student",
        "courses",
        Ends {
            left: ada,
            right: db,
        },
        &[("grade", "B")],
    )
    .expect("ada db");
    core.tie(
        "Student",
        "courses",
        Ends {
            left: bob,
            right: algo,
        },
        &[("grade", "")],
    )
    .expect("bob algo");

    let pack = core
        .query(r#"from Student where no = "S01" link courses"#)
        .expect("ada pack");
    assert_eq!(pack.rows().len(), 1);
    assert_eq!(pack.rows()[0].key(), ada);
    let ties = pack.bond("student.courses").expect("ties");
    assert_eq!(ties.len(), 2);
    assert_eq!(
        ties.iter()
            .find(|t| t.right() == algo)
            .expect("algo")
            .cells()
            .get("grade")
            .map(Cell::text),
        Some("A")
    );

    let pack = core
        .query(&format!(r#"from Student where courses has "{algo}""#))
        .expect("has");
    assert_eq!(pack.rows().len(), 2);

    let pack = core
        .query(r#"from Student where courses some (grade = "A")"#)
        .expect("some grade");
    assert_eq!(pack.rows().len(), 1);
    assert_eq!(pack.rows()[0].key(), ada);

    let pack = core
        .query(r#"from Student where courses some (code = "CS101")"#)
        .expect("some code");
    assert_eq!(pack.rows().len(), 2);

    let pack = core
        .query(&format!(
            r#"from Course where id in ("{algo}", "{db}") order by code"#
        ))
        .expect("hydrate");
    assert_eq!(pack.rows().len(), 2);

    let tie = ties.iter().find(|t| t.right() == db).expect("db tie");
    core.set_tie("Student", "courses", tie.key(), &[("grade", "A+")])
        .expect("grade");
    core.cut("Student", "courses", tie.key()).expect("drop db");
    let pack = core
        .query(r#"from Student where no = "S01" link courses"#)
        .expect("after drop");
    assert_eq!(pack.bond("student.courses").expect("ties").len(), 1);

    core.tie(
        "Student",
        "courses",
        Ends {
            left: ada,
            right: db,
        },
        &[("grade", "C")],
    )
    .expect("re-enroll");

    assert!(core.end("Course", db).is_err());

    let pack = core
        .query(r#"from Student where no = "S01" link courses"#)
        .expect("linked");
    let re = pack
        .bond("student.courses")
        .expect("ties")
        .iter()
        .find(|t| t.right() == db)
        .expect("db tie")
        .key();
    core.cut("Student", "courses", re).expect("drop re");
    core.end("Course", db).expect("end db");

    let pack = core
        .query(r#"from Student where no = "S01" link courses"#)
        .expect("after end");
    assert_eq!(pack.bond("student.courses").expect("ties").len(), 1);
    assert_eq!(pack.bond("student.courses").expect("ties")[0].right(), algo);

    let pack = core.query("from Course").expect("live courses");
    assert_eq!(pack.rows().len(), 1);
    assert_eq!(pack.rows()[0].key(), algo);

    assert!(
        core.tie(
            "Student",
            "courses",
            Ends {
                left: ada,
                right: db,
            },
            &[("grade", "X")],
        )
        .is_err()
    );
    assert!(core.end("Student", ada).is_err());
    assert!(core.end("Course", algo).is_err());
    let pack = core
        .query(r#"from Student where no = "S01" link courses"#)
        .expect("last");
    let last = pack.bond("student.courses").expect("ties")[0].key();
    core.cut("Student", "courses", last).expect("cut last");
    assert!(core.end("Course", algo).is_err());
    let pack = core
        .query(r#"from Student where no = "S02" link courses"#)
        .expect("bob ties");
    let bob_tie = pack.bond("student.courses").expect("ties")[0].key();
    core.cut("Student", "courses", bob_tie).expect("cut bob");
    core.end("Course", algo).expect("end algo");
    core.end("Student", ada).expect("end ada");
}

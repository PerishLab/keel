use keel::Graph;
use keel::adapt::db::Sqlite;
use keel::atom::string;
use keel::life::{Cell, Ends};
use keel::resource;

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

#[tokio::test]
async fn select() {
    let mut graph = Graph::new();
    graph.plug::<Course>().plug::<Student>();
    let core = crate::support::boot(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind");

    let algo = core
        .put("Course", &[("code", "CS101"), ("title", "algo")])
        .await
        .expect("algo");
    let db = core
        .put("Course", &[("code", "CS102"), ("title", "db")])
        .await
        .expect("db");
    let ada = core
        .put("Student", &[("no", "S01"), ("name", "ada")])
        .await
        .expect("ada");
    let bob = core
        .put("Student", &[("no", "S02"), ("name", "bob")])
        .await
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
    .await
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
    .await
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
    .await
    .expect("bob algo");

    let pack = core
        .query(r#"from Student where no = "S01" link courses"#)
        .await
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
        .await
        .expect("has");
    assert_eq!(pack.rows().len(), 2);

    let pack = core
        .query(r#"from Student where courses some (grade = "A")"#)
        .await
        .expect("some grade");
    assert_eq!(pack.rows().len(), 1);
    assert_eq!(pack.rows()[0].key(), ada);

    let pack = core
        .query(r#"from Student where courses some (code = "CS101")"#)
        .await
        .expect("some code");
    assert_eq!(pack.rows().len(), 2);

    let pack = core
        .query(&format!(
            r#"from Course where id in ("{algo}", "{db}") order by code"#
        ))
        .await
        .expect("hydrate");
    assert_eq!(pack.rows().len(), 2);

    let tie = ties.iter().find(|t| t.right() == db).expect("db tie");
    core.tune("Student", "courses", tie.key(), &[("grade", "A+")])
        .await
        .expect("grade");
    core.cut("Student", "courses", tie.key())
        .await
        .expect("drop db");
    let pack = core
        .query(r#"from Student where no = "S01" link courses"#)
        .await
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
    .await
    .expect("re-enroll");

    assert!(core.end("Course", db).await.is_err());

    let pack = core
        .query(r#"from Student where no = "S01" link courses"#)
        .await
        .expect("linked");
    let re = pack
        .bond("student.courses")
        .expect("ties")
        .iter()
        .find(|t| t.right() == db)
        .expect("db tie")
        .key();
    core.cut("Student", "courses", re).await.expect("drop re");
    core.end("Course", db).await.expect("end db");

    let pack = core
        .query(r#"from Student where no = "S01" link courses"#)
        .await
        .expect("after end");
    assert_eq!(pack.bond("student.courses").expect("ties").len(), 1);
    assert_eq!(pack.bond("student.courses").expect("ties")[0].right(), algo);

    let pack = core.query("from Course").await.expect("live courses");
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
        .await
        .is_err()
    );
    assert!(core.end("Student", ada).await.is_err());
    assert!(core.end("Course", algo).await.is_err());
    let pack = core
        .query(r#"from Student where no = "S01" link courses"#)
        .await
        .expect("last");
    let last = pack.bond("student.courses").expect("ties")[0].key();
    core.cut("Student", "courses", last)
        .await
        .expect("cut last");
    assert!(core.end("Course", algo).await.is_err());
    let pack = core
        .query(r#"from Student where no = "S02" link courses"#)
        .await
        .expect("bob ties");
    let knot = pack.bond("student.courses").expect("ties")[0].key();
    core.cut("Student", "courses", knot).await.expect("cut bob");
    core.end("Course", algo).await.expect("end algo");
    core.end("Student", ada).await.expect("end ada");
}

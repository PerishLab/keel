#![cfg(feature = "pg")]

use keel::adapt::pg::Postgres;
use keel::atom::string;
use keel::life::Ends;
use keel::resource;
use keel::{Cell, Graph, bind};

#[resource]
struct Course {
    #[field(string, unique)]
    code: string,
}

#[resource]
struct Student {
    #[field(string, unique)]
    no: string,
    #[field(string)]
    name: string,
    #[relation(Course, many2many, grade = string)]
    courses: Course,
}

fn url() -> String {
    std::env::var("KEEL_PG")
        .unwrap_or_else(|_| "host=127.0.0.1 port=5433 user=keel password=keel dbname=keel".into())
}

fn reset() {
    let mut client = postgres::Client::connect(&url(), postgres::NoTls).expect("connect");
    client
        .batch_execute("DROP SCHEMA public CASCADE; CREATE SCHEMA public;")
        .expect("reset");
}

#[test]
fn portable() {
    reset();
    let store = Postgres::at(url());
    let mut graph = Graph::new();
    graph.plug::<Course>().plug::<Student>();
    let core = bind(graph, store).expect("bind");

    let algo = core.put("Course", &[("code", "CS101")]).expect("course");
    assert!(core.put("Course", &[("code", "CS101")]).is_err());
    let ada = core
        .put("Student", &[("no", "S01"), ("name", "ada")])
        .expect("ada");
    core.tie(
        "Student",
        "courses",
        Ends {
            left: ada,
            right: algo,
        },
        &[("grade", "A")],
    )
    .expect("enroll");

    let pack = core
        .query(r#"from Student where no = "S01" link courses"#)
        .expect("pack");
    assert_eq!(pack.rows().len(), 1);
    let ties = pack.bond("student.courses").expect("bag");
    assert_eq!(ties.len(), 1);
    assert_eq!(ties[0].cells().get("grade"), Some(&Cell::Text("A".into())));

    let count = core
        .query(r#"from Student where courses has "1" count"#)
        .expect("count");
    assert_eq!(count.count(), Some(1));

    assert!(core.end("Course", algo).is_err());
    core.set("Student", ada, &[("name", "ada2")]).expect("set");
    let after = core.live("Student").expect("live");
    assert_eq!(
        after[0].cells().get("name"),
        Some(&Cell::Text("ada2".into()))
    );
}

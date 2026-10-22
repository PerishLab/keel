#![cfg(feature = "pg")]

use keel::adapt::pg::Postgres;
use keel::atom::{int, string};
use keel::life::Ends;
use keel::resource;
use keel::{Cell, Graph};

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

#[resource]
struct Ledger {
    #[field(string, unique)]
    tag: string,
}

#[resource]
struct Entry {
    #[field(serial, scope = ledger)]
    index: int,
    #[field(string)]
    note: string,
    #[relation(Ledger, many2one, root)]
    ledger: Ledger,
}

#[resource]
struct Note {
    #[field(string, unique)]
    tag: string,
    #[field(string, opt)]
    body: string,
}

static GATE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn url() -> String {
    std::env::var("KEEL_PG")
        .unwrap_or_else(|_| "host=127.0.0.1 port=5433 user=keel password=keel dbname=keel".into())
}

async fn reset() -> Postgres {
    use keel::Wire as _;
    let mut store = Postgres::at(url()).await.expect("connect");
    store
        .script("DROP SCHEMA public CASCADE; CREATE SCHEMA public;")
        .await
        .expect("reset");
    store
}

#[tokio::test]
async fn revived() {
    use keel::Wire as _;
    let _hold = GATE.lock().await;
    let store = reset().await;
    let mut graph = Graph::new();
    graph.plug::<Course>().plug::<Student>();
    let core = crate::support::boot(graph, store).await.expect("bind");
    core.put("Course", &[("code", "CS101")])
        .await
        .expect("before");

    let mut axe = Postgres::at(url()).await.expect("axe");
    axe.run(
        "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = current_database() AND pid <> pg_backend_pid()",
        &[],
    )
    .await
    .expect("terminate");

    assert!(core.live("Course").await.is_err());

    let after = core.live("Course").await.expect("after");
    assert_eq!(after.len(), 1);
    core.put("Course", &[("code", "CS102")])
        .await
        .expect("write");
}

#[tokio::test]
async fn portable() {
    let _hold = GATE.lock().await;
    let store = reset().await;
    let mut graph = Graph::new();
    graph.plug::<Course>().plug::<Student>();
    let core = crate::support::boot(graph, store).await.expect("bind");

    let algo = core
        .put("Course", &[("code", "CS101")])
        .await
        .expect("course");
    assert!(core.put("Course", &[("code", "CS101")]).await.is_err());
    let ada = core
        .put("Student", &[("no", "S01"), ("name", "ada")])
        .await
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
    .await
    .expect("enroll");

    let pack = core
        .query(r#"from Student where no = "S01" link courses"#)
        .await
        .expect("pack");
    assert_eq!(pack.rows().len(), 1);
    let ties = pack.bond("student.courses").expect("bag");
    assert_eq!(ties.len(), 1);
    assert_eq!(ties[0].cells().get("grade"), Some(&Cell::Text("A".into())));

    let count = core
        .query(r#"from Student where courses has "1" count"#)
        .await
        .expect("count");
    assert_eq!(count.count(), Some(1));

    assert!(core.end("Course", algo).await.is_err());
    core.set("Student", ada, &[("name", "ada2")])
        .await
        .expect("set");
    let after = core.live("Student").await.expect("live");
    assert_eq!(
        after[0].cells().get("name"),
        Some(&Cell::Text("ada2".into()))
    );
}

#[tokio::test]
async fn scoped() {
    let _hold = GATE.lock().await;
    let store = reset().await;
    let mut graph = Graph::new();
    graph.plug::<Ledger>().plug::<Entry>();
    let core = crate::support::boot(graph, store).await.expect("bind");

    let book = core.put("Ledger", &[("tag", "L01")]).await.expect("ledger");
    let seat = book.to_string();
    core.put("Entry", &[("note", "one"), ("ledger", &seat)])
        .await
        .expect("one");
    core.put("Entry", &[("note", "two"), ("ledger", &seat)])
        .await
        .expect("two");

    let held = core.live("Entry").await.expect("live");
    assert_eq!(held.len(), 2);
    assert_eq!(held[0].cells().get("index"), Some(&Cell::Int(1)));
    assert_eq!(held[1].cells().get("index"), Some(&Cell::Int(2)));
}

#[tokio::test]
async fn spare() {
    let _hold = GATE.lock().await;
    let store = reset().await;
    let mut graph = Graph::new();
    graph.plug::<Note>();
    let core = crate::support::boot(graph, store).await.expect("bind");

    core.put("Note", &[("tag", "N01")]).await.expect("bare");
    core.put("Note", &[("tag", "N02"), ("body", "held")])
        .await
        .expect("held");

    let held = core.live("Note").await.expect("live");
    assert_eq!(held.len(), 2);
    assert_eq!(
        held[1].cells().get("body"),
        Some(&Cell::Text("held".into()))
    );
}

#[tokio::test]
async fn foreign() {
    let _hold = GATE.lock().await;
    let store = reset().await;
    let mut graph = Graph::new();
    graph.plug::<Course>().plug::<Student>();
    let near = crate::support::boot(graph, store).await.expect("boot");
    let mut copy = Graph::new();
    copy.plug::<Course>().plug::<Student>();
    let far = keel::bind(copy, Postgres::at(url()).await.expect("far"))
        .await
        .expect("bind");
    let algo = near
        .put("Course", &[("code", "CS101")])
        .await
        .expect("course");
    assert_eq!(near.live("Course").await.expect("warm").len(), 1);
    assert_eq!(near.live("Course").await.expect("hot").len(), 1);

    far.put("Course", &[("code", "CS102")])
        .await
        .expect("far put");
    far.end("Course", algo).await.expect("far end");
    let rows = near.live("Course").await.expect("after");
    assert_eq!(rows.len(), 1);
    assert_ne!(rows[0].key(), algo);
}

#[tokio::test]
async fn vetoed() {
    use keel::Wire as _;
    let _hold = GATE.lock().await;
    let store = reset().await;
    let mut graph = Graph::new();
    graph.plug::<Course>().plug::<Student>();
    let core = crate::support::boot(graph, store).await.expect("bind");
    let mut axe = Postgres::at(url()).await.expect("axe");
    let seat = axe
        .rows(
            "SELECT tablename FROM pg_tables WHERE schemaname = current_schema() AND tablename LIKE '%pulse%'",
            &[],
        )
        .await
        .expect("seat")[0][0]
        .text();
    axe.script(&format!(
        "ALTER TABLE \"{seat}\" ADD CONSTRAINT veto CHECK (false) NOT VALID"
    ))
    .await
    .expect("veto");

    let put = core.put("Course", &[("code", "CS101")]).await;
    let courses = core.live("Course").await.expect("courses");
    assert!(put.is_err(), "write reported success without its pulse");
    assert!(courses.is_empty(), "write committed without its pulse");
}

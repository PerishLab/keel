use keel::atom::{string, url};
use keel::ddl;
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
fn wire() {
    let mut graph = Graph::new();
    graph.plug::<Class>().plug::<Student>();
    let db = keel::adapt::db::Sqlite::memory();
    let core = bind(graph, keel::adapt::http::Utopia, &db).expect("bind");
    let plan = core.plan();
    let student = plan.units().get("Student").expect("Student");
    assert_eq!(student.fields().len(), 2);
    assert_eq!(student.bonds().len(), 1);
    assert!(student.reign().expires());
    assert!(db.has("Student").expect("has Student"));
    assert!(db.has("Class").expect("has Class"));
    let link = ddl::join("Student", "classes");
    assert!(db.has(&link).expect("has join"));
    let cols = db.cols("Student").expect("cols");
    assert!(cols.iter().any(|c| c == ddl::KEY));
    assert!(cols.iter().any(|c| c == "nickname"));
    assert!(cols.iter().any(|c| c == "avatar"));
    assert!(cols.iter().any(|c| c == ddl::EXPIRES));
    assert!(cols.iter().any(|c| c == ddl::CREATED));
    assert!(cols.iter().any(|c| c == ddl::UPDATED));
}

#[test]
fn life() {
    let mut graph = Graph::new();
    graph.plug::<Class>().plug::<Student>();
    let db = keel::adapt::db::Sqlite::memory();
    let core = bind(graph, keel::adapt::http::Utopia, &db).expect("bind");
    let plan = core.plan();

    let a = db
        .put(
            plan,
            "Student",
            &[("nickname", "ada"), ("avatar", "https://a.example/a")],
        )
        .expect("put a");
    let b = db
        .put(
            plan,
            "Student",
            &[("nickname", "bob"), ("avatar", "https://b.example/b")],
        )
        .expect("put b");
    assert_ne!(a, b);

    let rows = db.live(plan, "Student").expect("live");
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|row| row.expires().is_none()));
    assert!(rows.iter().all(|row| row.created() > 0));
    assert_eq!(
        rows.iter()
            .find(|row| row.key() == a)
            .expect("a")
            .cells()
            .get("nickname")
            .map(String::as_str),
        Some("ada")
    );

    db.end(plan, "Student", a).expect("end a");
    let rows = db.live(plan, "Student").expect("live after end");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].key(), b);
    assert_eq!(
        rows[0].cells().get("nickname").map(String::as_str),
        Some("bob")
    );
}

#[test]
fn miss() {
    #[resource]
    struct Lone {
        #[relation(Ghost, n2m)]
        ghosts: string,
    }

    let mut graph = Graph::new();
    graph.plug::<Lone>();
    let db = keel::adapt::db::Sqlite::memory();
    let err = bind(graph, keel::adapt::http::Utopia, &db).expect_err("missing");
    assert!(matches!(err, keel::adapt::Error::Missing(_)));
}

use keel::adapt::db::Sqlite;
use keel::adapt::http;
use keel::atom::{string, url};
use keel::ddl;
use keel::resource;
use keel::{Cell, Ends, Graph, bind};

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
fn wire() {
    let mut graph = Graph::new();
    graph.plug::<Class>().plug::<Student>();
    let core = bind(graph, Sqlite::memory()).expect("bind");
    let student = core.plan().units().get("Student").expect("Student");
    assert_eq!(student.fields().len(), 2);
    assert_eq!(student.bonds().len(), 1);
    assert!(student.reign().expires());
    assert!(core.has("Student").expect("has Student"));
    assert!(core.has("Class").expect("has Class"));
    let link = ddl::join("Student", "classes");
    assert!(core.has(&link).expect("has join"));
    let cols = core.cols("Student").expect("cols");
    assert!(cols.iter().any(|c| c == ddl::KEY));
    assert!(cols.iter().any(|c| c == "nickname"));
    assert!(cols.iter().any(|c| c == "avatar"));
    assert!(cols.iter().any(|c| c == ddl::EXPIRES));
    assert!(cols.iter().any(|c| c == ddl::CREATED));
    assert!(cols.iter().any(|c| c == ddl::UPDATED));
    let paths = http::paths(core.plan());
    assert!(paths.iter().any(|p| p.route() == "/class"));
    assert!(paths.iter().any(|p| p.route() == "/student"));
}

#[test]
fn life() {
    let mut graph = Graph::new();
    graph.plug::<Class>().plug::<Student>();
    let core = bind(graph, Sqlite::memory()).expect("bind");

    let a = core
        .put(
            "Student",
            &[("nickname", "ada"), ("avatar", "https://a.example/a")],
        )
        .expect("put a");
    let b = core
        .put(
            "Student",
            &[("nickname", "bob"), ("avatar", "https://b.example/b")],
        )
        .expect("put b");
    assert_ne!(a, b);

    let rows = core.live("Student").expect("live");
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|row| row.expires().is_none()));
    assert!(rows.iter().all(|row| row.created() > 0));
    assert_eq!(
        rows.iter()
            .find(|row| row.key() == a)
            .expect("a")
            .cells()
            .get("nickname")
            .map(Cell::text),
        Some("ada")
    );

    core.set("Student", b, &[("nickname", "bobby")])
        .expect("set b");
    let rows = core.live("Student").expect("live after set");
    let bob = rows.iter().find(|row| row.key() == b).expect("b");
    assert_eq!(bob.cells().get("nickname").map(Cell::text), Some("bobby"));
    assert_eq!(
        bob.cells().get("avatar").map(Cell::text),
        Some("https://b.example/b")
    );
    assert!(bob.updated() >= bob.created());

    assert!(core.set("Student", b, &[]).is_err());
    assert!(core.set("Student", b, &[("missing", "x")]).is_err());
    assert!(core.set("Student", b, &[("id", "9")]).is_err());

    core.end("Student", a).expect("end a");
    let rows = core.live("Student").expect("live after end");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].key(), b);
    assert_eq!(
        rows[0].cells().get("nickname").map(Cell::text),
        Some("bobby")
    );
    assert!(core.set("Student", a, &[("nickname", "gone")]).is_err());
}

#[test]
fn bond() {
    let mut graph = Graph::new();
    graph.plug::<Class>().plug::<Student>();
    let core = bind(graph, Sqlite::memory()).expect("bind");

    let student = core
        .put(
            "Student",
            &[("nickname", "ada"), ("avatar", "https://a.example/a")],
        )
        .expect("student");
    let math = core.put("Class", &[("title", "math")]).expect("math");
    let art = core.put("Class", &[("title", "art")]).expect("art");

    let t1 = core
        .tie(
            "Student",
            "classes",
            Ends {
                left: student,
                right: math,
            },
            &[],
        )
        .expect("tie math");
    let t2 = core
        .tie(
            "Student",
            "classes",
            Ends {
                left: student,
                right: art,
            },
            &[],
        )
        .expect("tie art");
    assert_ne!(t1, t2);

    let ties = core.ties("Student", "classes", student).expect("ties");
    assert_eq!(ties.len(), 2);
    assert!(ties.iter().all(|tie| tie.left() == student));
    assert!(ties.iter().any(|tie| tie.right() == math));
    assert!(ties.iter().any(|tie| tie.right() == art));

    core.cut("Student", "classes", t1).expect("cut math");
    let ties = core
        .ties("Student", "classes", student)
        .expect("ties after cut");
    assert_eq!(ties.len(), 1);
    assert_eq!(ties[0].right(), art);
}

#[test]
fn miss() {
    #[resource]
    struct Lone {
        #[relation(Ghost, many2many)]
        ghosts: string,
    }

    let mut graph = Graph::new();
    graph.plug::<Lone>();
    match bind(graph, Sqlite::memory()) {
        Ok(_) => panic!("expected missing target"),
        Err(keel::adapt::Error::Missing(_)) => {}
        Err(err) => panic!("unexpected {err}"),
    }
}

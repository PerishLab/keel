use keel::atom::{string, url};
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
    let core = bind(graph, keel::adapt::http::Utopia, keel::adapt::db::Postgres).expect("bind");
    let plan = core.plan();
    let student = plan.units().get("Student").expect("Student");
    assert_eq!(student.fields().len(), 2);
    assert_eq!(student.bonds().len(), 1);
    assert!(student.reign().expires());
    assert!(student.reign().created());
    assert!(student.reign().updated());
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
    let err =
        bind(graph, keel::adapt::http::Utopia, keel::adapt::db::Postgres).expect_err("missing");
    assert!(matches!(err, keel::adapt::Error::Missing(_)));
}

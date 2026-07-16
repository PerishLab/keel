use keel::adapt::db::Sqlite;
use keel::atom::{int, string};
use keel::resource;
use keel::{Cell, Graph, bind};

#[resource]
struct Org {
    #[field(string, unique)]
    slug: string,
}

#[resource]
struct Repo {
    #[field(string, unique = org)]
    name: string,
    #[relation(Org, many2one)]
    org: Org,
}

#[test]
fn sole() {
    let mut graph = Graph::new();
    graph.plug::<Org>();
    let core = bind(graph, Sqlite::memory()).expect("bind");
    let lab = core.put("Org", &[("slug", "lab")]).expect("lab");
    assert!(core.put("Org", &[("slug", "lab")]).is_err());
    core.put("Org", &[("slug", "hub")]).expect("hub");
    core.end("Org", lab).expect("end lab");
    core.put("Org", &[("slug", "lab")]).expect("retake");
}

#[test]
fn per() {
    let mut graph = Graph::new();
    graph.plug::<Org>().plug::<Repo>();
    let core = bind(graph, Sqlite::memory()).expect("bind");
    let lab = core.put("Org", &[("slug", "lab")]).expect("lab");
    let hub = core.put("Org", &[("slug", "hub")]).expect("hub");

    let keel = core
        .put("Repo", &[("name", "keel"), ("org", &lab.to_string())])
        .expect("keel");
    assert!(
        core.put("Repo", &[("name", "keel"), ("org", &lab.to_string())])
            .is_err()
    );
    let twin = core
        .put("Repo", &[("name", "keel"), ("org", &hub.to_string())])
        .expect("same name other org");

    core.put("Repo", &[("name", "mast"), ("org", &lab.to_string())])
        .expect("mast");
    assert!(core.set("Repo", keel, &[("name", "mast")]).is_err());
    core.set("Repo", keel, &[("name", "keel")])
        .expect("self ok");

    assert!(
        core.set("Repo", twin, &[("org", &lab.to_string())])
            .is_err()
    );
    core.set("Repo", twin, &[("name", "sail"), ("org", &lab.to_string())])
        .expect("rename and move");
}

#[resource]
struct Issue {
    #[field(string)]
    title: string,
    #[field(serial, scope = repo)]
    index: int,
    #[relation(Repo, many2one)]
    repo: Repo,
}

#[test]
fn tally() {
    let mut graph = Graph::new();
    graph.plug::<Org>().plug::<Repo>().plug::<Issue>();
    let core = bind(graph, Sqlite::memory()).expect("bind");
    let lab = core.put("Org", &[("slug", "lab")]).expect("lab");
    let keel = core
        .put("Repo", &[("name", "keel"), ("org", &lab.to_string())])
        .expect("keel");
    let mast = core
        .put("Repo", &[("name", "mast"), ("org", &lab.to_string())])
        .expect("mast");

    let one = core
        .put("Issue", &[("title", "a"), ("repo", &keel.to_string())])
        .expect("one");
    let two = core
        .put("Issue", &[("title", "b"), ("repo", &keel.to_string())])
        .expect("two");
    let side = core
        .put("Issue", &[("title", "c"), ("repo", &mast.to_string())])
        .expect("side");

    let grab = |key: i64| {
        let rows = core.live("Issue").expect("live");
        rows.iter()
            .find(|row| row.key() == key)
            .and_then(|row| row.cells().get("index").cloned())
    };
    assert_eq!(grab(one), Some(Cell::Int(1)));
    assert_eq!(grab(two), Some(Cell::Int(2)));
    assert_eq!(grab(side), Some(Cell::Int(1)));

    core.end("Issue", two).expect("end two");
    let three = core
        .put("Issue", &[("title", "d"), ("repo", &keel.to_string())])
        .expect("three");
    assert_eq!(grab(three), Some(Cell::Int(3)));

    assert!(
        core.put(
            "Issue",
            &[("title", "x"), ("index", "9"), ("repo", &keel.to_string())],
        )
        .is_err()
    );
    assert!(core.set("Issue", one, &[("index", "9")]).is_err());

    let pack = core
        .query(&format!(
            r#"from Issue where repo = "{keel}" order by index desc"#
        ))
        .expect("order");
    assert_eq!(pack.rows()[0].key(), three);
}

#[resource]
struct React {
    #[field(string, unique = (fan, issue))]
    emoji: string,
    #[relation(Org, many2one, root)]
    fan: Org,
    #[relation(Repo, many2one)]
    issue: Repo,
}

#[test]
fn composite() {
    let mut graph = Graph::new();
    graph.plug::<Org>().plug::<Repo>().plug::<React>();
    let core = bind(graph, Sqlite::memory()).expect("bind");
    let lab = core.put("Org", &[("slug", "lab")]).expect("lab");
    let ada = core.put("Org", &[("slug", "ada")]).expect("ada");
    let one = core
        .put("Repo", &[("name", "one"), ("org", &lab.to_string())])
        .expect("one");
    let two = core
        .put("Repo", &[("name", "two"), ("org", &lab.to_string())])
        .expect("two");

    let seed = |emoji: &str, actor: i64, issue: i64| {
        core.put(
            "React",
            &[
                ("emoji", emoji),
                ("fan", &actor.to_string()),
                ("issue", &issue.to_string()),
            ],
        )
    };
    seed("up", lab, one).expect("first");
    seed("tada", lab, one).expect("same pair other emoji");
    seed("up", ada, one).expect("other actor same emoji");
    seed("up", lab, two).expect("other issue same emoji");
    assert!(seed("up", lab, one).is_err());

    let react = seed("heart", lab, two).expect("live");
    core.end("React", react).expect("undo");
    seed("heart", lab, two).expect("re-react after undo");
}

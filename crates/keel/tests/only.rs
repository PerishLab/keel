use keel::adapt::db::Sqlite;
use keel::atom::string;
use keel::resource;
use keel::{Graph, bind};

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

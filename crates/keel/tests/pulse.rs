use keel::adapt::db::Sqlite;
use keel::atom::string;
use keel::resource;
use keel::{Cell, Ends, Graph, bind};

#[resource]
struct Room {
    #[field(string)]
    name: string,
}

#[resource]
struct Actor {
    #[field(string, unique)]
    login: string,
    #[relation(Room, many2many)]
    rooms: Room,
}

fn text(row: &keel::Row, name: &str) -> String {
    row.cells().get(name).map(Cell::show).unwrap_or_default()
}

#[test]
fn beat() {
    let mut graph = Graph::new();
    graph.plug::<Room>().plug::<Actor>();
    let core = bind(graph, Sqlite::memory()).expect("bind");
    let ada = core.put("Actor", &[("login", "ada")]).expect("ada");
    let den = core.put("Room", &[("name", "den")]).expect("den");
    core.set("Actor", ada, &[("login", "ada2")]).expect("set");
    let tie = core
        .tie(
            "Actor",
            "rooms",
            Ends {
                left: ada,
                right: den,
            },
            &[],
        )
        .expect("tie");
    core.cut("Actor", "rooms", tie).expect("cut");
    core.end("Room", den).expect("end");

    let flow = core.flow(0).expect("flow");
    let seen: Vec<(String, String)> = flow
        .iter()
        .map(|row| (text(row, "verb"), text(row, "unit")))
        .collect();
    assert_eq!(
        seen,
        vec![
            ("put".into(), "actor".into()),
            ("put".into(), "room".into()),
            ("set".into(), "actor".into()),
            ("tie".into(), "actor.rooms".into()),
            ("cut".into(), "actor.rooms".into()),
            ("end".into(), "room".into()),
        ]
    );
    assert!(flow.iter().all(|row| text(row, "who") == "sudo"));

    let last = flow.last().expect("last").key();
    assert_eq!(core.flow(last).expect("tail").len(), 0);
}

#[test]
fn blame() {
    let mut graph = Graph::new();
    graph.plug::<Room>().plug::<Actor>();
    let core = bind(graph, Sqlite::memory()).expect("bind");
    let ada = core.put("Actor", &[("login", "ada")]).expect("ada");
    core.sudo()
        .put(
            "@grant",
            &[
                ("who", &ada.to_string()),
                ("verb", "put"),
                ("unit", "Actor"),
                ("scope", "all"),
            ],
        )
        .expect("seed");
    let mark = core.flow(0).expect("flow").last().expect("g").key();

    core.of(ada)
        .put("Actor", &[("login", "eve")])
        .expect("op put");
    let fresh = core.flow(mark).expect("flow");
    assert_eq!(fresh.len(), 2);
    assert_eq!(text(&fresh[0], "unit"), "actor");
    assert_eq!(text(&fresh[0], "who"), ada.to_string());
    assert_eq!(text(&fresh[1], "unit"), "@grant");
    assert_eq!(text(&fresh[1], "who"), ada.to_string());

    assert!(core.put("@pulse", &[("verb", "x")]).is_err());
    assert!(core.end("@pulse", 1).is_err());
}

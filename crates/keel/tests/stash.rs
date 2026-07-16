use keel::adapt::db::Sqlite;
use keel::atom::string;
use keel::resource;
use keel::{Ends, Graph, bind};
use std::time::{SystemTime, UNIX_EPOCH};

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

fn tick() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[test]
fn unseen() {
    let mut graph = Graph::new();
    graph.plug::<Room>().plug::<Actor>();
    let core = bind(graph, Sqlite::memory()).expect("bind");
    let ada = core.put("Actor", &[("login", "ada")]).expect("ada");

    let one = core.query("from Actor").expect("q1");
    let two = core.query("from Actor").expect("q2");
    assert_eq!(one, two);

    core.put("Actor", &[("login", "bob")]).expect("bob");
    let three = core.query("from Actor").expect("q3");
    assert_eq!(three.rows().len(), 2);

    let den = core.put("Room", &[("name", "den")]).expect("den");
    let before = core.query("from Actor link rooms").expect("link");
    assert_eq!(before.bond("actor.rooms").expect("bag").len(), 0);
    core.tie(
        "Actor",
        "rooms",
        Ends {
            left: ada,
            right: den,
        },
        &[],
    )
    .expect("tie");
    let after = core.query("from Actor link rooms").expect("link2");
    assert_eq!(after.bond("actor.rooms").expect("bag").len(), 1);

    let ties = core.ties("Actor", "rooms", ada).expect("ties");
    core.cut("Actor", "rooms", ties[0].key()).expect("cut");
    core.end("Room", den).expect("end");
    let gone = core.query("from Actor link rooms").expect("link3");
    assert_eq!(gone.bond("actor.rooms").expect("bag").len(), 0);
}

#[test]
fn horizon() {
    let mut graph = Graph::new();
    graph.plug::<Room>().plug::<Actor>();
    let core = bind(graph, Sqlite::memory()).expect("bind");
    let den = core.put("Room", &[("name", "den")]).expect("den");
    core.lease("Room", den, tick() + 1).expect("lease");

    let warm = core.query("from Room").expect("warm");
    assert_eq!(warm.rows().len(), 1);
    std::thread::sleep(std::time::Duration::from_secs(2));
    let cold = core.query("from Room").expect("cold");
    assert_eq!(cold.rows().len(), 0);
}

#[test]
fn twin() {
    let mut graph = Graph::new();
    graph.plug::<Room>().plug::<Actor>();
    let live = bind(graph, Sqlite::memory()).expect("bind");
    let mut copy = Graph::new();
    copy.plug::<Room>().plug::<Actor>();
    let bare = bind(copy, Sqlite::memory()).expect("bind").bare();

    for core in [&live, &bare] {
        core.put("Actor", &[("login", "ada")]).expect("ada");
        core.put("Actor", &[("login", "bob")]).expect("bob");
        core.set("Actor", 1, &[("login", "ada2")]).expect("set");
    }
    let q = r#"from Actor where login != "zoe" order by login desc"#;
    let one = live.query(q).expect("live");
    let two = bare.query(q).expect("bare");
    assert_eq!(one, two);
    assert_eq!(
        live.query("from Actor count").expect("c1"),
        bare.query("from Actor count").expect("c2")
    );
}

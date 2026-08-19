use keel::adapt::db::Sqlite;
use keel::atom::string;
use keel::resource;
use keel::{Ends, Graph};
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

#[tokio::test]
async fn unseen() {
    let mut graph = Graph::new();
    graph.plug::<Room>().plug::<Actor>();
    let core = crate::support::boot(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind");
    let ada = core.put("Actor", &[("login", "ada")]).await.expect("ada");

    let one = core.query("from Actor").await.expect("q1");
    let two = core.query("from Actor").await.expect("q2");
    assert_eq!(one, two);

    core.put("Actor", &[("login", "bob")]).await.expect("bob");
    let three = core.query("from Actor").await.expect("q3");
    assert_eq!(three.rows().len(), 2);

    let den = core.put("Room", &[("name", "den")]).await.expect("den");
    let before = core.query("from Actor link rooms").await.expect("link");
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
    .await
    .expect("tie");
    let after = core.query("from Actor link rooms").await.expect("link2");
    assert_eq!(after.bond("actor.rooms").expect("bag").len(), 1);

    let ties = core.ties("Actor", "rooms", ada).await.expect("ties");
    core.cut("Actor", "rooms", ties[0].key())
        .await
        .expect("cut");
    core.end("Room", den).await.expect("end");
    let gone = core.query("from Actor link rooms").await.expect("link3");
    assert_eq!(gone.bond("actor.rooms").expect("bag").len(), 0);
}

#[tokio::test]
async fn horizon() {
    let mut graph = Graph::new();
    graph.plug::<Room>().plug::<Actor>();
    let core = crate::support::boot(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind");
    let den = core.put("Room", &[("name", "den")]).await.expect("den");
    core.lease("Room", den, tick() + 2).await.expect("lease");

    let warm = core.query("from Room").await.expect("warm");
    assert_eq!(warm.rows().len(), 1);
    std::thread::sleep(std::time::Duration::from_secs(3));
    let cold = core.query("from Room").await.expect("cold");
    assert_eq!(cold.rows().len(), 0);
}

#[tokio::test]
async fn twin() {
    let mut graph = Graph::new();
    graph.plug::<Room>().plug::<Actor>();
    let live = crate::support::boot(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind");
    let mut copy = Graph::new();
    copy.plug::<Room>().plug::<Actor>();
    let bare = crate::support::boot(copy, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind")
        .bare();

    for core in [&live, &bare] {
        core.put("Actor", &[("login", "ada")]).await.expect("ada");
        core.put("Actor", &[("login", "bob")]).await.expect("bob");
        core.set("Actor", 1, &[("login", "ada2")])
            .await
            .expect("set");
    }
    let q = r#"from Actor where login != "zoe" order by login desc"#;
    let one = live.query(q).await.expect("live");
    let two = bare.query(q).await.expect("bare");
    assert_eq!(shape(&one), shape(&two));
    assert_eq!(
        live.query("from Actor count").await.expect("c1"),
        bare.query("from Actor count").await.expect("c2")
    );
}

fn shape(pack: &keel::Pack) -> Vec<(i64, std::collections::BTreeMap<String, keel::Cell>)> {
    pack.rows()
        .iter()
        .map(|row| (row.key(), row.cells().clone()))
        .collect()
}

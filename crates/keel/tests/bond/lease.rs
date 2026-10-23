use keel::adapt::db::Sqlite;
use keel::atom::string;
use keel::resource;
use keel::{Ends, Graph};

#[resource]
struct Room {
    #[field(string)]
    name: string,
    #[relation(Guest, many2many)]
    guests: Guest,
}

#[resource]
struct Guest {
    #[field(string, unique)]
    name: string,
    #[relation(Room, many2one, opt)]
    home: Room,
}

#[resource]
struct Desk {
    #[field(string)]
    spot: string,
    #[relation(Guest, one2one)]
    owner: Guest,
}

fn late() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
        + 3600
}

#[tokio::test]
async fn ride() {
    let mut graph = Graph::new();
    graph.plug::<Room>().plug::<Guest>();
    let core = crate::support::boot(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind");
    let at = late();

    let room = core.put("Room", &[("name", "den")]).await.expect("room");
    core.lease("Room", room, at).await.expect("lease");
    let rows = core.live("Room").await.expect("live");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].expires(), Some(at));

    core.lease("Room", room, at + 60).await.expect("renew");
    let rows = core.live("Room").await.expect("live");
    assert_eq!(rows[0].expires(), Some(at + 60));

    core.end("Room", room).await.expect("revoke");
    assert_eq!(core.live("Room").await.expect("live").len(), 0);
    assert!(core.lease("Room", room, at).await.is_err());
    assert!(core.end("Room", room).await.is_err());
}

#[tokio::test]
async fn strict() {
    let mut graph = Graph::new();
    graph.plug::<Room>().plug::<Guest>();
    let core = crate::support::boot(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind");
    let at = late();

    let room = core.put("Room", &[("name", "den")]).await.expect("room");
    let ada = core
        .put("Guest", &[("name", "ada"), ("home", "")])
        .await
        .expect("ada");

    assert!(core.lease("Room", room, at - 7200).await.is_err());

    core.tie(
        "Room",
        "guests",
        Ends {
            left: room,
            right: ada,
        },
        &[],
    )
    .await
    .expect("tie");
    core.lease("Room", room, at).await.expect("lease");
    let ties = core.ties("Room", "guests", room).await.expect("ties");
    assert_eq!(ties.len(), 1);
    core.cut("Room", "guests", ties[0].key())
        .await
        .expect("cut");
    assert!(
        core.tie(
            "Room",
            "guests",
            Ends {
                left: room,
                right: ada,
            },
            &[],
        )
        .await
        .is_err()
    );
    assert!(
        core.set("Guest", ada, &[("home", &room.to_string())])
            .await
            .is_err()
    );

    let dup = core.put("Guest", &[("name", "ada"), ("home", "")]).await;
    assert!(dup.is_err());
}

#[tokio::test]
async fn yields() {
    let mut graph = Graph::new();
    graph.plug::<Room>().plug::<Guest>().plug::<Desk>();
    let core = crate::support::boot(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind");
    let at = late();

    let bea = core
        .put("Guest", &[("name", "bea"), ("home", "")])
        .await
        .expect("bea");
    core.lease("Guest", bea, at).await.expect("lease bea");
    let heir = core
        .put("Guest", &[("name", "bea"), ("home", "")])
        .await
        .expect("a leased guest holds no key");
    let cid = core
        .put("Guest", &[("name", "cid"), ("home", "")])
        .await
        .expect("cid");
    assert!(
        core.set("Guest", cid, &[("name", "bea")]).await.is_err(),
        "a guest with no expiry still holds its key"
    );

    let dee = core
        .put("Guest", &[("name", "dee"), ("home", "")])
        .await
        .expect("dee");
    core.lease("Guest", dee, at).await.expect("lease dee");
    core.set("Guest", cid, &[("name", "dee")])
        .await
        .expect("an edit to a leased guest's name");

    let desk = core
        .put("Desk", &[("spot", "a1"), ("owner", &heir.to_string())])
        .await
        .expect("desk");
    core.lease("Desk", desk, at).await.expect("lease desk");
    core.put("Desk", &[("spot", "a2"), ("owner", &heir.to_string())])
        .await
        .expect("a leased desk holds no one2one key");
    assert!(
        core.put("Desk", &[("spot", "a3"), ("owner", &heir.to_string())])
            .await
            .is_err(),
        "a desk with no expiry still holds its one2one key"
    );
}

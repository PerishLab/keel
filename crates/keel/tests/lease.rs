use keel::adapt::db::Sqlite;
use keel::atom::string;
use keel::resource;
use keel::{Ends, Graph, bind};

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
    let core = bind(graph, Sqlite::memory().await.expect("db"))
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
    let core = bind(graph, Sqlite::memory().await.expect("db"))
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
    assert!(core.lease("Room", room, at).await.is_err());
    let ties = core.ties("Room", "guests", room).await.expect("ties");
    core.cut("Room", "guests", ties[0].key())
        .await
        .expect("cut");

    core.lease("Room", room, at).await.expect("lease");
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

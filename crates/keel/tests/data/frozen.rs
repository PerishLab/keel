use keel::adapt::db::Sqlite;
use keel::atom::string;
use keel::{Graph, bind, resource};

#[resource(frozen)]
struct Event {
    #[field(string, unique)]
    tag: string,
    #[field(string, opt)]
    note: string,
}

#[tokio::test]
async fn fact() {
    let mut graph = Graph::new();
    graph.plug::<Event>();
    let core = bind(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind");
    let key = core.put("Event", &[("tag", "evt_one")]).await.expect("put");
    assert!(core.set("Event", key, &[("note", "late")]).await.is_err());
    assert!(core.unset("Event", key, &["note"]).await.is_err());
    core.end("Event", key).await.expect("end");
}

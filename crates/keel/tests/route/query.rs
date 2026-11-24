use crate::web::{Rig, bag, root};
use axum::http::StatusCode;
use keel::Graph;
use keel::atom::string;
use keel::resource;
use serde_json::json;

#[resource]
struct Note {
    #[field(string)]
    body: string,
}

async fn rig() -> Rig {
    let mut graph = Graph::new();
    graph.plug::<Note>();
    Rig::open(graph).await
}

#[tokio::test]
async fn kinds() {
    let rig = rig().await;
    rig.made("/note", json!({ "body": "first" })).await;
    let asked = rig
        .post("/query/relation", json!({ "q": "from Note" }))
        .await;
    assert_eq!(asked.status, StatusCode::OK, "{}", asked.body);
    assert_eq!(root(&asked.body), "note");
    assert_eq!(bag(&asked.body, "note").len(), 1);
    let alias = rig.post("/query", json!({ "q": "from Note" })).await;
    assert_eq!(alias.status, StatusCode::OK, "{}", alias.body);
    assert_eq!(alias.body, asked.body);
}

#[tokio::test]
async fn unknown() {
    let rig = rig().await;
    let kind = rig.post("/query/world", json!({ "q": "from Note" })).await;
    assert_eq!(kind.status, StatusCode::NOT_FOUND);
    assert!(
        kind.body["error"]
            .as_str()
            .unwrap()
            .contains("no such kind")
    );
    let unit = rig
        .post("/query/relation", json!({ "q": "from Absent" }))
        .await;
    assert_eq!(unit.status, StatusCode::NOT_FOUND);
    let closed = rig
        .post("/query/relation", json!({ "q": "from @seal" }))
        .await;
    assert_eq!(closed.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn malformed() {
    let rig = rig().await;
    let syntax = rig.post("/query/relation", json!({ "q": "from" })).await;
    assert_eq!(syntax.status, StatusCode::BAD_REQUEST, "{}", syntax.body);
    let absent = rig.post("/query/relation", json!({})).await;
    assert_eq!(absent.status, StatusCode::BAD_REQUEST, "{}", absent.body);
    assert!(absent.body["error"].as_str().unwrap().contains("q string"));
    let alias = rig.post("/query", json!({ "q": "from" })).await;
    assert_eq!(alias.status, StatusCode::BAD_REQUEST, "{}", alias.body);
}

use crate::web::{Rig, bag};
use axum::http::StatusCode;
use keel::Graph;
use keel::atom::string;
use keel::resource;
use serde_json::{Value, json};

#[resource]
struct Topic {
    #[field(string)]
    name: string,
}

#[resource]
struct Reader {
    #[field(string)]
    name: string,
    #[relation(Topic, many2many, weight = string)]
    topics: Topic,
}

async fn rig() -> Rig {
    let mut graph = Graph::new();
    graph.plug::<Topic>().plug::<Reader>();
    Rig::open(graph).await
}

fn keys(value: &Value) -> Vec<&str> {
    value
        .as_object()
        .expect("object")
        .keys()
        .map(String::as_str)
        .collect()
}

#[tokio::test]
async fn fields() {
    let rig = rig().await;
    let topic = rig.made("/topic", json!({ "name": "rust" })).await;
    let reader = rig.made("/reader", json!({ "name": "ada" })).await;
    let tie = rig
        .post(
            &format!("/reader/{reader}/topics"),
            json!({ "right": topic, "weight": "high" }),
        )
        .await;
    assert_eq!(tie.status, StatusCode::CREATED, "{}", tie.body);

    let one = rig.get(&format!("/topic/{topic}")).await;
    assert_eq!(keys(&one.body), ["id", "name"]);
    let list = rig.get("/topic").await;
    assert_eq!(keys(&list.rows()[0]), ["id", "name"]);
    let edited = rig
        .patch(&format!("/topic/{topic}"), json!({ "name": "go" }))
        .await;
    assert_eq!(keys(&edited.body), ["id", "name"]);

    let pack = rig
        .ask(&format!(r#"from Reader where id = "{reader}" link topics"#))
        .await;
    assert_eq!(keys(&bag(&pack, "reader")[0]), ["id", "name"]);
    let ties = bag(&pack, "reader.topics");
    assert_eq!(keys(&ties[0]), ["id", "left", "right", "weight"]);
}

#[tokio::test]
async fn units() {
    let rig = rig().await;
    for unit in ["@seal", "@pulse"] {
        let list = rig.get(&format!("/{unit}")).await;
        assert_eq!(list.status, StatusCode::NOT_FOUND, "{unit}");
        let one = rig.get(&format!("/{unit}/1")).await;
        assert_eq!(one.status, StatusCode::NOT_FOUND, "{unit}");
        let made = rig.post(&format!("/{unit}"), json!({ "verb": "x" })).await;
        assert_eq!(made.status, StatusCode::NOT_FOUND, "{unit}");
        let set = rig
            .patch(&format!("/{unit}/1"), json!({ "verb": "x" }))
            .await;
        assert_eq!(set.status, StatusCode::NOT_FOUND, "{unit}");
        let end = rig.end(&format!("/{unit}/1")).await;
        assert_eq!(end.status, StatusCode::NOT_FOUND, "{unit}");
        let asked = rig
            .post("/query", json!({ "q": format!("from {unit}") }))
            .await;
        assert_eq!(asked.status, StatusCode::NOT_FOUND, "{unit}");
        let batch = rig
            .post(
                "/batch",
                json!({ "deeds": [{ "verb": "end", "unit": unit, "id": 1 }] }),
            )
            .await;
        assert_eq!(batch.status, StatusCode::NOT_FOUND, "{unit}");
        assert!(!list.body.to_string().contains("hash"));
    }

    let grants = rig.get("/@grant").await;
    assert_eq!(grants.status, StatusCode::OK, "{}", grants.body);
    assert_eq!(grants.rows().len(), 1);
    let asked = rig.post("/query", json!({ "q": "from @grant" })).await;
    assert_eq!(asked.status, StatusCode::OK, "{}", asked.body);
}

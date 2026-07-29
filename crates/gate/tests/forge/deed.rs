use crate::rig::{Forge, bag, forge};
use axum::http::StatusCode;
use serde_json::{Value, json};

pub struct Deck {
    pub rig: Forge,
    pub ada: i64,
    pub bob: i64,
    pub home: i64,
    pub twin: i64,
}

pub async fn deck() -> Deck {
    let rig = forge().await;
    let crown: [(&str, &str); 1] = [("authorization", &rig.crown)];
    let ada = rig.made("/actor", json!({ "login": "ada" }), &crown).await;
    let bob = rig.made("/actor", json!({ "login": "bob" }), &crown).await;
    let held = json!({ "name": "keel", "visibility": "public", "owner": ada });
    let home = rig.made("/repo", held, &crown).await;
    let held = json!({ "name": "keel", "visibility": "public", "owner": bob });
    let twin = rig.made("/repo", held, &crown).await;
    Deck {
        rig,
        ada,
        bob,
        home,
        twin,
    }
}

impl Deck {
    pub fn crown(&self) -> [(&str, &str); 1] {
        [("authorization", &self.rig.crown)]
    }

    pub async fn file(&self, title: &str, repo: i64, author: i64) -> i64 {
        let sent = json!({ "title": title, "closed": false, "repo": repo, "author": author });
        self.rig.made("/issue", sent, &self.crown()).await
    }
}

fn at(rows: &[Value], key: i64) -> Value {
    rows.iter()
        .find(|row| row["id"].as_i64() == Some(key))
        .cloned()
        .expect("row")
}

#[tokio::test]
async fn sudo() {
    let deck = deck().await;
    let head: [(&str, &str); 1] = [("authorization", "sudo feedbead")];
    let bad = deck
        .rig
        .post("/actor", json!({ "login": "eve" }), &head)
        .await;
    assert_eq!(bad.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn unique() {
    let deck = deck().await;
    let dup = deck
        .rig
        .post("/actor", json!({ "login": "ada" }), &deck.crown())
        .await;
    assert_eq!(dup.status, StatusCode::CONFLICT);
    let held = json!({ "name": "keel", "visibility": "public", "owner": deck.ada });
    let twin = deck.rig.post("/repo", held, &deck.crown()).await;
    assert_eq!(twin.status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn serial() {
    let deck = deck().await;
    let one = deck.file("a", deck.home, deck.ada).await;
    let two = deck.file("b", deck.home, deck.bob).await;
    let side = deck.file("c", deck.twin, deck.bob).await;
    let rows = bag(
        &deck.rig.ask("from Issue", &deck.crown()).await,
        "repo:issue",
    );
    assert_eq!(at(&rows, one)["index"], 1);
    assert_eq!(at(&rows, two)["index"], 2);
    assert_eq!(at(&rows, side)["index"], 1);
    assert_eq!(at(&rows, one)["closed"], false);
    let sent = json!({
        "title": "x",
        "index": 9,
        "closed": false,
        "repo": deck.home,
        "author": deck.ada
    });
    let owned = deck.rig.post("/issue", sent, &deck.crown()).await;
    assert_eq!(owned.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn state() {
    let deck = deck().await;
    let one = deck.file("a", deck.home, deck.ada).await;
    deck.file("b", deck.home, deck.bob).await;
    let path = format!("/issue/{one}");
    let shut = deck
        .rig
        .patch(&path, json!({ "closed": true }), &deck.crown())
        .await;
    assert_eq!(shut.status, StatusCode::OK, "{}", shut.body);
    assert_eq!(shut.body["closed"], true);
    let q = format!(
        r#"from Issue where repo = "{}" and closed = "false""#,
        deck.home
    );
    let open = bag(&deck.rig.ask(&q, &deck.crown()).await, "repo:issue");
    assert_eq!(open.len(), 1);
}

#[tokio::test]
async fn count() {
    let deck = deck().await;
    let path = format!("/actor/{}/stars", deck.ada);
    deck.rig
        .made(&path, json!({ "right": deck.twin }), &deck.crown())
        .await;
    let path = format!("/actor/{}/stars", deck.bob);
    deck.rig
        .made(&path, json!({ "right": deck.twin }), &deck.crown())
        .await;
    let q = format!(r#"from Actor where stars has "{}" count"#, deck.twin);
    let pack = deck.rig.ask(&q, &deck.crown()).await;
    assert_eq!(pack["count"], 2);
    assert!(pack.get("bags").is_none());
    assert!(pack["root"].is_string());
    let sent = json!({ "q": "from Issue count limit 1" });
    let lone = deck.rig.post("/query", sent, &deck.crown()).await;
    assert_eq!(lone.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn blocked() {
    let deck = deck().await;
    deck.file("a", deck.home, deck.ada).await;
    let path = format!("/repo/{}", deck.home);
    assert_eq!(
        deck.rig.end(&path, &deck.crown()).await.status,
        StatusCode::CONFLICT
    );
    let path = format!("/actor/{}", deck.ada);
    assert_eq!(
        deck.rig.end(&path, &deck.crown()).await.status,
        StatusCode::CONFLICT
    );
}

#[tokio::test]
async fn order() {
    let deck = deck().await;
    deck.file("a", deck.home, deck.ada).await;
    deck.file("b", deck.home, deck.bob).await;
    let q = format!(
        r#"from Issue where repo = "{}" order by index desc"#,
        deck.home
    );
    let rows = bag(&deck.rig.ask(&q, &deck.crown()).await, "repo:issue");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["index"], 2);
}

#[tokio::test]
async fn journal() {
    let deck = deck().await;
    let flow = deck.rig.core.flow(0).await.expect("flow");
    let held: Vec<_> = flow
        .iter()
        .filter(|row| row.text("who") == Some("sudo"))
        .collect();
    assert!(!held.is_empty());
    assert!(
        held.iter()
            .any(|row| row.text("unit") == Some("actor") && row.text("verb") == Some("put"))
    );
}

use crate::deed::{Deck, deck};
use axum::http::StatusCode;
use serde_json::json;

struct Born {
    deck: Deck,
    erin: i64,
    key: String,
}

async fn born() -> Born {
    let deck = deck().await;
    deck.rig.grant("anon", "put", ("Actor", "all")).await;
    let made = deck
        .rig
        .post("/register", json!({ "login": "erin" }), &[])
        .await;
    assert_eq!(made.status, StatusCode::CREATED, "{}", made.body);
    let key = made.body["token"].as_str().expect("token").to_string();
    assert!(key.len() >= 32);
    Born {
        erin: made.id(),
        key,
        deck,
    }
}

fn seat(baked: &str) -> String {
    let held = baked.split(';').next().expect("cookie");
    assert!(held.starts_with("session="));
    held.to_string()
}

#[tokio::test]
async fn minted() {
    let born = born().await;
    let bear = format!("token {}", born.key);
    let head: [(&str, &str); 1] = [("authorization", &bear)];
    let path = format!("/actor/{}", born.erin);
    assert_eq!(born.deck.rig.get(&path, &head).await.status, StatusCode::OK);
    let blind = born.deck.rig.get(&path, &[]).await;
    assert_eq!(blind.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn leased() {
    let born = born().await;
    let rig = &born.deck.rig;
    let out = rig.post("/login", json!({ "token": born.key }), &[]).await;
    assert_eq!(out.status, StatusCode::CREATED, "{}", out.body);
    let jar = seat(&out.cookie());
    let head: [(&str, &str); 1] = [("cookie", &jar)];
    let path = format!("/actor/{}", born.erin);
    assert_eq!(rig.get(&path, &head).await.status, StatusCode::OK);
    let veiled = rig.get("/session", &head).await;
    assert_eq!(veiled.status, StatusCode::NOT_FOUND);
    let asked = rig
        .post("/query", json!({ "q": "from Session" }), &head)
        .await;
    assert_eq!(asked.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn ended() {
    let born = born().await;
    let rig = &born.deck.rig;
    let out = rig.post("/login", json!({ "token": born.key }), &[]).await;
    let jar = seat(&out.cookie());
    let head: [(&str, &str); 1] = [("cookie", &jar)];
    let gone = rig.post("/logout", json!({}), &head).await;
    assert_eq!(gone.status, StatusCode::NO_CONTENT, "{}", gone.body);
    let path = format!("/actor/{}", born.erin);
    assert_eq!(rig.get(&path, &head).await.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn revoked() {
    let born = born().await;
    let rig = &born.deck.rig;
    let bear = format!("token {}", born.key);
    let head: [(&str, &str); 1] = [("authorization", &bear)];
    let veiled = rig.get("/token", &head).await;
    assert_eq!(veiled.status, StatusCode::NOT_FOUND);
    let gone = rig.post("/revoke", json!({}), &head).await;
    assert_eq!(gone.status, StatusCode::NO_CONTENT, "{}", gone.body);
    let path = format!("/actor/{}", born.erin);
    assert_eq!(rig.get(&path, &head).await.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn listed() {
    let born = born().await;
    let q = r#"from @grant where verb = "see" count"#;
    let pack = born.deck.rig.ask(q, &born.deck.crown()).await;
    let seen = pack["count"].as_i64().expect("count");
    assert!(seen >= 3, "{pack}");
}

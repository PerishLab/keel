use crate::deed::{Deck, deck};
use axum::http::StatusCode;
use serde_json::json;

pub struct Stage {
    pub deck: Deck,
    pub carol: i64,
    pub dave: i64,
    pub den: i64,
}

pub fn her() -> [(&'static str, &'static str); 1] {
    [("x-login", "carol")]
}

pub fn him() -> [(&'static str, &'static str); 1] {
    [("x-login", "dave")]
}

pub async fn stage() -> Stage {
    let deck = deck().await;
    let rig = &deck.rig;
    rig.grant("anon", "see", ("Repo", r#"pred visibility = "public""#))
        .await;
    rig.grant("all", "put", ("Repo", r#"pred owner = "@me""#))
        .await;
    rig.grant("all", "put", ("Issue", r#"pred author = "@me""#))
        .await;
    rig.grant("all", "set", ("Issue", r#"pred author = "@me""#))
        .await;
    rig.grant("all", "see", ("Issue", r#"pred author = "@me""#))
        .await;
    let carol = rig
        .made("/actor", json!({ "login": "carol" }), &deck.crown())
        .await;
    let dave = rig
        .made("/actor", json!({ "login": "dave" }), &deck.crown())
        .await;
    rig.grant(&carol.to_string(), "*", ("Actor", &format!("row {carol}")))
        .await;
    rig.grant(&dave.to_string(), "*", ("Actor", &format!("row {dave}")))
        .await;
    let held = json!({ "name": "den", "visibility": "private", "owner": carol });
    let den = rig.made("/repo", held, &her()).await;
    Stage {
        deck,
        carol,
        dave,
        den,
    }
}

#[tokio::test]
async fn owns() {
    let stage = stage().await;
    let held = json!({ "name": "loot", "visibility": "private", "owner": stage.carol });
    let steal = stage.deck.rig.post("/repo", held, &him()).await;
    assert_eq!(steal.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn absent() {
    let stage = stage().await;
    let rig = &stage.deck.rig;
    let path = format!("/repo/{}", stage.den);
    assert_eq!(rig.get(&path, &him()).await.status, StatusCode::NOT_FOUND);
    let blind = rig
        .patch(&path, json!({ "visibility": "public" }), &him())
        .await;
    assert_eq!(blind.status, StatusCode::NOT_FOUND);
    let list = rig.get("/repo", &[]).await;
    assert!(
        !list
            .rows()
            .iter()
            .any(|row| row["id"].as_i64() == Some(stage.den))
    );
}

#[tokio::test]
async fn opened() {
    let stage = stage().await;
    let rig = &stage.deck.rig;
    let path = format!("/repo/{}", stage.den);
    let open = rig
        .patch(&path, json!({ "visibility": "public" }), &her())
        .await;
    assert_eq!(open.status, StatusCode::OK, "{}", open.body);
    assert_eq!(rig.get(&path, &him()).await.status, StatusCode::OK);
    let grab = rig.patch(&path, json!({ "name": "grab" }), &him()).await;
    assert_eq!(grab.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn author() {
    let stage = stage().await;
    let rig = &stage.deck.rig;
    let mine = json!({
        "title": "mine",
        "closed": false,
        "repo": stage.den,
        "author": stage.dave
    });
    assert_eq!(
        rig.post("/issue", mine, &him()).await.status,
        StatusCode::CREATED
    );
    let forged = json!({
        "title": "forged",
        "closed": false,
        "repo": stage.den,
        "author": stage.carol
    });
    assert_eq!(
        rig.post("/issue", forged, &him()).await.status,
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn shared() {
    let stage = stage().await;
    let rig = &stage.deck.rig;
    let path = format!("/repo/{}", stage.den);
    let open = rig
        .patch(&path, json!({ "visibility": "public" }), &her())
        .await;
    assert_eq!(open.status, StatusCode::OK, "{}", open.body);
    let sent = json!({
        "who": stage.dave.to_string(),
        "verb": "set",
        "unit": "Repo",
        "scope": format!("row {}", stage.den)
    });
    let share = rig.post("/@grant", sent, &her()).await;
    assert_eq!(share.status, StatusCode::CREATED, "{}", share.body);
    let wrote = rig.patch(&path, json!({ "name": "ours" }), &him()).await;
    assert_eq!(wrote.status, StatusCode::OK, "{}", wrote.body);
    let sent = json!({ "who": "all", "verb": "put", "unit": "Repo", "scope": "all" });
    let grab = rig.post("/@grant", sent, &him()).await;
    assert_eq!(grab.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn moved() {
    let stage = stage().await;
    let rig = &stage.deck.rig;
    let sent = json!({
        "who": stage.carol.to_string(),
        "verb": "end",
        "unit": "Repo",
        "scope": format!("row {}", stage.den)
    });
    let early = rig.post("/@grant", sent.clone(), &him()).await;
    assert_eq!(early.status, StatusCode::FORBIDDEN);
    let path = format!("/repo/{}", stage.den);
    let shift = rig
        .patch(&path, json!({ "owner": stage.dave }), &her())
        .await;
    assert!(shift.status.is_success(), "{}", shift.body);
    let late = rig.post("/@grant", sent, &him()).await;
    assert_eq!(late.status, StatusCode::CREATED, "{}", late.body);
}

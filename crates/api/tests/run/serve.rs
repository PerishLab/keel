use crate::rig::{Site, send};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;

#[tokio::test]
async fn sudo() {
    let site = Site::new();
    let token = site.sealed().await;
    let app = api::mount(site.wire().await, "/api").await.expect("mount");
    let made = Request::post("/api/course")
        .header("authorization", format!("sudo {token}"))
        .header("content-type", "application/json")
        .body(Body::from(
            json!({ "code": "C1", "title": "keel" }).to_string(),
        ))
        .expect("made");
    let (status, body) = send(&app, made).await;
    assert!(status.is_success(), "{status} {body}");
    let id = body["id"].as_i64().expect("id");
    let one = Request::get(format!("/api/course/{id}"))
        .header("authorization", format!("sudo {token}"))
        .body(Body::empty())
        .expect("one");
    let (status, body) = send(&app, one).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["title"], "keel");
}

#[tokio::test]
async fn refused() {
    let site = Site::new();
    site.sealed().await;
    let app = api::mount(site.wire().await, "").await.expect("mount");
    let anon = Request::post("/course")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({ "code": "C1", "title": "keel" }).to_string(),
        ))
        .expect("anon");
    let (status, _) = send(&app, anon).await;
    assert!(status.is_client_error(), "{status}");
    let forged = Request::get("/course")
        .header("authorization", "sudo forged")
        .body(Body::empty())
        .expect("forged");
    let (status, _) = send(&app, forged).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn reopened() {
    let site = Site::new();
    let token = site.sealed().await;
    let first = api::mount(site.wire().await, "").await.expect("first");
    let made = Request::post("/course")
        .header("authorization", format!("sudo {token}"))
        .header("content-type", "application/json")
        .body(Body::from(
            json!({ "code": "C1", "title": "kept" }).to_string(),
        ))
        .expect("made");
    let (status, _) = send(&first, made).await;
    assert!(status.is_success());
    drop(first);
    let second = api::mount(site.wire().await, "").await.expect("second");
    let list = Request::get("/course")
        .header("authorization", format!("sudo {token}"))
        .body(Body::empty())
        .expect("list");
    let (status, body) = send(&second, list).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body[0]["title"], "kept");
}

#[tokio::test]
async fn collection() {
    use keel::wire::{Val, Wire};
    use std::time::Duration;
    let site = Site::new();
    let token = site.sealed().await;
    let core = keel::bind(crate::rig::graph(), site.wire().await)
        .await
        .expect("bind");
    let id = core
        .put("Course", &[("code", "C1"), ("title", "old")])
        .await
        .expect("put");
    core.end("Course", id).await.expect("end");
    drop(core);
    let mut estate = keel::config::Estate::default();
    estate.collection.retain = keel::config::Retain::Span(0);
    estate.collection.period = 1;
    let app = api::configured(site.wire().await, "", &estate)
        .await
        .expect("configured");
    let mut wire = site.wire().await;
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let rows = wire
                .rows("SELECT COUNT(*) FROM course", &[])
                .await
                .expect("inspect");
            if rows[0][0] == Val::Int(0) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("collected");
    let request = Request::get(format!("/course/{id}"))
        .header("authorization", format!("sudo {token}"))
        .body(Body::empty())
        .expect("request");
    let (status, _) = send(&app, request).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

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

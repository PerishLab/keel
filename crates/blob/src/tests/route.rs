use super::Vault;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use keel::Operator;
use keel::adapt::db::Sqlite;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tower::ServiceExt;
use url::Url;

pub(super) struct Ticket {
    pub(super) id: i64,
    pub(super) url: Url,
    checksum: String,
    length: String,
}

impl Ticket {
    pub(super) async fn upload(&self, bytes: &[u8]) -> StatusCode {
        self.send(bytes, &self.length).await
    }

    pub(super) async fn resize(&self, bytes: &[u8]) -> StatusCode {
        self.send(bytes, &bytes.len().to_string()).await
    }

    async fn send(&self, bytes: &[u8], length: &str) -> StatusCode {
        reqwest::Client::new()
            .put(self.url.clone())
            .header("content-length", length)
            .header("x-amz-checksum-sha256", &self.checksum)
            .body(bytes.to_vec())
            .send()
            .await
            .unwrap()
            .status()
    }
}

pub(super) async fn stow(vault: &Vault<Sqlite>, owner: i64, bytes: &[u8]) -> Ticket {
    let body = json!({
        "name": "sample",
        "mime": "application/octet-stream",
        "size": bytes.len(),
        "hash": format!("{:x}", Sha256::digest(bytes)),
        "owner": owner,
    });
    let request = Request::post("/asset")
        .extension(Operator(owner))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let response = vault.doors().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let bytes = to_bytes(response.into_body(), 4096).await.unwrap();
    let reply: Value = serde_json::from_slice(&bytes).unwrap();
    Ticket {
        id: reply["id"].as_i64().unwrap(),
        url: reply["put"].as_str().unwrap().parse().unwrap(),
        checksum: reply["headers"]["x-amz-checksum-sha256"]
            .as_str()
            .unwrap()
            .into(),
        length: reply["headers"]["content-length"].as_str().unwrap().into(),
    }
}

pub(super) async fn fetch(vault: &Vault<Sqlite>, owner: i64, id: i64) -> Option<Url> {
    let request = Request::get(format!("/asset/{id}"))
        .extension(Operator(owner))
        .body(Body::empty())
        .unwrap();
    let response = vault.doors().oneshot(request).await.unwrap();
    match response.status() {
        StatusCode::NOT_FOUND => None,
        StatusCode::FOUND => Some(
            response.headers()[header::LOCATION]
                .to_str()
                .unwrap()
                .parse()
                .unwrap(),
        ),
        status => panic!("unexpected response {status}"),
    }
}

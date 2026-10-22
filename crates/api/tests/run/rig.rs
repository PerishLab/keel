use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use keel::Graph;
use keel::adapt::db::Sqlite;
use keel::atom::string;
use keel::resource;
use serde_json::Value;
use std::path::PathBuf;
use tempfile::TempDir;
use tower::ServiceExt;

#[resource]
struct Course {
    #[field(string)]
    code: string,
    #[field(string)]
    title: string,
}

pub fn graph() -> Graph {
    let mut graph = Graph::new();
    graph.plug::<Course>();
    graph
}

pub struct Site {
    pub dir: TempDir,
}

impl Site {
    pub fn new() -> Site {
        Site {
            dir: tempfile::tempdir().expect("dir"),
        }
    }

    pub fn at(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    pub async fn wire(&self) -> Sqlite {
        Sqlite::file(self.at("estate.db")).await.expect("wire")
    }

    pub async fn sealed(&self) -> String {
        api::seal(self.wire().await, graph(), &self.at("sudo"))
            .await
            .expect("seal");
        std::fs::read_to_string(self.at("sudo")).expect("custody")
    }
}

pub async fn send(app: &Router, head: Request<Body>) -> (StatusCode, Value) {
    let res = app.clone().oneshot(head).await.expect("oneshot");
    let status = res.status();
    let bytes = res.into_body().collect().await.expect("body").to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

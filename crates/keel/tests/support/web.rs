use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use keel::Graph;
use keel::adapt::db::Sqlite;
use serde_json::{Value, json};
use tower::ServiceExt;

pub struct Reply {
    pub status: StatusCode,
    pub body: Value,
}

impl Reply {
    pub fn id(&self) -> i64 {
        self.body.get("id").and_then(Value::as_i64).expect("id")
    }

    pub fn rows(&self) -> Vec<Value> {
        self.body.as_array().cloned().expect("rows")
    }
}

pub struct Rig {
    app: Router,
}

impl Rig {
    pub async fn open(graph: Graph) -> Rig {
        let wire = Sqlite::memory().await.expect("wire");
        let core = crate::support::boot(graph, wire).await.expect("boot");
        let core = core.share();
        core.put("@grant", &grant()).await.expect("grant");
        Rig {
            app: keel::app(core, ""),
        }
    }

    pub async fn send(&self, verb: &str, path: &str, sent: Option<Value>) -> Reply {
        let mut head = Request::builder().method(verb).uri(path);
        let body = match sent {
            Some(value) => {
                head = head.header("content-type", "application/json");
                Body::from(value.to_string())
            }
            None => Body::empty(),
        };
        let req = head.body(body).expect("request");
        let res = self.app.clone().oneshot(req).await.expect("oneshot");
        let status = res.status();
        let bytes = res.into_body().collect().await.expect("collect").to_bytes();
        Reply {
            status,
            body: read(&bytes),
        }
    }

    pub async fn get(&self, path: &str) -> Reply {
        self.send("GET", path, None).await
    }

    pub async fn post(&self, path: &str, sent: Value) -> Reply {
        self.send("POST", path, Some(sent)).await
    }

    pub async fn patch(&self, path: &str, sent: Value) -> Reply {
        self.send("PATCH", path, Some(sent)).await
    }

    pub async fn end(&self, path: &str) -> Reply {
        self.send("DELETE", path, None).await
    }

    pub async fn ask(&self, q: &str) -> Value {
        let reply = self.post("/query", json!({ "q": q })).await;
        assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
        reply.body
    }

    pub async fn made(&self, path: &str, sent: Value) -> i64 {
        let reply = self.post(path, sent).await;
        assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.body);
        reply.id()
    }
}

pub fn bag(pack: &Value, unit: &str) -> Vec<Value> {
    pack.get("bags")
        .and_then(|bags| bags.get(unit))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

pub fn root(pack: &Value) -> String {
    pack.get("root")
        .and_then(Value::as_str)
        .expect("root")
        .to_string()
}

fn grant() -> [(&'static str, &'static str); 4] {
    [
        ("who", "anon"),
        ("verb", "*"),
        ("unit", "*"),
        ("scope", "all"),
    ]
}

fn read(bytes: &[u8]) -> Value {
    if bytes.is_empty() {
        return Value::Null;
    }
    serde_json::from_slice(bytes).unwrap_or(Value::Null)
}

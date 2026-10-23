use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use keel::adapt::db::Sqlite;
use keel::atom::string;
use keel::spec::Spec;
use keel::{Core, Graph, Wire, atom, bootstrap, resource};
use serde_json::{Value, json};
use tower::ServiceExt;

#[resource]
struct Note {
    #[field(string)]
    body: string,
}

struct Desk {
    app: Router,
    core: std::sync::Arc<Core<Sqlite>>,
    token: String,
}

impl Desk {
    async fn open() -> Desk {
        Desk::over(Sqlite::memory().await.expect("db")).await
    }

    async fn over(wire: Sqlite) -> Desk {
        let mut graph = Graph::new();
        graph.plug::<Note>();
        let mut boot = bootstrap(graph, wire).expect("bootstrap");
        let token = boot.mint().await.expect("mint");
        let core = boot.seal(&token).await.expect("seal").share();
        core.put(
            "@grant",
            &[
                ("who", "anon"),
                ("verb", "*"),
                ("unit", "*"),
                ("scope", "all"),
            ],
        )
        .await
        .expect("grant");
        Desk {
            app: keel::app(core.clone(), "/api"),
            core,
            token,
        }
    }

    async fn get(&self, path: &str, sudo: Option<&str>) -> (StatusCode, Value) {
        let mut head = Request::builder().method("GET").uri(path);
        if let Some(token) = sudo {
            head = head.header("authorization", format!("sudo {token}"));
        }
        let req = head.body(Body::empty()).expect("request");
        let res = self.app.clone().oneshot(req).await.expect("oneshot");
        let status = res.status();
        let bytes = res.into_body().collect().await.expect("collect").to_bytes();
        let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, body)
    }
}

#[tokio::test]
async fn row() {
    let desk = Desk::open().await;
    let key = desk.core.put("Note", &[("body", "a")]).await.expect("note");
    let token = desk.token.clone();
    let path = format!("/api/-/keel/row/Note/{key}");

    let (status, body) = desk.get(&path, Some(&token)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let keys: Vec<&str> = body
        .as_object()
        .expect("object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, ["created_at", "expires_at", "updated_at"]);
    assert_eq!(body["expires_at"], Value::Null);
    assert!(body["created_at"].as_i64().expect("created") > 0);

    let at = keel::life::tick() + 3600;
    desk.core.lease("Note", key, at).await.expect("lease");
    let (_, body) = desk.get(&path, Some(&token)).await;
    assert_eq!(body["expires_at"], json!(at));

    desk.core.end("Note", key).await.expect("end");
    let (status, body) = desk.get(&path, Some(&token)).await;
    assert_eq!(status, StatusCode::OK, "an ended row until it is gleaned");
    assert!(body["expires_at"].as_i64().is_some());

    let (status, _) = desk.get("/api/-/keel/row/Note/999", Some(&token)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = desk.get("/api/-/keel/row/@seal/1", Some(&token)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) = desk.get(&path, None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = desk.get(&path, Some("wrong")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn pulse() {
    let desk = Desk::open().await;
    let token = desk.token.clone();
    desk.core.put("Note", &[("body", "a")]).await.expect("a");
    desk.core.put("Note", &[("body", "b")]).await.expect("b");

    let (status, body) = desk.get("/api/-/keel/pulse", Some(&token)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let beats = body.as_array().expect("beats");
    let notes: Vec<&Value> = beats.iter().filter(|beat| beat["unit"] == "note").collect();
    assert_eq!(notes.len(), 2, "{body}");
    let last = beats.last().expect("last")["id"].as_i64().expect("id");

    let (_, body) = desk
        .get(&format!("/api/-/keel/pulse?after={last}"), Some(&token))
        .await;
    assert_eq!(body, json!([]));

    let (status, _) = desk.get("/api/-/keel/pulse", None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn window() {
    let path = std::env::temp_dir().join(format!("keel-desk-{}.sqlite", std::process::id()));
    let desk = Desk::over(Sqlite::file(&path).await.expect("db")).await;
    let token = desk.token.clone();
    desk.core.put("Note", &[("body", "a")]).await.expect("a");
    let seat = desk
        .core
        .plan()
        .units()
        .values()
        .find(|unit| unit.name() == "@pulse")
        .map(keel::ddl::seat)
        .expect("pulse");
    let mut side = Sqlite::file(&path).await.expect("side");
    side.script(&format!(
        "DELETE FROM {seat} WHERE id = (SELECT MIN(id) FROM {seat})"
    ))
    .await
    .expect("prune");

    let (status, body) = desk.get("/api/-/keel/pulse?after=0", Some(&token)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body, json!({ "error": "cursor past window" }));
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn reserved() {
    let mut graph = Graph::new();
    graph.add(Spec::build("-").field("body", atom::Kind::Text).seal());
    let err = crate::support::boot(graph, Sqlite::memory().await.expect("db"))
        .await
        .err()
        .expect("refused");
    assert!(err.to_string().contains("unit name - is Keel's"), "{err}");
}

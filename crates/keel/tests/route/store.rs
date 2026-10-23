use crate::web::Rig;
use axum::http::StatusCode;
use keel::adapt::db::Sqlite;
use keel::adapt::{Error, Store};
use keel::atom::string;
use keel::wire::Val;
use keel::{Graph, resource};
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[resource]
struct Pass {
    #[field(string, unique)]
    tag: string,
    #[field(string)]
    kind: string,
    #[field(string, unique = kind)]
    seat: string,
    #[field(string, unique, opt)]
    label: string,
}

#[derive(Clone, Default)]
struct Knobs {
    blind: Arc<AtomicBool>,
    named: Arc<AtomicBool>,
    down: Arc<AtomicBool>,
}

struct Shaky {
    wire: Sqlite,
    knobs: Knobs,
}

impl Shaky {
    fn gone(&self) -> Result<(), Error> {
        if self.knobs.down.load(Ordering::Relaxed) {
            return Err(Error::Store(Store::Down("connection closed".into())));
        }
        Ok(())
    }
}

impl keel::Wire for Shaky {
    fn grain(&self) -> keel::ddl::Grain {
        self.wire.grain()
    }

    async fn run(&mut self, sql: &str, args: &[Val]) -> Result<u64, Error> {
        self.gone()?;
        let named = self.knobs.named.load(Ordering::Relaxed);
        self.wire.run(sql, args).await.map_err(|err| match err {
            Error::Store(Store::Taken(col)) if named => {
                Error::Store(Store::Taken(format!("only_pass_{col}")))
            }
            other => other,
        })
    }

    async fn plant(&mut self, sql: &str, args: &[Val]) -> Result<i64, Error> {
        self.gone()?;
        self.wire.plant(sql, args).await
    }

    async fn rows(&mut self, sql: &str, args: &[Val]) -> Result<Vec<Vec<Val>>, Error> {
        self.gone()?;
        let check = sql.starts_with("SELECT 1 FROM") && sql.contains(" != ?2 AND ");
        if check && self.knobs.blind.load(Ordering::Relaxed) {
            return Ok(Vec::new());
        }
        self.wire.rows(sql, args).await
    }

    async fn script(&mut self, sql: &str) -> Result<(), Error> {
        self.gone()?;
        self.wire.script(sql).await
    }
}

async fn rig() -> (Rig, Knobs) {
    let knobs = Knobs::default();
    let wire = Shaky {
        wire: Sqlite::memory().await.expect("wire"),
        knobs: knobs.clone(),
    };
    let mut graph = Graph::new();
    graph.plug::<Pass>();
    (Rig::over(graph, wire).await, knobs)
}

#[tokio::test]
async fn arbiter() {
    let (rig, knobs) = rig().await;
    rig.made("/Pass", json!({ "tag": "a", "kind": "k", "seat": "s" }))
        .await;
    let other = rig
        .made("/Pass", json!({ "tag": "b", "kind": "k", "seat": "t" }))
        .await;
    knobs.blind.store(true, Ordering::Relaxed);

    let put = rig
        .post("/Pass", json!({ "tag": "a", "kind": "k", "seat": "u" }))
        .await;
    assert_eq!(put.status, StatusCode::CONFLICT, "{}", put.body);
    assert_eq!(put.body, json!({ "error": "field tag taken" }));

    let set = rig
        .patch(&format!("/Pass/{other}"), json!({ "seat": "s" }))
        .await;
    assert_eq!(set.status, StatusCode::CONFLICT, "{}", set.body);
    assert_eq!(set.body, json!({ "error": "field seat taken" }));

    knobs.named.store(true, Ordering::Relaxed);
    let set = rig
        .patch(&format!("/Pass/{other}"), json!({ "seat": "s" }))
        .await;
    assert_eq!(set.body, json!({ "error": "field seat taken" }));
}

#[tokio::test]
async fn down() {
    let (rig, knobs) = rig().await;
    assert_eq!(rig.get("/health").await.status, StatusCode::OK);
    knobs.down.store(true, Ordering::Relaxed);

    let list = rig.get("/Pass").await;
    assert_eq!(list.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(list.body, json!({ "error": "store unavailable" }));
    let health = rig.get("/health").await;
    assert_eq!(health.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(health.body, json!({ "error": "store unavailable" }));

    knobs.down.store(false, Ordering::Relaxed);
    assert_eq!(rig.get("/health").await.status, StatusCode::OK);
}

#[tokio::test]
async fn unreachable() {
    let path = std::path::Path::new("/dev/null/keel.db");
    match Sqlite::file(path).await {
        Err(Error::Store(Store::Down(_))) => {}
        Err(other) => panic!("a store that cannot be opened is {other:?}"),
        Ok(_) => panic!("opened {}", path.display()),
    }
}

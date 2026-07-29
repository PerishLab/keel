use axum::Router;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{Request as Head, StatusCode};
use axum::middleware::{self, Next};
use axum::response::Response;
use http_body_util::BodyExt;
use keel::adapt::db::Sqlite;
use keel::atom::{int, string};
use keel::{Cell, Core, Graph, Operator, app, bootstrap, resource};
use keel_gate::Gate;
use keel_relay::Relay;
use serde_json::{Value, json};
use std::sync::Arc;
use tower::ServiceExt;

#[resource]
pub struct Actor {
    #[field(string, unique)]
    login: string,
    #[relation(Repo, many2many)]
    stars: Repo,
}

#[resource]
pub struct Repo {
    #[field(string, unique = owner)]
    name: string,
    #[field(string)]
    visibility: string,
    #[relation(Actor, many2one, root)]
    owner: Actor,
}

#[resource]
pub struct Issue {
    #[field(string)]
    title: string,
    #[field(serial, scope = repo)]
    index: int,
    #[field(bool)]
    closed: bool,
    #[relation(Repo, many2one, root)]
    repo: Repo,
    #[relation(Actor, many2one)]
    author: Actor,
}

keel_gate::gate!(Actor);
keel_relay::relay!(Actor);

pub struct Call<'a> {
    pub verb: &'a str,
    pub path: &'a str,
    pub sent: Option<Value>,
    pub head: &'a [(&'a str, &'a str)],
}

pub struct Reply {
    pub status: StatusCode,
    pub body: Value,
    pub head: Vec<(String, String)>,
}

impl Reply {
    pub fn id(&self) -> i64 {
        self.body.get("id").and_then(Value::as_i64).expect("id")
    }

    pub fn rows(&self) -> Vec<Value> {
        self.body.as_array().cloned().expect("rows")
    }

    pub fn cookie(&self) -> String {
        self.head
            .iter()
            .find(|(name, _)| name == "set-cookie")
            .map(|(_, value)| value.clone())
            .expect("cookie")
    }
}

pub struct Forge {
    app: Router,
    pub core: Arc<Core<Sqlite>>,
    pub crown: String,
}

pub async fn forge() -> Forge {
    let mut graph = Graph::new();
    graph.plug::<Actor>().plug::<Repo>().plug::<Issue>();
    plug(&mut graph);
    wire(&mut graph);
    let store = Sqlite::memory().await.expect("store");
    let mut open = bootstrap(graph, store).expect("bootstrap");
    let token = open.mint().await.expect("mint");
    let core = open.seal(&token).await.expect("seal");
    let core = core.identify("Actor").expect("identity").share();
    let svc = core.put("Actor", &[("login", "gate")]).await.expect("svc");
    let mail = core
        .put("Actor", &[("login", "relay")])
        .await
        .expect("post");
    Relay::rise(core.clone(), mail).await.expect("relay").run();
    let door = Gate::rise(core.clone(), svc).expect("rise");
    door.seed().await.expect("seed");
    let app = door
        .wall(app(core.clone(), ""))
        .layer(middleware::from_fn_with_state(core.clone(), guard));
    Forge {
        app,
        core,
        crown: format!("sudo {token}"),
    }
}

impl Forge {
    pub async fn send(&self, call: Call<'_>) -> Reply {
        let mut head = Head::builder().method(call.verb).uri(call.path);
        for (name, value) in call.head {
            head = head.header(*name, *value);
        }
        let body = match call.sent {
            Some(value) => {
                head = head.header("content-type", "application/json");
                Body::from(value.to_string())
            }
            None => Body::empty(),
        };
        let req = head.body(body).expect("request");
        let res = self.app.clone().oneshot(req).await.expect("oneshot");
        let status = res.status();
        let worn = res
            .headers()
            .iter()
            .map(|(name, value)| {
                (
                    name.as_str().to_string(),
                    value.to_str().unwrap_or_default().to_string(),
                )
            })
            .collect();
        let bytes = res.into_body().collect().await.expect("collect").to_bytes();
        Reply {
            status,
            body: read(&bytes),
            head: worn,
        }
    }

    pub async fn get(&self, path: &str, head: &[(&str, &str)]) -> Reply {
        self.send(Call {
            verb: "GET",
            path,
            sent: None,
            head,
        })
        .await
    }

    pub async fn post(&self, path: &str, sent: Value, head: &[(&str, &str)]) -> Reply {
        self.send(Call {
            verb: "POST",
            path,
            sent: Some(sent),
            head,
        })
        .await
    }

    pub async fn patch(&self, path: &str, sent: Value, head: &[(&str, &str)]) -> Reply {
        self.send(Call {
            verb: "PATCH",
            path,
            sent: Some(sent),
            head,
        })
        .await
    }

    pub async fn end(&self, path: &str, head: &[(&str, &str)]) -> Reply {
        self.send(Call {
            verb: "DELETE",
            path,
            sent: None,
            head,
        })
        .await
    }

    pub async fn made(&self, path: &str, sent: Value, head: &[(&str, &str)]) -> i64 {
        let reply = self.post(path, sent, head).await;
        assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.body);
        reply.id()
    }

    pub async fn ask(&self, q: &str, head: &[(&str, &str)]) -> Value {
        let reply = self.post("/query", json!({ "q": q }), head).await;
        assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
        reply.body
    }

    pub async fn grant(&self, who: &str, verb: &str, scope: (&str, &str)) {
        let sent = json!({ "who": who, "verb": verb, "unit": scope.0, "scope": scope.1 });
        let head: [(&str, &str); 1] = [("authorization", &self.crown)];
        let reply = self.post("/@grant", sent, &head).await;
        assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.body);
    }
}

async fn guard(State(core): State<Arc<Core<Sqlite>>>, mut req: Request, next: Next) -> Response {
    let login = req
        .headers()
        .get("x-login")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    if let Some(login) = login
        && let Some(key) = whom(&core, &login).await
    {
        req.extensions_mut().insert(Operator(key));
    }
    next.run(req).await
}

async fn whom(core: &Core<Sqlite>, login: &str) -> Option<i64> {
    let rows = core.live("Actor").await.ok()?;
    rows.iter()
        .find(|row| row.cells().get("login").map(Cell::text) == Some(login))
        .map(|row| row.key())
}

pub fn bag(pack: &Value, unit: &str) -> Vec<Value> {
    pack.get("bags")
        .and_then(|bags| bags.get(unit))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn read(bytes: &[u8]) -> Value {
    if bytes.is_empty() {
        return Value::Null;
    }
    serde_json::from_slice(bytes).unwrap_or(Value::Null)
}

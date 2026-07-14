use crate::adapt::Error;
use crate::face::Core;
use crate::store::Store;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::Arc;

const HOST: &str = "127.0.0.1";
const PORT: u16 = 3000;

pub async fn serve<S: Store + 'static>(core: Arc<Core<S>>) -> Result<(), Error> {
    listen(core, HOST, PORT, "").await
}

pub async fn listen<S: Store + 'static>(
    core: Arc<Core<S>>,
    host: &str,
    port: u16,
    prefix: &str,
) -> Result<(), Error> {
    let api = Router::new()
        .route("/health", get(health))
        .route("/query", post(run::<S>))
        .route("/{unit}", get(list::<S>).post(create::<S>))
        .route(
            "/{unit}/{id}",
            get(one::<S>).patch(edit::<S>).delete(remove::<S>),
        )
        .route("/{unit}/{id}/{bond}", get(no_read).post(attach::<S>))
        .route("/{unit}/{id}/{bond}/{tie}", delete(detach::<S>))
        .with_state(core);
    let prefix = prefix.trim_end_matches('/');
    let app = if prefix.is_empty() {
        api
    } else {
        Router::new().nest(prefix, api)
    };
    let addr: SocketAddr = format!("{host}:{port}")
        .parse()
        .map_err(|e| Error::Adapt(format!("bad addr: {e}")))?;
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| Error::Adapt(format!("bind: {e}")))?;
    let base = if prefix.is_empty() {
        format!("http://{addr}")
    } else {
        format!("http://{addr}{prefix}")
    };
    eprintln!("keel: ready on {base}");
    axum::serve(listener, app)
        .await
        .map_err(|e| Error::Adapt(format!("serve: {e}")))
}

async fn health() -> impl IntoResponse {
    Json(json!({ "ok": true }))
}

#[derive(Deserialize)]
struct QueryBody {
    q: String,
}

async fn run<S: Store>(
    State(core): State<Arc<Core<S>>>,
    Json(body): Json<QueryBody>,
) -> Result<Json<Value>, Fault> {
    let pack = core.query(&body.q).map_err(Fault::from)?;
    Ok(Json(pack_json(&pack)))
}

async fn list<S: Store>(
    State(core): State<Arc<Core<S>>>,
    Path(unit): Path<String>,
) -> Result<Json<Value>, Fault> {
    let name = unit_name(core.as_ref(), &unit)?;
    let rows = core.live(&name).map_err(Fault::from)?;
    let body: Vec<Value> = rows.iter().map(row_json).collect();
    Ok(Json(Value::Array(body)))
}

async fn create<S: Store>(
    State(core): State<Arc<Core<S>>>,
    Path(unit): Path<String>,
    Json(body): Json<Map<String, Value>>,
) -> Result<(StatusCode, Json<Value>), Fault> {
    let name = unit_name(core.as_ref(), &unit)?;
    let fields = cells(&body)?;
    let pairs: Vec<(&str, &str)> = fields
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let key = core.put(&name, &pairs).map_err(Fault::from)?;
    Ok((StatusCode::CREATED, Json(json!({ "id": key }))))
}

async fn one<S: Store>(
    State(core): State<Arc<Core<S>>>,
    Path((unit, id)): Path<(String, i64)>,
) -> Result<Json<Value>, Fault> {
    let name = unit_name(core.as_ref(), &unit)?;
    let q = format!(r#"from {name} where id = "{id}""#);
    let pack = core.query(&q).map_err(Fault::from)?;
    match pack.rows().first() {
        Some(row) => Ok(Json(row_json(row))),
        None => Err(Fault {
            status: StatusCode::NOT_FOUND,
            note: format!("missing row {id}"),
        }),
    }
}

async fn edit<S: Store>(
    State(core): State<Arc<Core<S>>>,
    Path((unit, id)): Path<(String, i64)>,
    Json(body): Json<Map<String, Value>>,
) -> Result<Json<Value>, Fault> {
    let name = unit_name(core.as_ref(), &unit)?;
    let fields = cells(&body)?;
    let pairs: Vec<(&str, &str)> = fields
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    core.set(&name, id, &pairs).map_err(Fault::from)?;
    let q = format!(r#"from {name} where id = "{id}""#);
    let pack = core.query(&q).map_err(Fault::from)?;
    match pack.rows().first() {
        Some(row) => Ok(Json(row_json(row))),
        None => Err(Fault {
            status: StatusCode::NOT_FOUND,
            note: format!("missing row {id}"),
        }),
    }
}

async fn remove<S: Store>(
    State(core): State<Arc<Core<S>>>,
    Path((unit, id)): Path<(String, i64)>,
) -> Result<StatusCode, Fault> {
    let name = unit_name(core.as_ref(), &unit)?;
    core.end(&name, id).map_err(Fault::from)?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct TieBody {
    right: i64,
}

async fn no_read() -> StatusCode {
    StatusCode::NOT_FOUND
}

async fn attach<S: Store>(
    State(core): State<Arc<Core<S>>>,
    Path((unit, id, bond)): Path<(String, i64, String)>,
    Json(body): Json<TieBody>,
) -> Result<(StatusCode, Json<Value>), Fault> {
    let name = unit_name(core.as_ref(), &unit)?;
    let bond = bond_name(core.as_ref(), &name, &bond)?;
    let key = core
        .tie(
            &name,
            &bond,
            crate::life::Ends {
                left: id,
                right: body.right,
            },
        )
        .map_err(Fault::from)?;
    Ok((StatusCode::CREATED, Json(json!({ "id": key }))))
}

async fn detach<S: Store>(
    State(core): State<Arc<Core<S>>>,
    Path((unit, id, bond, tie)): Path<(String, i64, String, i64)>,
) -> Result<StatusCode, Fault> {
    let name = unit_name(core.as_ref(), &unit)?;
    let bond = bond_name(core.as_ref(), &name, &bond)?;
    let ties = core.ties(&name, &bond, id).map_err(Fault::from)?;
    if !ties.iter().any(|row| row.key() == tie) {
        return Err(Fault {
            status: StatusCode::NOT_FOUND,
            note: format!("missing tie {tie}"),
        });
    }
    core.cut(&name, &bond, tie).map_err(Fault::from)?;
    Ok(StatusCode::NO_CONTENT)
}

fn unit_name<S: Store>(core: &Core<S>, route: &str) -> Result<String, Fault> {
    crate::query::resolve(core.plan(), route).map_err(Fault::from)
}

fn bond_name<S: Store>(core: &Core<S>, unit: &str, bond: &str) -> Result<String, Fault> {
    let node = core
        .plan()
        .units()
        .get(unit)
        .ok_or_else(|| Fault::miss(unit))?;
    node.bonds()
        .iter()
        .find(|edge| edge.name().eq_ignore_ascii_case(bond))
        .map(|edge| edge.name().to_string())
        .ok_or_else(|| Fault::bad(format!("unknown bond {bond}")))
}

fn cells(body: &Map<String, Value>) -> Result<BTreeMap<String, String>, Fault> {
    let mut out = BTreeMap::new();
    for (key, value) in body {
        let text = match value {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Null => String::new(),
            _ => {
                return Err(Fault::bad(format!("field {key} must be scalar")));
            }
        };
        out.insert(key.clone(), text);
    }
    Ok(out)
}

fn pack_json(pack: &crate::query::Pack) -> Value {
    let mut bags = Map::new();
    for (key, bag) in pack.bags() {
        let list = match bag {
            crate::query::Bag::Unit(rows) => Value::Array(rows.iter().map(row_json).collect()),
            crate::query::Bag::Bond(ties) => Value::Array(ties.iter().map(tie_json).collect()),
        };
        bags.insert(key.clone(), list);
    }
    json!({ "root": pack.root(), "bags": bags })
}

fn row_json(row: &crate::life::Row) -> Value {
    let mut map = Map::new();
    map.insert("id".into(), json!(row.key()));
    for (k, v) in row.cells() {
        map.insert(k.clone(), Value::String(v.clone()));
    }
    map.insert("expires_at".into(), json!(row.expires()));
    map.insert("created_at".into(), json!(row.created()));
    map.insert("updated_at".into(), json!(row.updated()));
    Value::Object(map)
}

fn tie_json(tie: &crate::life::Tie) -> Value {
    json!({
        "id": tie.key(),
        "left": tie.left(),
        "right": tie.right(),
        "expires_at": tie.expires(),
        "created_at": tie.created(),
        "updated_at": tie.updated(),
    })
}

struct Fault {
    status: StatusCode,
    note: String,
}

impl Fault {
    fn miss(name: &str) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            note: format!("missing resource: {name}"),
        }
    }

    fn bad(note: String) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            note,
        }
    }
}

impl From<Error> for Fault {
    fn from(err: Error) -> Self {
        match err {
            Error::Missing(name) => Self::miss(&name),
            Error::Adapt(note) if note.starts_with("missing row") => Self {
                status: StatusCode::NOT_FOUND,
                note,
            },
            Error::Adapt(note) => Self {
                status: StatusCode::BAD_REQUEST,
                note,
            },
        }
    }
}

impl IntoResponse for Fault {
    fn into_response(self) -> axum::response::Response {
        (self.status, Json(json!({ "error": self.note }))).into_response()
    }
}

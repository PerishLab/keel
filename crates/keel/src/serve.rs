use crate::adapt::Error;
use crate::face::{Core, Face};
use crate::wire::Wire;
use axum::Extension;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::Arc;

const HOST: &str = "127.0.0.1";
const PORT: u16 = 3000;

pub async fn serve<W: Wire + 'static>(core: Arc<Core<W>>) -> Result<(), Error> {
    listen(core, HOST, PORT, "").await
}

#[derive(Clone, Copy, Debug)]
pub struct Operator(pub i64);

pub fn app<W: Wire + 'static>(core: Arc<Core<W>>, prefix: &str) -> Router {
    let api = Router::new()
        .route("/health", get(health))
        .route("/query", post(run::<W>))
        .route("/{unit}", get(list::<W>).post(create::<W>))
        .route(
            "/{unit}/{id}",
            get(one::<W>).patch(edit::<W>).delete(remove::<W>),
        )
        .route("/{unit}/{id}/{bond}", get(no_read).post(attach::<W>))
        .route(
            "/{unit}/{id}/{bond}/{tie}",
            patch(patch_tie::<W>).delete(detach::<W>),
        )
        .with_state(core);
    let prefix = prefix.trim_end_matches('/');
    if prefix.is_empty() {
        api
    } else {
        Router::new().nest(prefix, api)
    }
}

async fn front<'a, W: Wire>(
    core: &'a Core<W>,
    headers: &HeaderMap,
    op: Option<&Operator>,
    verb: &str,
) -> Result<Face<'a, W>, Fault> {
    let told = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok());
    if let Some(token) = told.and_then(|value| value.strip_prefix("sudo ")) {
        if core.seal(token).await.map_err(Fault::from)? {
            eprintln!("keel: sudo {verb}");
            return Ok(core.sudo());
        }
        return Err(Fault {
            status: StatusCode::UNAUTHORIZED,
            note: "bad sudo token".into(),
        });
    }
    Ok(match op {
        Some(Operator(id)) => core.of(*id),
        None => core.anon(),
    })
}

pub async fn listen<W: Wire + 'static>(
    core: Arc<Core<W>>,
    host: &str,
    port: u16,
    prefix: &str,
) -> Result<(), Error> {
    let app = app(core, prefix);
    let prefix = prefix.trim_end_matches('/');
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

async fn run<W: Wire>(
    State(core): State<Arc<Core<W>>>,
    headers: HeaderMap,
    op: Option<Extension<Operator>>,
    Json(body): Json<QueryBody>,
) -> Result<Json<Value>, Fault> {
    let face = front(core.as_ref(), &headers, op.as_deref(), "see").await?;
    let tree = crate::query::parse(&body.q).map_err(Fault::from)?;
    let units = crate::query::involved(core.plan(), &tree).map_err(Fault::from)?;
    if core.plan().shrouds(&units) {
        return Err(Fault {
            status: StatusCode::NOT_FOUND,
            note: "no such unit".into(),
        });
    }
    let pack = face.ask(&tree).await.map_err(Fault::from)?;
    Ok(Json(pack_json(&pack)))
}

async fn list<W: Wire>(
    State(core): State<Arc<Core<W>>>,
    Path(unit): Path<String>,
    headers: HeaderMap,
    op: Option<Extension<Operator>>,
) -> Result<Json<Value>, Fault> {
    let name = unit_name(core.as_ref(), &unit)?;
    let face = front(core.as_ref(), &headers, op.as_deref(), "see").await?;
    let rows = face.live(&name).await.map_err(Fault::from)?;
    let body: Vec<Value> = rows.iter().map(row_json).collect();
    Ok(Json(Value::Array(body)))
}

async fn create<W: Wire>(
    State(core): State<Arc<Core<W>>>,
    Path(unit): Path<String>,
    headers: HeaderMap,
    op: Option<Extension<Operator>>,
    Json(body): Json<Map<String, Value>>,
) -> Result<(StatusCode, Json<Value>), Fault> {
    let name = unit_name(core.as_ref(), &unit)?;
    let fields = cells(&body)?;
    let pairs: Vec<(&str, &str)> = fields
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let face = front(core.as_ref(), &headers, op.as_deref(), "put").await?;
    let key = face.put(&name, &pairs).await.map_err(Fault::from)?;
    Ok((StatusCode::CREATED, Json(json!({ "id": key }))))
}

async fn one<W: Wire>(
    State(core): State<Arc<Core<W>>>,
    Path((unit, id)): Path<(String, i64)>,
    headers: HeaderMap,
    op: Option<Extension<Operator>>,
) -> Result<Json<Value>, Fault> {
    let name = unit_name(core.as_ref(), &unit)?;
    let q = format!(r#"from {name} where id = "{id}""#);
    let face = front(core.as_ref(), &headers, op.as_deref(), "see").await?;
    let pack = face.query(&q).await.map_err(Fault::from)?;
    match pack.rows().first() {
        Some(row) => Ok(Json(row_json(row))),
        None => Err(Fault {
            status: StatusCode::NOT_FOUND,
            note: format!("missing row {id}"),
        }),
    }
}

async fn edit<W: Wire>(
    State(core): State<Arc<Core<W>>>,
    Path((unit, id)): Path<(String, i64)>,
    headers: HeaderMap,
    op: Option<Extension<Operator>>,
    Json(body): Json<Map<String, Value>>,
) -> Result<Json<Value>, Fault> {
    let name = unit_name(core.as_ref(), &unit)?;
    let fields = cells(&body)?;
    let pairs: Vec<(&str, &str)> = fields
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let face = front(core.as_ref(), &headers, op.as_deref(), "set").await?;
    face.set(&name, id, &pairs).await.map_err(Fault::from)?;
    let q = format!(r#"from {name} where id = "{id}""#);
    let pack = face.query(&q).await.map_err(Fault::from)?;
    match pack.rows().first() {
        Some(row) => Ok(Json(row_json(row))),
        None => Err(Fault {
            status: StatusCode::NOT_FOUND,
            note: format!("missing row {id}"),
        }),
    }
}

async fn remove<W: Wire>(
    State(core): State<Arc<Core<W>>>,
    Path((unit, id)): Path<(String, i64)>,
    headers: HeaderMap,
    op: Option<Extension<Operator>>,
) -> Result<StatusCode, Fault> {
    let name = unit_name(core.as_ref(), &unit)?;
    let face = front(core.as_ref(), &headers, op.as_deref(), "end").await?;
    face.end(&name, id).await.map_err(Fault::from)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn no_read() -> StatusCode {
    StatusCode::NOT_FOUND
}

async fn attach<W: Wire>(
    State(core): State<Arc<Core<W>>>,
    Path((unit, id, bond)): Path<(String, i64, String)>,
    headers: HeaderMap,
    op: Option<Extension<Operator>>,
    Json(body): Json<Map<String, Value>>,
) -> Result<(StatusCode, Json<Value>), Fault> {
    let name = unit_name(core.as_ref(), &unit)?;
    let bond = bond_name(core.as_ref(), &name, &bond)?;
    let right = body
        .get("right")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| Fault::bad("right needs integer".into()))?;
    let fields = cells_skip(&body, &["right"])?;
    let pairs: Vec<(&str, &str)> = fields
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let face = front(core.as_ref(), &headers, op.as_deref(), "tie").await?;
    let key = face
        .tie(&name, &bond, crate::life::Ends { left: id, right }, &pairs)
        .await
        .map_err(Fault::from)?;
    Ok((StatusCode::CREATED, Json(json!({ "id": key }))))
}

async fn patch_tie<W: Wire>(
    State(core): State<Arc<Core<W>>>,
    Path((unit, id, bond, tie)): Path<(String, i64, String, i64)>,
    headers: HeaderMap,
    op: Option<Extension<Operator>>,
    Json(body): Json<Map<String, Value>>,
) -> Result<StatusCode, Fault> {
    let name = unit_name(core.as_ref(), &unit)?;
    let bond = bond_name(core.as_ref(), &name, &bond)?;
    let face = front(core.as_ref(), &headers, op.as_deref(), "tie").await?;
    let ties = face.ties(&name, &bond, id).await.map_err(Fault::from)?;
    if !ties.iter().any(|row| row.key() == tie) {
        return Err(Fault {
            status: StatusCode::NOT_FOUND,
            note: format!("missing tie {tie}"),
        });
    }
    let fields = cells(&body)?;
    let pairs: Vec<(&str, &str)> = fields
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    face.set_tie(&name, &bond, tie, &pairs)
        .await
        .map_err(Fault::from)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn detach<W: Wire>(
    State(core): State<Arc<Core<W>>>,
    Path((unit, id, bond, tie)): Path<(String, i64, String, i64)>,
    headers: HeaderMap,
    op: Option<Extension<Operator>>,
) -> Result<StatusCode, Fault> {
    let name = unit_name(core.as_ref(), &unit)?;
    let bond = bond_name(core.as_ref(), &name, &bond)?;
    let face = front(core.as_ref(), &headers, op.as_deref(), "cut").await?;
    let ties = face.ties(&name, &bond, id).await.map_err(Fault::from)?;
    if !ties.iter().any(|row| row.key() == tie) {
        return Err(Fault {
            status: StatusCode::NOT_FOUND,
            note: format!("missing tie {tie}"),
        });
    }
    face.cut(&name, &bond, tie).await.map_err(Fault::from)?;
    Ok(StatusCode::NO_CONTENT)
}

fn unit_name<W: Wire>(core: &Core<W>, route: &str) -> Result<String, Fault> {
    let name = crate::query::resolve(core.plan(), route).map_err(Fault::from)?;
    if core.plan().veiled(&name) {
        return Err(Fault {
            status: StatusCode::NOT_FOUND,
            note: "no such route".into(),
        });
    }
    Ok(name)
}

fn bond_name<W: Wire>(core: &Core<W>, unit: &str, bond: &str) -> Result<String, Fault> {
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
    cells_skip(body, &[])
}

fn cells_skip(body: &Map<String, Value>, skip: &[&str]) -> Result<BTreeMap<String, String>, Fault> {
    let mut out = BTreeMap::new();
    for (key, value) in body {
        if skip.iter().any(|s| *s == key) {
            continue;
        }
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
    if let Some(n) = pack.count() {
        return json!({ "root": pack.root(), "count": n });
    }
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
        map.insert(k.clone(), cell_json(v));
    }
    map.insert("expires_at".into(), json!(row.expires()));
    map.insert("created_at".into(), json!(row.created()));
    map.insert("updated_at".into(), json!(row.updated()));
    Value::Object(map)
}

fn tie_json(tie: &crate::life::Tie) -> Value {
    let mut map = Map::new();
    map.insert("id".into(), json!(tie.key()));
    map.insert("left".into(), json!(tie.left()));
    map.insert("right".into(), json!(tie.right()));
    for (k, v) in tie.cells() {
        map.insert(k.clone(), cell_json(v));
    }
    map.insert("expires_at".into(), json!(tie.expires()));
    map.insert("created_at".into(), json!(tie.created()));
    map.insert("updated_at".into(), json!(tie.updated()));
    Value::Object(map)
}

fn cell_json(cell: &crate::life::Cell) -> Value {
    match cell {
        crate::life::Cell::Text(value) => Value::String(value.clone()),
        crate::life::Cell::Int(value) => json!(value),
        crate::life::Cell::Bool(value) => Value::Bool(*value),
    }
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
            Error::Adapt(note) if note.starts_with("missing tie") => Self {
                status: StatusCode::NOT_FOUND,
                note,
            },
            Error::Adapt(note) if note.starts_with("live ties remain") => Self {
                status: StatusCode::CONFLICT,
                note,
            },
            Error::Adapt(note) if note.starts_with("live ref exists") => Self {
                status: StatusCode::CONFLICT,
                note,
            },
            Error::Adapt(note) if note.ends_with(" taken") => Self {
                status: StatusCode::CONFLICT,
                note,
            },
            Error::Adapt(note) if note.starts_with("refused") => Self {
                status: StatusCode::FORBIDDEN,
                note,
            },
            Error::Adapt(note) if note.starts_with("left not live") => Self {
                status: StatusCode::BAD_REQUEST,
                note,
            },
            Error::Adapt(note) if note.starts_with("right not live") => Self {
                status: StatusCode::BAD_REQUEST,
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

use super::*;
use crate::face::Core;
use crate::wire::Wire;
use axum::Extension;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::sync::Arc;

#[derive(Deserialize)]
pub(crate) struct Body {
    q: String,
}

pub(crate) async fn run<W: Wire>(
    State(core): State<Arc<Core<W>>>,
    headers: HeaderMap,
    op: Option<Extension<Operator>>,
    Json(body): Json<Body>,
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
    Ok(Json(pack.emit()))
}

pub(crate) async fn list<W: Wire>(
    State(core): State<Arc<Core<W>>>,
    Path(unit): Path<String>,
    headers: HeaderMap,
    op: Option<Extension<Operator>>,
) -> Result<Json<Value>, Fault> {
    let name = core.as_ref().unit(&unit)?;
    let face = front(core.as_ref(), &headers, op.as_deref(), "see").await?;
    let rows = face.live(&name).await.map_err(Fault::from)?;
    let body: Vec<Value> = rows.iter().map(Emit::emit).collect();
    Ok(Json(Value::Array(body)))
}

pub(crate) async fn create<W: Wire>(
    State(core): State<Arc<Core<W>>>,
    Path(unit): Path<String>,
    headers: HeaderMap,
    op: Option<Extension<Operator>>,
    Json(body): Json<Map<String, Value>>,
) -> Result<(StatusCode, Json<Value>), Fault> {
    let name = core.as_ref().unit(&unit)?;
    let fields = cells(&body, &[])?;
    let pairs: Vec<(&str, &str)> = fields
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let face = front(core.as_ref(), &headers, op.as_deref(), "put").await?;
    let key = face.put(&name, &pairs).await.map_err(Fault::from)?;
    Ok((StatusCode::CREATED, Json(json!({ "id": key }))))
}

pub(crate) async fn one<W: Wire>(
    State(core): State<Arc<Core<W>>>,
    Path((unit, id)): Path<(String, i64)>,
    headers: HeaderMap,
    op: Option<Extension<Operator>>,
) -> Result<Json<Value>, Fault> {
    let name = core.as_ref().unit(&unit)?;
    let q = format!(r#"from {name} where id = "{id}""#);
    let face = front(core.as_ref(), &headers, op.as_deref(), "see").await?;
    let pack = face.query(&q).await.map_err(Fault::from)?;
    match pack.rows().first() {
        Some(row) => Ok(Json(row.emit())),
        None => Err(Fault {
            status: StatusCode::NOT_FOUND,
            note: format!("missing row {id}"),
        }),
    }
}

pub(crate) async fn edit<W: Wire>(
    State(core): State<Arc<Core<W>>>,
    Path((unit, id)): Path<(String, i64)>,
    headers: HeaderMap,
    op: Option<Extension<Operator>>,
    Json(body): Json<Map<String, Value>>,
) -> Result<Json<Value>, Fault> {
    let name = core.as_ref().unit(&unit)?;
    let fields = cells(&body, &[])?;
    let pairs: Vec<(&str, &str)> = fields
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let face = front(core.as_ref(), &headers, op.as_deref(), "set").await?;
    face.set(&name, id, &pairs).await.map_err(Fault::from)?;
    let q = format!(r#"from {name} where id = "{id}""#);
    let pack = face.query(&q).await.map_err(Fault::from)?;
    match pack.rows().first() {
        Some(row) => Ok(Json(row.emit())),
        None => Err(Fault {
            status: StatusCode::NOT_FOUND,
            note: format!("missing row {id}"),
        }),
    }
}

pub(crate) async fn remove<W: Wire>(
    State(core): State<Arc<Core<W>>>,
    Path((unit, id)): Path<(String, i64)>,
    headers: HeaderMap,
    op: Option<Extension<Operator>>,
) -> Result<StatusCode, Fault> {
    let name = core.as_ref().unit(&unit)?;
    let face = front(core.as_ref(), &headers, op.as_deref(), "end").await?;
    face.end(&name, id).await.map_err(Fault::from)?;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn refuse() -> StatusCode {
    StatusCode::NOT_FOUND
}

pub(crate) async fn attach<W: Wire>(
    State(core): State<Arc<Core<W>>>,
    Path((unit, id, bond)): Path<(String, i64, String)>,
    headers: HeaderMap,
    op: Option<Extension<Operator>>,
    Json(body): Json<Map<String, Value>>,
) -> Result<(StatusCode, Json<Value>), Fault> {
    let name = core.as_ref().unit(&unit)?;
    let bond = core.as_ref().bond(&name, &bond)?;
    let right = body
        .get("right")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| Fault::bad("right needs integer".into()))?;
    let fields = cells(&body, &["right"])?;
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

pub(crate) async fn retie<W: Wire>(
    State(core): State<Arc<Core<W>>>,
    Path((unit, id, bond, tie)): Path<(String, i64, String, i64)>,
    headers: HeaderMap,
    op: Option<Extension<Operator>>,
    Json(body): Json<Map<String, Value>>,
) -> Result<StatusCode, Fault> {
    let name = core.as_ref().unit(&unit)?;
    let bond = core.as_ref().bond(&name, &bond)?;
    let face = front(core.as_ref(), &headers, op.as_deref(), "tie").await?;
    let ties = face.ties(&name, &bond, id).await.map_err(Fault::from)?;
    if !ties.iter().any(|row| row.key() == tie) {
        return Err(Fault {
            status: StatusCode::NOT_FOUND,
            note: format!("missing tie {tie}"),
        });
    }
    let fields = cells(&body, &[])?;
    let pairs: Vec<(&str, &str)> = fields
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    face.set_tie(&name, &bond, tie, &pairs)
        .await
        .map_err(Fault::from)?;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn detach<W: Wire>(
    State(core): State<Arc<Core<W>>>,
    Path((unit, id, bond, tie)): Path<(String, i64, String, i64)>,
    headers: HeaderMap,
    op: Option<Extension<Operator>>,
) -> Result<StatusCode, Fault> {
    let name = core.as_ref().unit(&unit)?;
    let bond = core.as_ref().bond(&name, &bond)?;
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

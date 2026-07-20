use crate::*;
use axum::extract::{Request, State};
use axum::http::HeaderMap;
use axum::middleware::Next;
use axum::response::Response;
use keel::Wire;
use keel::{Cell, Operator};

pub(crate) async fn pass<W: Wire + 'static>(
    State(gate): State<Gate<W>>,
    mut req: Request,
    next: Next,
) -> Response {
    if let Some(key) = whom(&gate, req.headers()).await {
        req.extensions_mut().insert(Operator(key));
    }
    next.run(req).await
}

pub(crate) async fn whom<W: Wire>(gate: &Gate<W>, headers: &HeaderMap) -> Option<i64> {
    let key = resolve(gate, headers).await?;
    if barred(gate, key).await {
        return None;
    }
    Some(key)
}

pub(crate) async fn resolve<W: Wire>(gate: &Gate<W>, headers: &HeaderMap) -> Option<i64> {
    let face = gate.core.of(gate.svc);
    if let Some(token) = bearer(headers) {
        let q = format!(r#"from Token where hash = "{}""#, digest(&token));
        return actor(face.query(&q).await.ok()?.rows().first()?);
    }
    let sid = crumb(headers)?;
    let q = format!(r#"from Session where hash = "{}""#, digest(&sid));
    actor(face.query(&q).await.ok()?.rows().first()?)
}

pub(crate) async fn barred<W: Wire>(gate: &Gate<W>, key: i64) -> bool {
    let Some(field) = gate.bar.as_ref() else {
        return false;
    };
    let Some(whom) = gate.core.identity() else {
        return false;
    };
    let q = format!(r#"from {whom} where id = "{key}""#);
    let Ok(pack) = gate.core.of(gate.svc).query(&q).await else {
        return false;
    };
    match pack.rows().first() {
        Some(row) => row.cells().get(field).map(Cell::show) == Some("true".into()),
        None => false,
    }
}

pub(crate) fn actor(row: &keel::Row) -> Option<i64> {
    match row.cells().get("actor") {
        Some(Cell::Int(key)) => Some(*key),
        _ => None,
    }
}

pub(crate) fn bearer(headers: &HeaderMap) -> Option<String> {
    headers
        .get("authorization")?
        .to_str()
        .ok()?
        .strip_prefix("token ")
        .map(str::to_string)
}

pub(crate) fn crumb(headers: &HeaderMap) -> Option<String> {
    let jar = headers.get("cookie")?.to_str().ok()?;
    jar.split(';')
        .filter_map(|part| part.trim().strip_prefix("session="))
        .next()
        .map(str::to_string)
}

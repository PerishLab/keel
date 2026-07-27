use crate::*;
use axum::extract::{Request, State};
use axum::http::HeaderMap;
use axum::middleware::Next;
use axum::response::Response;
use keel::Wire;
use keel::{Op, Operator, form};

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
        let ask = form("Token").when("hash", Op::Eq, &digest(&token));
        return face.one(&ask).await.ok()??.int("actor");
    }
    let sid = crumb(headers)?;
    let ask = form("Session").when("hash", Op::Eq, &digest(&sid));
    face.one(&ask).await.ok()??.int("actor")
}

pub(crate) async fn barred<W: Wire>(gate: &Gate<W>, key: i64) -> bool {
    let Some(field) = gate.bar.as_ref() else {
        return false;
    };
    let Some(whom) = gate.core.identity() else {
        return false;
    };
    let ask = form(whom).when("id", Op::Eq, &key.to_string());
    let Ok(held) = gate.core.of(gate.svc).one(&ask).await else {
        return false;
    };
    held.and_then(|row| row.flag(field)) == Some(true)
}

pub fn bearer(headers: &HeaderMap) -> Option<String> {
    headers
        .get("authorization")?
        .to_str()
        .ok()?
        .strip_prefix("token ")
        .map(str::to_string)
}

pub fn crumb(headers: &HeaderMap) -> Option<String> {
    let jar = headers.get("cookie")?.to_str().ok()?;
    jar.split(';')
        .filter_map(|part| part.trim().strip_prefix("session="))
        .next()
        .map(str::to_string)
}

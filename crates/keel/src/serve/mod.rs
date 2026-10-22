use crate::adapt::Error;
use crate::face::{Core, Face};
use crate::wire::Wire;
use axum::extract::Path;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use serde_json::json;
use serde_json::{Map, Value};
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
        .route(
            "/query",
            post(|seat: Seat<W>, Json(body): Json<Body>| Plea::from(seat).run(body)),
        )
        .route(
            "/batch",
            post(|seat: Seat<W>, Json(body): Json<Batch>| Plea::from(seat).batch(body)),
        )
        .route(
            "/{unit}",
            get(|seat: Seat<W>, Path(unit): Path<String>| Plea::from(seat).list(unit)).post(
                |seat: Seat<W>, Path(unit): Path<String>, Json(body): Json<Map<String, Value>>| {
                    Plea::from(seat).create(unit, body)
                },
            ),
        )
        .route(
            "/{unit}/{id}",
            get(|seat: Seat<W>, Path(at): Path<(String, i64)>| Plea::from(seat).one(at))
                .patch(
                    |seat: Seat<W>,
                     Path(at): Path<(String, i64)>,
                     Json(body): Json<Map<String, Value>>| {
                        Plea::from(seat).edit(at, body)
                    },
                )
                .delete(|seat: Seat<W>, Path(at): Path<(String, i64)>| Plea::from(seat).remove(at)),
        )
        .route(
            "/{unit}/{id}/{bond}",
            get(refuse).post(
                |seat: Seat<W>,
                 Path(at): Path<(String, i64, String)>,
                 Json(body): Json<Map<String, Value>>| {
                    Plea::from(seat).attach(at, body)
                },
            ),
        )
        .route(
            "/{unit}/{id}/{bond}/{tie}",
            patch(
                |seat: Seat<W>, Path(knot): Path<Knot>, Json(body): Json<Map<String, Value>>| {
                    Plea::from(seat).tune(knot, body)
                },
            )
            .delete(|seat: Seat<W>, Path(knot): Path<Knot>| Plea::from(seat).detach(knot)),
        )
        .with_state(core);
    let prefix = prefix.trim_end_matches('/');
    if prefix.is_empty() {
        api
    } else {
        Router::new().nest(prefix, api)
    }
}

pub async fn admit<'a, W: Wire>(
    core: &'a Core<W>,
    headers: &HeaderMap,
    op: Option<&Operator>,
    verb: &str,
) -> Result<Face<'a, W>, StatusCode> {
    let told = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok());
    if let Some(token) = told.and_then(|value| value.strip_prefix("sudo ")) {
        if core
            .seal(token)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        {
            eprintln!("keel: sudo {verb}");
            return Ok(core.sudo());
        }
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(match op {
        Some(Operator(id)) => core.of(*id),
        None => core.anon(),
    })
}

async fn front<'a, W: Wire>(
    core: &'a Core<W>,
    headers: &HeaderMap,
    op: Option<&Operator>,
    verb: &str,
) -> Result<Face<'a, W>, Fault> {
    admit(core, headers, op, verb)
        .await
        .map_err(|status| Fault {
            status,
            note: if status == StatusCode::UNAUTHORIZED {
                "bad sudo token".into()
            } else {
                "sudo verification failed".into()
            },
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

async fn refuse() -> StatusCode {
    StatusCode::NOT_FOUND
}

mod fault;
mod json;
mod plea;
mod route;
mod write;

pub(crate) use fault::*;
pub(crate) use json::*;
pub(crate) use plea::*;
pub(crate) use route::*;
pub(crate) use write::*;

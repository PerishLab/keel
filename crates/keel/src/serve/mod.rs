use crate::face::{Core, Face};
use crate::wire::Wire;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use serde_json::json;
use serde_json::{Map, Value};
use std::sync::Arc;

#[derive(Clone, Copy, Debug)]
pub struct Operator(pub i64);

pub fn app<W: Wire + 'static>(core: Arc<Core<W>>, prefix: &str) -> Router {
    let api = Router::new()
        .route("/health", get(health::<W>))
        .route(
            "/-/keel/row/{unit}/{id}",
            get(|seat: Seat<W>, Path(at): Path<(String, i64)>| Plea::from(seat).trace(at)),
        )
        .route(
            "/-/keel/pulse",
            get(|seat: Seat<W>, Query(after): Query<After>| Plea::from(seat).pulse(after)),
        )
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
            .map_err(|err| Fault::from(err).status)?
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

async fn health<W: Wire + 'static>(State(core): State<Arc<Core<W>>>) -> Result<Json<Value>, Fault> {
    core.touch().await.map_err(Fault::from)?;
    Ok(Json(json!({ "ok": true })))
}

async fn refuse() -> StatusCode {
    StatusCode::NOT_FOUND
}

mod desk;
mod fault;
mod json;
mod plea;
mod route;
mod write;

pub(crate) use desk::*;
pub(crate) use fault::*;
pub(crate) use json::*;
pub(crate) use plea::*;
pub(crate) use route::*;
pub(crate) use write::*;

use crate::adapt::Error;
use crate::face::{Core, Face};
use crate::wire::Wire;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use serde_json::json;
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
        .route("/batch", post(batch::<W>))
        .route("/{unit}", get(list::<W>).post(create::<W>))
        .route(
            "/{unit}/{id}",
            get(one::<W>).patch(edit::<W>).delete(remove::<W>),
        )
        .route("/{unit}/{id}/{bond}", get(refuse).post(attach::<W>))
        .route(
            "/{unit}/{id}/{bond}/{tie}",
            patch(tune::<W>).delete(detach::<W>),
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

mod fault;
mod json;
mod route;
mod write;

pub(crate) use fault::*;
pub(crate) use json::*;
pub(crate) use route::*;
pub(crate) use write::*;

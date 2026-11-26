mod bucket;
mod catalog;
mod collect;
mod make;
mod upload;

pub use catalog::catalog;

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use bucket::Store;
use catalog::{Binding, by};
use keel::Wire;
use keel::adapt::Error;
use keel::{Core, Operator};
use serde_json::{Map, Value, json};
use std::sync::Arc;
use upload::Upload;

pub const TTL: u64 = 60 * 10;

pub struct Vault<W: Wire> {
    core: Arc<Core<W>>,
    store: Arc<Store>,
    prefix: Arc<str>,
}

impl<W: Wire> Clone for Vault<W> {
    fn clone(&self) -> Self {
        Self {
            core: self.core.clone(),
            store: self.store.clone(),
            prefix: self.prefix.clone(),
        }
    }
}

pub struct Shed<'a> {
    pub endpoint: &'a str,
    pub name: &'a str,
    pub region: &'a str,
    pub key: &'a str,
    pub secret: &'a str,
}

impl<W: Wire + 'static> Vault<W> {
    pub async fn open(core: Arc<Core<W>>, shed: Shed<'_>) -> Result<Self, Error> {
        let store = Store::open(shed).await?;
        let prefix = catalog::prefix(&core, store.address()).await?;
        let vault = Self {
            core,
            store: Arc::new(store),
            prefix: prefix.into(),
        };
        vault.watch();
        Ok(vault)
    }

    pub fn shelf(self, router: Router) -> Router {
        router.merge(self.doors())
    }

    fn doors(&self) -> Router {
        Router::new()
            .route("/asset", post(stow::<W>))
            .route("/asset/{id}", get(fetch::<W>))
            .with_state(self.clone())
    }
}

async fn stow<W: Wire + 'static>(
    State(vault): State<Vault<W>>,
    op: Option<axum::Extension<Operator>>,
    Json(body): Json<Map<String, Value>>,
) -> Result<(StatusCode, Json<Value>), Fault> {
    let who = op
        .map(|axum::Extension(Operator(id))| id)
        .ok_or(Fault::shut())?;
    let upload = Upload::read(&body)?;
    let object = format!("{}{}", vault.prefix, catalog::nonce().map_err(Fault::from)?);
    let fields = flat(&body)?;
    let pairs: Vec<(&str, &str)> = fields
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let id = vault
        .core
        .of(who)
        .put("Asset", &pairs)
        .await
        .map_err(Fault::from)?;
    let bound = Binding {
        asset: id.to_string(),
        object,
        size: upload.size,
        hash: upload.hash.clone(),
    };
    bound.record(&vault.core).await.map_err(Fault::from)?;
    let headers = upload.headers();
    let put = vault.store.put(&bound.object, &headers);
    let headers: std::collections::BTreeMap<_, _> = headers.into_iter().collect();
    Ok((
        StatusCode::CREATED,
        Json(json!({ "id": id, "put": put.as_str(), "headers": headers })),
    ))
}

async fn fetch<W: Wire + 'static>(
    State(vault): State<Vault<W>>,
    Path(id): Path<i64>,
    op: Option<axum::Extension<Operator>>,
) -> Result<Response, Fault> {
    let who = op.map(|axum::Extension(Operator(id))| id);
    let target = id.to_string();
    let query = by("Asset", "id", &target);
    let face = match who {
        Some(who) => vault.core.of(who),
        None => vault.core.anon(),
    };
    let asset = face
        .one(&query)
        .await
        .map_err(Fault::from)?
        .ok_or_else(Fault::miss)?;
    let bound = vault
        .core
        .sudo()
        .one(&by(catalog::PARCEL, "asset", &target))
        .await
        .map_err(Fault::from)?
        .ok_or_else(Fault::miss)?;
    let binding = Binding::read(&bound).map_err(Fault::from)?;
    if !binding.matches(&asset, &vault.prefix) {
        return Err(Fault::miss());
    }
    let mut headers = HeaderMap::new();
    headers.insert(
        header::LOCATION,
        vault.store.get(&binding.object).as_str().parse().unwrap(),
    );
    Ok((StatusCode::FOUND, headers).into_response())
}

fn flat(body: &Map<String, Value>) -> Result<Vec<(String, String)>, Fault> {
    let mut out = Vec::new();
    for (key, value) in body {
        let text = match value {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            _ => return Err(Fault::bad()),
        };
        out.push((key.clone(), text));
    }
    Ok(out)
}

pub struct Fault {
    status: StatusCode,
    note: String,
}

impl Fault {
    fn shut() -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            note: "no operator".into(),
        }
    }

    fn miss() -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            note: "missing asset".into(),
        }
    }

    fn bad() -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            note: "bad request".into(),
        }
    }
}

impl From<keel::adapt::Error> for Fault {
    fn from(err: keel::adapt::Error) -> Self {
        let note = err.to_string();
        let status = if let keel::adapt::Error::Store(store) = &err {
            match store {
                keel::adapt::Store::Down(_) => StatusCode::SERVICE_UNAVAILABLE,
                keel::adapt::Store::Taken(_) => StatusCode::CONFLICT,
                keel::adapt::Store::Fault(_) => StatusCode::INTERNAL_SERVER_ERROR,
            }
        } else if note.contains("refused") {
            StatusCode::FORBIDDEN
        } else if note.ends_with(" taken") {
            StatusCode::CONFLICT
        } else {
            StatusCode::BAD_REQUEST
        };
        Self { status, note }
    }
}

impl IntoResponse for Fault {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": self.note }))).into_response()
    }
}

#[cfg(test)]
mod tests;

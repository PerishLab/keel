mod make;

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use keel::Wire;
use keel::{Core, Operator};
use rusty_s3::actions::S3Action;
use rusty_s3::{Bucket, Credentials, UrlStyle};
use serde_json::{Map, Value, json};
use std::sync::Arc;
use std::time::Duration;
use url::Url;

pub const TTL: u64 = 60 * 10;

pub struct Vault<W: Wire> {
    core: Arc<Core<W>>,
    bucket: Bucket,
    creds: Credentials,
}

impl<W: Wire> Clone for Vault<W> {
    fn clone(&self) -> Self {
        Self {
            core: self.core.clone(),
            bucket: self.bucket.clone(),
            creds: self.creds.clone(),
        }
    }
}

impl<W: Wire + 'static> Vault<W> {
    pub fn open(
        core: Arc<Core<W>>,
        endpoint: &str,
        name: &str,
        region: &str,
        key: &str,
        secret: &str,
    ) -> Result<Self, keel::adapt::Error> {
        let base: Url = endpoint
            .parse()
            .map_err(|_| keel::adapt::Error::Adapt("bad s3 endpoint".into()))?;
        let bucket = Bucket::new(base, UrlStyle::Path, name.to_string(), region.to_string())
            .map_err(|_| keel::adapt::Error::Adapt("bad s3 bucket".into()))?;
        let creds = Credentials::new(key, secret);
        Ok(Self {
            core,
            bucket,
            creds,
        })
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

    fn put(&self, id: i64) -> Url {
        self.bucket
            .put_object(Some(&self.creds), &object(id))
            .sign(Duration::from_secs(TTL))
    }

    fn get(&self, id: i64) -> Url {
        self.bucket
            .get_object(Some(&self.creds), &object(id))
            .sign(Duration::from_secs(TTL))
    }
}

fn object(id: i64) -> String {
    format!("asset/{id}")
}

async fn stow<W: Wire + 'static>(
    State(vault): State<Vault<W>>,
    op: Option<axum::Extension<Operator>>,
    Json(body): Json<Map<String, Value>>,
) -> Result<(StatusCode, Json<Value>), Fault> {
    let who = op
        .map(|axum::Extension(Operator(id))| id)
        .ok_or(Fault::shut())?;
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
    Ok((
        StatusCode::CREATED,
        Json(json!({ "id": id, "put": vault.put(id).to_string() })),
    ))
}

async fn fetch<W: Wire + 'static>(
    State(vault): State<Vault<W>>,
    Path(id): Path<i64>,
    op: Option<axum::Extension<Operator>>,
) -> Result<Response, Fault> {
    let who = op.map(|axum::Extension(Operator(id))| id);
    let seen = match who {
        Some(who) => vault
            .core
            .of(who)
            .query(&format!(r#"from Asset where id = "{id}""#))
            .await
            .map_err(Fault::from)?,
        None => vault
            .core
            .anon()
            .query(&format!(r#"from Asset where id = "{id}""#))
            .await
            .map_err(Fault::from)?,
    };
    if seen.rows().first().map(|row| row.key()) != Some(id) {
        return Err(Fault::miss());
    }
    let mut headers = HeaderMap::new();
    headers.insert(header::LOCATION, vault.get(id).to_string().parse().unwrap());
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
        let status = if note.contains("refused") {
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

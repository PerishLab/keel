mod make;
mod rite;

use axum::http::StatusCode;
use axum::middleware::{self};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use keel::Core;
use keel::Wire;
use serde_json::{Map, Value, json};
use std::sync::Arc;

pub const TTL: i64 = 60 * 60 * 24 * 14;

pub struct Gate<W: Wire> {
    core: Arc<Core<W>>,
    svc: i64,
    bar: Option<String>,
    secure: bool,
}

impl<W: Wire> Clone for Gate<W> {
    fn clone(&self) -> Self {
        Self {
            core: self.core.clone(),
            svc: self.svc,
            bar: self.bar.clone(),
            secure: self.secure,
        }
    }
}

impl<W: Wire + 'static> Gate<W> {
    pub async fn rise(core: Arc<Core<W>>, svc: i64) -> Result<Self, keel::adapt::Error> {
        let whom = core.identity().unwrap_or("").to_string();
        let ask = keel::form("@grant")
            .when("who", keel::Op::Eq, &svc.to_string())
            .count();
        let held = core.sudo().ask(&ask).await?;
        let gate = Self {
            core,
            svc,
            bar: None,
            secure: false,
        };
        if held.count() == Some(0) {
            let who = svc.to_string();
            gate.sow(&[
                (&who, "see", "Token", "all"),
                (&who, "see", "Session", "all"),
                (&who, "see", &whom, "all"),
                (&who, "put", "Session", "all"),
            ])
            .await?;
        }
        Ok(gate)
    }

    pub fn bar(mut self, field: &str) -> Self {
        self.bar = Some(field.to_string());
        self
    }

    pub fn secure(mut self) -> Self {
        self.secure = true;
        self
    }

    pub fn wall(self, router: Router) -> Router {
        let doors = self.doors();
        self.screen(router.merge(doors))
    }

    pub fn screen(self, router: Router) -> Router {
        router.layer(middleware::from_fn_with_state(self, pass::<W>))
    }

    fn doors(&self) -> Router {
        Router::new()
            .route("/register", post(register::<W>))
            .route("/login", post(login::<W>))
            .route("/logout", post(logout::<W>))
            .route("/revoke", post(revoke::<W>))
            .with_state(self.clone())
    }
}

pub(crate) fn flat(body: &Map<String, Value>) -> Result<Vec<(String, String)>, Deny> {
    let mut out = Vec::new();
    for (key, value) in body {
        let text = match value {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            _ => return Err(Deny::misfit()),
        };
        out.push((key.clone(), text));
    }
    Ok(out)
}

pub fn wild() -> String {
    let mut seed = [0u8; 32];
    getrandom::fill(&mut seed).expect("os entropy");
    seed.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn bake(sid: &str, secure: bool) -> String {
    let mut jar = format!("session={sid}; HttpOnly; SameSite=Lax; Path=/");
    if secure {
        jar.push_str("; Secure");
    }
    jar
}

pub fn digest(token: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub struct Deny {
    status: StatusCode,
    note: String,
}

impl Deny {
    pub fn status(&self) -> StatusCode {
        self.status
    }

    pub fn note(&self) -> &str {
        &self.note
    }

    fn misfit() -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            note: "bad request".into(),
        }
    }

    fn gone(unit: &str) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            note: format!("no {}", unit.to_lowercase()),
        }
    }
}

impl From<keel::adapt::Error> for Deny {
    fn from(err: keel::adapt::Error) -> Self {
        let note = err.to_string();
        let status = if note.contains("refused") {
            StatusCode::FORBIDDEN
        } else if note.contains("missing") {
            StatusCode::NOT_FOUND
        } else {
            StatusCode::BAD_REQUEST
        };
        Self { status, note }
    }
}

impl IntoResponse for Deny {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": self.note }))).into_response()
    }
}

pub(crate) use door::*;
pub(crate) use guard::*;

pub use guard::{bearer, crumb};

mod door;
mod guard;

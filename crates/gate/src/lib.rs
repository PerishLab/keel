mod make;

use axum::extract::{Request, State};
use axum::http::{HeaderMap, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use keel::Wire;
use keel::{Cell, Core, Operator};
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
        let sudo = core.sudo();
        let held = sudo
            .query(&format!(r#"from @grant where who = "{svc}" count"#))
            .await?;
        if held.count() == Some(0) {
            let whom = core.identity().unwrap_or("").to_string();
            for (verb, unit) in [
                ("see", "Token"),
                ("see", "Session"),
                ("see", whom.as_str()),
                ("put", "Session"),
            ] {
                sudo.put(
                    "@grant",
                    &[
                        ("who", &svc.to_string()),
                        ("verb", verb),
                        ("unit", unit),
                        ("scope", "all"),
                    ],
                )
                .await?;
            }
        }
        Ok(Self {
            core,
            svc,
            bar: None,
            secure: false,
        })
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

async fn pass<W: Wire + 'static>(
    State(gate): State<Gate<W>>,
    mut req: Request,
    next: Next,
) -> Response {
    if let Some(key) = whom(&gate, req.headers()).await {
        req.extensions_mut().insert(Operator(key));
    }
    next.run(req).await
}

async fn whom<W: Wire>(gate: &Gate<W>, headers: &HeaderMap) -> Option<i64> {
    let key = resolve(gate, headers).await?;
    if barred(gate, key).await {
        return None;
    }
    Some(key)
}

async fn resolve<W: Wire>(gate: &Gate<W>, headers: &HeaderMap) -> Option<i64> {
    let face = gate.core.of(gate.svc);
    if let Some(token) = bearer(headers) {
        let q = format!(r#"from Token where hash = "{}""#, digest(&token));
        return actor(face.query(&q).await.ok()?.rows().first()?);
    }
    let sid = crumb(headers)?;
    let q = format!(r#"from Session where hash = "{}""#, digest(&sid));
    actor(face.query(&q).await.ok()?.rows().first()?)
}

async fn barred<W: Wire>(gate: &Gate<W>, key: i64) -> bool {
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

fn actor(row: &keel::Row) -> Option<i64> {
    match row.cells().get("actor") {
        Some(Cell::Int(key)) => Some(*key),
        _ => None,
    }
}

fn bearer(headers: &HeaderMap) -> Option<String> {
    headers
        .get("authorization")?
        .to_str()
        .ok()?
        .strip_prefix("token ")
        .map(str::to_string)
}

fn crumb(headers: &HeaderMap) -> Option<String> {
    let jar = headers.get("cookie")?.to_str().ok()?;
    jar.split(';')
        .filter_map(|part| part.trim().strip_prefix("session="))
        .next()
        .map(str::to_string)
}

async fn register<W: Wire + 'static>(
    State(gate): State<Gate<W>>,
    Json(body): Json<Map<String, Value>>,
) -> Result<(StatusCode, Json<Value>), Deny> {
    let whom = gate.core.identity().ok_or(Deny::misfit())?.to_string();
    let fields = flat(&body)?;
    let pairs: Vec<(&str, &str)> = fields
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let key = gate
        .core
        .anon()
        .put(&whom, &pairs)
        .await
        .map_err(Deny::from)?;
    let token = wild();
    gate.core
        .put(
            "Token",
            &[
                ("name", "first"),
                ("hash", &digest(&token)),
                ("actor", &key.to_string()),
            ],
        )
        .await
        .map_err(Deny::from)?;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "id": key, "token": token })),
    ))
}

async fn login<W: Wire + 'static>(
    State(gate): State<Gate<W>>,
    Json(body): Json<Map<String, Value>>,
) -> Result<(StatusCode, HeaderMap, Json<Value>), Deny> {
    let token = body
        .get("token")
        .and_then(Value::as_str)
        .ok_or(Deny::misfit())?;
    let face = gate.core.of(gate.svc);
    let q = format!(r#"from Token where hash = "{}""#, digest(token));
    let pack = face.query(&q).await.map_err(Deny::from)?;
    let Some(key) = pack.rows().first().and_then(actor) else {
        return Err(Deny {
            status: StatusCode::UNAUTHORIZED,
            note: "unknown token".into(),
        });
    };
    let sid = wild();
    let row = face
        .put(
            "Session",
            &[("hash", &digest(&sid)), ("actor", &key.to_string())],
        )
        .await
        .map_err(Deny::from)?;
    gate.core
        .lease("Session", row, now() + TTL)
        .await
        .map_err(Deny::from)?;
    let mut headers = HeaderMap::new();
    let jar = bake(&sid, gate.secure);
    headers.insert("set-cookie", jar.parse().map_err(|_| Deny::misfit())?);
    Ok((StatusCode::CREATED, headers, Json(json!({ "id": row }))))
}

async fn logout<W: Wire + 'static>(
    State(gate): State<Gate<W>>,
    headers: HeaderMap,
) -> Result<StatusCode, Deny> {
    let sid = crumb(&headers).ok_or(Deny::misfit())?;
    let face = gate.core.of(gate.svc);
    let q = format!(r#"from Session where hash = "{}""#, digest(&sid));
    let pack = face.query(&q).await.map_err(Deny::from)?;
    let Some(row) = pack.rows().first() else {
        return Err(Deny {
            status: StatusCode::NOT_FOUND,
            note: "no session".into(),
        });
    };
    let Some(key) = actor(row) else {
        return Err(Deny::misfit());
    };
    gate.core
        .of(key)
        .end("Session", row.key())
        .await
        .map_err(Deny::from)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn revoke<W: Wire + 'static>(
    State(gate): State<Gate<W>>,
    headers: HeaderMap,
) -> Result<StatusCode, Deny> {
    let token = bearer(&headers).ok_or(Deny::misfit())?;
    let face = gate.core.of(gate.svc);
    let q = format!(r#"from Token where hash = "{}""#, digest(&token));
    let pack = face.query(&q).await.map_err(Deny::from)?;
    let Some(row) = pack.rows().first() else {
        return Err(Deny {
            status: StatusCode::NOT_FOUND,
            note: "no token".into(),
        });
    };
    let Some(key) = actor(row) else {
        return Err(Deny::misfit());
    };
    gate.core
        .of(key)
        .end("Token", row.key())
        .await
        .map_err(Deny::from)?;
    Ok(StatusCode::NO_CONTENT)
}

fn flat(body: &Map<String, Value>) -> Result<Vec<(String, String)>, Deny> {
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

fn wild() -> String {
    let mut seed = [0u8; 32];
    getrandom::fill(&mut seed).expect("os entropy");
    seed.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn bake(sid: &str, secure: bool) -> String {
    let mut jar = format!("session={sid}; HttpOnly; SameSite=Lax; Path=/");
    if secure {
        jar.push_str("; Secure");
    }
    jar
}

fn digest(token: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn now() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub struct Deny {
    status: StatusCode,
    note: String,
}

impl Deny {
    fn misfit() -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            note: "bad request".into(),
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

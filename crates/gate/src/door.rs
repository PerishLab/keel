use crate::*;
use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use keel::Wire;
use serde_json::{Map, Value, json};

pub(crate) async fn register<W: Wire + 'static>(
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

pub(crate) async fn login<W: Wire + 'static>(
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

pub(crate) async fn logout<W: Wire + 'static>(
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

pub(crate) async fn revoke<W: Wire + 'static>(
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

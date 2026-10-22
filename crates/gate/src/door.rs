use crate::*;
use axum::Json;
use axum::http::{HeaderMap, StatusCode};
use keel::{Op, Wire, form};
use serde_json::{Map, Value, json};

impl<W: Wire + 'static> Gate<W> {
    pub(crate) async fn register(
        &self,
        body: Map<String, Value>,
    ) -> Result<(StatusCode, Json<Value>), Deny> {
        let whom = self.core.identity().ok_or(Deny::misfit())?.to_string();
        let fields = flat(&body)?;
        let pairs: Vec<(&str, &str)> = fields
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        let key = self
            .core
            .anon()
            .put(&whom, &pairs)
            .await
            .map_err(Deny::from)?;
        let token = self.token(key, "first").await.map_err(Deny::from)?;
        Ok((
            StatusCode::CREATED,
            Json(json!({ "id": key, "token": token })),
        ))
    }

    pub(crate) async fn login(
        &self,
        body: Map<String, Value>,
    ) -> Result<(StatusCode, HeaderMap, Json<Value>), Deny> {
        let token = body
            .get("token")
            .and_then(Value::as_str)
            .ok_or(Deny::misfit())?;
        let face = self.core.of(self.svc);
        let ask = form("Token").when("hash", Op::Eq, &digest(token));
        let held = face.one(&ask).await.map_err(Deny::from)?;
        let Some(key) = held.and_then(|row| row.int("actor")) else {
            return Err(Deny {
                status: StatusCode::UNAUTHORIZED,
                note: "unknown token".into(),
            });
        };
        let (row, sid) = self.session(key).await.map_err(Deny::from)?;
        let mut headers = HeaderMap::new();
        let jar = bake(&sid, self.secure);
        headers.insert("set-cookie", jar.parse().map_err(|_| Deny::misfit())?);
        Ok((StatusCode::CREATED, headers, Json(json!({ "id": row }))))
    }

    pub(crate) async fn quit(&self, headers: HeaderMap) -> Result<StatusCode, Deny> {
        let sid = crumb(&headers).ok_or(Deny::misfit())?;
        self.logout(&sid).await?;
        Ok(StatusCode::NO_CONTENT)
    }

    pub(crate) async fn annul(&self, headers: HeaderMap) -> Result<StatusCode, Deny> {
        let token = bearer(&headers).ok_or(Deny::misfit())?;
        self.revoke(&token).await?;
        Ok(StatusCode::NO_CONTENT)
    }
}

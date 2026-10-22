use super::*;
use crate::wire::Wire;
use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::{Map, Value, json};

#[derive(Deserialize)]
pub(crate) struct Knot {
    unit: String,
    id: i64,
    bond: String,
    tie: i64,
}

#[derive(Deserialize)]
pub(crate) struct Body {
    q: String,
}

impl<W: Wire> Plea<W> {
    pub(crate) async fn run(self, body: Body) -> Result<Json<Value>, Fault> {
        let face = self.face("see").await?;
        let tree = crate::query::parse(&body.q).map_err(Fault::from)?;
        let units = tree.involved(self.core.plan()).map_err(Fault::from)?;
        if self.core.plan().shrouds(&units) {
            return Err(Fault {
                status: StatusCode::NOT_FOUND,
                note: "no such unit".into(),
            });
        }
        let pack = face.ask(&tree).await.map_err(Fault::from)?;
        Ok(Json(pack.emit()))
    }

    pub(crate) async fn list(self, unit: String) -> Result<Json<Value>, Fault> {
        let name = self.core.unit(&unit)?;
        let face = self.face("see").await?;
        let rows = face.live(&name).await.map_err(Fault::from)?;
        let body: Vec<Value> = rows.iter().map(Emit::emit).collect();
        Ok(Json(Value::Array(body)))
    }

    pub(crate) async fn create(
        self,
        unit: String,
        body: Map<String, Value>,
    ) -> Result<(StatusCode, Json<Value>), Fault> {
        let name = self.core.unit(&unit)?;
        let fields = cells(&body, &[])?;
        let pairs: Vec<(&str, &str)> = fields
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        let face = self.face("put").await?;
        let key = face.put(&name, &pairs).await.map_err(Fault::from)?;
        Ok((StatusCode::CREATED, Json(json!({ "id": key }))))
    }

    pub(crate) async fn one(self, (unit, id): (String, i64)) -> Result<Json<Value>, Fault> {
        let name = self.core.unit(&unit)?;
        let q = format!(r#"from {name} where id = "{id}""#);
        let face = self.face("see").await?;
        let pack = face.query(&q).await.map_err(Fault::from)?;
        match pack.rows().first() {
            Some(row) => Ok(Json(row.emit())),
            None => Err(Fault {
                status: StatusCode::NOT_FOUND,
                note: format!("missing row {id}"),
            }),
        }
    }

    pub(crate) async fn edit(
        self,
        (unit, id): (String, i64),
        body: Map<String, Value>,
    ) -> Result<Response, Fault> {
        let name = self.core.unit(&unit)?;
        let fields = cells(&body, &[])?;
        let pairs: Vec<(&str, &str)> = fields
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        let face = self.face("set").await?;
        face.set(&name, id, &pairs).await.map_err(Fault::from)?;
        let q = format!(r#"from {name} where id = "{id}""#);
        let pack = face.query(&q).await.map_err(Fault::from)?;
        match pack.rows().first() {
            Some(row) => Ok(Json(row.emit()).into_response()),
            None => Ok(StatusCode::NO_CONTENT.into_response()),
        }
    }

    pub(crate) async fn remove(self, (unit, id): (String, i64)) -> Result<StatusCode, Fault> {
        let name = self.core.unit(&unit)?;
        let face = self.face("end").await?;
        face.end(&name, id).await.map_err(Fault::from)?;
        Ok(StatusCode::NO_CONTENT)
    }

    pub(crate) async fn attach(
        self,
        (unit, id, bond): (String, i64, String),
        body: Map<String, Value>,
    ) -> Result<(StatusCode, Json<Value>), Fault> {
        let name = self.core.unit(&unit)?;
        let bond = self.core.bond(&name, &bond)?;
        let right = body
            .get("right")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| Fault::bad("right needs integer".into()))?;
        let fields = cells(&body, &["right"])?;
        let pairs: Vec<(&str, &str)> = fields
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        let face = self.face("tie").await?;
        let key = face
            .tie(&name, &bond, crate::life::Ends { left: id, right }, &pairs)
            .await
            .map_err(Fault::from)?;
        Ok((StatusCode::CREATED, Json(json!({ "id": key }))))
    }

    pub(crate) async fn tune(
        self,
        Knot {
            unit,
            id,
            bond,
            tie,
        }: Knot,
        body: Map<String, Value>,
    ) -> Result<StatusCode, Fault> {
        let name = self.core.unit(&unit)?;
        let bond = self.core.bond(&name, &bond)?;
        let face = self.face("tie").await?;
        let ties = face.ties(&name, &bond, id).await.map_err(Fault::from)?;
        if !ties.iter().any(|row| row.key() == tie) {
            return Err(Fault {
                status: StatusCode::NOT_FOUND,
                note: format!("missing tie {tie}"),
            });
        }
        let fields = cells(&body, &[])?;
        let pairs: Vec<(&str, &str)> = fields
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        face.tune(&name, &bond, tie, &pairs)
            .await
            .map_err(Fault::from)?;
        Ok(StatusCode::NO_CONTENT)
    }

    pub(crate) async fn detach(
        self,
        Knot {
            unit,
            id,
            bond,
            tie,
        }: Knot,
    ) -> Result<StatusCode, Fault> {
        let name = self.core.unit(&unit)?;
        let bond = self.core.bond(&name, &bond)?;
        let face = self.face("cut").await?;
        let ties = face.ties(&name, &bond, id).await.map_err(Fault::from)?;
        if !ties.iter().any(|row| row.key() == tie) {
            return Err(Fault {
                status: StatusCode::NOT_FOUND,
                note: format!("missing tie {tie}"),
            });
        }
        face.cut(&name, &bond, tie).await.map_err(Fault::from)?;
        Ok(StatusCode::NO_CONTENT)
    }
}

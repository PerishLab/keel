use super::*;
use crate::face::Who;
use crate::wire::Wire;
use axum::Json;
use axum::http::StatusCode;
use serde::Deserialize;
use serde_json::{Map, Value, json};

#[derive(Deserialize)]
pub(crate) struct After {
    #[serde(default)]
    after: i64,
}

impl<W: Wire> Plea<W> {
    async fn sudo(&self, verb: &str) -> Result<Face<'_, W>, Fault> {
        let face = self.face(verb).await?;
        if face.who() != Who::Sudo {
            return Err(Fault {
                status: StatusCode::FORBIDDEN,
                note: "refused: sudo only".into(),
            });
        }
        Ok(face)
    }

    pub(crate) async fn trace(self, (unit, id): (String, i64)) -> Result<Json<Value>, Fault> {
        self.sudo("trace").await?;
        let name = crate::query::resolve(self.core.plan(), &unit).map_err(Fault::from)?;
        match self.core.trace(&name, id).await.map_err(Fault::from)? {
            Some(row) => Ok(Json(json!({
                "created_at": row.created(),
                "updated_at": row.updated(),
                "expires_at": row.expires(),
            }))),
            None => Err(Fault {
                status: StatusCode::NOT_FOUND,
                note: format!("missing row {id}"),
            }),
        }
    }

    pub(crate) async fn pulse(self, after: After) -> Result<Json<Value>, Fault> {
        let face = self.sudo("pulse").await?;
        let rows = face.flow(after.after).await.map_err(Fault::from)?;
        let beats = rows
            .iter()
            .map(|row| {
                let mut map = Map::new();
                map.insert("id".into(), json!(row.key()));
                for (k, v) in row.cells() {
                    map.insert(k.clone(), v.emit());
                }
                map.insert("created_at".into(), json!(row.created()));
                Value::Object(map)
            })
            .collect();
        Ok(Json(Value::Array(beats)))
    }
}

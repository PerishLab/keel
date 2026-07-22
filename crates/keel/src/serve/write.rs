use super::*;
use crate::adapt::Error;
use crate::face::{Core, Tx};
use crate::life::Ends;
use crate::wire::Wire;
use axum::Extension;
use axum::Json;
use axum::extract::State;
use axum::http::HeaderMap;
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Deserialize)]
pub(crate) struct Batch {
    deeds: Vec<Deed>,
}

#[derive(Deserialize)]
#[serde(tag = "verb", rename_all = "lowercase")]
enum Deed {
    Put {
        unit: String,
        #[serde(default)]
        fields: Map<String, Value>,
    },
    Set {
        unit: String,
        id: i64,
        #[serde(default)]
        fields: Map<String, Value>,
    },
    End {
        unit: String,
        id: i64,
        at: Option<i64>,
    },
    Tie {
        owner: String,
        bond: String,
        left: i64,
        right: i64,
        #[serde(default)]
        fields: Map<String, Value>,
    },
    Tune {
        owner: String,
        bond: String,
        id: i64,
        #[serde(default)]
        fields: Map<String, Value>,
    },
    Cut {
        owner: String,
        bond: String,
        id: i64,
    },
}

fn pairs(cells: &BTreeMap<String, String>) -> Vec<(&str, &str)> {
    cells
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect()
}

pub(crate) async fn batch<W: Wire + 'static>(
    State(core): State<Arc<Core<W>>>,
    headers: HeaderMap,
    op: Option<Extension<Operator>>,
    Json(body): Json<Batch>,
) -> Result<Json<Value>, Fault> {
    let mut sets: Vec<BTreeMap<String, String>> = Vec::new();
    for deed in &body.deeds {
        if let Some(fields) = deed.fields() {
            sets.push(cells(fields, &[])?);
        } else {
            sets.push(BTreeMap::new());
        }
    }
    let face = front(core.as_ref(), &headers, op.as_deref(), "batch").await?;
    let ids = face
        .batch(async |tx| {
            let mut ids = Vec::new();
            for (deed, cells) in body.deeds.iter().zip(&sets) {
                if let Some(id) = deed.run(tx, cells).await? {
                    ids.push(id);
                }
            }
            Ok(ids)
        })
        .await
        .map_err(Fault::from)?;
    Ok(Json(json!({ "ids": ids })))
}

impl Deed {
    fn fields(&self) -> Option<&Map<String, Value>> {
        match self {
            Deed::Put { fields, .. }
            | Deed::Set { fields, .. }
            | Deed::Tie { fields, .. }
            | Deed::Tune { fields, .. } => Some(fields),
            Deed::End { .. } | Deed::Cut { .. } => None,
        }
    }

    async fn run<W: Wire + 'static>(
        &self,
        tx: &mut Tx<'_, W>,
        cells: &BTreeMap<String, String>,
    ) -> Result<Option<i64>, Error> {
        match self {
            Deed::Put { unit, .. } => tx.put(unit, &pairs(cells)).await.map(Some),
            Deed::Set { unit, id, .. } => tx.set(unit, *id, &pairs(cells)).await.map(|()| None),
            Deed::End { unit, id, at } => match at {
                Some(at) => tx.lease(unit, *id, *at).await.map(|()| None),
                None => tx.end(unit, *id).await.map(|()| None),
            },
            Deed::Tie {
                owner,
                bond,
                left,
                right,
                ..
            } => tx
                .tie(
                    owner,
                    bond,
                    Ends {
                        left: *left,
                        right: *right,
                    },
                    &pairs(cells),
                )
                .await
                .map(Some),
            Deed::Tune {
                owner, bond, id, ..
            } => tx
                .tune(owner, bond, *id, &pairs(cells))
                .await
                .map(|()| None),
            Deed::Cut { owner, bond, id } => tx.cut(owner, bond, *id).await.map(|()| None),
        }
    }
}

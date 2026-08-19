use super::*;
use crate::adapt::Error;
use crate::face::Who;
use crate::life::{Cell, Row, Work};
use crate::plan::Plan;
use crate::query;
use crate::wire::Wire;

pub(crate) struct Hop {
    pub(crate) unit: String,
    pub(crate) key: i64,
    pub(crate) row: Option<Row>,
}

pub(crate) fn descend(anchor: &str, pred: &str, who: Who, chain: &[Hop]) -> bool {
    for hop in chain {
        let Some(row) = hop.row.as_ref() else {
            continue;
        };
        if hop.unit != anchor {
            continue;
        }
        let mark = Mark {
            key: Some(hop.key),
            cells: row.cells(),
        };
        if mark.suits(anchor, pred, who) {
            return true;
        }
    }
    false
}

impl Mark<'_> {
    pub(crate) fn suits(&self, unit: &str, pred: &str, who: Who) -> bool {
        let text = match who {
            Who::Op(id) => pred.replace("\"@me\"", &format!("\"{id}\"")),
            _ if pred.contains("\"@me\"") => return false,
            _ => pred.to_string(),
        };
        let Ok(tree) = query::parse(&format!("from {unit} where {text}")) else {
            return false;
        };
        query::cover(self.key, self.cells, tree.preds())
    }

    pub(crate) async fn anchors<W: Wire>(
        &self,
        plan: &Plan,
        work: &mut Work<'_, W>,
        unit: &str,
    ) -> Result<Vec<Hop>, Error> {
        let mut out = Vec::new();
        if let Some(key) = self.key {
            out.push(Hop {
                unit: unit.to_string(),
                key,
                row: None,
            });
        }
        let mut name = unit.to_string();
        let mut cells = self.cells.clone();
        for _ in 0..DEPTH {
            let Ok(node) = plan.find(&name) else {
                break;
            };
            let Some(edge) = node.root() else {
                break;
            };
            let Some(Cell::Int(up)) = cells.get(edge.name()).cloned() else {
                break;
            };
            let mate = plan.find(edge.target())?;
            let target = mate.key();
            let row = work.one(mate, up).await?;
            out.push(Hop {
                unit: target.clone(),
                key: up,
                row: row.clone(),
            });
            let Some(row) = row else {
                break;
            };
            name = target;
            cells = row.cells().clone();
        }
        Ok(out)
    }
}

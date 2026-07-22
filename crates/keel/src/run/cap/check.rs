use super::*;
use crate::adapt::Error;
use crate::face::Who;
use crate::life::{Cell, Row, Work};
use crate::plan::Plan;
use crate::query;
use crate::wire::Wire;

pub(crate) struct Hop {
    unit: String,
    key: i64,
    row: Option<Row>,
}

pub struct Deed<'a>(&'a Row);

impl Deed<'_> {
    fn who(&self) -> &str {
        cell(self.0, "who")
    }

    fn verb(&self) -> &str {
        cell(self.0, "verb")
    }

    fn place(&self) -> &str {
        cell(self.0, "unit")
    }

    fn anchor(&self, plan: &Plan) -> String {
        let place = self.place();
        if place == "*" {
            return place.to_string();
        }
        plan.find(place)
            .map(|unit| unit.key())
            .unwrap_or_else(|_| place.to_string())
    }

    fn span(&self) -> &str {
        cell(self.0, "scope")
    }

    fn does(&self, verb: &str) -> bool {
        let deed = self.verb();
        deed == "*" || deed == verb
    }

    fn names(&self, who: Who) -> bool {
        match self.who() {
            "anon" => true,
            "all" => matches!(who, Who::Op(_)),
            id => match who {
                Who::Op(op) => id.parse::<i64>().is_ok_and(|n| n == op),
                _ => false,
            },
        }
    }

    async fn bears<W: Wire>(
        &self,
        plan: &Plan,
        work: &mut Work<'_, W>,
        who: Who,
    ) -> Result<bool, Error> {
        if self.names(who) {
            return Ok(true);
        }
        let Who::Op(op) = who else {
            return Ok(false);
        };
        let Some((place, id)) = self.who().split_once(' ') else {
            return Ok(false);
        };
        let Ok(id) = id.parse::<i64>() else {
            return Ok(false);
        };
        let Ok(node) = plan.find(place) else {
            return Ok(false);
        };
        let Some(edge) = node.crew() else {
            return Ok(false);
        };
        let ties = work.ties(node, edge, id).await?;
        Ok(ties.iter().any(|tie| tie.right() == op))
    }

    async fn held<W: Wire>(
        &self,
        plan: &Plan,
        work: &mut Work<'_, W>,
        plea: &Plea<'_>,
        chain: &[Hop],
    ) -> Result<bool, Error> {
        if !self.bears(plan, work, plea.who).await? || !self.does(plea.verb) {
            return Ok(false);
        }
        let span = self.span();
        if span == "all" {
            let anchor = self.anchor(plan);
            return Ok(anchor == "*" || anchor == plea.unit);
        }
        let anchor = self.anchor(plan);
        if let Some(id) = span.strip_prefix("row ") {
            let Ok(id) = id.parse::<i64>() else {
                return Ok(false);
            };
            return Ok(chain.iter().any(|hop| hop.unit == anchor && hop.key == id));
        }
        let Some(pred) = span.strip_prefix("pred ") else {
            return Ok(false);
        };
        if anchor == plea.unit {
            return Ok(plea.mark.suits(plea.unit, pred, plea.who));
        }
        if plea.verb != "see" {
            return Ok(false);
        }
        Ok(descend(&anchor, pred, plea.who, chain))
    }
}

pub async fn check<W: Wire>(
    plan: &Plan,
    wire: &mut W,
    plea: &Plea<'_>,
    deeds: &[Row],
) -> Result<bool, Error> {
    let mut work = Work::new(wire, plan);
    let chain = plea.mark.anchors(plan, &mut work, plea.unit).await?;
    for deed in deeds {
        if Deed(deed).held(plan, &mut work, plea, &chain).await? {
            return Ok(true);
        }
    }
    Ok(false)
}

pub async fn broad<W: Wire>(
    plan: &Plan,
    wire: &mut W,
    plea: &Plea<'_>,
    deeds: &[Row],
) -> Result<bool, Error> {
    let mut work = Work::new(wire, plan);
    for deed in deeds {
        let deed = Deed(deed);
        if !deed.bears(plan, &mut work, plea.who).await? || !deed.does(plea.verb) {
            continue;
        }
        let anchor = deed.anchor(plan);
        let wide = anchor == "*" || anchor == plea.unit;
        if wide && deed.span() == "all" {
            return Ok(true);
        }
    }
    Ok(false)
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

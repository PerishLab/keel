use super::*;
use crate::adapt::Error;
use crate::ddl;
use crate::face::Who;
use crate::life::{Cell, Row, Work};
use crate::plan::{Plan, Unit};
use crate::query;
use crate::wire::Wire;

pub(crate) struct Hop {
    unit: String,
    key: i64,
    row: Option<Row>,
}

pub async fn check<W: Wire>(
    plan: &Plan,
    wire: &mut W,
    plea: &Plea<'_>,
    deeds: &[Row],
) -> Result<bool, Error> {
    let mut work = Work::new(wire, plan);
    let chain = anchors(plan, &mut work, plea.unit, plea.mark).await?;
    for deed in deeds {
        if held(plan, &mut work, deed, plea, &chain).await? {
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
        if !bearer(plan, &mut work, cell(deed, "who"), plea.who).await?
            || !verb_hit(cell(deed, "verb"), plea.verb)
        {
            continue;
        }
        let place = cell(deed, "unit");
        let wide = place == "*" || ddl::table(place) == plea.unit;
        if wide && cell(deed, "scope") == "all" {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(crate) async fn held<W: Wire>(
    plan: &Plan,
    work: &mut Work<'_, W>,
    deed: &Row,
    plea: &Plea<'_>,
    chain: &[Hop],
) -> Result<bool, Error> {
    if !bearer(plan, work, cell(deed, "who"), plea.who).await?
        || !verb_hit(cell(deed, "verb"), plea.verb)
    {
        return Ok(false);
    }
    let place = cell(deed, "unit");
    let span = cell(deed, "scope");
    if span == "all" {
        return Ok(place == "*" || ddl::table(place) == plea.unit);
    }
    if let Some(id) = span.strip_prefix("row ") {
        let Ok(id) = id.parse::<i64>() else {
            return Ok(false);
        };
        let anchor = ddl::table(place);
        return Ok(chain.iter().any(|hop| hop.unit == anchor && hop.key == id));
    }
    if let Some(pred) = span.strip_prefix("pred ") {
        let anchor = ddl::table(place);
        if anchor == plea.unit {
            return Ok(pred_hit(plea.unit, pred, plea.who, plea.mark));
        }
        if plea.verb != "see" {
            return Ok(false);
        }
        return Ok(descend(&anchor, pred, plea.who, chain));
    }
    Ok(false)
}

pub(crate) fn descend(anchor: &str, pred: &str, who: Who, chain: &[Hop]) -> bool {
    for hop in chain {
        if hop.unit != anchor {
            continue;
        }
        let Some(row) = hop.row.as_ref() else {
            continue;
        };
        let mark = Mark {
            key: Some(hop.key),
            cells: row.cells(),
        };
        if pred_hit(anchor, pred, who, &mark) {
            return true;
        }
    }
    false
}

pub(crate) fn pred_hit(unit: &str, pred: &str, who: Who, mark: &Mark<'_>) -> bool {
    let text = match who {
        Who::Op(id) => pred.replace("\"@me\"", &format!("\"{id}\"")),
        _ if pred.contains("\"@me\"") => return false,
        _ => pred.to_string(),
    };
    let Ok(tree) = query::parse(&format!("from {unit} where {text}")) else {
        return false;
    };
    query::cover(mark.key, mark.cells, tree.preds())
}

pub(crate) fn who_hit(deed: &str, who: Who) -> bool {
    match deed {
        "anon" => true,
        "all" => matches!(who, Who::Op(_)),
        id => match who {
            Who::Op(op) => id.parse::<i64>().is_ok_and(|n| n == op),
            _ => false,
        },
    }
}

pub(crate) async fn bearer<W: Wire>(
    plan: &Plan,
    work: &mut Work<'_, W>,
    deed: &str,
    who: Who,
) -> Result<bool, Error> {
    if who_hit(deed, who) {
        return Ok(true);
    }
    let Who::Op(op) = who else {
        return Ok(false);
    };
    let Some((place, id)) = deed.split_once(' ') else {
        return Ok(false);
    };
    let Ok(id) = id.parse::<i64>() else {
        return Ok(false);
    };
    let Some(node) = seat(plan, &ddl::table(place)) else {
        return Ok(false);
    };
    let Some(edge) = node.crew() else {
        return Ok(false);
    };
    let ties = work.ties(node, edge, id).await?;
    Ok(ties.iter().any(|tie| tie.right() == op))
}

pub(crate) fn verb_hit(deed: &str, verb: &str) -> bool {
    deed == "*" || deed == verb
}

pub(crate) async fn anchors<W: Wire>(
    plan: &Plan,
    work: &mut Work<'_, W>,
    unit: &str,
    mark: &Mark<'_>,
) -> Result<Vec<Hop>, Error> {
    let mut out = Vec::new();
    if let Some(key) = mark.key {
        out.push(Hop {
            unit: unit.to_string(),
            key,
            row: None,
        });
    }
    let mut name = unit.to_string();
    let mut cells = mark.cells.clone();
    for _ in 0..DEPTH {
        let Some(node) = seat(plan, &name) else {
            break;
        };
        let Some(edge) = node.root() else {
            break;
        };
        let Some(Cell::Int(up)) = cells.get(edge.name()).cloned() else {
            break;
        };
        let target = ddl::table(edge.target());
        let mate = plan.find(edge.target())?;
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

pub(crate) fn seat<'a>(plan: &'a Plan, table: &str) -> Option<&'a Unit> {
    plan.units()
        .values()
        .find(|node| ddl::table(node.name()) == table)
}

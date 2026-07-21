use super::*;
use crate::adapt::Error;
use crate::ddl;
use crate::face::Who;
use crate::life::{Cell, Row, Work};
use crate::plan::{Plan, Unit};
use crate::query;
use crate::wire::Wire;

pub async fn check<W: Wire>(
    plan: &Plan,
    wire: &mut W,
    who: Who,
    verb: &str,
    unit: &str,
    mark: &Mark<'_>,
) -> Result<bool, Error> {
    let deeds = plan.find(GRANT)?;
    let mut work = Work::new(wire, plan);
    let chain = anchors(plan, &mut work, unit, mark).await?;
    for deed in work.scan(deeds).await? {
        if held(plan, &mut work, &deed, who, verb, unit, mark, &chain).await? {
            return Ok(true);
        }
    }
    Ok(false)
}

pub async fn broad<W: Wire>(
    plan: &Plan,
    wire: &mut W,
    who: Who,
    verb: &str,
    unit: &str,
) -> Result<bool, Error> {
    let deeds = plan.find(GRANT)?;
    let mut work = Work::new(wire, plan);
    for deed in work.scan(deeds).await? {
        if !bearer(plan, &mut work, cell(&deed, "who"), who).await?
            || !verb_hit(cell(&deed, "verb"), verb)
        {
            continue;
        }
        let place = cell(&deed, "unit");
        let wide = place == "*" || ddl::table(place) == unit;
        if wide && cell(&deed, "scope") == "all" {
            return Ok(true);
        }
    }
    Ok(false)
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn held<W: Wire>(
    plan: &Plan,
    work: &mut Work<'_, W>,
    deed: &Row,
    who: Who,
    verb: &str,
    unit: &str,
    mark: &Mark<'_>,
    chain: &[(String, i64)],
) -> Result<bool, Error> {
    if !bearer(plan, work, cell(deed, "who"), who).await? || !verb_hit(cell(deed, "verb"), verb) {
        return Ok(false);
    }
    let place = cell(deed, "unit");
    let span = cell(deed, "scope");
    if span == "all" {
        return Ok(place == "*" || ddl::table(place) == unit);
    }
    if let Some(id) = span.strip_prefix("row ") {
        let Ok(id) = id.parse::<i64>() else {
            return Ok(false);
        };
        let anchor = ddl::table(place);
        return Ok(chain.iter().any(|(u, k)| *u == anchor && *k == id));
    }
    if let Some(pred) = span.strip_prefix("pred ") {
        let anchor = ddl::table(place);
        if anchor == unit {
            return Ok(pred_hit(unit, pred, who, mark));
        }
        if verb != "see" {
            return Ok(false);
        }
        let Some(node) = seat(plan, &anchor) else {
            return Ok(false);
        };
        return descend(work, node, &anchor, pred, who, chain).await;
    }
    Ok(false)
}

pub(crate) async fn descend<W: Wire>(
    work: &mut Work<'_, W>,
    node: &Unit,
    anchor: &str,
    pred: &str,
    who: Who,
    chain: &[(String, i64)],
) -> Result<bool, Error> {
    for (up, id) in chain {
        if up != anchor {
            continue;
        }
        let Some(row) = work.one(node, *id).await? else {
            continue;
        };
        let mark = Mark {
            key: Some(*id),
            cells: row.cells(),
        };
        if pred_hit(anchor, pred, who, &mark) {
            return Ok(true);
        }
    }
    Ok(false)
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
) -> Result<Vec<(String, i64)>, Error> {
    let mut out = Vec::new();
    if let Some(key) = mark.key {
        out.push((unit.to_string(), key));
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
        out.push((target.clone(), up));
        let mate = plan.find(edge.target())?;
        let Some(row) = work.one(mate, up).await? else {
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

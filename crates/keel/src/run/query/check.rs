use super::*;
use crate::adapt::Error;
use crate::atom;
use crate::ddl;
use crate::plan::Plan;

pub fn resolve(plan: &Plan, unit: &str) -> Result<String, Error> {
    let want = ddl::table(unit);
    plan.units()
        .values()
        .find(|node| ddl::table(node.name()) == want)
        .map(|node| node.name().to_string())
        .ok_or_else(|| Error::Missing(unit.into()))
}

pub(crate) fn check_links(plan: &Plan, name: &str, links: &[String]) -> Result<(), Error> {
    let unit = plan
        .units()
        .get(name)
        .ok_or_else(|| Error::Missing(name.into()))?;
    for bond in links {
        edge(unit, bond)?;
    }
    Ok(())
}

pub(crate) fn check(
    plan: &Plan,
    name: &str,
    preds: &[Pred],
    sort: Option<&Sort>,
) -> Result<(), Error> {
    let unit = plan
        .units()
        .get(name)
        .ok_or_else(|| Error::Missing(name.into()))?;
    for pred in preds {
        check_pred(plan, unit, pred)?;
    }
    if let Some(sort) = sort {
        slot(unit, sort.field())?;
    }
    Ok(())
}

pub(crate) fn check_pred(plan: &Plan, unit: &crate::plan::Unit, pred: &Pred) -> Result<(), Error> {
    match pred.op() {
        Op::Has => {
            edge(unit, pred.field())?;
            key_text(pred.value())?;
            Ok(())
        }
        Op::Some => {
            let bond = edge(unit, pred.field())?;
            let nest = pred
                .nest()
                .ok_or_else(|| Error::Adapt("some needs inner".into()))?;
            check_nest(plan, unit, &bond, nest)
        }
        _ => check_cell(unit, pred),
    }
}

pub(crate) fn check_cell(unit: &crate::plan::Unit, pred: &Pred) -> Result<(), Error> {
    let kind = slot(unit, pred.field())?;
    if pred.op() == Op::Like && !matches!(kind, atom::Kind::Text | atom::Kind::Link) {
        return Err(Error::Adapt("like wants a text field".into()));
    }
    for value in pred.values() {
        fit(kind, value)?;
    }
    Ok(())
}

pub(crate) fn fit(kind: atom::Kind, value: &str) -> Result<(), Error> {
    match kind {
        atom::Kind::Text | atom::Kind::Link => Ok(()),
        atom::Kind::Int => key_text(value).map(|_| ()),
        atom::Kind::Bool => match value {
            "true" | "false" => Ok(()),
            _ => Err(Error::Adapt("value needs bool".into())),
        },
    }
}

pub(crate) fn check_nest(
    plan: &Plan,
    unit: &crate::plan::Unit,
    bond: &str,
    nest: &Pred,
) -> Result<(), Error> {
    if matches!(nest.op(), Op::Has | Op::Some) {
        return Err(Error::Adapt("nested bond pred denied".into()));
    }
    let edge = unit
        .bonds()
        .iter()
        .find(|e| e.name() == bond)
        .ok_or_else(|| Error::Adapt(format!("unknown bond {bond}")))?;
    let kind = nest_kind(plan, edge, nest.field())?;
    for value in nest.values() {
        fit(kind, value)?;
    }
    Ok(())
}

pub(crate) fn nest_kind(
    plan: &Plan,
    edge: &crate::plan::Edge,
    field: &str,
) -> Result<atom::Kind, Error> {
    if field == ddl::KEY {
        return Ok(atom::Kind::Int);
    }
    if let Some(slot) = edge.fields().iter().find(|s| s.name() == field) {
        return Ok(slot.kind());
    }
    let target = plan
        .units()
        .get(edge.target())
        .ok_or_else(|| Error::Missing(edge.target().into()))?;
    slot(target, field)
}

pub(crate) fn slot(unit: &crate::plan::Unit, field: &str) -> Result<atom::Kind, Error> {
    if field == ddl::KEY {
        return Ok(atom::Kind::Int);
    }
    if let Some(slot) = unit.fields().iter().find(|slot| slot.name() == field) {
        return Ok(slot.kind());
    }
    let point = unit
        .bonds()
        .iter()
        .any(|e| e.kind().point() && e.name() == field);
    if point {
        return Ok(atom::Kind::Int);
    }
    Err(Error::Adapt(format!("unknown field {field}")))
}

pub fn involved(plan: &Plan, tree: &Tree) -> Result<Vec<String>, Error> {
    let name = resolve(plan, tree.from())?;
    let unit = plan
        .units()
        .get(&name)
        .ok_or_else(|| Error::Missing(name.clone()))?;
    let mut out = vec![ddl::table(&name)];
    for pred in tree.preds() {
        if matches!(pred.op(), Op::Has | Op::Some)
            && let Ok(bond) = edge(unit, pred.field())
            && let Some(e) = unit.bonds().iter().find(|e| e.name() == bond)
        {
            out.push(ddl::table(e.target()));
        }
    }
    for bond in tree.links() {
        if let Ok(bond) = edge(unit, bond)
            && let Some(e) = unit.bonds().iter().find(|e| e.name() == bond)
        {
            out.push(ddl::table(e.target()));
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

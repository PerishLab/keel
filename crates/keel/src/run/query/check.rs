use super::*;
use crate::adapt::Error;
use crate::atom;
use crate::ddl;
use crate::plan::{Plan, Unit};
use std::collections::BTreeMap;

pub(crate) struct Scope {
    unit: Unit,
    name: String,
    range: BTreeMap<String, Unit>,
}

impl Scope {
    pub(crate) fn unit(&self) -> &Unit {
        &self.unit
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn at(&self, name: &str) -> Result<&Unit, Error> {
        self.range
            .get(name)
            .ok_or_else(|| Error::Missing(name.into()))
    }
}

pub(crate) fn analyze(plan: &Plan, tree: &Tree) -> Result<Scope, Error> {
    let name = resolve(plan, tree.from())?;
    let unit = plan
        .units()
        .get(&name)
        .ok_or_else(|| Error::Missing(name.clone()))?
        .clone();
    check(plan, &name, tree.preds(), tree.sort())?;
    check_links(plan, &name, tree.links())?;
    let range = range(plan, &unit, tree)?;
    Ok(Scope { unit, name, range })
}

pub(crate) fn range(
    plan: &Plan,
    unit: &Unit,
    tree: &Tree,
) -> Result<BTreeMap<String, Unit>, Error> {
    let mut out = BTreeMap::new();
    for bond in reached(unit, tree) {
        let target = plan
            .units()
            .get(bond.target())
            .ok_or_else(|| Error::Missing(bond.target().into()))?;
        out.insert(bond.target().to_string(), target.clone());
    }
    Ok(out)
}

pub(crate) fn reached<'a>(unit: &'a Unit, tree: &Tree) -> Vec<&'a crate::plan::Edge> {
    let mut out = Vec::new();
    for pred in tree.preds() {
        if matches!(pred.op(), Op::Has | Op::Some)
            && let Ok(bond) = edge(unit, pred.field())
        {
            out.push(bond);
        }
    }
    for name in tree.links() {
        if let Ok(bond) = edge(unit, name) {
            out.push(bond);
        }
    }
    out
}

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
        unit.kind(sort.field())?;
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
            check_nest(plan, bond, nest)
        }
        _ => check_cell(unit, pred),
    }
}

pub(crate) fn check_cell(unit: &crate::plan::Unit, pred: &Pred) -> Result<(), Error> {
    let kind = unit.kind(pred.field())?;
    if pred.op() == Op::Like && !matches!(kind, atom::Kind::Text | atom::Kind::Link) {
        return Err(Error::Adapt("like wants a text field".into()));
    }
    for value in pred.values() {
        kind.fit(value)?;
    }
    Ok(())
}

pub(crate) fn check_nest(plan: &Plan, edge: &crate::plan::Edge, nest: &Pred) -> Result<(), Error> {
    if matches!(nest.op(), Op::Has | Op::Some) {
        return Err(Error::Adapt("nested bond pred denied".into()));
    }
    let kind = nest_kind(plan, edge, nest.field())?;
    for value in nest.values() {
        kind.fit(value)?;
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
    target.kind(field)
}

pub fn involved(plan: &Plan, tree: &Tree) -> Result<Vec<String>, Error> {
    let name = resolve(plan, tree.from())?;
    let unit = plan
        .units()
        .get(&name)
        .ok_or_else(|| Error::Missing(name.clone()))?;
    let mut out = vec![ddl::table(&name)];
    for bond in reached(unit, tree) {
        out.push(ddl::table(bond.target()));
    }
    out.sort();
    out.dedup();
    Ok(out)
}

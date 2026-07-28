use super::*;
use crate::adapt::Error;
use crate::atom;
use crate::ddl;
use crate::plan::{Plan, Unit};
use std::collections::BTreeMap;

pub struct Scope {
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

    fn check(&self, tree: &Tree) -> Result<(), Error> {
        for pred in tree.preds() {
            self.pred(pred)?;
        }
        for sort in tree.sorts() {
            self.unit.kind(sort.field())?;
        }
        for bond in tree.links() {
            edge(&self.unit, bond)?;
        }
        Ok(())
    }

    fn pred(&self, pred: &Pred) -> Result<(), Error> {
        match pred.op() {
            Op::Has => edge(&self.unit, pred.field()).map(|_| ()),
            Op::Some => {
                let bond = edge(&self.unit, pred.field())?;
                self.nest(bond, inner(pred)?)
            }
            _ => self.cell(pred),
        }
    }

    fn cell(&self, pred: &Pred) -> Result<(), Error> {
        let kind = self.unit.kind(pred.field())?;
        if pred.op() == Op::Like && !matches!(kind, atom::Kind::Text | atom::Kind::Link) {
            return Err(Error::Adapt("like wants a text field".into()));
        }
        Ok(())
    }

    fn nest(&self, bond: &crate::plan::Edge, nest: &Pred) -> Result<(), Error> {
        if matches!(nest.op(), Op::Has | Op::Some) {
            return Err(Error::Adapt("nested bond pred denied".into()));
        }
        self.kind(bond, nest.field()).map(|_| ())
    }

    fn kind(&self, bond: &crate::plan::Edge, field: &str) -> Result<atom::Kind, Error> {
        if field == ddl::KEY {
            return Ok(atom::Kind::Int);
        }
        if let Some(slot) = bond.fields().iter().find(|s| s.name() == field) {
            return Ok(slot.kind());
        }
        self.at(bond.target())?.kind(field)
    }

    pub(crate) fn verify(&self, tree: &Tree) -> Result<(), Error> {
        for pred in tree.preds() {
            self.weigh(pred)?;
        }
        Ok(())
    }

    fn weigh(&self, pred: &Pred) -> Result<(), Error> {
        match pred.op() {
            Op::Has => key(pred.value()).map(|_| ()),
            Op::Some => {
                let bond = edge(&self.unit, pred.field())?;
                let nest = inner(pred)?;
                fits(self.kind(bond, nest.field())?, nest.values())
            }
            Op::Null => Ok(()),
            _ => fits(self.unit.kind(pred.field())?, pred.values()),
        }
    }
}

pub(crate) fn inner(pred: &Pred) -> Result<&Pred, Error> {
    pred.nest()
        .ok_or_else(|| Error::Adapt("some needs inner".into()))
}

pub(crate) fn fits(kind: atom::Kind, values: &[String]) -> Result<(), Error> {
    for value in values {
        kind.fit(value)?;
    }
    Ok(())
}

pub fn analyze(plan: &Plan, tree: &Tree) -> Result<Scope, Error> {
    let name = resolve(plan, tree.from())?;
    let unit = plan
        .units()
        .get(&name)
        .ok_or_else(|| Error::Missing(name.clone()))?
        .clone();
    let range = range(plan, &unit, tree)?;
    let scope = Scope { unit, name, range };
    scope.check(tree)?;
    Ok(scope)
}

pub(crate) fn range(
    plan: &Plan,
    unit: &Unit,
    tree: &Tree,
) -> Result<BTreeMap<String, Unit>, Error> {
    let mut out = BTreeMap::new();
    for bond in reached(unit, tree) {
        let target = plan.find(bond.target())?;
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
    Ok(plan.find(unit)?.key())
}

impl Tree {
    pub fn involved(&self, plan: &Plan) -> Result<Vec<String>, Error> {
        let unit = plan.find(self.from())?;
        let mut out = vec![unit.key()];
        for bond in reached(unit, self) {
            out.push(plan.find(bond.target())?.key());
        }
        out.sort();
        out.dedup();
        Ok(out)
    }
}

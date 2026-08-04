use super::Edge;
use crate::adapt::Error;
use crate::spec::Spec;
use std::collections::BTreeSet;

impl Edge {
    pub fn closure(&self) -> bool {
        self.closure
    }

    pub(crate) fn derived(&self) -> bool {
        self.derived
    }
}

pub(super) fn expand(
    spec: &Spec,
    names: &BTreeSet<String>,
    bonds: &mut Vec<Edge>,
) -> Result<(), Error> {
    for edge in bonds.iter().filter(|edge| edge.closure) {
        check(spec, names, edge)?;
    }
    let made = bonds
        .iter()
        .filter(|edge| edge.closure)
        .map(derive)
        .collect::<Vec<_>>();
    bonds.extend(made);
    Ok(())
}

fn check(spec: &Spec, names: &BTreeSet<String>, edge: &Edge) -> Result<(), Error> {
    if edge.kind != crate::bond::Kind::Many2many {
        return Err(Error::Adapt("closure needs a self many2many".into()));
    }
    if !edge.target.eq_ignore_ascii_case(spec.name()) {
        return Err(Error::Adapt("closure needs a self many2many".into()));
    }
    let name = format!("{}_closure", edge.name);
    if names.contains(&name.to_ascii_lowercase()) {
        return Err(Error::Adapt(format!("duplicate field {name}")));
    }
    Ok(())
}

fn derive(edge: &Edge) -> Edge {
    Edge {
        name: format!("{}_closure", edge.name),
        kind: crate::bond::Kind::Many2many,
        target: edge.target.clone(),
        fields: Vec::new(),
        need: true,
        root: false,
        crew: false,
        closure: false,
        derived: true,
    }
}

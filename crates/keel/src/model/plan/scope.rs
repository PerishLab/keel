use super::{Edge, Slot};
use crate::spec::Only;

pub(super) fn check(fields: &[Slot], bonds: &[Edge]) -> Result<(), crate::adapt::Error> {
    for slot in fields {
        unique(slot, fields, bonds)?;
        if let Some(scope) = slot.serial() {
            let held = bonds
                .iter()
                .any(|edge| edge.kind().point() && edge.name() == scope);
            if !held {
                return Err(crate::adapt::Error::Adapt(format!(
                    "serial scope {scope} is not a ref"
                )));
            }
        }
    }
    Ok(())
}

fn unique(slot: &Slot, fields: &[Slot], bonds: &[Edge]) -> Result<(), crate::adapt::Error> {
    let Only::Per(scopes) = slot.only() else {
        return Ok(());
    };
    for scope in scopes {
        let field = fields
            .iter()
            .find(|field| field.name() == scope && field.name() != slot.name())
            .map(Slot::need);
        let bond = bonds
            .iter()
            .find(|edge| edge.kind().point() && edge.name() == scope)
            .map(Edge::need);
        match field.or(bond) {
            None => {
                return Err(crate::adapt::Error::Adapt(format!(
                    "unique scope {scope} is not a field or ref"
                )));
            }
            Some(false) => {
                return Err(crate::adapt::Error::Adapt(format!(
                    "unique scope {scope} of {} is optional",
                    slot.name()
                )));
            }
            Some(true) => {}
        }
    }
    Ok(())
}

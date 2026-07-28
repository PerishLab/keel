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
    let invalid = scopes.iter().find(|scope| {
        !fields
            .iter()
            .any(|field| field.name() == *scope && field.name() != slot.name())
            && !bonds
                .iter()
                .any(|edge| edge.kind().point() && edge.name() == *scope)
    });
    match invalid {
        Some(scope) => Err(crate::adapt::Error::Adapt(format!(
            "unique scope {scope} is not a field or ref"
        ))),
        None => Ok(()),
    }
}

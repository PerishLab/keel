use super::*;
use crate::adapt::Error;
use crate::atom;
use crate::bond;
use crate::ddl;
use crate::plan::{Edge, Plan, Slot, Unit};
use crate::wire::Val;
use std::collections::BTreeMap;

pub(super) fn find<'a>(plan: &'a Plan, name: &str) -> Result<&'a Unit, Error> {
    if let Some(unit) = plan.units().get(name) {
        return Ok(unit);
    }
    let want = ddl::table(name);
    plan.units()
        .values()
        .find(|unit| ddl::table(unit.name()) == want)
        .ok_or_else(|| Error::Missing(name.into()))
}

pub(super) fn edge<'a>(
    plan: &'a Plan,
    owner: &str,
    bond: &str,
) -> Result<(&'a Unit, &'a Edge), Error> {
    let unit = find(plan, owner)?;
    let edge = unit
        .bonds()
        .iter()
        .find(|edge| edge.name().eq_ignore_ascii_case(bond) && edge.kind() == bond::Kind::Many2many)
        .ok_or_else(|| Error::Adapt(format!("missing bond {bond}")))?;
    Ok((unit, edge))
}

pub(super) fn refs(unit: &Unit) -> impl Iterator<Item = &crate::plan::Edge> {
    unit.bonds().iter().filter(|edge| edge.kind().point())
}

pub(super) fn pluck<'a>(fields: &[(&'a str, &'a str)], name: &str) -> &'a str {
    seek(fields, name).unwrap_or("")
}

pub(super) fn seek<'a>(fields: &[(&'a str, &'a str)], name: &str) -> Option<&'a str> {
    fields.iter().find(|(k, _)| *k == name).map(|(_, v)| *v)
}

pub(super) fn worth(
    slot: &Slot,
    fields: &[(&str, &str)],
    myself: Option<(i64, &Row)>,
) -> Result<Option<Val>, Error> {
    if let Some(raw) = seek(fields, slot.name()) {
        return Ok(Some(bind(slot, raw)?));
    }
    let held = myself.and_then(|(_, row)| row.cells().get(slot.name()));
    Ok(held.map(cell_val))
}

pub(super) fn anchor(
    fields: &[(&str, &str)],
    myself: Option<(i64, &Row)>,
    rel: &str,
) -> Result<Val, Error> {
    if let Some(raw) = seek(fields, rel) {
        if raw.is_empty() {
            return Ok(Val::Null);
        }
        let key = raw
            .parse::<i64>()
            .map_err(|_| Error::Adapt(format!("ref {rel} needs id")))?;
        return Ok(Val::Int(key));
    }
    let held = myself.and_then(|(_, row)| row.cells().get(rel));
    Ok(held.map(cell_val).unwrap_or(Val::Null))
}

pub(super) fn cell_val(cell: &Cell) -> Val {
    match cell {
        Cell::Text(value) => Val::Text(value.clone()),
        Cell::Int(value) => Val::Int(*value),
        Cell::Bool(value) => Val::Int(*value as i64),
    }
}

pub(super) fn sheet(unit: &Unit) -> String {
    let mut cols = vec![ddl::KEY.to_string()];
    for slot in unit.fields() {
        cols.push(ddl::col(slot.name()));
    }
    for edge in refs(unit) {
        cols.push(ddl::col(&ddl::side(edge.name())));
    }
    cols.push(ddl::EXPIRES.to_string());
    cols.push(ddl::CREATED.to_string());
    cols.push(ddl::UPDATED.to_string());
    cols.join(", ")
}

pub(super) fn known(unit: &Unit, name: &str) -> bool {
    unit.fields().iter().any(|s| s.name() == name) || refs(unit).any(|e| e.name() == name)
}

pub(super) fn check(unit: &Unit, fields: &[(&str, &str)]) -> Result<(), Error> {
    for slot in unit.fields() {
        if slot.serial().is_some() {
            if seek(fields, slot.name()).is_some() {
                return Err(Error::Adapt(format!("serial field {}", slot.name())));
            }
            continue;
        }
        if !fields.iter().any(|(k, _)| *k == slot.name()) {
            return Err(Error::Adapt(format!("missing field {}", slot.name())));
        }
    }
    for (k, _) in fields {
        if !known(unit, k) {
            return Err(Error::Adapt(format!("unknown field {k}")));
        }
    }
    Ok(())
}

pub(super) fn part(unit: &Unit, fields: &[(&str, &str)]) -> Result<(), Error> {
    if fields.is_empty() {
        return Err(Error::Adapt("empty set".into()));
    }
    for (k, _) in fields {
        if *k == ddl::KEY || *k == ddl::EXPIRES || *k == ddl::CREATED || *k == ddl::UPDATED {
            return Err(Error::Adapt(format!("control field {k}")));
        }
        let held = unit
            .fields()
            .iter()
            .any(|s| s.name() == *k && s.serial().is_some());
        if held {
            return Err(Error::Adapt(format!("serial field {k}")));
        }
        if !known(unit, k) {
            return Err(Error::Adapt(format!("unknown field {k}")));
        }
    }
    Ok(())
}

pub(super) fn bond_part(edge: &Edge, fields: &[(&str, &str)]) -> Result<(), Error> {
    for (k, _) in fields {
        if *k == "right"
            || *k == "left"
            || *k == ddl::KEY
            || *k == ddl::EXPIRES
            || *k == ddl::CREATED
            || *k == ddl::UPDATED
        {
            return Err(Error::Adapt(format!("control field {k}")));
        }
        if !edge.fields().iter().any(|s| s.name() == *k) {
            return Err(Error::Adapt(format!("unknown field {k}")));
        }
    }
    Ok(())
}

pub(super) fn read_tie(edge: &Edge, line: &[Val]) -> Result<Tie, Error> {
    let key = line[0].int();
    let left = line[1].int();
    let right = line[2].int();
    let expires = line[3].opt();
    let created = line[4].int();
    let updated = line[5].int();
    let mut cells = BTreeMap::new();
    for (i, slot) in edge.fields().iter().enumerate() {
        cells.insert(slot.name().to_string(), pick(slot, &line[6 + i]));
    }
    Ok(Tie {
        key,
        left,
        right,
        cells,
        expires,
        created,
        updated,
    })
}

pub(super) fn read(unit: &Unit, line: &[Val]) -> Result<Row, Error> {
    let key = line[0].int();
    let mut cells = BTreeMap::new();
    let mut at = 1;
    for slot in unit.fields() {
        cells.insert(slot.name().to_string(), pick(slot, &line[at]));
        at += 1;
    }
    for edge in refs(unit) {
        if let Some(key) = line[at].opt() {
            cells.insert(edge.name().to_string(), Cell::Int(key));
        }
        at += 1;
    }
    let expires = line[at].opt();
    let created = line[at + 1].int();
    let updated = line[at + 2].int();
    Ok(Row {
        key,
        cells,
        expires,
        created,
        updated,
    })
}

pub(super) fn pick(slot: &Slot, cell: &Val) -> Cell {
    match slot.kind() {
        atom::Kind::Text | atom::Kind::Link => Cell::Text(cell.text()),
        atom::Kind::Int => Cell::Int(cell.int()),
        atom::Kind::Bool => Cell::Bool(cell.int() != 0),
    }
}

pub(super) fn bind(slot: &Slot, value: &str) -> Result<Val, Error> {
    match slot.kind() {
        atom::Kind::Text | atom::Kind::Link => Ok(Val::Text(value.into())),
        atom::Kind::Int => value
            .parse::<i64>()
            .map(Val::Int)
            .map_err(|_| Error::Adapt(format!("field {} needs int", slot.name()))),
        atom::Kind::Bool => match value {
            "true" => Ok(Val::Int(1)),
            "false" => Ok(Val::Int(0)),
            _ => Err(Error::Adapt(format!("field {} needs bool", slot.name()))),
        },
    }
}

pub(super) fn fit(slots: &[Slot], col: &str, value: &str) -> Result<Val, Error> {
    let slot = slots
        .iter()
        .find(|slot| slot.name() == col)
        .ok_or_else(|| Error::Adapt(format!("unknown field {col}")))?;
    bind(slot, value)
}

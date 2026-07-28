use super::*;
use crate::adapt::Error;
use crate::atom;
use crate::plan::{Edge, Slot, Unit};
use crate::wire::Val;
use std::collections::BTreeMap;

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
        return Ok(Some(slot.bind(raw)?));
    }
    let held = myself.and_then(|(_, row)| row.cells().get(slot.name()));
    Ok(held.map(Val::from))
}

pub(super) fn anchor(
    unit: &Unit,
    fields: &[(&str, &str)],
    myself: Option<(i64, &Row)>,
    name: &str,
) -> Result<Val, Error> {
    if let Some(slot) = unit.fields().iter().find(|slot| slot.name() == name) {
        if let Some(raw) = seek(fields, name) {
            return slot.bind(raw);
        }
        if let Some(held) = myself.and_then(|(_, row)| row.cells().get(name)) {
            return Ok(Val::from(held));
        }
        if let Some(value) = slot.rule().fallback() {
            return slot.bind(value);
        }
        return Ok(Val::Null);
    }
    if let Some(raw) = seek(fields, name) {
        if raw.is_empty() {
            return Ok(Val::Null);
        }
        let key = raw
            .parse::<i64>()
            .map_err(|_| Error::Adapt(format!("ref {name} needs id")))?;
        return Ok(Val::Int(key));
    }
    let held = myself.and_then(|(_, row)| row.cells().get(name));
    Ok(held.map(Val::from).unwrap_or(Val::Null))
}

impl From<&Cell> for Val {
    fn from(cell: &Cell) -> Self {
        match cell {
            Cell::Text(value) => Val::Text(value.clone()),
            Cell::Int(value) => Val::Int(*value),
            Cell::Bool(value) => Val::Int(*value as i64),
        }
    }
}

impl Tie {
    pub(super) fn read(edge: &Edge, line: &[Val]) -> Result<Tie, Error> {
        let key = line[0].int();
        let left = line[1].int();
        let right = line[2].int();
        let expires = line[3].opt();
        let created = line[4].int();
        let updated = line[5].int();
        let mut cells = BTreeMap::new();
        for (i, slot) in edge.fields().iter().enumerate() {
            let cell = pick(slot, &line[6 + i])?
                .ok_or_else(|| Error::Adapt(format!("required field {} is null", slot.name())))?;
            cells.insert(slot.name().to_string(), cell);
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
}

impl Row {
    pub(super) fn read(unit: &Unit, line: &[Val]) -> Result<Row, Error> {
        let key = line[0].int();
        let mut cells = BTreeMap::new();
        let mut at = 1;
        for slot in unit.fields() {
            if let Some(cell) = pick(slot, &line[at])? {
                cells.insert(slot.name().to_string(), cell);
            }
            at += 1;
        }
        for edge in unit.refs() {
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
}

pub(super) fn pick(slot: &Slot, cell: &Val) -> Result<Option<Cell>, Error> {
    if *cell == Val::Null {
        return if slot.need() {
            Err(Error::Adapt(format!(
                "required field {} is null",
                slot.name()
            )))
        } else {
            Ok(None)
        };
    }
    Ok(Some(match slot.kind() {
        atom::Kind::Text | atom::Kind::Link => Cell::Text(cell.text()),
        atom::Kind::Int => Cell::Int(cell.int()),
        atom::Kind::Bool => Cell::Bool(cell.int() != 0),
    }))
}

pub(super) fn fit(slots: &[Slot], col: &str, value: &str) -> Result<Val, Error> {
    let slot = slots
        .iter()
        .find(|slot| slot.name() == col)
        .ok_or_else(|| Error::Adapt(format!("unknown field {col}")))?;
    slot.bind(value)
}

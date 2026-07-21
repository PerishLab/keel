use crate::atom;
use crate::ddl;
use crate::life::{Cell, Row};
use crate::plan::Plan;
use std::collections::BTreeMap;

pub fn blend(plan: &Plan, unit: &str, base: &mut BTreeMap<String, Cell>, fields: &[(&str, &str)]) {
    let Some(node) = plan.find(&ddl::table(unit)).ok() else {
        return;
    };
    for (k, v) in fields {
        if v.is_empty() {
            base.remove(*k);
            continue;
        }
        if let Some(slot) = node.fields().iter().find(|s| s.name() == *k) {
            base.insert((*k).to_string(), shape(slot.kind(), v));
            continue;
        }
        let refd = node
            .bonds()
            .iter()
            .any(|e| e.kind().point() && e.name() == *k);
        if refd && let Ok(id) = v.parse::<i64>() {
            base.insert((*k).to_string(), Cell::Int(id));
        }
    }
}

pub fn mold(plan: &Plan, unit: &str, fields: &[(&str, &str)]) -> BTreeMap<String, Cell> {
    let mut out = BTreeMap::new();
    let Some(node) = plan.find(&ddl::table(unit)).ok() else {
        return out;
    };
    for slot in node.fields() {
        let raw = get(fields, slot.name());
        if raw.is_empty() {
            continue;
        }
        out.insert(slot.name().to_string(), shape(slot.kind(), raw));
    }
    for edge in node.bonds().iter().filter(|e| e.kind().point()) {
        let raw = get(fields, edge.name());
        if let Ok(id) = raw.parse::<i64>() {
            out.insert(edge.name().to_string(), Cell::Int(id));
        }
    }
    out
}

pub(crate) fn shape(kind: atom::Kind, raw: &str) -> Cell {
    match kind {
        atom::Kind::Int => raw.parse::<i64>().map(Cell::Int).unwrap_or(Cell::Int(0)),
        atom::Kind::Bool => Cell::Bool(raw == "true"),
        _ => Cell::Text(raw.to_string()),
    }
}

pub(crate) fn cell<'a>(row: &'a Row, name: &str) -> &'a str {
    row.cells().get(name).map(Cell::text).unwrap_or("")
}

pub fn field<'a>(fields: &[(&'a str, &'a str)], name: &str) -> &'a str {
    get(fields, name)
}

pub(crate) fn get<'a>(fields: &[(&'a str, &'a str)], name: &str) -> &'a str {
    fields
        .iter()
        .find(|(k, _)| *k == name)
        .map(|(_, v)| *v)
        .unwrap_or("")
}

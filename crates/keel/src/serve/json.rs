use super::*;
use crate::face::Core;
use crate::wire::Wire;
use axum::http::StatusCode;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

pub(crate) fn unit_name<W: Wire>(core: &Core<W>, route: &str) -> Result<String, Fault> {
    let name = crate::query::resolve(core.plan(), route).map_err(Fault::from)?;
    if core.plan().veiled(&name) {
        return Err(Fault {
            status: StatusCode::NOT_FOUND,
            note: "no such route".into(),
        });
    }
    Ok(name)
}

pub(crate) fn bond_name<W: Wire>(core: &Core<W>, unit: &str, bond: &str) -> Result<String, Fault> {
    let node = core
        .plan()
        .units()
        .get(unit)
        .ok_or_else(|| Fault::miss(unit))?;
    node.bonds()
        .iter()
        .find(|edge| edge.name().eq_ignore_ascii_case(bond))
        .map(|edge| edge.name().to_string())
        .ok_or_else(|| Fault::bad(format!("unknown bond {bond}")))
}

pub(crate) fn cells(body: &Map<String, Value>) -> Result<BTreeMap<String, String>, Fault> {
    cells_skip(body, &[])
}

pub(crate) fn cells_skip(
    body: &Map<String, Value>,
    skip: &[&str],
) -> Result<BTreeMap<String, String>, Fault> {
    let mut out = BTreeMap::new();
    for (key, value) in body {
        if skip.iter().any(|s| *s == key) {
            continue;
        }
        let text = match value {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Null => String::new(),
            _ => {
                return Err(Fault::bad(format!("field {key} must be scalar")));
            }
        };
        out.insert(key.clone(), text);
    }
    Ok(out)
}

pub(crate) fn pack_json(pack: &crate::query::Pack) -> Value {
    if let Some(n) = pack.count() {
        return json!({ "root": pack.root(), "count": n });
    }
    let mut bags = Map::new();
    for (key, bag) in pack.bags() {
        let list = match bag {
            crate::query::Bag::Unit(rows) => Value::Array(rows.iter().map(row_json).collect()),
            crate::query::Bag::Bond(ties) => Value::Array(ties.iter().map(tie_json).collect()),
        };
        bags.insert(key.clone(), list);
    }
    json!({ "root": pack.root(), "bags": bags })
}

pub(crate) fn row_json(row: &crate::life::Row) -> Value {
    let mut map = Map::new();
    map.insert("id".into(), json!(row.key()));
    for (k, v) in row.cells() {
        map.insert(k.clone(), cell_json(v));
    }
    map.insert("expires_at".into(), json!(row.expires()));
    map.insert("created_at".into(), json!(row.created()));
    map.insert("updated_at".into(), json!(row.updated()));
    Value::Object(map)
}

pub(crate) fn tie_json(tie: &crate::life::Tie) -> Value {
    let mut map = Map::new();
    map.insert("id".into(), json!(tie.key()));
    map.insert("left".into(), json!(tie.left()));
    map.insert("right".into(), json!(tie.right()));
    for (k, v) in tie.cells() {
        map.insert(k.clone(), cell_json(v));
    }
    map.insert("expires_at".into(), json!(tie.expires()));
    map.insert("created_at".into(), json!(tie.created()));
    map.insert("updated_at".into(), json!(tie.updated()));
    Value::Object(map)
}

pub(crate) fn cell_json(cell: &crate::life::Cell) -> Value {
    match cell {
        crate::life::Cell::Text(value) => Value::String(value.clone()),
        crate::life::Cell::Int(value) => json!(value),
        crate::life::Cell::Bool(value) => Value::Bool(*value),
    }
}

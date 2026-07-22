use super::*;
use crate::face::Core;
use crate::wire::Wire;
use axum::http::StatusCode;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

pub(crate) trait Emit {
    fn emit(&self) -> Value;
}

pub(crate) trait Reach {
    fn unit(&self, route: &str) -> Result<String, Fault>;
    fn bond(&self, unit: &str, bond: &str) -> Result<String, Fault>;
}

impl<W: Wire> Reach for Core<W> {
    fn unit(&self, route: &str) -> Result<String, Fault> {
        let name = crate::query::resolve(self.plan(), route).map_err(Fault::from)?;
        if self.plan().veiled(&name) {
            return Err(Fault {
                status: StatusCode::NOT_FOUND,
                note: "no such route".into(),
            });
        }
        Ok(name)
    }

    fn bond(&self, unit: &str, bond: &str) -> Result<String, Fault> {
        let node = self.plan().find(unit).map_err(|_| Fault::miss(unit))?;
        node.bonds()
            .iter()
            .find(|edge| edge.name().eq_ignore_ascii_case(bond))
            .map(|edge| edge.name().to_string())
            .ok_or_else(|| Fault::bad(format!("unknown bond {bond}")))
    }
}

pub(crate) fn cells(
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

impl Emit for crate::query::Pack {
    fn emit(&self) -> Value {
        if let Some(n) = self.count() {
            return json!({ "root": self.root(), "count": n });
        }
        let mut bags = Map::new();
        for (key, bag) in self.bags() {
            bags.insert(key.clone(), bag.emit());
        }
        json!({ "root": self.root(), "bags": bags })
    }
}

impl Emit for crate::query::Bag {
    fn emit(&self) -> Value {
        match self {
            crate::query::Bag::Unit(rows) => Value::Array(rows.iter().map(Emit::emit).collect()),
            crate::query::Bag::Bond(ties) => Value::Array(ties.iter().map(Emit::emit).collect()),
        }
    }
}

impl Emit for crate::life::Row {
    fn emit(&self) -> Value {
        let mut map = Map::new();
        map.insert("id".into(), json!(self.key()));
        for (k, v) in self.cells() {
            map.insert(k.clone(), v.emit());
        }
        reign(&mut map, self.expires(), self.created(), self.updated());
        Value::Object(map)
    }
}

impl Emit for crate::life::Tie {
    fn emit(&self) -> Value {
        let mut map = Map::new();
        map.insert("id".into(), json!(self.key()));
        map.insert("left".into(), json!(self.left()));
        map.insert("right".into(), json!(self.right()));
        for (k, v) in self.cells() {
            map.insert(k.clone(), v.emit());
        }
        reign(&mut map, self.expires(), self.created(), self.updated());
        Value::Object(map)
    }
}

impl Emit for crate::life::Cell {
    fn emit(&self) -> Value {
        match self {
            crate::life::Cell::Text(value) => Value::String(value.clone()),
            crate::life::Cell::Int(value) => json!(value),
            crate::life::Cell::Bool(value) => Value::Bool(*value),
        }
    }
}

fn reign(map: &mut Map<String, Value>, expires: Option<i64>, created: i64, updated: i64) {
    map.insert("expires_at".into(), json!(expires));
    map.insert("created_at".into(), json!(created));
    map.insert("updated_at".into(), json!(updated));
}

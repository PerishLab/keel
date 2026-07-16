use crate::adapt::Error;
use crate::atom;
use crate::ddl;
use crate::face::Who;
use crate::life::{Cell, Row};
use crate::plan::{Plan, Unit};
use crate::query;
use crate::store::Store;
use std::collections::BTreeMap;

pub const GRANT: &str = "@grant";
pub const SEAL: &str = "@seal";
pub const PULSE: &str = "@pulse";
pub const WINDOW: usize = 4096;
pub const VERBS: [&str; 6] = ["see", "put", "set", "end", "tie", "cut"];
pub const DEPTH: usize = 16;

pub struct Mark<'a> {
    pub key: Option<i64>,
    pub cells: &'a BTreeMap<String, Cell>,
}

pub fn genesis<S: Store>(plan: &Plan, store: &S) -> Result<Option<String>, Error> {
    if !store.live(plan, SEAL)?.is_empty() {
        return Ok(None);
    }
    let token = wild();
    store.put(plan, SEAL, &[("hash", &digest(&token))])?;
    Ok(Some(token))
}

pub fn sealed<S: Store>(plan: &Plan, store: &S, token: &str) -> Result<bool, Error> {
    let rows = store.live(plan, SEAL)?;
    let want = digest(token);
    Ok(rows.first().is_some_and(|row| cell(row, "hash") == want))
}

fn wild() -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    let mut out = String::new();
    for _ in 0..4 {
        let word = RandomState::new().build_hasher().finish();
        out.push_str(&format!("{word:016x}"));
    }
    out
}

fn digest(token: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn vet(plan: &Plan, fields: &[(&str, &str)]) -> Result<(), Error> {
    let verb = get(fields, "verb");
    if verb != "*" && !VERBS.contains(&verb) {
        return Err(Error::Adapt(format!("unknown verb {verb}")));
    }
    let who = get(fields, "who");
    let named = who == "anon" || who == "all" || who.parse::<i64>().is_ok();
    if !named {
        return Err(Error::Adapt("who is an id, anon, or all".into()));
    }
    let unit = get(fields, "unit");
    let place = if unit == "*" {
        None
    } else {
        Some(query::resolve(plan, unit)?)
    };
    scope(place.as_deref(), get(fields, "scope"))
}

fn scope(unit: Option<&str>, value: &str) -> Result<(), Error> {
    if value == "all" {
        return Ok(());
    }
    if let Some(id) = value.strip_prefix("row ") {
        if unit.is_none() {
            return Err(Error::Adapt("row scope needs a unit".into()));
        }
        id.parse::<i64>()
            .map_err(|_| Error::Adapt("row scope needs id".into()))?;
        return Ok(());
    }
    if let Some(pred) = value.strip_prefix("pred ") {
        let Some(unit) = unit else {
            return Err(Error::Adapt("pred scope needs a unit".into()));
        };
        let tree = query::parse(&format!("from {unit} where {pred}"))?;
        let plain = tree
            .preds()
            .iter()
            .all(|p| !matches!(p.op(), query::Op::Has | query::Op::Some));
        if !plain {
            return Err(Error::Adapt("scope pred is cells only".into()));
        }
        return Ok(());
    }
    Err(Error::Adapt(
        "scope is all | row <id> | pred <where>".into(),
    ))
}

pub fn check<S: Store>(
    plan: &Plan,
    store: &S,
    who: Who,
    verb: &str,
    unit: &str,
    mark: &Mark<'_>,
) -> Result<bool, Error> {
    let chain = anchors(plan, store, unit, mark)?;
    for deed in store.live(plan, GRANT)? {
        if held(&deed, who, verb, unit, mark, &chain)? {
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn broad<S: Store>(
    plan: &Plan,
    store: &S,
    who: Who,
    verb: &str,
    unit: &str,
) -> Result<bool, Error> {
    for deed in store.live(plan, GRANT)? {
        if !who_hit(cell(&deed, "who"), who) || !verb_hit(cell(&deed, "verb"), verb) {
            continue;
        }
        let place = cell(&deed, "unit");
        let wide = place == "*" || ddl::table(place) == unit;
        if wide && cell(&deed, "scope") == "all" {
            return Ok(true);
        }
    }
    Ok(false)
}

fn held(
    deed: &Row,
    who: Who,
    verb: &str,
    unit: &str,
    mark: &Mark<'_>,
    chain: &[(String, i64)],
) -> Result<bool, Error> {
    if !who_hit(cell(deed, "who"), who) || !verb_hit(cell(deed, "verb"), verb) {
        return Ok(false);
    }
    let place = cell(deed, "unit");
    let span = cell(deed, "scope");
    if span == "all" {
        return Ok(place == "*" || ddl::table(place) == unit);
    }
    if let Some(id) = span.strip_prefix("row ") {
        let Ok(id) = id.parse::<i64>() else {
            return Ok(false);
        };
        let anchor = ddl::table(place);
        return Ok(chain.iter().any(|(u, k)| *u == anchor && *k == id));
    }
    if let Some(pred) = span.strip_prefix("pred ") {
        if ddl::table(place) != unit {
            return Ok(false);
        }
        return Ok(pred_hit(unit, pred, who, mark));
    }
    Ok(false)
}

fn pred_hit(unit: &str, pred: &str, who: Who, mark: &Mark<'_>) -> bool {
    let text = match who {
        Who::Op(id) => pred.replace("\"@me\"", &format!("\"{id}\"")),
        _ if pred.contains("\"@me\"") => return false,
        _ => pred.to_string(),
    };
    let Ok(tree) = query::parse(&format!("from {unit} where {text}")) else {
        return false;
    };
    query::cover(mark.key, mark.cells, tree.preds())
}

fn who_hit(deed: &str, who: Who) -> bool {
    match deed {
        "anon" => true,
        "all" => matches!(who, Who::Op(_)),
        id => match who {
            Who::Op(op) => id.parse::<i64>().is_ok_and(|n| n == op),
            _ => false,
        },
    }
}

fn verb_hit(deed: &str, verb: &str) -> bool {
    deed == "*" || deed == verb
}

fn anchors<S: Store>(
    plan: &Plan,
    store: &S,
    unit: &str,
    mark: &Mark<'_>,
) -> Result<Vec<(String, i64)>, Error> {
    let mut out = Vec::new();
    if let Some(key) = mark.key {
        out.push((unit.to_string(), key));
    }
    let mut name = unit.to_string();
    let mut cells = mark.cells.clone();
    for _ in 0..DEPTH {
        let Some(node) = seat(plan, &name) else {
            break;
        };
        let Some(edge) = node.root() else {
            break;
        };
        let Some(Cell::Int(up)) = cells.get(edge.name()).cloned() else {
            break;
        };
        let target = ddl::table(edge.target());
        out.push((target.clone(), up));
        let Some(row) = store.one(plan, edge.target(), up)? else {
            break;
        };
        name = target;
        cells = row.cells().clone();
    }
    Ok(out)
}

fn seat<'a>(plan: &'a Plan, table: &str) -> Option<&'a Unit> {
    plan.units()
        .values()
        .find(|node| ddl::table(node.name()) == table)
}

pub fn blend(plan: &Plan, unit: &str, base: &mut BTreeMap<String, Cell>, fields: &[(&str, &str)]) {
    let Some(node) = seat(plan, &ddl::table(unit)) else {
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
    let Some(node) = seat(plan, &ddl::table(unit)) else {
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

fn shape(kind: atom::Kind, raw: &str) -> Cell {
    match kind {
        atom::Kind::Int => raw.parse::<i64>().map(Cell::Int).unwrap_or(Cell::Int(0)),
        atom::Kind::Bool => Cell::Bool(raw == "true"),
        _ => Cell::Text(raw.to_string()),
    }
}

fn cell<'a>(row: &'a Row, name: &str) -> &'a str {
    row.cells().get(name).map(Cell::text).unwrap_or("")
}

pub fn field<'a>(fields: &[(&'a str, &'a str)], name: &str) -> &'a str {
    get(fields, name)
}

fn get<'a>(fields: &[(&'a str, &'a str)], name: &str) -> &'a str {
    fields
        .iter()
        .find(|(k, _)| *k == name)
        .map(|(_, v)| *v)
        .unwrap_or("")
}

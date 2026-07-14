use crate::adapt::Error;
use crate::ddl;
use crate::plan::{Plan, Unit};
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Row {
    key: i64,
    cells: BTreeMap<String, String>,
    expires: Option<i64>,
    created: i64,
    updated: i64,
}

impl Row {
    pub fn key(&self) -> i64 {
        self.key
    }

    pub fn cells(&self) -> &BTreeMap<String, String> {
        &self.cells
    }

    pub fn expires(&self) -> Option<i64> {
        self.expires
    }

    pub fn created(&self) -> i64 {
        self.created
    }

    pub fn updated(&self) -> i64 {
        self.updated
    }
}

pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn put(
    conn: &Connection,
    plan: &Plan,
    name: &str,
    fields: &[(&str, &str)],
) -> Result<i64, Error> {
    let unit = find(plan, name)?;
    check(unit, fields)?;
    let tick = now();
    let mut cols: Vec<&str> = unit.fields().iter().map(|s| s.name()).collect();
    cols.push(ddl::EXPIRES);
    cols.push(ddl::CREATED);
    cols.push(ddl::UPDATED);
    let marks = (1..=cols.len())
        .map(|i| format!("?{i}"))
        .collect::<Vec<_>>()
        .join(", ");
    let text = format!(
        "INSERT INTO {} ({}) VALUES ({})",
        ddl::table(unit.name()),
        cols.join(", "),
        marks
    );
    let mut stmt = conn.prepare(&text).map_err(fail)?;
    let mut vals: Vec<rusqlite::types::Value> = unit
        .fields()
        .iter()
        .map(|slot| {
            let hit = fields
                .iter()
                .find(|(k, _)| *k == slot.name())
                .map(|(_, v)| *v)
                .unwrap_or("");
            rusqlite::types::Value::Text(hit.to_string())
        })
        .collect();
    vals.push(rusqlite::types::Value::Null);
    vals.push(rusqlite::types::Value::Integer(tick));
    vals.push(rusqlite::types::Value::Integer(tick));
    stmt.execute(rusqlite::params_from_iter(vals))
        .map_err(fail)?;
    Ok(conn.last_insert_rowid())
}

pub fn live(conn: &Connection, plan: &Plan, name: &str) -> Result<Vec<Row>, Error> {
    let unit = find(plan, name)?;
    let tick = now();
    let text = format!(
        "SELECT * FROM {} WHERE {} IS NULL OR {} > ?1 ORDER BY {}",
        ddl::table(unit.name()),
        ddl::EXPIRES,
        ddl::EXPIRES,
        ddl::KEY
    );
    let mut stmt = conn.prepare(&text).map_err(fail)?;
    let mut rows = stmt.query(params![tick]).map_err(fail)?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().map_err(fail)? {
        out.push(read(unit, row)?);
    }
    Ok(out)
}

pub fn end(conn: &Connection, plan: &Plan, name: &str, key: i64) -> Result<(), Error> {
    let unit = find(plan, name)?;
    let tick = now();
    let text = format!(
        "UPDATE {} SET {} = ?1, {} = ?1 WHERE {} = ?2",
        ddl::table(unit.name()),
        ddl::EXPIRES,
        ddl::UPDATED,
        ddl::KEY
    );
    let n = conn.execute(&text, params![tick, key]).map_err(fail)?;
    if n == 0 {
        return Err(Error::Adapt(format!("missing row {key}")));
    }
    Ok(())
}

fn find<'a>(plan: &'a Plan, name: &str) -> Result<&'a Unit, Error> {
    if let Some(unit) = plan.units().get(name) {
        return Ok(unit);
    }
    let want = ddl::table(name);
    plan.units()
        .values()
        .find(|unit| ddl::table(unit.name()) == want)
        .ok_or_else(|| Error::Missing(name.into()))
}

fn check(unit: &Unit, fields: &[(&str, &str)]) -> Result<(), Error> {
    if fields.len() != unit.fields().len() {
        return Err(Error::Adapt("field count mismatch".into()));
    }
    for slot in unit.fields() {
        if !fields.iter().any(|(k, _)| *k == slot.name()) {
            return Err(Error::Adapt(format!("missing field {}", slot.name())));
        }
    }
    for (k, _) in fields {
        if !unit.fields().iter().any(|s| s.name() == *k) {
            return Err(Error::Adapt(format!("unknown field {k}")));
        }
    }
    Ok(())
}

fn read(unit: &Unit, row: &rusqlite::Row<'_>) -> Result<Row, Error> {
    let key: i64 = row.get(ddl::KEY).map_err(fail)?;
    let mut cells = BTreeMap::new();
    for slot in unit.fields() {
        let value: String = row.get(slot.name()).map_err(fail)?;
        cells.insert(slot.name().to_string(), value);
    }
    let expires: Option<i64> = row.get(ddl::EXPIRES).optional().map_err(fail)?.flatten();
    let created: i64 = row.get(ddl::CREATED).map_err(fail)?;
    let updated: i64 = row.get(ddl::UPDATED).map_err(fail)?;
    Ok(Row {
        key,
        cells,
        expires,
        created,
        updated,
    })
}

fn fail(err: rusqlite::Error) -> Error {
    Error::Adapt(err.to_string())
}

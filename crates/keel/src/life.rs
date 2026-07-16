use crate::adapt::Error;
use crate::atom;
use crate::bond;
use crate::ddl;
use crate::plan::{Edge, Plan, Slot, Unit};
use crate::spec::Only;
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Cell {
    Bool(bool),
    Int(i64),
    Text(String),
}

impl Cell {
    pub fn text(&self) -> &str {
        match self {
            Cell::Text(value) => value,
            _ => "",
        }
    }

    pub fn show(&self) -> String {
        match self {
            Cell::Text(value) => value.clone(),
            Cell::Int(value) => value.to_string(),
            Cell::Bool(value) => value.to_string(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Row {
    key: i64,
    cells: BTreeMap<String, Cell>,
    expires: Option<i64>,
    created: i64,
    updated: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Ends {
    pub left: i64,
    pub right: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Tie {
    key: i64,
    left: i64,
    right: i64,
    cells: BTreeMap<String, Cell>,
    expires: Option<i64>,
    created: i64,
    updated: i64,
}

impl Row {
    pub fn key(&self) -> i64 {
        self.key
    }

    pub fn cells(&self) -> &BTreeMap<String, Cell> {
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

impl Tie {
    pub fn key(&self) -> i64 {
        self.key
    }

    pub fn left(&self) -> i64 {
        self.left
    }

    pub fn right(&self) -> i64 {
        self.right
    }

    pub fn cells(&self) -> &BTreeMap<String, Cell> {
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

pub struct Work<'a> {
    conn: &'a Connection,
}

impl<'a> Work<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    pub fn put(&self, plan: &Plan, name: &str, fields: &[(&str, &str)]) -> Result<i64, Error> {
        let unit = find(plan, name)?;
        if unit.name() == crate::cap::GRANT {
            crate::cap::vet(plan, fields)?;
        }
        check(unit, fields)?;
        let tick = now();
        let mut cols: Vec<String> = unit.fields().iter().map(|s| ddl::col(s.name())).collect();
        for edge in refs(unit) {
            cols.push(ddl::col(&ddl::side(edge.name())));
        }
        cols.push(ddl::EXPIRES.into());
        cols.push(ddl::CREATED.into());
        cols.push(ddl::UPDATED.into());
        let marks = (1..=cols.len())
            .map(|i| format!("?{i}"))
            .collect::<Vec<_>>()
            .join(", ");
        let text = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            ddl::seat(unit.name()),
            cols.join(", "),
            marks
        );
        let mut stmt = self.conn.prepare(&text).map_err(fail)?;
        let mut vals: Vec<rusqlite::types::Value> = Vec::new();
        for slot in unit.fields() {
            if let Some(scope) = slot.serial() {
                vals.push(self.next(unit, slot, scope, fields)?);
                continue;
            }
            let hit = pluck(fields, slot.name());
            vals.push(bind(slot, hit)?);
        }
        for edge in refs(unit) {
            let hit = pluck(fields, edge.name());
            vals.push(self.point(plan, unit, edge, hit, None)?);
        }
        vals.push(rusqlite::types::Value::Null);
        vals.push(rusqlite::types::Value::Integer(tick));
        vals.push(rusqlite::types::Value::Integer(tick));
        self.solid(unit, fields, None)?;
        stmt.execute(rusqlite::params_from_iter(vals))
            .map_err(fail)?;
        Ok(self.conn.last_insert_rowid())
    }

    fn solid(
        &self,
        unit: &Unit,
        fields: &[(&str, &str)],
        myself: Option<(i64, &Row)>,
    ) -> Result<(), Error> {
        for slot in unit.fields() {
            if *slot.only() == Only::Free {
                continue;
            }
            self.taken(unit, slot, fields, myself)?;
        }
        Ok(())
    }

    fn taken(
        &self,
        unit: &Unit,
        slot: &Slot,
        fields: &[(&str, &str)],
        myself: Option<(i64, &Row)>,
    ) -> Result<(), Error> {
        let Some(value) = worth(slot, fields, myself)? else {
            return Ok(());
        };
        let me = myself.map(|(key, _)| key).unwrap_or(0);
        let mut text = format!(
            "SELECT 1 FROM {} WHERE {} = ?1 AND {} != ?2 AND ({} IS NULL OR {} > ?3)",
            ddl::seat(unit.name()),
            ddl::col(slot.name()),
            ddl::KEY,
            ddl::EXPIRES,
            ddl::EXPIRES
        );
        let mut vals = vec![
            value,
            rusqlite::types::Value::Integer(me),
            rusqlite::types::Value::Integer(now()),
        ];
        if let Only::Per(rel) = slot.only() {
            let col = ddl::col(&ddl::side(rel));
            text.push_str(&format!(
                " AND ({col} = ?4 OR (?4 IS NULL AND {col} IS NULL))"
            ));
            vals.push(anchor(fields, myself, rel)?);
        }
        text.push_str(" LIMIT 1");
        let mut stmt = self.conn.prepare(&text).map_err(fail)?;
        if stmt
            .exists(rusqlite::params_from_iter(vals))
            .map_err(fail)?
        {
            return Err(Error::Adapt(format!("field {} taken", slot.name())));
        }
        Ok(())
    }

    fn point(
        &self,
        plan: &Plan,
        unit: &Unit,
        edge: &Edge,
        value: &str,
        myself: Option<i64>,
    ) -> Result<rusqlite::types::Value, Error> {
        if value.is_empty() {
            if edge.need() {
                return Err(Error::Adapt(format!("missing ref {}", edge.name())));
            }
            return Ok(rusqlite::types::Value::Null);
        }
        let key = value
            .parse::<i64>()
            .map_err(|_| Error::Adapt(format!("ref {} needs id", edge.name())))?;
        if !self.live_has(plan, edge.target(), key)? {
            return Err(Error::Adapt("right not live".into()));
        }
        if edge.kind() == bond::Kind::One2one {
            self.lone(unit, edge, key, myself)?;
        }
        Ok(rusqlite::types::Value::Integer(key))
    }

    fn next(
        &self,
        unit: &Unit,
        slot: &Slot,
        scope: &str,
        fields: &[(&str, &str)],
    ) -> Result<rusqlite::types::Value, Error> {
        let col = ddl::col(&ddl::side(scope));
        let text = format!(
            "SELECT COALESCE(MAX({}), 0) + 1 FROM {} WHERE {col} = ?1 OR (?1 IS NULL AND {col} IS NULL)",
            ddl::col(slot.name()),
            ddl::seat(unit.name())
        );
        let mut stmt = self.conn.prepare(&text).map_err(fail)?;
        let hold = anchor(fields, None, scope)?;
        let key: i64 = stmt
            .query_row(rusqlite::params![hold], |row| row.get(0))
            .map_err(fail)?;
        Ok(rusqlite::types::Value::Integer(key))
    }

    fn lone(&self, unit: &Unit, edge: &Edge, key: i64, myself: Option<i64>) -> Result<(), Error> {
        let tick = now();
        let text = format!(
            "SELECT 1 FROM {} WHERE {} = ?1 AND {} != ?2 AND ({} IS NULL OR {} > ?3) LIMIT 1",
            ddl::seat(unit.name()),
            ddl::col(&ddl::side(edge.name())),
            ddl::KEY,
            ddl::EXPIRES,
            ddl::EXPIRES
        );
        let mut stmt = self.conn.prepare(&text).map_err(fail)?;
        let taken = stmt
            .exists(params![key, myself.unwrap_or(0), tick])
            .map_err(fail)?;
        if taken {
            return Err(Error::Adapt("live ref exists".into()));
        }
        Ok(())
    }

    fn entry(
        &self,
        plan: &Plan,
        unit: &Unit,
        col: &str,
        val: &str,
        myself: i64,
    ) -> Result<(String, rusqlite::types::Value), Error> {
        if let Some(edge) = refs(unit).find(|e| e.name() == col) {
            let cell = self.point(plan, unit, edge, val, Some(myself))?;
            return Ok((ddl::side(edge.name()), cell));
        }
        Ok((col.to_string(), fit(unit.fields(), col, val)?))
    }

    pub fn one(&self, plan: &Plan, name: &str, key: i64) -> Result<Option<Row>, Error> {
        let unit = find(plan, name)?;
        match self.peek(unit, key) {
            Ok(row) => Ok(Some(row)),
            Err(Error::Adapt(note)) if note.starts_with("missing row") => Ok(None),
            Err(err) => Err(err),
        }
    }

    fn peek(&self, unit: &Unit, key: i64) -> Result<Row, Error> {
        let tick = now();
        let text = format!(
            "SELECT * FROM {} WHERE {} = ?1 AND ({} IS NULL OR {} > ?2)",
            ddl::seat(unit.name()),
            ddl::KEY,
            ddl::EXPIRES,
            ddl::EXPIRES
        );
        let mut stmt = self.conn.prepare(&text).map_err(fail)?;
        let mut rows = stmt.query(params![key, tick]).map_err(fail)?;
        match rows.next().map_err(fail)? {
            Some(row) => read(unit, row),
            None => Err(Error::Adapt(format!("missing row {key}"))),
        }
    }

    pub fn live(&self, plan: &Plan, name: &str) -> Result<Vec<Row>, Error> {
        let unit = find(plan, name)?;
        let tick = now();
        let text = format!(
            "SELECT * FROM {} WHERE {} IS NULL OR {} > ?1 ORDER BY {}",
            ddl::seat(unit.name()),
            ddl::EXPIRES,
            ddl::EXPIRES,
            ddl::KEY
        );
        let mut stmt = self.conn.prepare(&text).map_err(fail)?;
        let mut rows = stmt.query(params![tick]).map_err(fail)?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().map_err(fail)? {
            out.push(read(unit, row)?);
        }
        Ok(out)
    }

    pub fn end(&self, plan: &Plan, name: &str, key: i64) -> Result<(), Error> {
        let unit = find(plan, name)?;
        if self.live_in(plan, unit.name(), key)? || self.live_out(unit, key)? {
            return Err(Error::Adapt("live ties remain".into()));
        }
        let tick = now();
        let text = format!(
            "UPDATE {} SET {} = ?1, {} = ?1 WHERE {} = ?2",
            ddl::seat(unit.name()),
            ddl::EXPIRES,
            ddl::UPDATED,
            ddl::KEY
        );
        let n = self.conn.execute(&text, params![tick, key]).map_err(fail)?;
        if n == 0 {
            return Err(Error::Adapt(format!("missing row {key}")));
        }
        Ok(())
    }

    fn live_in(&self, plan: &Plan, target: &str, key: i64) -> Result<bool, Error> {
        for unit in plan.units().values() {
            for edge in unit.bonds() {
                if edge.target() != target {
                    continue;
                }
                if self.live_from(unit, edge, key)? {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    fn live_from(&self, unit: &Unit, edge: &Edge, key: i64) -> Result<bool, Error> {
        let tick = now();
        let (place, col) = if edge.kind().point() {
            (ddl::seat(unit.name()), ddl::col(&ddl::side(edge.name())))
        } else {
            (
                ddl::joint(unit.name(), edge.name()),
                ddl::col(&ddl::side(edge.target())),
            )
        };
        let text = format!(
            "SELECT 1 FROM {} WHERE {} = ?1 AND ({} IS NULL OR {} > ?2) LIMIT 1",
            place,
            col,
            ddl::EXPIRES,
            ddl::EXPIRES
        );
        let mut stmt = self.conn.prepare(&text).map_err(fail)?;
        stmt.exists(params![key, tick]).map_err(fail)
    }

    fn live_out(&self, unit: &Unit, key: i64) -> Result<bool, Error> {
        let tick = now();
        for edge in unit.bonds() {
            if edge.kind() != bond::Kind::Many2many {
                continue;
            }
            let left = ddl::col(&ddl::side(unit.name()));
            let text = format!(
                "SELECT 1 FROM {} WHERE {} = ?1 AND ({} IS NULL OR {} > ?2) LIMIT 1",
                ddl::joint(unit.name(), edge.name()),
                left,
                ddl::EXPIRES,
                ddl::EXPIRES
            );
            let mut stmt = self.conn.prepare(&text).map_err(fail)?;
            if stmt.exists(params![key, tick]).map_err(fail)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn set(
        &self,
        plan: &Plan,
        name: &str,
        key: i64,
        fields: &[(&str, &str)],
    ) -> Result<(), Error> {
        let unit = find(plan, name)?;
        if unit.name() == crate::cap::GRANT {
            return Err(Error::Adapt("grant rows are put or end".into()));
        }
        part(unit, fields)?;
        let base = self.peek(unit, key)?;
        self.solid(unit, fields, Some((key, &base)))?;
        let tick = now();
        let mut text = format!("UPDATE {} SET ", ddl::seat(unit.name()));
        let mut vals: Vec<rusqlite::types::Value> = Vec::new();
        for (i, (col, val)) in fields.iter().enumerate() {
            if i > 0 {
                text.push_str(", ");
            }
            let (name, cell) = self.entry(plan, unit, col, val, key)?;
            text.push_str(&ddl::col(&name));
            text.push_str(" = ?");
            text.push_str(&(i + 1).to_string());
            vals.push(cell);
        }
        let n = fields.len();
        text.push_str(&format!(
            ", {} = ?{} WHERE {} = ?{} AND ({} IS NULL OR {} > ?{})",
            ddl::UPDATED,
            n + 1,
            ddl::KEY,
            n + 2,
            ddl::EXPIRES,
            ddl::EXPIRES,
            n + 3
        ));
        vals.push(rusqlite::types::Value::Integer(tick));
        vals.push(rusqlite::types::Value::Integer(key));
        vals.push(rusqlite::types::Value::Integer(tick));
        let changed = self
            .conn
            .execute(&text, rusqlite::params_from_iter(vals))
            .map_err(fail)?;
        if changed == 0 {
            return Err(Error::Adapt(format!("missing row {key}")));
        }
        Ok(())
    }

    pub fn tie(
        &self,
        plan: &Plan,
        owner: &str,
        bond: &str,
        ends: Ends,
        fields: &[(&str, &str)],
    ) -> Result<i64, Error> {
        let (unit, edge) = edge(plan, owner, bond)?;
        bond_part(edge, fields)?;
        if !self.live_has(plan, unit.name(), ends.left)? {
            return Err(Error::Adapt("left not live".into()));
        }
        if !self.live_has(plan, edge.target(), ends.right)? {
            return Err(Error::Adapt("right not live".into()));
        }
        if self.live_pair(plan, owner, bond, ends.left, ends.right)? {
            return Err(Error::Adapt("live pair exists".into()));
        }
        let tick = now();
        let src = ddl::col(&ddl::side(unit.name()));
        let dst = ddl::col(&ddl::side(edge.target()));
        let mut cols = vec![src, dst];
        for slot in edge.fields() {
            cols.push(ddl::col(slot.name()));
        }
        cols.push(ddl::EXPIRES.to_string());
        cols.push(ddl::CREATED.to_string());
        cols.push(ddl::UPDATED.to_string());
        let marks = (1..=cols.len())
            .map(|i| format!("?{i}"))
            .collect::<Vec<_>>()
            .join(", ");
        let text = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            ddl::joint(unit.name(), edge.name()),
            cols.join(", "),
            marks
        );
        let mut vals: Vec<rusqlite::types::Value> = Vec::new();
        vals.push(rusqlite::types::Value::Integer(ends.left));
        vals.push(rusqlite::types::Value::Integer(ends.right));
        for slot in edge.fields() {
            let hit = fields
                .iter()
                .find(|(k, _)| *k == slot.name())
                .map(|(_, v)| *v)
                .unwrap_or("");
            vals.push(bind(slot, hit)?);
        }
        vals.push(rusqlite::types::Value::Null);
        vals.push(rusqlite::types::Value::Integer(tick));
        vals.push(rusqlite::types::Value::Integer(tick));
        self.conn
            .execute(&text, rusqlite::params_from_iter(vals))
            .map_err(fail)?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn set_tie(
        &self,
        plan: &Plan,
        owner: &str,
        bond: &str,
        key: i64,
        fields: &[(&str, &str)],
    ) -> Result<(), Error> {
        let (unit, edge) = edge(plan, owner, bond)?;
        bond_part(edge, fields)?;
        if fields.is_empty() {
            return Err(Error::Adapt("empty set".into()));
        }
        let ends = self.tie_ends(plan, owner, bond, key)?;
        if !self.live_has(plan, unit.name(), ends.left)? {
            return Err(Error::Adapt("left not live".into()));
        }
        if !self.live_has(plan, edge.target(), ends.right)? {
            return Err(Error::Adapt("right not live".into()));
        }
        let tick = now();
        let mut text = format!("UPDATE {} SET ", ddl::joint(unit.name(), edge.name()));
        let mut vals: Vec<rusqlite::types::Value> = Vec::new();
        for (i, (col, val)) in fields.iter().enumerate() {
            if i > 0 {
                text.push_str(", ");
            }
            text.push_str(&ddl::col(col));
            text.push_str(" = ?");
            text.push_str(&(i + 1).to_string());
            vals.push(fit(edge.fields(), col, val)?);
        }
        let n = fields.len();
        text.push_str(&format!(
            ", {} = ?{} WHERE {} = ?{} AND ({} IS NULL OR {} > ?{})",
            ddl::UPDATED,
            n + 1,
            ddl::KEY,
            n + 2,
            ddl::EXPIRES,
            ddl::EXPIRES,
            n + 3
        ));
        vals.push(rusqlite::types::Value::Integer(tick));
        vals.push(rusqlite::types::Value::Integer(key));
        vals.push(rusqlite::types::Value::Integer(tick));
        let changed = self
            .conn
            .execute(&text, rusqlite::params_from_iter(vals))
            .map_err(fail)?;
        if changed == 0 {
            return Err(Error::Adapt(format!("missing tie {key}")));
        }
        Ok(())
    }

    fn tie_ends(&self, plan: &Plan, owner: &str, bond: &str, key: i64) -> Result<Ends, Error> {
        let (unit, edge) = edge(plan, owner, bond)?;
        let tick = now();
        let src = ddl::col(&ddl::side(unit.name()));
        let dst = ddl::col(&ddl::side(edge.target()));
        let text = format!(
            "SELECT {}, {} FROM {} WHERE {} = ?1 AND ({} IS NULL OR {} > ?2)",
            src,
            dst,
            ddl::joint(unit.name(), edge.name()),
            ddl::KEY,
            ddl::EXPIRES,
            ddl::EXPIRES
        );
        let mut stmt = self.conn.prepare(&text).map_err(fail)?;
        let mut rows = stmt.query(params![key, tick]).map_err(fail)?;
        let row = rows
            .next()
            .map_err(fail)?
            .ok_or_else(|| Error::Adapt(format!("missing tie {key}")))?;
        Ok(Ends {
            left: row.get(0).map_err(fail)?,
            right: row.get(1).map_err(fail)?,
        })
    }

    pub fn ties(&self, plan: &Plan, owner: &str, bond: &str, left: i64) -> Result<Vec<Tie>, Error> {
        let (unit, edge) = edge(plan, owner, bond)?;
        let tick = now();
        let src = ddl::col(&ddl::side(unit.name()));
        let dst = ddl::col(&ddl::side(edge.target()));
        let mut cols = vec![
            ddl::KEY.to_string(),
            src,
            dst,
            ddl::EXPIRES.to_string(),
            ddl::CREATED.to_string(),
            ddl::UPDATED.to_string(),
        ];
        for slot in edge.fields() {
            cols.push(ddl::col(slot.name()));
        }
        let text = format!(
            "SELECT {} FROM {} WHERE {} = ?1 AND ({} IS NULL OR {} > ?2) ORDER BY {}",
            cols.join(", "),
            ddl::joint(unit.name(), edge.name()),
            ddl::col(&ddl::side(unit.name())),
            ddl::EXPIRES,
            ddl::EXPIRES,
            ddl::KEY
        );
        let mut stmt = self.conn.prepare(&text).map_err(fail)?;
        let mut rows = stmt.query(params![left, tick]).map_err(fail)?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().map_err(fail)? {
            out.push(read_tie(edge, row)?);
        }
        Ok(out)
    }

    pub fn cut(&self, plan: &Plan, owner: &str, bond: &str, key: i64) -> Result<(), Error> {
        let (unit, edge) = edge(plan, owner, bond)?;
        let tick = now();
        let text = format!(
            "UPDATE {} SET {} = ?1, {} = ?1 WHERE {} = ?2",
            ddl::joint(unit.name(), edge.name()),
            ddl::EXPIRES,
            ddl::UPDATED,
            ddl::KEY
        );
        let n = self.conn.execute(&text, params![tick, key]).map_err(fail)?;
        if n == 0 {
            return Err(Error::Adapt(format!("missing tie {key}")));
        }
        Ok(())
    }

    pub fn live_has(&self, plan: &Plan, name: &str, key: i64) -> Result<bool, Error> {
        let unit = find(plan, name)?;
        let tick = now();
        let text = format!(
            "SELECT 1 FROM {} WHERE {} = ?1 AND ({} IS NULL OR {} > ?2) LIMIT 1",
            ddl::seat(unit.name()),
            ddl::KEY,
            ddl::EXPIRES,
            ddl::EXPIRES
        );
        let mut stmt = self.conn.prepare(&text).map_err(fail)?;
        let found = stmt.exists(params![key, tick]).map_err(fail)?;
        Ok(found)
    }

    pub fn live_pair(
        &self,
        plan: &Plan,
        owner: &str,
        bond: &str,
        left: i64,
        right: i64,
    ) -> Result<bool, Error> {
        let (unit, edge) = edge(plan, owner, bond)?;
        let tick = now();
        let src = ddl::col(&ddl::side(unit.name()));
        let dst = ddl::col(&ddl::side(edge.target()));
        let text = format!(
            "SELECT 1 FROM {} WHERE {} = ?1 AND {} = ?2 AND ({} IS NULL OR {} > ?3) LIMIT 1",
            ddl::joint(unit.name(), edge.name()),
            src,
            dst,
            ddl::EXPIRES,
            ddl::EXPIRES
        );
        let mut stmt = self.conn.prepare(&text).map_err(fail)?;
        let found = stmt.exists(params![left, right, tick]).map_err(fail)?;
        Ok(found)
    }
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
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

fn edge<'a>(plan: &'a Plan, owner: &str, bond: &str) -> Result<(&'a Unit, &'a Edge), Error> {
    let unit = find(plan, owner)?;
    let edge = unit
        .bonds()
        .iter()
        .find(|edge| edge.name().eq_ignore_ascii_case(bond) && edge.kind() == bond::Kind::Many2many)
        .ok_or_else(|| Error::Adapt(format!("missing bond {bond}")))?;
    Ok((unit, edge))
}

fn refs(unit: &Unit) -> impl Iterator<Item = &crate::plan::Edge> {
    unit.bonds().iter().filter(|edge| edge.kind().point())
}

fn pluck<'a>(fields: &[(&'a str, &'a str)], name: &str) -> &'a str {
    seek(fields, name).unwrap_or("")
}

fn seek<'a>(fields: &[(&'a str, &'a str)], name: &str) -> Option<&'a str> {
    fields.iter().find(|(k, _)| *k == name).map(|(_, v)| *v)
}

fn worth(
    slot: &Slot,
    fields: &[(&str, &str)],
    myself: Option<(i64, &Row)>,
) -> Result<Option<rusqlite::types::Value>, Error> {
    if let Some(raw) = seek(fields, slot.name()) {
        return Ok(Some(bind(slot, raw)?));
    }
    let held = myself.and_then(|(_, row)| row.cells().get(slot.name()));
    Ok(held.map(cell_val))
}

fn anchor(
    fields: &[(&str, &str)],
    myself: Option<(i64, &Row)>,
    rel: &str,
) -> Result<rusqlite::types::Value, Error> {
    if let Some(raw) = seek(fields, rel) {
        if raw.is_empty() {
            return Ok(rusqlite::types::Value::Null);
        }
        let key = raw
            .parse::<i64>()
            .map_err(|_| Error::Adapt(format!("ref {rel} needs id")))?;
        return Ok(rusqlite::types::Value::Integer(key));
    }
    let held = myself.and_then(|(_, row)| row.cells().get(rel));
    Ok(held.map(cell_val).unwrap_or(rusqlite::types::Value::Null))
}

fn cell_val(cell: &Cell) -> rusqlite::types::Value {
    match cell {
        Cell::Text(value) => rusqlite::types::Value::Text(value.clone()),
        Cell::Int(value) => rusqlite::types::Value::Integer(*value),
        Cell::Bool(value) => rusqlite::types::Value::Integer(*value as i64),
    }
}

fn known(unit: &Unit, name: &str) -> bool {
    unit.fields().iter().any(|s| s.name() == name) || refs(unit).any(|e| e.name() == name)
}

fn check(unit: &Unit, fields: &[(&str, &str)]) -> Result<(), Error> {
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

fn part(unit: &Unit, fields: &[(&str, &str)]) -> Result<(), Error> {
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

fn bond_part(edge: &Edge, fields: &[(&str, &str)]) -> Result<(), Error> {
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

fn read_tie(edge: &Edge, row: &rusqlite::Row<'_>) -> Result<Tie, Error> {
    let key: i64 = row.get(0).map_err(fail)?;
    let left: i64 = row.get(1).map_err(fail)?;
    let right: i64 = row.get(2).map_err(fail)?;
    let expires: Option<i64> = row.get(3).optional().map_err(fail)?.flatten();
    let created: i64 = row.get(4).map_err(fail)?;
    let updated: i64 = row.get(5).map_err(fail)?;
    let mut cells = BTreeMap::new();
    for (i, slot) in edge.fields().iter().enumerate() {
        cells.insert(slot.name().to_string(), pick(slot, row, 6 + i)?);
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

fn read(unit: &Unit, row: &rusqlite::Row<'_>) -> Result<Row, Error> {
    let key: i64 = row.get(ddl::KEY).map_err(fail)?;
    let mut cells = BTreeMap::new();
    for slot in unit.fields() {
        cells.insert(slot.name().to_string(), pick(slot, row, slot.name())?);
    }
    for edge in refs(unit) {
        let col = ddl::side(edge.name());
        let value: Option<i64> = row.get(col.as_str()).map_err(fail)?;
        if let Some(key) = value {
            cells.insert(edge.name().to_string(), Cell::Int(key));
        }
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

fn pick<I: rusqlite::RowIndex>(slot: &Slot, row: &rusqlite::Row<'_>, at: I) -> Result<Cell, Error> {
    match slot.kind() {
        atom::Kind::Text | atom::Kind::Link => {
            let value: String = row.get(at).map_err(fail)?;
            Ok(Cell::Text(value))
        }
        atom::Kind::Int => {
            let value: i64 = row.get(at).map_err(fail)?;
            Ok(Cell::Int(value))
        }
        atom::Kind::Bool => {
            let value: i64 = row.get(at).map_err(fail)?;
            Ok(Cell::Bool(value != 0))
        }
    }
}

fn bind(slot: &Slot, value: &str) -> Result<rusqlite::types::Value, Error> {
    match slot.kind() {
        atom::Kind::Text | atom::Kind::Link => Ok(rusqlite::types::Value::Text(value.into())),
        atom::Kind::Int => value
            .parse::<i64>()
            .map(rusqlite::types::Value::Integer)
            .map_err(|_| Error::Adapt(format!("field {} needs int", slot.name()))),
        atom::Kind::Bool => match value {
            "true" => Ok(rusqlite::types::Value::Integer(1)),
            "false" => Ok(rusqlite::types::Value::Integer(0)),
            _ => Err(Error::Adapt(format!("field {} needs bool", slot.name()))),
        },
    }
}

fn fit(slots: &[Slot], col: &str, value: &str) -> Result<rusqlite::types::Value, Error> {
    let slot = slots
        .iter()
        .find(|slot| slot.name() == col)
        .ok_or_else(|| Error::Adapt(format!("unknown field {col}")))?;
    bind(slot, value)
}

fn fail(err: rusqlite::Error) -> Error {
    Error::Adapt(err.to_string())
}

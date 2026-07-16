use crate::adapt::Error;
use crate::ddl;
use crate::life::{self, Ends, Row, Tie};
use crate::plan::Plan;
use crate::store::Store;
use crate::wire::{Val, Wire};
use rusqlite::Connection;
use rusqlite::types::{Value, ValueRef};
use std::sync::Mutex;

enum Place {
    Memory,
    File(String),
}

pub struct Sqlite {
    place: Place,
    conn: Mutex<Option<Connection>>,
}

impl Sqlite {
    pub fn memory() -> Self {
        Self {
            place: Place::Memory,
            conn: Mutex::new(None),
        }
    }

    pub fn file(path: impl Into<String>) -> Self {
        Self {
            place: Place::File(path.into()),
            conn: Mutex::new(None),
        }
    }

    fn step(&self, word: &str) -> Result<(), Error> {
        let guard = self.conn.lock().map_err(lock)?;
        let conn = guard.as_ref().ok_or_else(unwired)?;
        conn.execute_batch(word).map_err(sql)
    }

    fn open(&self) -> Result<Connection, Error> {
        match &self.place {
            Place::Memory => Connection::open_in_memory().map_err(sql),
            Place::File(path) => Connection::open(path).map_err(sql),
        }
    }
}

impl Store for Sqlite {
    fn wire(&self, plan: &Plan) -> Result<(), Error> {
        if plan.units().is_empty() {
            return Err(Error::Adapt("db plan is empty".into()));
        }
        for unit in plan.units().values() {
            let reign = unit.reign();
            if !reign.expires() || !reign.created() || !reign.updated() {
                return Err(Error::Adapt("reign incomplete".into()));
            }
        }
        let conn = self.open()?;
        for stmt in ddl::script(plan, ddl::Grain::Lite) {
            conn.execute_batch(&stmt).map_err(sql)?;
        }
        let mut guard = self.conn.lock().map_err(lock)?;
        *guard = Some(conn);
        Ok(())
    }

    fn put(&self, plan: &Plan, name: &str, fields: &[(&str, &str)]) -> Result<i64, Error> {
        let guard = self.conn.lock().map_err(lock)?;
        let conn = guard.as_ref().ok_or_else(unwired)?;
        life::Work::new(&Sql(conn)).put(plan, name, fields)
    }

    fn set(&self, plan: &Plan, name: &str, key: i64, fields: &[(&str, &str)]) -> Result<(), Error> {
        let guard = self.conn.lock().map_err(lock)?;
        let conn = guard.as_ref().ok_or_else(unwired)?;
        life::Work::new(&Sql(conn)).set(plan, name, key, fields)
    }

    fn live(&self, plan: &Plan, name: &str) -> Result<Vec<Row>, Error> {
        let guard = self.conn.lock().map_err(lock)?;
        let conn = guard.as_ref().ok_or_else(unwired)?;
        life::Work::new(&Sql(conn)).live(plan, name)
    }

    fn one(&self, plan: &Plan, name: &str, key: i64) -> Result<Option<Row>, Error> {
        let guard = self.conn.lock().map_err(lock)?;
        let conn = guard.as_ref().ok_or_else(unwired)?;
        life::Work::new(&Sql(conn)).one(plan, name, key)
    }

    fn end(&self, plan: &Plan, name: &str, key: i64) -> Result<(), Error> {
        let guard = self.conn.lock().map_err(lock)?;
        let conn = guard.as_ref().ok_or_else(unwired)?;
        life::Work::new(&Sql(conn)).end(plan, name, key)
    }

    fn lease(&self, plan: &Plan, name: &str, key: i64, at: i64) -> Result<(), Error> {
        let guard = self.conn.lock().map_err(lock)?;
        let conn = guard.as_ref().ok_or_else(unwired)?;
        life::Work::new(&Sql(conn)).lease(plan, name, key, at)
    }

    fn pulse(&self, plan: &Plan, verb: &str, unit: &str, key: i64, who: &str) -> Result<(), Error> {
        let guard = self.conn.lock().map_err(lock)?;
        let conn = guard.as_ref().ok_or_else(unwired)?;
        life::Work::new(&Sql(conn)).pulse(plan, verb, unit, key, who)
    }

    fn tie(
        &self,
        plan: &Plan,
        owner: &str,
        bond: &str,
        ends: Ends,
        fields: &[(&str, &str)],
    ) -> Result<i64, Error> {
        let guard = self.conn.lock().map_err(lock)?;
        let conn = guard.as_ref().ok_or_else(unwired)?;
        life::Work::new(&Sql(conn)).tie(plan, owner, bond, ends, fields)
    }

    fn set_tie(
        &self,
        plan: &Plan,
        owner: &str,
        bond: &str,
        key: i64,
        fields: &[(&str, &str)],
    ) -> Result<(), Error> {
        let guard = self.conn.lock().map_err(lock)?;
        let conn = guard.as_ref().ok_or_else(unwired)?;
        life::Work::new(&Sql(conn)).set_tie(plan, owner, bond, key, fields)
    }

    fn ties(&self, plan: &Plan, owner: &str, bond: &str, left: i64) -> Result<Vec<Tie>, Error> {
        let guard = self.conn.lock().map_err(lock)?;
        let conn = guard.as_ref().ok_or_else(unwired)?;
        life::Work::new(&Sql(conn)).ties(plan, owner, bond, left)
    }

    fn cut(&self, plan: &Plan, owner: &str, bond: &str, key: i64) -> Result<(), Error> {
        let guard = self.conn.lock().map_err(lock)?;
        let conn = guard.as_ref().ok_or_else(unwired)?;
        life::Work::new(&Sql(conn)).cut(plan, owner, bond, key)
    }

    fn live_has(&self, plan: &Plan, name: &str, key: i64) -> Result<bool, Error> {
        let guard = self.conn.lock().map_err(lock)?;
        let conn = guard.as_ref().ok_or_else(unwired)?;
        life::Work::new(&Sql(conn)).live_has(plan, name, key)
    }

    fn has(&self, name: &str) -> Result<bool, Error> {
        let guard = self.conn.lock().map_err(lock)?;
        let conn = guard.as_ref().ok_or_else(unwired)?;
        let table = ddl::table(name);
        let mut stmt = conn
            .prepare("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1")
            .map_err(sql)?;
        let found = stmt.exists([table.as_str()]).map_err(sql)?;
        Ok(found)
    }

    fn cols(&self, name: &str) -> Result<Vec<String>, Error> {
        let guard = self.conn.lock().map_err(lock)?;
        let conn = guard.as_ref().ok_or_else(unwired)?;
        let table = ddl::table(name);
        let mut stmt = conn
            .prepare(&format!("PRAGMA table_info({table})"))
            .map_err(sql)?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .map_err(sql)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(sql)?);
        }
        Ok(out)
    }

    fn begin(&self) -> Result<(), Error> {
        self.step("BEGIN")
    }

    fn commit(&self) -> Result<(), Error> {
        self.step("COMMIT")
    }

    fn undo(&self) -> Result<(), Error> {
        self.step("ROLLBACK")
    }
}

struct Sql<'a>(&'a Connection);

impl Wire for Sql<'_> {
    fn run(&self, text: &str, args: &[Val]) -> Result<u64, Error> {
        let n = self
            .0
            .execute(text, rusqlite::params_from_iter(cast(args)))
            .map_err(sql)?;
        Ok(n as u64)
    }

    fn plant(&self, text: &str, args: &[Val]) -> Result<i64, Error> {
        self.0
            .execute(text, rusqlite::params_from_iter(cast(args)))
            .map_err(sql)?;
        Ok(self.0.last_insert_rowid())
    }

    fn rows(&self, text: &str, args: &[Val]) -> Result<Vec<Vec<Val>>, Error> {
        let mut stmt = self.0.prepare(text).map_err(sql)?;
        let width = stmt.column_count();
        let mut cursor = stmt
            .query(rusqlite::params_from_iter(cast(args)))
            .map_err(sql)?;
        let mut out = Vec::new();
        while let Some(row) = cursor.next().map_err(sql)? {
            let mut line = Vec::with_capacity(width);
            for at in 0..width {
                line.push(lift(row.get_ref(at).map_err(sql)?)?);
            }
            out.push(line);
        }
        Ok(out)
    }

    fn script(&self, text: &str) -> Result<(), Error> {
        self.0.execute_batch(text).map_err(sql)
    }
}

fn cast(args: &[Val]) -> Vec<Value> {
    args.iter()
        .map(|arg| match arg {
            Val::Null => Value::Null,
            Val::Int(value) => Value::Integer(*value),
            Val::Text(value) => Value::Text(value.clone()),
        })
        .collect()
}

fn lift(cell: ValueRef<'_>) -> Result<Val, Error> {
    match cell {
        ValueRef::Null => Ok(Val::Null),
        ValueRef::Integer(value) => Ok(Val::Int(value)),
        ValueRef::Text(bytes) => Ok(Val::Text(String::from_utf8_lossy(bytes).into_owned())),
        _ => Err(Error::Adapt("unsupported column type".into())),
    }
}

fn sql(err: rusqlite::Error) -> Error {
    Error::Adapt(err.to_string())
}

fn lock(_: std::sync::PoisonError<std::sync::MutexGuard<'_, Option<Connection>>>) -> Error {
    Error::Adapt("sqlite lock poisoned".into())
}

fn unwired() -> Error {
    Error::Adapt("sqlite not wired".into())
}

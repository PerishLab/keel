use crate::adapt::Error;
use crate::ddl;
use crate::life::{self, Ends, Row, Tie};
use crate::plan::Plan;
use crate::store::Store;
use rusqlite::Connection;
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
        for stmt in ddl::script(plan) {
            conn.execute_batch(&stmt).map_err(sql)?;
        }
        let mut guard = self.conn.lock().map_err(lock)?;
        *guard = Some(conn);
        Ok(())
    }

    fn put(&self, plan: &Plan, name: &str, fields: &[(&str, &str)]) -> Result<i64, Error> {
        let guard = self.conn.lock().map_err(lock)?;
        let conn = guard.as_ref().ok_or_else(unwired)?;
        life::Work::new(conn).put(plan, name, fields)
    }

    fn set(&self, plan: &Plan, name: &str, key: i64, fields: &[(&str, &str)]) -> Result<(), Error> {
        let guard = self.conn.lock().map_err(lock)?;
        let conn = guard.as_ref().ok_or_else(unwired)?;
        life::Work::new(conn).set(plan, name, key, fields)
    }

    fn live(&self, plan: &Plan, name: &str) -> Result<Vec<Row>, Error> {
        let guard = self.conn.lock().map_err(lock)?;
        let conn = guard.as_ref().ok_or_else(unwired)?;
        life::Work::new(conn).live(plan, name)
    }

    fn end(&self, plan: &Plan, name: &str, key: i64) -> Result<(), Error> {
        let guard = self.conn.lock().map_err(lock)?;
        let conn = guard.as_ref().ok_or_else(unwired)?;
        life::Work::new(conn).end(plan, name, key)
    }

    fn tie(&self, plan: &Plan, owner: &str, bond: &str, ends: Ends) -> Result<i64, Error> {
        let guard = self.conn.lock().map_err(lock)?;
        let conn = guard.as_ref().ok_or_else(unwired)?;
        life::Work::new(conn).tie(plan, owner, bond, ends)
    }

    fn ties(&self, plan: &Plan, owner: &str, bond: &str, left: i64) -> Result<Vec<Tie>, Error> {
        let guard = self.conn.lock().map_err(lock)?;
        let conn = guard.as_ref().ok_or_else(unwired)?;
        life::Work::new(conn).ties(plan, owner, bond, left)
    }

    fn cut(&self, plan: &Plan, owner: &str, bond: &str, key: i64) -> Result<(), Error> {
        let guard = self.conn.lock().map_err(lock)?;
        let conn = guard.as_ref().ok_or_else(unwired)?;
        life::Work::new(conn).cut(plan, owner, bond, key)
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

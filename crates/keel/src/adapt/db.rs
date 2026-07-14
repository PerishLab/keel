use crate::adapt::Error;
use crate::ddl;
use crate::plan::Plan;
use rusqlite::Connection;
use std::sync::Mutex;

pub trait Db {
    fn wire(&self, plan: &Plan) -> Result<(), Error>;
}

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

    pub fn has(&self, name: &str) -> Result<bool, Error> {
        let guard = self.conn.lock().map_err(lock)?;
        let conn = guard.as_ref().ok_or_else(unwired)?;
        let table = ddl::table(name);
        let mut stmt = conn
            .prepare("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1")
            .map_err(sql)?;
        let found = stmt.exists([table.as_str()]).map_err(sql)?;
        Ok(found)
    }

    pub fn cols(&self, name: &str) -> Result<Vec<String>, Error> {
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

    fn open(&self) -> Result<Connection, Error> {
        match &self.place {
            Place::Memory => Connection::open_in_memory().map_err(sql),
            Place::File(path) => Connection::open(path).map_err(sql),
        }
    }
}

impl Db for Sqlite {
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

use crate::adapt::Error;
use crate::ddl;
use crate::life::{self, Ends, Row, Tie};
use crate::plan::Plan;
use crate::store::Store;
use crate::wire::{Val, Wire};
use postgres::Client;
use postgres::types::Type;
use std::sync::Mutex;

pub struct Postgres {
    url: String,
    conn: Mutex<Option<Client>>,
}

impl Postgres {
    pub fn at(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            conn: Mutex::new(None),
        }
    }
}

impl Store for Postgres {
    fn wire(&self, plan: &Plan) -> Result<(), Error> {
        if plan.units().is_empty() {
            return Err(Error::Adapt("db plan is empty".into()));
        }
        let mut client = Client::connect(&self.url, postgres::NoTls).map_err(pg)?;
        for stmt in ddl::script(plan, ddl::Grain::Pg) {
            client.batch_execute(&stmt).map_err(pg)?;
        }
        let mut guard = self.conn.lock().map_err(lock)?;
        *guard = Some(client);
        Ok(())
    }

    fn put(&self, plan: &Plan, name: &str, fields: &[(&str, &str)]) -> Result<i64, Error> {
        self.work(|wire| life::Work::new(wire).put(plan, name, fields))
    }

    fn set(&self, plan: &Plan, name: &str, key: i64, fields: &[(&str, &str)]) -> Result<(), Error> {
        self.work(|wire| life::Work::new(wire).set(plan, name, key, fields))
    }

    fn live(&self, plan: &Plan, name: &str) -> Result<Vec<Row>, Error> {
        self.work(|wire| life::Work::new(wire).live(plan, name))
    }

    fn one(&self, plan: &Plan, name: &str, key: i64) -> Result<Option<Row>, Error> {
        self.work(|wire| life::Work::new(wire).one(plan, name, key))
    }

    fn end(&self, plan: &Plan, name: &str, key: i64) -> Result<(), Error> {
        self.work(|wire| life::Work::new(wire).end(plan, name, key))
    }

    fn lease(&self, plan: &Plan, name: &str, key: i64, at: i64) -> Result<(), Error> {
        self.work(|wire| life::Work::new(wire).lease(plan, name, key, at))
    }

    fn pulse(&self, plan: &Plan, verb: &str, unit: &str, key: i64, who: &str) -> Result<(), Error> {
        self.work(|wire| life::Work::new(wire).pulse(plan, verb, unit, key, who))
    }

    fn tie(
        &self,
        plan: &Plan,
        owner: &str,
        bond: &str,
        ends: Ends,
        fields: &[(&str, &str)],
    ) -> Result<i64, Error> {
        self.work(|wire| life::Work::new(wire).tie(plan, owner, bond, ends, fields))
    }

    fn set_tie(
        &self,
        plan: &Plan,
        owner: &str,
        bond: &str,
        key: i64,
        fields: &[(&str, &str)],
    ) -> Result<(), Error> {
        self.work(|wire| life::Work::new(wire).set_tie(plan, owner, bond, key, fields))
    }

    fn ties(&self, plan: &Plan, owner: &str, bond: &str, left: i64) -> Result<Vec<Tie>, Error> {
        self.work(|wire| life::Work::new(wire).ties(plan, owner, bond, left))
    }

    fn cut(&self, plan: &Plan, owner: &str, bond: &str, key: i64) -> Result<(), Error> {
        self.work(|wire| life::Work::new(wire).cut(plan, owner, bond, key))
    }

    fn live_has(&self, plan: &Plan, name: &str, key: i64) -> Result<bool, Error> {
        self.work(|wire| life::Work::new(wire).live_has(plan, name, key))
    }

    fn has(&self, name: &str) -> Result<bool, Error> {
        let table = ddl::table(name);
        let rows = self.probe(
            "SELECT 1 FROM information_schema.tables WHERE table_name = $1",
            &[Val::Text(table)],
        )?;
        Ok(!rows.is_empty())
    }

    fn cols(&self, name: &str) -> Result<Vec<String>, Error> {
        let table = ddl::table(name);
        let rows = self.probe(
            "SELECT column_name FROM information_schema.columns WHERE table_name = $1 ORDER BY ordinal_position",
            &[Val::Text(table)],
        )?;
        Ok(rows.into_iter().map(|line| line[0].text()).collect())
    }

    fn begin(&self) -> Result<(), Error> {
        self.work(|wire| wire.script("BEGIN"))
    }

    fn commit(&self) -> Result<(), Error> {
        self.work(|wire| wire.script("COMMIT"))
    }

    fn undo(&self) -> Result<(), Error> {
        self.work(|wire| wire.script("ROLLBACK"))
    }
}

impl Postgres {
    fn work<T>(&self, run: impl FnOnce(&Pg<'_>) -> Result<T, Error>) -> Result<T, Error> {
        let mut guard = self.conn.lock().map_err(lock)?;
        let client = guard.as_mut().ok_or_else(unwired)?;
        let wire = Pg {
            conn: std::cell::RefCell::new(client),
        };
        run(&wire)
    }

    fn probe(&self, sql: &str, args: &[Val]) -> Result<Vec<Vec<Val>>, Error> {
        self.work(|wire| wire.rows(sql, args))
    }
}

struct Pg<'a> {
    conn: std::cell::RefCell<&'a mut Client>,
}

impl Wire for Pg<'_> {
    fn run(&self, text: &str, args: &[Val]) -> Result<u64, Error> {
        let sql = dollar(text);
        let held = own(args);
        let slots = lean(&held);
        self.conn.borrow_mut().execute(&sql, &slots).map_err(pg)
    }

    fn plant(&self, text: &str, args: &[Val]) -> Result<i64, Error> {
        let sql = format!("{} RETURNING {}", dollar(text), ddl::KEY);
        let held = own(args);
        let slots = lean(&held);
        let row = self.conn.borrow_mut().query_one(&sql, &slots).map_err(pg)?;
        Ok(row.get::<_, i64>(0))
    }

    fn rows(&self, text: &str, args: &[Val]) -> Result<Vec<Vec<Val>>, Error> {
        let sql = dollar(text);
        let held = own(args);
        let slots = lean(&held);
        let rows = self.conn.borrow_mut().query(&sql, &slots).map_err(pg)?;
        let mut out = Vec::new();
        for row in &rows {
            let mut line = Vec::with_capacity(row.len());
            for at in 0..row.len() {
                line.push(lift(row, at)?);
            }
            out.push(line);
        }
        Ok(out)
    }

    fn script(&self, text: &str) -> Result<(), Error> {
        self.conn.borrow_mut().batch_execute(text).map_err(pg)
    }
}

enum Hold {
    Null,
    Int(i64),
    Text(String),
}

fn own(args: &[Val]) -> Vec<Hold> {
    args.iter()
        .map(|arg| match arg {
            Val::Null => Hold::Null,
            Val::Int(value) => Hold::Int(*value),
            Val::Text(value) => Hold::Text(value.clone()),
        })
        .collect()
}

fn lean(held: &[Hold]) -> Vec<&(dyn postgres::types::ToSql + Sync)> {
    held.iter()
        .map(|slot| match slot {
            Hold::Null => &None::<i64> as &(dyn postgres::types::ToSql + Sync),
            Hold::Int(value) => value as &(dyn postgres::types::ToSql + Sync),
            Hold::Text(value) => value as &(dyn postgres::types::ToSql + Sync),
        })
        .collect()
}

fn dollar(text: &str) -> String {
    text.replace('?', "$")
}

fn lift(row: &postgres::Row, at: usize) -> Result<Val, Error> {
    match row.columns()[at].type_() {
        &Type::INT8 => {
            let value: Option<i64> = row.try_get(at).map_err(pg)?;
            Ok(value.map(Val::Int).unwrap_or(Val::Null))
        }
        &Type::INT4 => {
            let value: Option<i32> = row.try_get(at).map_err(pg)?;
            Ok(value.map(|n| Val::Int(n as i64)).unwrap_or(Val::Null))
        }
        &Type::TEXT | &Type::VARCHAR | &Type::BPCHAR | &Type::NAME => {
            let value: Option<String> = row.try_get(at).map_err(pg)?;
            Ok(value.map(Val::Text).unwrap_or(Val::Null))
        }
        other => Err(Error::Adapt(format!("unsupported pg type {other}"))),
    }
}

fn pg(err: postgres::Error) -> Error {
    Error::Adapt(err.to_string())
}

fn lock<T>(_: std::sync::PoisonError<T>) -> Error {
    Error::Adapt("pg lock poisoned".into())
}

fn unwired() -> Error {
    Error::Adapt("pg not wired".into())
}

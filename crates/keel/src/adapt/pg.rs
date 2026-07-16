use crate::adapt::Error;
use crate::ddl;
use crate::life::{self, Ends, Row, Tie};
use crate::plan::Plan;
use crate::store::Store;
use crate::wire::{Val, Wire};
use postgres::Client;
use postgres::types::Type;
use std::sync::Mutex;
use std::sync::mpsc::{Sender, SyncSender, channel, sync_channel};
use std::thread;

enum Cmd {
    Wire(String),
    Run(String, Vec<Val>),
    Plant(String, Vec<Val>),
    Rows(String, Vec<Val>),
}

enum Reply {
    Count(u64),
    Id(i64),
    Rows(Vec<Vec<Val>>),
    Done,
}

type Job = (Cmd, SyncSender<Result<Reply, Error>>);

pub struct Postgres {
    hand: Mutex<Sender<Job>>,
}

impl Postgres {
    pub fn at(url: impl Into<String>) -> Self {
        let url = url.into();
        let (tx, rx) = channel::<Job>();
        thread::spawn(move || {
            let mut client = match Client::connect(&url, postgres::NoTls) {
                Ok(client) => client,
                Err(_) => return,
            };
            while let Ok((cmd, back)) = rx.recv() {
                let _ = back.send(serve(&mut client, cmd));
            }
        });
        Self {
            hand: Mutex::new(tx),
        }
    }

    fn ask(&self, cmd: Cmd) -> Result<Reply, Error> {
        let (back, wait) = sync_channel::<Result<Reply, Error>>(1);
        self.hand
            .lock()
            .map_err(|_| Error::Adapt("pg hand poisoned".into()))?
            .send((cmd, back))
            .map_err(|_| Error::Adapt("pg thread gone".into()))?;
        wait.recv()
            .map_err(|_| Error::Adapt("pg thread gone".into()))?
    }

    fn work<T>(&self, run: impl FnOnce(&Pg<'_>) -> Result<T, Error>) -> Result<T, Error> {
        run(&Pg { store: self })
    }
}

fn serve(client: &mut Client, cmd: Cmd) -> Result<Reply, Error> {
    match cmd {
        Cmd::Wire(sql) => client.batch_execute(&sql).map(|_| Reply::Done).map_err(pg),
        Cmd::Run(sql, args) => {
            let held = own(&args);
            let slots = lean(&held);
            client.execute(&sql, &slots).map(Reply::Count).map_err(pg)
        }
        Cmd::Plant(sql, args) => {
            let held = own(&args);
            let slots = lean(&held);
            let row = client.query_one(&sql, &slots).map_err(pg)?;
            Ok(Reply::Id(row.get::<_, i64>(0)))
        }
        Cmd::Rows(sql, args) => {
            let held = own(&args);
            let slots = lean(&held);
            let rows = client.query(&sql, &slots).map_err(pg)?;
            let mut out = Vec::new();
            for row in &rows {
                out.push(line(row)?);
            }
            Ok(Reply::Rows(out))
        }
    }
}

struct Pg<'a> {
    store: &'a Postgres,
}

impl Wire for Pg<'_> {
    fn run(&self, text: &str, args: &[Val]) -> Result<u64, Error> {
        match self.store.ask(Cmd::Run(dollar(text), args.to_vec()))? {
            Reply::Count(n) => Ok(n),
            _ => Err(Error::Adapt("pg reply shape".into())),
        }
    }

    fn plant(&self, text: &str, args: &[Val]) -> Result<i64, Error> {
        let sql = format!("{} RETURNING {}", dollar(text), ddl::KEY);
        match self.store.ask(Cmd::Plant(sql, args.to_vec()))? {
            Reply::Id(id) => Ok(id),
            _ => Err(Error::Adapt("pg reply shape".into())),
        }
    }

    fn rows(&self, text: &str, args: &[Val]) -> Result<Vec<Vec<Val>>, Error> {
        match self.store.ask(Cmd::Rows(dollar(text), args.to_vec()))? {
            Reply::Rows(out) => Ok(out),
            _ => Err(Error::Adapt("pg reply shape".into())),
        }
    }

    fn script(&self, text: &str) -> Result<(), Error> {
        self.store.ask(Cmd::Wire(text.into())).map(|_| ())
    }
}

impl Store for Postgres {
    fn wire(&self, plan: &Plan) -> Result<(), Error> {
        if plan.units().is_empty() {
            return Err(Error::Adapt("db plan is empty".into()));
        }
        for stmt in ddl::script(plan, ddl::Grain::Pg) {
            self.ask(Cmd::Wire(stmt))?;
        }
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
        let rows = self.work(|wire| {
            wire.rows(
                "SELECT 1 FROM information_schema.tables WHERE table_name = $1",
                &[Val::Text(table)],
            )
        })?;
        Ok(!rows.is_empty())
    }

    fn cols(&self, name: &str) -> Result<Vec<String>, Error> {
        let table = ddl::table(name);
        let rows = self.work(|wire| {
            wire.rows(
                "SELECT column_name FROM information_schema.columns WHERE table_name = $1 ORDER BY ordinal_position",
                &[Val::Text(table)],
            )
        })?;
        Ok(rows.into_iter().map(|line| line[0].text()).collect())
    }

    fn begin(&self) -> Result<(), Error> {
        self.ask(Cmd::Wire("BEGIN".into())).map(|_| ())
    }

    fn commit(&self) -> Result<(), Error> {
        self.ask(Cmd::Wire("COMMIT".into())).map(|_| ())
    }

    fn undo(&self) -> Result<(), Error> {
        self.ask(Cmd::Wire("ROLLBACK".into())).map(|_| ())
    }
}

enum Bag {
    Null,
    Int(i64),
    Text(String),
}

fn own(args: &[Val]) -> Vec<Bag> {
    args.iter()
        .map(|arg| match arg {
            Val::Null => Bag::Null,
            Val::Int(value) => Bag::Int(*value),
            Val::Text(value) => Bag::Text(value.clone()),
        })
        .collect()
}

fn lean(held: &[Bag]) -> Vec<&(dyn postgres::types::ToSql + Sync)> {
    held.iter()
        .map(|slot| match slot {
            Bag::Null => &None::<i64> as &(dyn postgres::types::ToSql + Sync),
            Bag::Int(value) => value as &(dyn postgres::types::ToSql + Sync),
            Bag::Text(value) => value as &(dyn postgres::types::ToSql + Sync),
        })
        .collect()
}

fn line(row: &postgres::Row) -> Result<Vec<Val>, Error> {
    let mut out = Vec::with_capacity(row.len());
    for at in 0..row.len() {
        out.push(lift(row, at)?);
    }
    Ok(out)
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

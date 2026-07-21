use crate::adapt::Error;
use crate::ddl;
use crate::ddl::Grain;
use crate::wire::{Val, Wire};
use sqlx::postgres::{PgConnectOptions, PgConnection, PgRow};
use sqlx::{
    AssertSqlSafe, ConnectOptions as _, Connection as _, Row as _, TypeInfo as _, ValueRef as _,
};

pub struct Postgres {
    conn: PgConnection,
    opts: PgConnectOptions,
    sound: bool,
}

impl Postgres {
    pub async fn at(url: impl Into<String>) -> Result<Self, Error> {
        let opts = opts(&url.into())?;
        let conn = opts.clone().connect().await.map_err(sql)?;
        Ok(Self {
            conn,
            opts,
            sound: true,
        })
    }

    fn mark<T>(&mut self, out: Result<T, sqlx::Error>) -> Result<T, Error> {
        self.sound &= out.is_ok();
        out.map_err(sql)
    }
}

fn opts(url: &str) -> Result<PgConnectOptions, Error> {
    if url.contains("://") {
        return url
            .parse()
            .map_err(|e| Error::Adapt(format!("pg url: {e}")));
    }
    let mut opts = PgConnectOptions::new();
    for pair in url.split_whitespace() {
        let Some((key, value)) = pair.split_once('=') else {
            return Err(Error::Adapt(format!("pg conninfo: {pair}")));
        };
        opts = match key {
            "host" => opts.host(value),
            "port" => opts.port(value.parse().map_err(|_| Error::Adapt("pg port".into()))?),
            "user" => opts.username(value),
            "password" => opts.password(value),
            "dbname" => opts.database(value),
            _ => return Err(Error::Adapt(format!("pg conninfo key: {key}"))),
        };
    }
    Ok(opts)
}

impl Wire for Postgres {
    fn grain(&self) -> Grain {
        Grain::Pg
    }

    async fn run(&mut self, text: &str, args: &[Val]) -> Result<u64, Error> {
        let done = load(&dollar(text), args).execute(&mut self.conn).await;
        Ok(self.mark(done)?.rows_affected())
    }

    async fn plant(&mut self, text: &str, args: &[Val]) -> Result<i64, Error> {
        let text = format!("{} RETURNING {}", dollar(text), ddl::KEY);
        let row = load(&text, args).fetch_one(&mut self.conn).await;
        self.mark(row)?.try_get::<i64, _>(0).map_err(sql)
    }

    async fn rows(&mut self, text: &str, args: &[Val]) -> Result<Vec<Vec<Val>>, Error> {
        let found = load(&dollar(text), args).fetch_all(&mut self.conn).await;
        let rows = self.mark(found)?;
        let mut out = Vec::new();
        for row in &rows {
            out.push(line(row)?);
        }
        Ok(out)
    }

    async fn script(&mut self, text: &str) -> Result<(), Error> {
        let done = sqlx::raw_sql(AssertSqlSafe(text.to_string()))
            .execute(&mut self.conn)
            .await;
        self.mark(done).map(|_| ())
    }

    async fn revive(&mut self) -> Result<(), Error> {
        if self.sound {
            return Ok(());
        }
        if self.conn.ping().await.is_err() {
            self.conn = self.opts.clone().connect().await.map_err(sql)?;
        }
        self.sound = true;
        Ok(())
    }
}

fn load(
    text: &str,
    args: &[Val],
) -> sqlx::query::Query<'static, sqlx::Postgres, sqlx::postgres::PgArguments> {
    let mut query = sqlx::query::<sqlx::Postgres>(AssertSqlSafe(text.to_string()));
    for arg in args {
        query = match arg {
            Val::Null => query.bind(None::<i64>),
            Val::Int(value) => query.bind(*value),
            Val::Text(value) => query.bind(value.clone()),
        };
    }
    query
}

fn line(row: &PgRow) -> Result<Vec<Val>, Error> {
    let mut out = Vec::with_capacity(row.len());
    for at in 0..row.len() {
        out.push(lift(row, at)?);
    }
    Ok(out)
}

fn lift(row: &PgRow, at: usize) -> Result<Val, Error> {
    let cell = row.try_get_raw(at).map_err(sql)?;
    if cell.is_null() {
        return Ok(Val::Null);
    }
    match cell.type_info().name() {
        "INT8" => row.try_get::<i64, _>(at).map(Val::Int).map_err(sql),
        "INT4" => row
            .try_get::<i32, _>(at)
            .map(|n| Val::Int(n as i64))
            .map_err(sql),
        "TEXT" | "VARCHAR" | "BPCHAR" | "NAME" => {
            row.try_get::<String, _>(at).map(Val::Text).map_err(sql)
        }
        other => Err(Error::Adapt(format!("unsupported pg type {other}"))),
    }
}

fn dollar(text: &str) -> String {
    text.replace('?', "$")
}

fn sql(err: sqlx::Error) -> Error {
    Error::Adapt(err.to_string())
}

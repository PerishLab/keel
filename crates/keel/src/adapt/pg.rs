use crate::adapt::{Error, Store};
use crate::ddl;
use crate::ddl::Grain;
use crate::wire::{Val, Wire};
use sqlx::encode::IsNull;
use sqlx::error::BoxDynError;
use sqlx::postgres::types::Oid;
use sqlx::postgres::{PgArgumentBuffer, PgConnectOptions, PgConnection, PgRow, PgTypeInfo};
use sqlx::{
    AssertSqlSafe, ConnectOptions as _, Connection as _, Row as _, TypeInfo as _, ValueRef as _,
};

struct Void;

impl sqlx::Type<sqlx::Postgres> for Void {
    fn type_info() -> PgTypeInfo {
        PgTypeInfo::with_oid(Oid(0))
    }

    fn compatible(_: &PgTypeInfo) -> bool {
        true
    }
}

impl sqlx::Encode<'_, sqlx::Postgres> for Void {
    fn encode_by_ref(&self, _: &mut PgArgumentBuffer) -> Result<IsNull, BoxDynError> {
        Ok(IsNull::Yes)
    }
}

pub struct Postgres {
    conn: PgConnection,
    opts: PgConnectOptions,
    sound: bool,
}

impl Postgres {
    pub async fn at(url: impl Into<String>) -> Result<Self, Error> {
        let opts = opts(&url.into())?;
        let conn = opts.clone().connect().await.map_err(gone)?;
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

    async fn epoch(&mut self) -> Result<Option<String>, Error> {
        let rows = self.rows("SELECT pg_current_snapshot()::text", &[]).await?;
        Ok(rows.first().and_then(|row| row.first()).map(Val::text))
    }

    async fn revive(&mut self) -> Result<(), Error> {
        if self.sound {
            return Ok(());
        }
        if self.conn.ping().await.is_err() {
            self.conn = self.opts.clone().connect().await.map_err(gone)?;
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
            Val::Null => query.bind(Void),
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
        other => Err(Error::Store(Store::Fault(format!(
            "unsupported pg type {other}"
        )))),
    }
}

fn dollar(text: &str) -> String {
    text.replace('?', "$")
}

fn gone(err: sqlx::Error) -> Error {
    Error::Store(Store::Down(err.to_string()))
}

fn sql(err: sqlx::Error) -> Error {
    let text = err.to_string();
    Error::Store(match &err {
        sqlx::Error::Database(db) if db.is_unique_violation() => {
            Store::Taken(db.constraint().unwrap_or_default().to_string())
        }
        sqlx::Error::Database(db) => match db.code().as_deref() {
            Some(code) if code.starts_with("08") || code.starts_with("57P") => Store::Down(text),
            Some(code) if code.starts_with("53") => Store::Down(text),
            _ => Store::Fault(text),
        },
        sqlx::Error::Io(_)
        | sqlx::Error::Tls(_)
        | sqlx::Error::PoolTimedOut
        | sqlx::Error::PoolClosed
        | sqlx::Error::WorkerCrashed => Store::Down(text),
        _ => Store::Fault(text),
    })
}

impl Postgres {
    pub async fn wipe(&mut self) -> Result<(), Error> {
        self.script("DROP SCHEMA public CASCADE; CREATE SCHEMA public;")
            .await
    }
}

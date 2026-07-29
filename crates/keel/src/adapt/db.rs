use crate::adapt::Error;
use crate::ddl::Grain;
use crate::wire::{Val, Wire};
use sqlx::sqlite::{SqliteConnectOptions, SqliteConnection, SqliteRow};
use sqlx::{AssertSqlSafe, ConnectOptions, Row as _, TypeInfo as _, ValueRef as _};

pub struct Sqlite {
    conn: SqliteConnection,
}

impl Sqlite {
    pub async fn memory() -> Result<Self, Error> {
        Self::join(SqliteConnectOptions::new().in_memory(true)).await
    }

    pub async fn file(path: impl AsRef<std::path::Path>) -> Result<Self, Error> {
        Self::join(
            SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true),
        )
        .await
    }

    async fn join(opts: SqliteConnectOptions) -> Result<Self, Error> {
        let conn = opts.connect().await.map_err(sql)?;
        Ok(Self { conn })
    }
}

impl Wire for Sqlite {
    fn grain(&self) -> Grain {
        Grain::Lite
    }

    async fn run(&mut self, text: &str, args: &[Val]) -> Result<u64, Error> {
        let done = load(text, args)
            .execute(&mut self.conn)
            .await
            .map_err(sql)?;
        Ok(done.rows_affected())
    }

    async fn plant(&mut self, text: &str, args: &[Val]) -> Result<i64, Error> {
        let done = load(text, args)
            .execute(&mut self.conn)
            .await
            .map_err(sql)?;
        Ok(done.last_insert_rowid())
    }

    async fn rows(&mut self, text: &str, args: &[Val]) -> Result<Vec<Vec<Val>>, Error> {
        let rows = load(text, args)
            .fetch_all(&mut self.conn)
            .await
            .map_err(sql)?;
        let mut out = Vec::new();
        for row in &rows {
            out.push(line(row)?);
        }
        Ok(out)
    }

    async fn script(&mut self, text: &str) -> Result<(), Error> {
        sqlx::raw_sql(AssertSqlSafe(text.to_string()))
            .execute(&mut self.conn)
            .await
            .map(|_| ())
            .map_err(sql)
    }
}

fn load(
    text: &str,
    args: &[Val],
) -> sqlx::query::Query<'static, sqlx::Sqlite, sqlx::sqlite::SqliteArguments> {
    let mut query = sqlx::query::<sqlx::Sqlite>(AssertSqlSafe(text.to_string()));
    for arg in args {
        query = match arg {
            Val::Null => query.bind(None::<i64>),
            Val::Int(value) => query.bind(*value),
            Val::Text(value) => query.bind(value.clone()),
        };
    }
    query
}

fn line(row: &SqliteRow) -> Result<Vec<Val>, Error> {
    let mut out = Vec::with_capacity(row.len());
    for at in 0..row.len() {
        out.push(lift(row, at)?);
    }
    Ok(out)
}

fn lift(row: &SqliteRow, at: usize) -> Result<Val, Error> {
    let cell = row.try_get_raw(at).map_err(sql)?;
    if cell.is_null() {
        return Ok(Val::Null);
    }
    match cell.type_info().name() {
        "INTEGER" | "BOOLEAN" => row.try_get::<i64, _>(at).map(Val::Int).map_err(sql),
        "TEXT" => row.try_get::<String, _>(at).map(Val::Text).map_err(sql),
        _ => Err(Error::Adapt("unsupported column type".into())),
    }
}

fn sql(err: sqlx::Error) -> Error {
    Error::Adapt(err.to_string())
}

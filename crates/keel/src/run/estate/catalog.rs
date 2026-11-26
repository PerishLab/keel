use crate::adapt::Error;
use crate::ddl::Grain;
use crate::model::manifest::Manifest;
use crate::plan::Plan;
use crate::wire::{Val, Wire};

const TABLE: &str = "@estate";
const CLOCK: &str = "@clock";
const DERIVATIVE: &str = "@derivative";
const GENERATION: &str = "@generation";
pub(super) const FORMAT: i64 = 12;

pub(crate) struct Catalog<'a, W>(pub(crate) &'a mut W);

impl<W: Wire> Catalog<'_, W> {
    pub(super) async fn present(&mut self) -> Result<bool, Error> {
        let wire = &mut *self.0;
        let rows = match wire.grain() {
        Grain::Lite => {
            wire.rows(
                "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
                &[Val::Text(TABLE.into())],
            )
            .await?
        }
        Grain::Pg => {
            wire.rows(
                "SELECT 1 FROM information_schema.tables WHERE table_schema = current_schema() AND table_name = ?1",
                &[Val::Text(TABLE.into())],
            )
            .await?
        }
    };
        Ok(!rows.is_empty())
    }

    pub(super) async fn empty(&mut self) -> Result<bool, Error> {
        let wire = &mut *self.0;
        let rows = match wire.grain() {
        Grain::Lite => {
            wire.rows(
                "SELECT name FROM sqlite_master WHERE name NOT LIKE 'sqlite_%' LIMIT 1",
                &[],
            )
            .await?
        }
        Grain::Pg => {
            wire.rows(
                "SELECT c.relname::text FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace WHERE n.nspname = current_schema() AND c.relkind IN ('r', 'p', 'v', 'm', 'S') LIMIT 1",
                &[],
            )
            .await?
        }
    };
        Ok(rows.is_empty())
    }

    pub(crate) async fn status(&mut self) -> Result<crate::Status, Error> {
        if self.present().await? {
            super::verify(&mut *self.0).await?;
            return Ok(crate::Status::Occupied);
        }
        if self.empty().await? {
            return Ok(crate::Status::Vacant);
        }
        Err(Error::Estate(super::Fault::Unsealed))
    }

    pub(super) async fn shape(&mut self) -> Result<String, Error> {
        Ok(frame(&self.rows().await?))
    }

    pub(super) async fn rows(&mut self) -> Result<Vec<Vec<Val>>, Error> {
        let wire = &mut *self.0;
        let rows = match wire.grain() {
        Grain::Lite => {
            wire.rows(
                "SELECT type, name, tbl_name, sql FROM sqlite_master WHERE name NOT LIKE 'sqlite_%' ORDER BY type, name",
                &[],
            )
            .await?
        }
        Grain::Pg => wire.rows(POSTGRES, &[]).await?,
    };
        Ok(rows)
    }
}

pub(crate) async fn bootstrap<W: Wire>(
    plan: &Plan,
    manifest: &Manifest,
    token: &str,
    wire: &mut W,
) -> Result<(), Error> {
    wire.script("BEGIN").await?;
    let out = async {
        if vacant(wire, manifest).await? {
            return seed(plan, manifest, token, wire).await;
        }
        if !crate::cap::sealed(plan, wire, token).await? {
            return Err(Error::Estate(super::Fault::Occupied));
        }
        Ok(())
    }
    .await;
    finish(wire, out).await
}

pub(crate) async fn custody<W: Wire>(
    plan: &Plan,
    manifest: &Manifest,
    wire: &mut W,
    sink: impl FnOnce(&str) -> Result<(), Error> + Send,
) -> Result<bool, Error> {
    wire.script("BEGIN").await?;
    let out = async {
        if !vacant(wire, manifest).await? {
            return Ok(false);
        }
        let token = crate::cap::wild()?;
        seed(plan, manifest, &token, wire).await?;
        sink(&token)?;
        Ok(true)
    }
    .await;
    finish(wire, out).await
}

async fn finish<W: Wire, T>(wire: &mut W, out: Result<T, Error>) -> Result<T, Error> {
    let result = match out {
        Ok(value) => wire.script("COMMIT").await.map(|()| value),
        Err(error) => Err(error),
    };
    if result.is_err() {
        let _ = wire.script("ROLLBACK").await;
    }
    result
}

async fn vacant<W: Wire>(wire: &mut W, manifest: &Manifest) -> Result<bool, Error> {
    if wire.grain() == Grain::Pg {
        wire.script(
            "SELECT pg_advisory_xact_lock(hashtext('keel bootstrap'), hashtext(current_schema()::text))",
        )
        .await?;
    }
    if Catalog(wire).present().await? {
        if super::verify(wire).await?.digest != manifest.digest() {
            return Err(Error::Estate(super::Fault::Occupied));
        }
        return Ok(false);
    }
    if !Catalog(wire).empty().await? {
        return Err(Error::Estate(super::Fault::Unsealed));
    }
    Ok(true)
}

async fn seed<W: Wire>(
    plan: &Plan,
    manifest: &Manifest,
    token: &str,
    wire: &mut W,
) -> Result<(), Error> {
    for stmt in crate::ddl::script(plan, wire.grain()) {
        wire.script(&stmt).await?;
    }
    for stmt in script(wire.grain()) {
        wire.script(&stmt).await?;
    }
    let generation = super::next(wire, "generation").await?;
    crate::cap::genesis(plan, wire, token).await?;
    wire.run(
        "INSERT INTO \"@generation\" (id, state, digest, created, retired) VALUES (?1, ?2, ?3, ?4, NULL)",
        &[
            Val::Int(generation),
            Val::Text("active".into()),
            Val::Text(manifest.digest()),
            Val::Int(crate::life::tick()),
        ],
    )
    .await?;
    crate::life::Work::new(wire, plan)
        .etch(&crate::model::manifest::rows::spill(manifest), generation)
        .await?;
    let sealed = Catalog(wire).shape().await?;
    wire.run(
        "INSERT INTO \"@estate\" (id, format, active, shape) VALUES (?1, ?2, ?3, ?4)",
        &[
            Val::Int(1),
            Val::Int(FORMAT),
            Val::Int(generation),
            Val::Text(sealed),
        ],
    )
    .await?;
    Ok(())
}

pub(super) fn frame(rows: &[Vec<Val>]) -> String {
    let mut out = String::new();
    for row in rows {
        out.push('r');
        out.push_str(&row.len().to_string());
        out.push(':');
        for cell in row {
            let (tag, text) = match cell {
                Val::Null => ('n', String::new()),
                Val::Int(value) => ('i', value.to_string()),
                Val::Text(value) => ('s', value.clone()),
            };
            out.push(tag);
            out.push_str(&text.len().to_string());
            out.push(':');
            out.push_str(&text);
        }
    }
    out
}

pub(super) fn script(grain: Grain) -> Vec<String> {
    let int = match grain {
        Grain::Lite => "INTEGER",
        Grain::Pg => "BIGINT",
    };
    vec![
        format!(
            "CREATE TABLE \"{TABLE}\" (id {int} PRIMARY KEY NOT NULL, format {int} NOT NULL, active {int} NOT NULL, shape TEXT NOT NULL);"
        ),
        format!(
            "CREATE TABLE \"{GENERATION}\" (id {int} PRIMARY KEY NOT NULL, state TEXT NOT NULL, digest TEXT NOT NULL, created {int} NOT NULL, retired {int});"
        ),
        format!("CREATE TABLE \"{CLOCK}\" (name TEXT PRIMARY KEY NOT NULL, value {int} NOT NULL);"),
        format!(
            "CREATE TABLE \"{DERIVATIVE}\" (generation {int} NOT NULL, path TEXT NOT NULL, key {int} NOT NULL, created {int} NOT NULL, PRIMARY KEY (generation, path, key));"
        ),
    ]
}

const POSTGRES: &str = "
SELECT kind, name, detail FROM (
    SELECT 'table'::text AS kind, c.relname::text AS name, c.relkind::text AS detail
    FROM pg_catalog.pg_class c
    JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
    WHERE n.nspname = current_schema() AND c.relkind IN ('r', 'p', 'v', 'm', 'S')
    UNION ALL
    SELECT 'column'::text, c.relname::text || '.' || a.attname::text,
        pg_catalog.format_type(a.atttypid, a.atttypmod) || '|' ||
        a.attnotnull::text || '|' ||
        COALESCE(pg_catalog.pg_get_expr(d.adbin, d.adrelid), '') || '|' ||
        a.attidentity::text
    FROM pg_catalog.pg_attribute a
    JOIN pg_catalog.pg_class c ON c.oid = a.attrelid
    JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
    LEFT JOIN pg_catalog.pg_attrdef d ON d.adrelid = a.attrelid AND d.adnum = a.attnum
    WHERE n.nspname = current_schema() AND c.relkind IN ('r', 'p')
        AND a.attnum > 0 AND NOT a.attisdropped
    UNION ALL
    SELECT 'index'::text, t.relname::text || '.' || i.relname::text,
        replace(
            replace(
                pg_catalog.pg_get_indexdef(i.oid),
                ' ON ONLY ' || pg_catalog.quote_ident(n.nspname) || '.',
                ' ON ONLY '
            ),
            ' ON ' || pg_catalog.quote_ident(n.nspname) || '.',
            ' ON '
        )
    FROM pg_catalog.pg_index x
    JOIN pg_catalog.pg_class i ON i.oid = x.indexrelid
    JOIN pg_catalog.pg_class t ON t.oid = x.indrelid
    JOIN pg_catalog.pg_namespace n ON n.oid = t.relnamespace
    WHERE n.nspname = current_schema()
    UNION ALL
    SELECT 'constraint'::text, c.relname::text || '.' || x.conname::text,
        pg_catalog.pg_get_constraintdef(x.oid, true)
    FROM pg_catalog.pg_constraint x
    JOIN pg_catalog.pg_class c ON c.oid = x.conrelid
    JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
    WHERE n.nspname = current_schema()
    UNION ALL
    SELECT 'trigger'::text, c.relname::text || '.' || t.tgname::text,
        pg_catalog.pg_get_triggerdef(t.oid, true)
    FROM pg_catalog.pg_trigger t
    JOIN pg_catalog.pg_class c ON c.oid = t.tgrelid
    JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
    WHERE n.nspname = current_schema() AND NOT t.tgisinternal
) estate
ORDER BY kind, name, detail
";

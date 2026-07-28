use crate::adapt::Error;
use crate::ddl::Grain;
use crate::wire::{Val, Wire};

const TABLE: &str = "@estate";
const CLOCK: &str = "@clock";
const DERIVATIVE: &str = "@derivative";
const GENERATION: &str = "@generation";
pub(super) const FORMAT: i64 = 9;

pub(super) async fn present<W: Wire>(wire: &mut W) -> Result<bool, Error> {
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

pub(super) async fn empty<W: Wire>(wire: &mut W) -> Result<bool, Error> {
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

pub(super) async fn shape<W: Wire>(wire: &mut W) -> Result<String, Error> {
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
    Ok(frame(&rows))
}

fn frame(rows: &[Vec<Val>]) -> String {
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
            "CREATE TABLE \"{GENERATION}\" (id {int} PRIMARY KEY NOT NULL, state TEXT NOT NULL, digest TEXT NOT NULL, manifest TEXT NOT NULL, created {int} NOT NULL, retired {int});"
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

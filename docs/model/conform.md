# Conformance plan (store portability)

keel's law: engine replaceable, modeling invariant, scenario suite = TCK
(`docs/run/verify.md`). SQLite has been the only store; this plan validates the
claim with a real Postgres store, and stages the blob plane on MinIO.

## Substrate

- `docker-compose.yml` runs Postgres 17 (port 5433). `runseal :conform`
  (to build) runs the L2 suite against sqlite and pg and asserts byte-equal
  packs — the TCK gate.

## PG store: the Wire seam (staged)

The engine logic (life / capability / cache / txn) is driver-agnostic; only
the SQL projection differs. Introduce one seam, keep one copy of the logic:

- **Val**: `Null | Int(i64) | Text(String)` — keel's own cell/param value.
- **Wire** trait: `run(sql, &[Val]) -> u64`, `run_id(sql, &[Val]) -> i64`
  (insert, returns key), `rows(sql, &[Val]) -> Vec<Vec<Val>>`. No driver
  row types leak into `life`.
- `life::Work<W: Wire>` reads results by indexing `Vec<Val>`; every
  `conn.prepare/execute/query` becomes a Wire call.

Step 1: refactor `life` onto Wire with the rusqlite impl as the only Wire;
existing suite proves behavior unchanged. Step 2: add the `pg` feature, a
postgres-backed Wire, and dialect handling (`?N`→`$N`, `RETURNING id`,
`BIGSERIAL`, `information_schema` for `cols`); ddl gains a dialect flag.

## Dialect deltas (SQLite → PG)

| Site | SQLite | Postgres |
|------|--------|----------|
| params | `?N` | `$N` |
| insert id | `last_insert_rowid()` | `RETURNING id` |
| key column | `INTEGER PRIMARY KEY` | `BIGINT GENERATED ALWAYS AS IDENTITY` |
| introspect | `PRAGMA table_info` | `information_schema.columns` |
| partial unique | `... WHERE expires_at IS NULL` | same |
| txn | `BEGIN/COMMIT/ROLLBACK` | same |

## Blob plane (forgejo F5)

`keel-blob` (caller space, gate/relay pattern): a Hook-like metadata unit
(name/size/hash + root chain) via a declaration macro, plus routes that
capability-gate then stream bytes to/from MinIO. keel never stores bytes.
`forgejo/docker-compose.yml` runs Postgres + MinIO.


## Delivered

- **Wire seam** (keel #63): `life` is driver-agnostic; sqlite is one Wire.
- **PG store** (`adapt::pg::Postgres`, `pg` feature): a second Wire +
  `ddl::Grain::Pg` dialect (BIGINT identity keys, `$N` params, RETURNING id,
  information_schema introspection). `tests/pg.rs` runs the course scenario
  against real Postgres (docker-compose pg:5433) — unique, fat ties, link,
  count, K3, set all green. **The engine-replaceable law is now proven, not
  claimed.** Gate the pg test behind the running container.

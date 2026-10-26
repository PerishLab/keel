# Agents

Read the canonical [PerishLab delivery governance](https://github.com/PerishLab/.github/blob/main/GOVERNANCE.md)
at work start and again before delivery or Issue closure. That document owns
organization-wide Issue, pull-request and acceptance policy; this file keeps
repository-specific constraints without copying that policy.

Keel is a data-model description engine. Business callers declare resources,
fields, relations, and runtime values; the engine owns control fields,
lifecycle, authority, transactions, storage projection, events, cache, estate
evolution, and genesis. HTTP and stores are projections of that closure, not
business authoring surfaces.

## Product boundary

- Authentication stops before Keel. A caller proves credentials and injects
  one live identity row id as `Operator`; Keel never learns login, password,
  session, recovery, OIDC, or product identity policy.
- `Core` is the possession face. `Face` is the operator-scoped face. Resource
  effects must return through one of them; caller packages carry no hidden
  engine privilege.
- The `keel` library reads no configuration file, discovers no path, and
  knows no environment variable. Callers construct `Estate`, open a `Wire`,
  choose listen values, and hand them over. Only the executables read their
  flags and environment.
- Keel ships two executables so it can be operated and tested from outside
  (PerishLab/.github#127). `keel-api` serves one sealed estate: it reads the
  model from the estate, never compiles business types, and takes `--store`
  (`sqlite:PATH`, a `postgres://` URL or keyword conninfo), `--listen`
  (default `0.0.0.0:8080`) and `--prefix`, or `KEEL_STORE`, `KEEL_LISTEN` and
  `KEEL_PREFIX`. On SIGTERM or SIGINT it stops accepting connections, drains
  in-flight requests and exits 0. `keel bootstrap` seals a vacant estate from
  a manifest file and writes the sudo token only to a new custody file, mode
  0600, removing it if the seal fails; `keel status` reports vacant, sealed or
  unknown; `keel model` prints the canonical manifest of a file or of a sealed
  estate.
- Ordinary `bind` never initializes an empty namespace. `bootstrap` exposes
  status, possession mint, and transactional seal hotspots while the caller
  owns custody. `adopt` is the explicit exact-projection exception for an
  unsealed namespace.
- Source code admits no comments. Refactor unclear behavior into names, types,
  modules, and tests. Repository prose belongs only to the admitted root
  documents; release notes live on Depot.

## Repository map

- `crates/keel` — model compiler, runtime engine, adapters, HTTP projection.
- `crates/api` — `keel-api`, and the store opening and ceremonies the CLI
  shares.
- `crates/cli` — the `keel` operator CLI.
- `crates/macro` — resource declaration macros.
- `crates/gate` — default credential package in caller space.
- `crates/relay` — default webhook consumer in caller space.
- `crates/blob` — capability-gated object metadata and presigned-byte package;
  bytes never enter Keel.
- `crates/column` — append-only record package keeping records verbatim in
  immutable Parquet parts; records never enter Keel.
- `Containerfile` — the `keel-api` image. Its build context is only this file
  and a `keel-api` binary beside it; it bakes in no configuration.
- `.cargo` — the `perish` registry configuration.
- `.runseal` — committed inert Runseal resources.

## Maintenance laws

- Keep the dependency direction centered on `keel`; packages compose its
  public faces and never receive a privileged backchannel.
- Keep stores behind `Wire`. Driver row types, SQL dialects, and backend
  identity must not leak into model or lifecycle semantics.
- Preserve the six engine verbs. New ceremonies compose verbs; they do not
  create private write paths.
- A `Core` serves a cached read only while its `Wire` epoch is unchanged since
  the cached read: SQLite's `data_version`, Postgres's current snapshot. Commits
  from other connections, including other processes, therefore invalidate it.
  A `Wire` without an epoch reads uncached.
- A unique field is a key over the rows that have no expiry, so a leased row
  holds no key. The partial unique index, the write checks for fields and for
  one2one refs, and the estate guard read a key with that one liveness, so one
  process and many give the same answer. Occupancy that outlives a lease is a
  product's own model.
- A key has no null part: a unique scope is a required field or ref, so a
  model check refuses an optional one before any store reads the key.
- An adapter reports a store fault by kind: `Store::Down` for a store that
  cannot answer, `Store::Taken` for a unique key the store refused, and
  `Store::Fault` for the rest. HTTP answers them 503, 409 and 500, and
  `/health` fails when one round trip to the store does. A refused key answers
  `field <name> taken`, as Keel's own check does. No caller sees driver text
  or a physical name; the driver's text goes only to the server log.
- `/-/keel` is Keel's operator interface, open to sudo only: a row's
  `created_at`, `updated_at` and `expires_at`, and the pulse after a cursor, in
  Keel's own words. It serves operators, debugging and black-box proof.
  Products never depend on it, and a product deadline stays a product field. A
  model cannot name a unit `-`.
- Resource paths answer only what a model declares: a row is its `id`, fields
  and refs, and a tie is its `id`, `left`, `right` and fields. Expiry and
  timestamps stay inside Keel. `@seal` and `@pulse` are refused on every
  resource path, in `/batch` and in `/query`; `@grant` is managed there by
  design.
- A lifecycle check that reads another row locks it when the store can:
  `end` takes each row it ends `FOR UPDATE` before it checks ties, refs and
  root children, and `put`, `set` and `tie` read each target `FOR SHARE`. On
  Postgres, under `READ COMMITTED`, the later of two racing writers waits and
  re-reads the committed row, so no live row or tie refers to an ended one
  across processes. SQLite's single writer already serializes them.
- A query reads a tie only while its target is live. `link`, `has` and
  `some` take ties through one reader, so a tie to an ended row is gone from
  every path, and an error reading ties fails the query instead of matching
  nothing.
- A row retires from the ties it holds, and a tie holds its target only while
  its holder is live. `end` and `lease` retire a row and its root children
  first, then refuse while a live row or a tie with a live holder outside them
  still refers to any of them, so references within one cascade never hold it
  open.
- Preserve fail-closed behavior for unknown schema changes, physical drift,
  unparsed query state, ambiguous identity, lost bootstrap custody, and stale
  stream cursors.
- Vocabulary deltas live with the implementation. Register only real compound
  product atoms in `ectropy.toml`; do not maintain a parallel source glossary.
- A public semantic change updates this document and re-affirms it with
  `plumb affirm --write` in the same change when it moves a claim here.

## keel-column

- It is a package beside the engine, like `keel-blob`: it composes only
  Keel's public faces, changes no `Wire`, `Grain`, verb or estate evolution,
  and knows no caller's record semantics.
- A table declares a time column, key columns, a sort order over keys and a
  partition grain (an optional key and a time width). Every record keeps its
  raw bytes verbatim in the `raw` column beside the time and key values its
  caller extracted; returning a record returns those bytes exactly.
- `Table::schema` exposes non-null UInt64 time, ordered Utf8 keys and Binary
  raw, followed by declared nullable fields. `Table::field` adds Bool, Int,
  Uint, Text or Bytes, mapped to Boolean, Int64, UInt64, Utf8 or Binary. Names
  are distinct across all columns; additional fields cannot become sort or
  partition keys. `fields` exposes their identity, order, type and nullability.
- `describe` and `parse` round-trip a canonical description at most 1 MiB:
  format 1 for field-free tables, format 2 for nullable fields. Unknown
  versions, types, fields and invalid declarations refuse. Existing format-1
  descriptions remain unchanged. Every additional field is nullable; omitted
  suffix values mean null, with no inferred value or backfill.
- A column root binds its physical declaration in `model.json`, under its
  writer lock, by synced atomic publication before record recovery or writes.
  Ordinary `Store::open` requires the exact model, including ordered keys,
  fields, sort and grain; it never upgrades. Invalid or changed bindings
  refuse before recovery can truncate segments or remove orphan parts.
- `Store::evolve` opens an existing bound root explicitly with an appended
  nullable field suffix. Existing field names, types, order, nullability and
  the base layout must match. While holding the exclusive writer lock it
  validates history against the saved model, then atomically replaces and
  syncs the binding before recovery; it rewrites no part or segment. An equal
  model is an idempotent reopen. The old writer must close first; this is not
  live evolution. A publication failure may leave the old or new complete
  binding; reread it rather than assuming a rollback. Torn historical tails
  must first recover under the saved model. Missing bindings, incompatible
  changes and contradictory history refuse before publication.
- `Record` and legacy append/write keep their shape and supply null for all
  additional fields. `Record::values` constructs a `Row` with a positional
  nullable-value prefix; `Store::write` and `Table::rows` accept typed rows.
  Type or width mismatches refuse before writes. Raw remains byte-for-byte
  caller data, independent of extracted values. Typed WAL entries are
  CRC-framed beside unchanged legacy entries and share token/fsync recovery.
- A historical nonempty root without a binding requires explicit
  `Store::adopt` with its original table declaration. Adoption first reads
  referenced part schemas, key/time ordering and partition membership, and
  verifies complete acknowledged open segments, record widths and partitions,
  refusing unknown segment names or multiple unpublished nonempty segments;
  it then publishes the binding and runs ordinary recovery. Its validation
  refusals leave record files unchanged. Torn legacy segments must first be
  recovered by their original writer; adoption never guesses their model.
- Adoption attests the caller's original declaration: observations cannot
  recover column labels from positional open values or prove a unique grain
  from finite records. The binding describes physical storage, not estate
  identity or business schema; Keel remains the authority for live Part rows.
  Model-aware writers enforce it; older writers cannot, so writer rollback
  across this boundary requires an explicit operator compatibility review.
- A part is one immutable Parquet file written from a batch of one partition,
  sorted by the sort keys then time: zstd, the time column delta-encoded, key
  columns dictionary-encoded with bloom filters, page statistics. A part is
  published without overwriting; an existing path refuses. Scans select by a
  `[from, to)` window and exact key values, prune by row-group statistics and
  bloom filters, and return raw bytes in sort order within a part.
- `Table::query` and `Store::query` validate a `Query` before storage reads,
  including an empty store: a nonempty half-open time window when both bounds
  are given, declared exact key predicates, distinct declared projection
  columns, and batches of 1 through 65536 rows. `Query::select` returns Arrow
  batches in requested column order; `Query::count` returns the exact matched
  count in `Stats` without emitting batches. Both reuse the existing physical
  columns and pruning; count and projections without `raw` never decode it.
  A historical part may have an exact physical prefix of the current schema;
  missing nullable suffix columns return typed null arrays. Wrong column
  names, types, nullability or order refuse. New scalar fields are selectable;
  predicates remain time and declared exact key predicates only.
- Query defaults bound candidate rows to ten million, matched rows to one
  million, file reads and open snapshot payload each to 256 MiB, and batches
  to 1024 rows. Callers may set `Limit` and batch size. A refusal or callback
  error fails the query; previously emitted batches are partial until the
  final success. A limit never silently truncates a successful count.
- Query `Stats` distinguish opened files, admitted row groups, candidate rows
  (all rows in admitted groups plus visited open records), matched rows,
  successfully returned rows and batches, file bytes read through the reader,
  and copied open snapshot payload bytes (time, all keys, selected raw and
  selected scalar payload).
  Read bytes include metadata and bloom reads, even repeated ranges; they
  describe neither physical disk I/O nor CPU. Snapshot payload excludes
  allocation overhead and Arrow buffers; these limits are no RSS guarantee.
- A store query selects live part paths and copies matching open records
  under one lock, then reads parts and returns batches outside it. Count
  copies no open payload; other projections copy raw and scalar payload only
  when selected.
  Ordering remains per part and sorted open records, with no global ordering,
  pagination, persistent snapshot or pin beyond the retention grace.
- An append carries a caller batch token and is acknowledged only after its
  partition's open segment is fsynced: CRC-framed records closed by the token
  frame, so a torn or unclosed tail is truncated on recovery and an
  unacknowledged batch never becomes visible. A token already applied to a
  partition, in its open segment or its recent parts, is acknowledged without
  writing; per-record deduplication belongs to later merging.
- A segment seals at 16 MiB of raw plus encoded scalar values or 5 minutes
  of age into a part
  whose `Part` row is published in Keel; a part is visible only through a live
  row. Callers plug `stock` into their Graph and so host a Keel estate.
- Retention ends a partition's rows in one transaction and drops its open
  segment; part files are deleted only after a grace, so scans in flight keep
  their files. Recovery deletes every file no live row references and every
  segment already published.
- One process holds a root's lock; scans take a snapshot of live parts and
  open segments and read files outside the lock.
- Column execution uses arrow and parquet, never DataFusion; their roughly
  monthly major releases are a carried upgrade.

## Operating

- Never commit on `main`; work on a topic branch or Concord Member.
- Guard proves every commit through the hooks `plumb configuration install`
  projects. Before landing, run:

```sh
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo check --locked --workspace --all-features --release
plumb doctor .
ectropy .
```

- The L2 contracts are `cargo test -p keel --test route` and
  `cargo test -p keel-gate --test forge`; they drive the Router in process.
- Issue-led work is delivered through its Concord Member after the complete
  local guard is green; `plumb land` lands only work no Issue carries.

## Release

- wharf publishes `keel-macro`, `keel`, `keel-relay`, `keel-blob`,
  `keel-column`, and `keel-gate` to the `perish` registry at
  `cargo.perish.uk`, in dependency order, reading each one back from the
  index. The same marker carries the `keel` CLI for every target and
  `keel-api` for Linux only, outside the installed manager, as the image
  `ghcr.io/perishlab/keel:<version>` built from the root `Containerfile`.
  Keel declares no skill; its release authority carries the distribution
  record wharf keeps for every marker.
- A release follows Plumb's lifecycle (`plumb release --help`); wharf
  publishes it, and a rerun publishes only what is missing.
- A stable version owes its bilingual changelog on Depot before the next marker
  is stamped. A release with no caller migration still says so explicitly.
- Never publish a crate by hand. A crate published outside wharf leaves no
  distribution record.

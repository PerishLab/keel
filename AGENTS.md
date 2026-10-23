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
- A part is one immutable Parquet file written from a batch of one partition,
  sorted by the sort keys then time: zstd, the time column delta-encoded, key
  columns dictionary-encoded with bloom filters, page statistics. A part is
  published without overwriting; an existing path refuses. Scans select by a
  `[from, to)` window and exact key values, prune by row-group statistics and
  bloom filters, and return raw bytes in sort order within a part.
- An append carries a caller batch token and is acknowledged only after its
  partition's open segment is fsynced: CRC-framed records closed by the token
  frame, so a torn or unclosed tail is truncated on recovery and an
  unacknowledged batch never becomes visible. A token already applied to a
  partition, in its open segment or its recent parts, is acknowledged without
  writing; per-record deduplication belongs to later merging.
- A segment seals at 16 MiB of raw records or 5 minutes of age into a part
  whose `Part` row is published in Keel; a part is visible only through a live
  row. Callers plug `stock` into their Graph and so host a Keel estate.
- Retention ends a partition's rows in one transaction and drops its open
  segment; part files are deleted only after a grace, so scans in flight keep
  their files. Recovery deletes every file no live row references and every
  segment already published.
- One process holds a root's lock; scans take a snapshot of live parts and
  open segments and read files outside the lock.
- Dependencies are arrow and parquet only, never DataFusion; their roughly
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

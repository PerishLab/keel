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
- Keel is a library and reads no configuration file, discovers no path, and
  knows no environment variable. Callers construct `Estate`, open a `Wire`,
  choose listen values, and hand them over.
- Ordinary `bind` never initializes an empty namespace. `bootstrap` exposes
  status, possession mint, and transactional seal hotspots while the caller
  owns custody. `adopt` is the explicit exact-projection exception for an
  unsealed namespace.
- Source code admits no comments. Refactor unclear behavior into names, types,
  modules, and tests. Repository prose belongs only to the admitted root
  documents; release notes live on Depot.

## Repository map

- `crates/keel` — model compiler, runtime engine, adapters, HTTP projection.
- `crates/macro` — resource declaration macros.
- `crates/gate` — default credential package in caller space.
- `crates/relay` — default webhook consumer in caller space.
- `crates/blob` — capability-gated object metadata and presigned-byte package;
  bytes never enter Keel.
- `crates/column` — append-only record package keeping records verbatim in
  immutable Parquet parts; records never enter Keel.
- `.cargo` — the `perish` registry configuration.
- `.runseal` — committed inert Runseal resources.

## Maintenance laws

- Keep the dependency direction centered on `keel`; packages compose its
  public faces and never receive a privileged backchannel.
- Keep stores behind `Wire`. Driver row types, SQL dialects, and backend
  identity must not leak into model or lifecycle semantics.
- Preserve the six engine verbs. New ceremonies compose verbs; they do not
  create private write paths.
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
- An acknowledged append is durable in an open segment; sealing turns it into a
  part and drops duplicate record ids within the partition; a part is visible
  only once published in a manifest held as Keel resources, updated in the
  same transaction; retention drops a whole partition, its parts and manifest
  entries atomically; scans cover published parts and the open segment. A
  caller hosting the manifest therefore hosts a Keel estate.
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

- Keel is a Cargo-only product. wharf publishes `keel-macro`, `keel`,
  `keel-relay`, `keel-blob`, `keel-column`, and `keel-gate` to the `perish`
  registry at `cargo.perish.uk`, in dependency order, reading each one back
  from the index.
  It declares no binaries and no skill; its release authority carries the
  distribution record wharf keeps for every marker.
- A release follows Plumb's lifecycle (`plumb release --help`); wharf
  publishes it, and a rerun publishes only what is missing.
- A stable version owes its bilingual changelog on Depot before the next marker
  is stamped. A release with no caller migration still says so explicitly.
- Never publish a crate by hand. A crate published outside wharf leaves no
  distribution record.

# Agents

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
  documents or release CHANGELOG.

## Repository map

- `crates/keel` — model compiler, runtime engine, adapters, HTTP projection.
- `crates/macro` — resource declaration macros.
- `crates/gate` — default credential package in caller space.
- `crates/relay` — default webhook consumer in caller space.
- `crates/blob` — capability-gated object metadata and presigned-byte package;
  bytes never enter Keel.
- [ARCHITECTURE.md](ARCHITECTURE.md) — current component and dataflow map.
- [DESIGN.md](DESIGN.md) — stable modeling and runtime laws.

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
- A public semantic change updates the relevant root projection and its Plumb
  document seal in the same change.

## Operating

- Never commit on `main`; work on a topic branch or Concord Member.
- `.forgejo/workflows/guard.yml` is the canonical guard lane. Before landing,
  run:

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
- Land only through `plumb land` after the complete local guard is green.

## Release

Plumb owns publication of `keel-macro`, `keel`, `keel-relay`, `keel-blob`, and
`keel-gate` to the `perish` registry. Stable release requires bilingual
`docs/CHANGELOG/v<version>/{en,zh}/{INDEX.md,MIGRATION.md}`. A release with no
caller migration still says so explicitly.

```sh
plumb release dispatch --channel <channel> --version <version>
```

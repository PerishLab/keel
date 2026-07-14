# Agents

`keel` is a data model description engine. Business callers define models and
relations only. The engine owns control fields and lifecycle. HTTP/store are
engine projections, not business authoring surfaces.

## Product boundary

- **Business**: `#[resource]`, `#[field]`, `#[relation]`, `Graph::plug`, `bind`.
- **Engine face (`Core`)**: `put` / `live` / `end` / `tie` / `ties` / `cut`.
- **Runtime policy**: repo-rooted `keel.toml` (`[listen]`, `[store]`); load via
  `config::load(root)`; missing file => defaults (memory store, 127.0.0.1:3000).
- **HTTP (axum)**: maps to Core; host/port from config; no pagination/query DSL.
- **Sidecar**: `sidecar.toml` manages `keel-api` process; health should match
  `[listen]` in `keel.toml` (do not dual-author ports).
- No reverse relation generation. No capability/auth/identity yet.
- No field validation yet (later on Store trait).

## Growth order

1. Pure data + sqlite DDL (done)
2. Row + n2m lifecycle (done)
3. Core face + axum + sidecar (current)
4. HTTP details (pagination, query DSL, cache) — undecided
5. Capability → identity → real estate scenarios

## Laws

- Single word, block depth <= 4, path depth <= 4, comments denied by default.
- Vocabulary deltas in `docs/vocabulary.md`.
- Boundaries in `negentropy.toml`.

## Operating

- Never commit on `main`; branch, then commit.
- `runseal :guard` before land; `runseal :land` only landing path.
- Operator flows: `.runseal/wrappers` TypeScript only.

## Directory map

- `crates/keel/` — engine library
- `crates/macro/` — proc macros
- `crates/api/` — demo HTTP binary for sidecar
- `keel.toml` — runtime policy (listen/store)
- `sidecar.toml` — local process plan
- `docs/` — vocabulary
- `.runseal/` / `.forgejo/` — guard and CI

## Common commands

```sh
runseal :init
runseal :guard
cargo run -p keel-api --locked
sidecar start --config sidecar.toml
```

# Agents

`keel` is a data model description engine. Business callers define models and
relations only. The engine owns control fields and lifecycle. HTTP/store are
engine projections, not business authoring surfaces.

## Product boundary

- **Business**: `#[resource]`, `#[field]`, `#[relation]`, `Graph::plug`, `bind`.
- **Engine face (`Core`)**: `put` / `set` / `live` / `end` / `tie` / `ties` / `cut`.
- **Runtime policy**: repo-rooted `keel.toml` (`[listen]`, `[store]`); load via
  `config::load(root)`; missing file => defaults (memory store, 127.0.0.1:3000).
- **HTTP (axum)**: resource REST + edge **write** (`tie`/`cut` routes); no
  association **reads**. Global `POST {prefix}/query` DSL; host/port/prefix
  from `keel.toml`. Filter/order/page/link only inside DSL.
- **Sidecar**: `sidecar.toml` manages `keel-api` process; health should match
  `[listen]` in `keel.toml` (do not dual-author ports).
- **Edge**: always-pack `/query`; `link` → bond bags (H0); root-only order/page
  (`docs/edge.md`). Bond attrs + `has` + live-unique + K1 (`docs/bond.md`).
- **Capability**: `docs/capability.md` — landed: `@grant` bootstrap,
  operator faces, root-chain subtree checks, mint/attenuation, genesis
  token window, `keel-gate` package (S2+S3 of `docs/spec.md`).
- **Trigger**: `docs/trigger.md` — landed: `@pulse` stream, who-attributed
  events, coverage-bound `flow`, `keel-relay` webhooks (S4 of spec).
- No reverse relation generation.
- No field validation yet (later on the engine seam).

## Growth order

Staged delivery contract: `docs/spec.md` (S0 legislation → S1 data →
S2 capability → S3 gate → S4 trigger → S5 cache → S6 estate). A stage is
done when its scenario is green; no stage begins against unlanded law.

## Laws

- Single word, block depth <= 4, path depth <= 4, comments denied by default.
- Vocabulary deltas in `docs/vocabulary.md`.
- Boundaries in `negentropy.toml`.
- Edge navigation: `docs/edge.md` (flat bags; root-only order/page).
- Capability: `docs/capability.md` (six verbs, `@grant`, operator, gate).
- Trigger: `docs/trigger.md` (thin events, observe-only, relay).
- Unique: `docs/unique.md` (serialized write path; live-unique; serial).
- Cache: `docs/cache.md` (invisible; under coverage; generations+horizon).
- Transaction: `docs/txn.md` (commit point per verb; batch all-or-nothing).
- Lease: `docs/lease.md` (end with an instant; no resurrection; L3 edges).

## Operating

- Never commit on `main`; branch, then commit.
- `runseal :guard` before land; `runseal :land` only landing path.
- Operator flows: `.runseal/wrappers` TypeScript only.

## Directory map

- `crates/keel/` — engine library
- `crates/macro/` — proc macros
- `crates/api/` — demo binaries (`keel-api`, `forge`) for sidecar/scenarios
- `crates/gate/` — `keel-gate`: default credential package (caller space)
- `crates/relay/` — `keel-relay`: default webhook package (caller space)
- `keel.toml` — runtime policy (listen/store)
- `sidecar.toml` — local process plan
- `docs/` — vocabulary, verify, edge, capability, trigger laws
- `.runseal/` / `.forgejo/` — guard and CI

## Verification

Cold-start contract: `docs/verify.md` (L1 unit / L2 smoke / L3 static).
`:guard` runs L1+L3 and then `:smoke` (L2).

## Common commands

```sh
runseal :init
runseal :guard
runseal :smoke
runseal :course
runseal :forge
runseal :ship
cargo run -p keel-api --locked
sidecar start --config sidecar.toml
```

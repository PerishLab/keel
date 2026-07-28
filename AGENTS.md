# Agents

`keel` is a data model description engine. Business callers define models and
relations only. The engine owns control fields and lifecycle. HTTP/store are
engine projections, not business authoring surfaces.

## Product boundary

- **Business**: `#[resource]`, `#[field]`, `#[relation]`, `Graph::plug`, `bind`.
- **Engine face (`Core`)**: `put` / `set` / `live` / `end` / `tie` / `ties` / `cut`.
- **Runtime policy**: repo-rooted `keel.toml`; the engine face owns
  `[listen]`, `[identity]`, `[cache]` (cascading default < file < `KEEL_*`
  env; load via `config::load(root)`; missing file => defaults,
  127.0.0.1:3000; malformed file => refuse to boot). `[store]` is each
  caller binary's own section: adaptors carry config fragments
  (`adapt::db::Store`, `adapt::pg::Store`) and the caller composes one at
  bootstrap, then hands the opened store to `bind` — the engine never
  interprets store config. The environment is read only through the
  cascade (plumb `docs/config.md`); config vocabulary is never imported
  from plumb, only the mechanism.
- **HTTP (axum)**: resource REST + edge **write** (`tie`/`cut` routes); no
  association **reads**. Global `POST {prefix}/query` DSL; host/port/prefix
  from `keel.toml`. Filter/order/page/link only inside DSL.
- **Sidecar**: `sidecar.toml` manages `keel-api` process; health should match
  `[listen]` in `keel.toml` (do not dual-author ports).
- **Edge**: always-pack `/query`; `link` → bond bags (H0); root-only order/page
  (`docs/model/edge.md`). Bond attrs + `has` + live-unique + K1
  (`docs/model/bond.md`).
- **Capability**: `docs/run/capability.md` — landed: `@grant` bootstrap,
  operator faces, root-chain subtree checks, mint/attenuation, genesis
  token window, `keel-gate` package (S2+S3 of `docs/model/spec.md`).
- **Trigger**: `docs/run/trigger.md` — landed: `@pulse` stream, who-attributed
  events, coverage-bound `flow`, `keel-relay` webhooks (S4 of spec).
- No reverse relation generation.
- No field validation yet (later on the engine seam).

## Growth order

Staged delivery contract: `docs/model/spec.md` (S0 legislation → S1 data →
S2 capability → S3 gate → S4 trigger → S5 cache → S6 estate). A stage is
done when its scenario is green; no stage begins against unlanded law.

## Laws

- Single word, block depth <= 4, path depth <= 4, comments denied by default.
- Vocabulary deltas in `docs/model/vocabulary.md`.
- Boundaries and registered compound terms in `ectropy.toml`.
- Edge navigation: `docs/model/edge.md` (flat bags; root-only order/page).
- Capability: `docs/run/capability.md` (six verbs, `@grant`, operator, gate).
- Trigger: `docs/run/trigger.md` (thin events, observe-only, relay).
- Unique: `docs/model/unique.md` (serialized write path; live-unique; serial).
- Cache: `docs/run/cache.md` (invisible; under coverage; generations+horizon).
- Transaction: `docs/run/txn.md` (commit point per verb; batch all-or-nothing).
- Lease: `docs/run/lease.md` (end with an instant; no resurrection; L3 edges).
- Name: `docs/model/name.md` (flat declaration, tree identity; `{root}:{name}`;
  full key persists, short name resolves).

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
- `keel.toml` — runtime policy (listen/store/identity/cache)
- `sidecar.toml` — local process plan
- `docs/` — vocabulary, verify, edge, capability, trigger laws
- `.runseal/` / `.forgejo/` — guard and CI

## Verification

Cold-start contract: `docs/run/verify.md` (L1 unit / L2 smoke / L3 static).
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

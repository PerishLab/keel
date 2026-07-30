# Agents

`keel` is a data model description engine. Business callers define models and
relations only. The engine owns control fields and lifecycle. HTTP/store are
engine projections, not business authoring surfaces.

## Product boundary

- **Resource closure**: keel owns resource identity, shape, lifecycle,
  relations, authority, transactions, storage projection, events, cache,
  estate evolution, and the genesis possession that grounds them. The closure
  ends at `Operator`.
- **Authentication boundary**: callers prove credentials and inject one live
  identity row id as `Operator`. Keel never learns login, password, session,
  recovery, OIDC, or product identity policy. Once admitted, every resource
  effect returns through the corresponding operator face.
- **Business**: `#[resource]`, `#[field]`, `#[relation]`, `Graph::plug`, `bind`;
  ordinary bind never installs an empty estate; `bootstrap` exposes Keel's
  possession mint and transactional seal hotspots without choosing custody;
  `.adopt()` is the explicit exact-projection ceremony for an unsealed estate;
  `.hook()` registers one idempotent non-estate derivative consumer.
- **Engine face (`Core`)**: `put` / `set` / `live` / `end` / `tie` / `ties` / `cut`.
- **Runtime policy**: keel is a library and owns no configuration file. It
  reads no file, discovers no path, and knows no environment variable. The
  caller constructs values and hands them over: `Estate` (with `Generation`,
  `Cleanup`, `Retain`) is keel vocabulary the caller fills and passes to
  `bind(...).estate(&estate)`; listen host/port/prefix are arguments to
  `listen`; the caller opens its own store and hands the opened store to
  `bind`. A caller that wants a config file declares its own, in its own
  format, under its own environment prefix.
- **HTTP (axum)**: resource REST + edge **write** (`tie`/`cut` routes); no
  association **reads**. Global `POST {prefix}/query` DSL; host/port/prefix are
  arguments the caller supplies. Filter/order/page/link only inside DSL.
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
- Bootstrap: `docs/run/bootstrap.md` (caller custody, explicit seal, replay).
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
- `crates/gate/` — `keel-gate`: default credential package (caller space)
- `crates/relay/` — `keel-relay`: default webhook package (caller space)
- `docs/` — vocabulary, verify, edge, capability, trigger laws
- `.runseal/` / `.forgejo/` — guard and CI

## Verification

Cold-start contract: `docs/run/verify.md` (L1 unit / L2 scenario / L3 static).
`:guard` runs all three. L2 lives in `crates/keel/tests/route` and
`crates/gate/tests/forge`; it drives the Router in process, and keel ships no
binary for it to boot.

## Common commands

```sh
runseal :init
runseal :guard
runseal :release
cargo test -p keel --test route
cargo test -p keel-gate --test forge
```

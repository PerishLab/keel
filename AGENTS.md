# Agents

`keel` is a data model description engine. Business callers define models and
relations only. The engine owns every control field and control capability.

## Product boundary

- **Business surface**: `#[resource]`, business `#[field]`, `#[relation]`,
  `Graph::plug`, `bind` with adapt ports.
- **Engine surface (not for business)**: reign (expires/created/updated),
  create/update/query/migration, capability, auth, identity.
- Adapt modules (`adapt::http`, `adapt::db`) are for adaptor authors wiring a
  plan, not for domain authors expressing control.
- Long-term suite role: fourth piece beside negentropy, runseal, sidecar.
  Real auth.perish.top stress comes only after pure data + adaptors mature.
- No sidecar product topology in this repo until multi-process need appears.

## Growth order

1. Pure data layer (current)
2. Mature http/db adaptors
3. Capability
4. Identity
5. Real estate scenarios

## Laws

- Single word, block depth <= 4, path depth <= 4, comments denied by default.
- Vocabulary deltas live in `docs/vocabulary.md`.
- Boundary exemptions live in `negentropy.toml`.

## Operating

- Never commit on `main`; branch, then commit.
- `runseal :guard` before land; `runseal :land` is the only landing path.
- Operator flows are TypeScript under `.runseal/wrappers` only.

## Directory map

- `crates/keel/` — engine library
- `crates/macro/` — proc macros
- `docs/` — vocabulary and design notes
- `docker-compose.yml` / `Dockerfile` — local postgres + tool image (official images)
- `.runseal/` — guard/init/land
- `.forgejo/` — Actions

## Common commands

```sh
runseal :init
runseal :guard
cargo test --workspace
negentropy --strict .
```

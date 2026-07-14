# Agents

`keel` is a personal-style data model description library. This repository is
guarded by the house suite: `negentropy` for structure, `runseal` for operator
flow, Forgejo for source and CI.

## Product boundary

- Library-first. Publish a Rust crate that describes data models in a house
  style; consumers own serialization, storage, and product semantics.
- No process control plane. `sidecar` is out of scope for this repo until a
  multi-process local runtime appears.
- No release pipeline yet. R2/`manage.sh`/release workflows land when publish is
  decided.

## Laws

- Single word, block depth <= 4, path depth <= 4, comments denied by default.
- Vocabulary deltas live in `docs/vocabulary.md`; compounds only in
  `vocabulary.toml` with rationale.
- Boundary exemptions live in `negentropy.toml`.

## Operating

- Never commit on `main`; the pre-commit hook refuses it. Branch, then commit.
- `runseal :guard` must pass before landing.
- `runseal :land` is the only landing path (Forgejo PR + guard + squash-merge).
- Repo-local operator flows are TypeScript under `.runseal/wrappers`. Do not add
  Python or uv for operator flows.

## Directory map

- `crates/keel/` — library source
- `docs/` — durable design and vocabulary notes
- `.runseal/` — Deno wrappers, hooks, pinned `negentropy.version`
- `.forgejo/` — Actions workflows
- `.local/` — gitignored private resources (secrets, ssh, tmp)

## Common commands

```sh
runseal :init
runseal :guard
cargo test --workspace
negentropy --strict .
```

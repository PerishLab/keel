# Vocabulary

Single-word naming is only livable against a maintained vocabulary. The vocabulary
is the set of atoms the codebase agrees to mean something, and it never freezes.

## Deltas, not just diffs

An agent that changes code also maintains the vocabulary. A new atom, a retired
atom, or a shifted meaning is a vocabulary delta, recorded alongside the code diff
so the shared meaning stays true.

## Current deltas

- `keel` — the library crate; data model description engine.
- `atom` — business field type marker (`string`, `url`) and its `Kind`.
- `bond` — relation cardinality marker (`n2m`) and its `Kind`.
- `spec` — sealed business resource shape before plan expansion.
- `graph` — set of plugged resource specs.
- `plan` — engine expansion of a graph for adaptors.
- `reign` — engine-owned control fields (expires, created, updated); never a business field.
- `unit` — one planned resource inside a plan.
- `slot` — one planned business field.
- `edge` — one planned relation.
- `core` — bound runtime handle after adapt wire; no business create/update/query.
- `bind` — attach graph to http and db adapt ports.
- `wire` — adaptor applies a plan; not a business verb.
- `plug` — insert one resource form into a graph.
- `seal` — finish a spec builder; business shape freezes.
- `adapt` — adaptor port namespace (http, db).
- `lift` — raise a graph or spec into the next engine form.
- `link` — macro-side relation attribute parse.
- `data` — local data-plane notes (`docs/data.md`) and compose postgres baseline.

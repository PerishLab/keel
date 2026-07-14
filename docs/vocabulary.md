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
- `sqlite` — cold-start db adapt implementation (closed loop, no daemon).
- `ddl` — engine SQL projection of a plan (tables, reign columns, joins).
- `cols` — adaptor diagnostic: list columns of a wired table.
- `has` — adaptor diagnostic: whether a table exists after wire.
- `cast` — map a field atom kind to a storage type name.
- `form` — DDL for one resource unit table.
- `arc` — DDL for one relation join table.
- `stamp` — append reign control columns onto a DDL column list.
- `life` — engine-internal row lifecycle (put / live / end).
- `put` — engine insert of business cells with reign stamps.
- `live` — rows in the effective time slice.
- `end` — stamp expires_at to now; row leaves the live slice.
- `row` — one engine-read record (key, cells, reign times).
- `cell` — one business field value inside a row.
- `tie` — create one n2m edge with reign stamps.
- `ties` — live n2m edges from one owner key.
- `cut` — end one n2m edge via expires_at.
- `work` — connection-scoped engine lifecycle operator.
- `path` — one plan-derived http route descriptor.
- `side` — foreign-key column name for a resource (`student_id`).
- `ends` — left/right key pair for a n2m tie.
- `face` — Core module: unified engine facade over plan + store.
- `store` — trait for engine row/edge lifecycle backends.
- `serve` / `listen` — axum entrypoints projecting Core over HTTP.
- `share` — wrap Core in Arc for concurrent HTTP handlers.
- `config` — repo-rooted runtime policy load (`keel.toml`).
- `listen` — host/port section of runtime policy.
- `store` — also the toml section naming where data lives (memory/file).
- `kind` — store backend selector (`memory` | `file`).
- `query` — text DSL entry for engine reads; also the `/query` HTTP route.
- `tree` — query AST (from + slice); execution only runs trees.
- `slice` — time-slice primitive on a tree (currently only live).
- `form` — build a tree for one unit without parsing text.
- `ask` — run a tree on Core; also type alias for Tree.
- `parse` — turn query text into a tree.
- `run` — execute a tree against plan + store.
- `digest` — normalized tree string for cache/identity later.
- `pred` — one where clause (field, op, value).
- `op` — predicate operator (currently only eq).
- `prefix` — HTTP api path prefix under listen.
- `smoke` — L2 process verification of REST + /query.
- `verify` — cold-start verification boundary document.

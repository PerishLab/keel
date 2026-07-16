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
- `bond` — relation kind (`many2many`) and its `Kind`.
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
- `set` — partial update of live business cells; bumps updated_at.
- `live` — rows in the effective time slice.
- `end` — stamp expires_at to now; row leaves the live slice.
- `row` — one engine-read record (key, cells, reign times).
- `cell` — one business field value inside a row.
- `tie` — create one many2many edge with reign stamps.
- `ties` — live many2many edges from one owner key.
- `cut` — end one many2many edge via expires_at.
- `work` — connection-scoped engine lifecycle operator.
- `path` — one plan-derived http route descriptor.
- `side` — foreign-key column name for a resource (`student_id`).
- `ends` — left/right key pair for a many2many tie.
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
- `pred` — one where clause (field or engine `id`, op, value).
- `op` — predicate operator (`=` `!=` `<` `<=` `>` `>=` `in` `has` `some`).
- `sort` — order clause on a tree (field + rank).
- `rank` — sort direction (`asc` | `desc`).
- `limit` — max rows returned after filter/sort/cursor.
- `after` — id cursor: rows strictly after that key in ordered result.
- `link` — tree clause that expands one forward bond into flat bags.
- `bag` — one named flat array inside pack.bags (unit rows or ties).
- `pack` — query result object `{ root, bags }`; clients assemble by id.
- `root` — pack field and tree `from` unit; sole subject of order/page.
- `prefix` — HTTP api path prefix under listen.
- `smoke` — L2 process verification of REST + /query.
- `course` — classic student/course selection scenario gate (`:course`).
- `bond` — also: many2many association may carry business field attrs on the join.
- `verify` — cold-start verification boundary document.
- `atom` — delta: atoms now `string`, `url`, `int`, `bool`; kinds Text/Link/Int/Bool.
- `cell` — delta: typed value (`Cell::Text/Int/Bool`), not a raw string.
- `pick` — read one typed cell out of a store row by slot kind.
- `bind` — also: cast one caller value to a storage value by slot kind.
- `fit` — match a value against a slot or pred kind; reject mistyped input.
- `show` — render a cell as display text (digest/sort-free).
- `many2many` — relation kind, spelled out; digit form `n2m` retired with no
  alias. Family law: `many2one` / `one2one` next; `one2many` deliberately
  absent (FK side declares).
- `many2one` — relation kind: FK column `{field}_id` on the declaring side;
  target must be live on put/set; inbound live refs block `end` (K3).
- `opt` — relation marker: many2one column may be NULL; empty value clears.
- `need` — spec/plan flag: relation is required (not `opt`).
- `point` — validate and cast one ref value (live target) to a storage key.
- `refs` — the many2one edges of a unit.
- `pluck` — read one caller field value by name, empty when absent.
- `known` — field name is a slot or a ref of the unit.
- `entry` — resolve one set column to storage column + casted value.
- `free` — spec builder: declare an `opt` relation (need = false).
- `one2one` — relation kind: many2one plus live-unique on the ref column;
  second live holder rejected (`live ref exists`, 409).
- `lone` — engine check: no other live row holds this one2one target.
- `point` — also: Kind method, true for single-target kinds (many2one/one2one).
- `only` — field uniqueness seat: `Free` | `All` (`unique`) | `Per(rel)`
  (`unique = rel`); live rows only (`docs/unique.md`).
- `sole` / `per` — spec builder verbs for the two unique forms.
- `solid` — engine gate: all unique slots hold before a write lands.
- `taken` — one unique slot probe; conflict reads `field {name} taken` (409).
- `worth` — effective value of a slot for a write (incoming else current).
- `anchor` — effective scope ref value for a `Per` unique check.
- `lock` — DDL second lock: partial unique index over live rows.
- `peek` — read one live row by key (engine-internal).
- `seek` — find one caller field value by name, `None` when absent.
- `serial` — engine-allocated per-scope monotonic int (`#[field(serial,
  scope = rel)]`); caller writes rejected; never reused (U5).
- `next` — allocate the next serial value inside the serialized write step.
- `tally` — macro parse of the serial field form.
- `col` / `seat` / `joint` — ddl: quoted SQL identifiers for business
  columns, unit tables, join tables; single-word fields may collide with
  SQL keywords (`index`, `order`), so generated SQL always quotes.
- `count` — query terminal: count pack `{root, count}` without bags; stands
  alone (`docs/edge.md`). Tree flag named `tally`.
- `forge` — flagship scenario binary + `:forge` gate; one act per spec
  stage (`docs/spec.md`).
- `root` — relation marker (C-M2): the one `many2one` that carries the
  unit's root chain; at most one per unit; always required.

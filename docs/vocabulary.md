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
- `cap` — capability module: `@grant` seat and write vetting.
- `@grant` — engine unit holding grants (who, verb, unit, scope); admin is
  put/end/query only — `set` on grant rows rejected (amend = end + put).
- `vet` — validate one grant row before it lands (verbs, who, unit, scope).
- `face` — delta: `Face` = operator-scoped verb surface; `Core` methods
  remain the possession (sudo) surface.
- `who` — face identity: `Sudo` | `Op(id)` | `Anon`.
- `of` / `sudo` / `anon` — Core face constructors.
- `check` — capability core: verb on row allowed iff a live grant covers
  (who, verb, unit, row) via all / row-subtree / pred scope.
- `mark` — the row under check: optional key + typed cells.
- `anchors` — root-chain walk (unit,id) pairs, self upward, depth-capped.
- `mint` — put grants the creator full verbs on the created row.
- `narrow` / `beneath` — attenuation gate on grant writes; delegation is
  inherent to holding; revoke is symmetric.
- `mold` / `blend` — build / merge typed cells from caller fields for
  pred-scope evaluation (put postcondition, set pre+post).
- `sift` / `strain` / `spot` — see-coverage filtering of reads (rows,
  packs incl. bond bags and count).
- `broad` — coverage probe for all-scope on a unit (attenuation floor).
- `@seal` — engine unit holding the sudo token hash; minted at first bind,
  surfaced once on stderr; the unique wire window (`authorization: sudo`).
- `genesis` / `sealed` — mint the axiom token / verify a presented token.
- `operator` — HTTP extension (`Operator(id)`) injected by caller
  middleware; keel never authenticates (401 belongs to middleware).
- `front` — serve helper: sudo window else injected operator else anon.
- `gate` — forge demo middleware: `x-login` → operator (deliberately toy).
- `app` — exported Router builder so caller middleware can wrap keel.
- `lease` — `end` completed with an instant (`docs/lease.md`): schedule
  death, renew while live, never resurrect; same verb, same coverage.
- `fresh` — live with `expires_at` NULL (unleased); new edges require it.
- `identity` — `keel.toml [identity] unit` names the operator unit;
  `Core::identify` carries it; anon put on it mints to the created row
  (C-14 exception).
- `keel-gate` — the default credential package crate (caller space):
  `gate!(Actor)` ships Token/Session bound to the caller's identity unit.
- `rise` — gate ceremony: seed the service operator's enumerable grants.
- `wall` / `pass` — gate middleware seat: credentials → injected operator.
- `doors` — shipped routes: register (anon put + possession token,
  mini-genesis), login (svc put Session + lease TTL), logout (operator end).
- `bearer` / `crumb` — authorization token / session cookie readers.
- `deny` — gate fault mapping (401/403/404/400).
- `make` / `gear` — macro-only files; scan excludes retired at negentropy
  v0.4.0 (macro items parse; templates under law).
- `@pulse` — engine unit: the write log's public face; one row per write
  verb (verb, unit, key, who); engine key is the total order; caller
  writes rejected.
- `beat` — emit one pulse row after a landed verb (who-attributed;
  `@pulse`/`@seal` excepted; emission failure logged, never blocking).
- `flow` — pull the stream after a cursor; floor-crossing cursor errors.
- `trim` — prune pulse rows beyond `WINDOW` (hard delete, engine-owned).
- `craft`/`shift`/`fell`/`knot`/`bend`/`snip` — who-attributed core verb
  bodies (put/set/end+lease/tie/set_tie/cut) shared by Core (sudo) and
  Face (operator); `lane` names a bond event path.
- `mate` — right join column: `{target}_id`, or `{bond}_id` when the bond
  is self-referential (fixes the `{table}_id` collision; F6 verified).
- `heard` / `caught` — delivery-time coverage on flow: live targets check
  normally; dead targets (end, cut ties) need all-scope see (T-M4 v1).
- `keel-relay` — the default webhook package crate (caller space):
  `relay!(Actor)` ships a Hook unit (url, unit/verb equality filters —
  data columns, not a second language); svc reads hooks, owners' coverage
  gates delivery (T-4), batch cursor holds on any failed post
  (at-least-once, T-7).
- `tick` / `serve` / `fits` / `letter` / `knock` — relay loop: one poll;
  one hook's deliveries; filter match; thin event json; raw http post.
- `stash` — the engine cache: engine-view packs under coverage; key =
  digest, validity = per-unit generation vector + lease horizon
  (`docs/cache.md`).
- `deal` — one cache entry (gens, until, pack).
- `bump` / `stamp` — advance a unit generation at `beat`; snapshot the
  vector for an entry.
- `horizon` — nearest lease `expires_at` inside a pack; entry dies there.
- `bare` — Core builder: cache off (`[cache] kind = "none"`).
- `hold` — cache capacity constant; overflow clears (eviction, never error).
- `involved` — the units a tree reads (root, link and pred targets);
  the invalidation footprint of a cached entry.
- `ship` — publish the crate family to the perish registry from clean
  main; idempotent per version (sparse-index probe skips published).
- `shelf` — sparse index path prefix for a crate name.
- `crew` — relation marker (C-M1): the one many2many that is a unit's
  membership roster; grant `who = "<unit> <id>"` admits its live members.
- `bearer` — who-match incl. group expansion through a crew bond.
- `whole` — validate a grant `who` value (id / group / anon / all).
- `cast` — spec builder: which marker a bond carries (Bond/Free/Root/Crew).
- `scopes` — macro parse of a unique scope, single ref or a ref tuple (U4).

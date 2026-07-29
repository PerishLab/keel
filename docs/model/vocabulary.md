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
- `tune` — partial update of the business cells one many2many edge carries;
  never touches its ends, its key, or reign. `tie` makes the pair, `tune`
  revises what the pair carries, `cut` ends it.
- `cut` — end one many2many edge via expires_at.
- `work` — connection-scoped engine lifecycle operator.
- `path` — one plan-derived http route descriptor.
- `side` — foreign-key column name for a resource (`student_id`).
- `ends` — left/right key pair for a many2many tie.
- `face` — Core module: unified engine facade over plan + store.
- `store` — trait for engine row/edge lifecycle backends.
- `serve` / `listen` — axum entrypoints projecting Core over HTTP.
- `share` — wrap Core in Arc for concurrent HTTP handlers.
- `config` — keel vocabulary the caller constructs; the library reads no file.
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
- `sort` — one field + rank in a tree's ordered sort list.
- `rank` — sort direction (`asc` | `desc`).
- `limit` — max rows returned after filter/sort/cursor.
- `after` — id cursor: rows strictly after that key in ordered result.
- `link` — tree clause that expands one forward bond into flat bags.
- `bag` — one named flat array inside pack.bags (unit rows or ties).
- `pack` — query result object `{ root, bags }`; clients assemble by id.
- `root` — pack field and tree `from` unit; sole subject of order/page.
- `prefix` — HTTP api path prefix under listen.
- `smoke` — L2 process verification of REST + /query.
- `course` — classic student/course selection scenario gate (route tests).
- `bond` — also: many2many association may carry business field attrs on the join.
- `verify` — cold-start verification boundary document.
- `atom` — delta: atoms now `string`, `url`, `int`, `bool`; kinds Text/Link/Int/Bool.
- `cell` — delta: typed value (`Cell::Text/Int/Bool`), not a raw string.
- `pick` / `bind` — read one typed store cell / cast one caller value by slot kind.
- `fit` — match a value against a slot or pred kind; reject mistyped input.
- `show` — render a cell as display text (digest/sort-free).
- `many2many` — relation kind, spelled out; digit form `n2m` retired. Family
  law continues with `many2one` / `one2one`; the FK side declares.
- `many2one` — FK column `{field}_id`; target must be live on writes and live
  inbound refs block `end` (K3).
- `opt` — field/relation marker: projected NULL; omitted scalar stays absent,
  empty text stays data, and an empty relation value remains a clear shorthand.
- `need` — scalar or single relation is required (not `opt`); `unset` /
  `loosen` is the public/internal verb clearing optional scalar or point
  fields to NULL; required and serial fields refuse.
- `point` — cast a live ref to a key; Kind test for many2one/one2one.
- `refs` — the many2one edges of a unit.
- `pluck` — read one caller field value by name, empty when absent.
- `known` — field name is a slot or a ref of the unit.
- `entry` — resolve one set column to storage column + casted value.
- `free` — spec builder: declare an `opt` relation (need = false).
- `one2one` — many2one plus live-unique ref; a second holder is rejected (409).
- `lone` — engine check: no other live row holds this one2one target.
- `only` — uniqueness: `Free` | `All` | `Per(rel)` over live rows only.
- `sole` / `per` — spec builder verbs for the two unique forms.
- `solid` — engine gate: all unique slots hold before a write lands.
- `taken` — one unique slot probe; conflict reads `field {name} taken` (409).
- `worth` — effective value of a slot for a write (incoming else current).
- `anchor` — effective scope ref value for a `Per` unique check.
- `lock` — DDL second lock: partial unique index over live rows.
- `peek` — read one live row by key (engine-internal).
- `seek` — find one caller field value by name, `None` when absent.
- `rule` — manifest field `default`/`values`/`min`/`max`; never runtime config.
- `fallback` — typed default for omitted puts and evolution fills.
- `serial` — engine-allocated per-scope monotonic int (`#[field(serial,
  scope = rel)]`); caller writes rejected; never reused (U5).
- `next` — allocate the next serial value inside the serialized write step.
- `tally` — macro parse of the serial field form.
- `col` / `seat` / `joint` — quoted SQL identifiers for columns, unit tables,
  and join tables; generated SQL always quotes keyword collisions.
- `count` — query terminal: count pack `{root, count}` without bags; stands
  alone (`docs/model/edge.md`). Tree flag named `tally`.
- `forge` — flagship scenario binary + `:forge` gate; one act per spec
  stage (`docs/model/spec.md`).
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
- `@seal` — engine unit holding the bootstrap-kept sudo hash; the unique wire window.
- `bootstrap` / `mint` / `seal` — open genesis / obtain sudo / commit estate; `genesis` / `sealed` store its hash / verify a token.
- `operator` — HTTP extension (`Operator(id)`) injected by caller
  middleware; keel never authenticates (401 belongs to middleware).
- `front` / `admit` — serve projection/resolver: sudo, operator, or anon.
- `gate` — forge demo middleware: `x-login` → operator (deliberately toy).
- `app` — exported Router builder so caller middleware can wrap keel.
- `lease` — `end` completed with an instant (`docs/run/lease.md`): schedule
  death, renew while live, never resurrect; same verb, same coverage.
- `fresh` — live with `expires_at` NULL (unleased); new edges require it.
- `identity` — `Core::identify` names the operator unit;
  the caller chooses it; anon put on it mints to the created row
  (C-14 exception).
- `keel-gate` — the default credential package crate (caller space):
  `gate!(Actor)` ships Token/Session bound to the caller's identity unit.
- `rise` / `seed` / `ready` — gate construction / explicit seed / verification; `birth` is sudo-only identity+self-grant.
- `wall` / `pass` — gate middleware seat: credentials → injected operator.
- `doors` — shipped routes: register (anon put + possession token,
  mini-genesis), login (svc put Session + lease TTL), logout (operator end).
- `bearer` / `crumb` — authorization token / session cookie readers.
- `deny` — gate fault mapping (401/403/404/400).
- `make` / `gear` — macro-only files; ectropy parses macro items directly
  while templates remain under law.
- `@pulse` — engine unit: the write log's public face; one row per write
  verb (verb, unit, key, who); engine key is the total order; caller
  writes rejected.
- `beat` — emit one pulse row after a landed verb (who-attributed;
  `@pulse`/`@seal` excepted; emission failure logged, never blocking).
- `flow` — pull the stream after a cursor; floor-crossing cursor errors.
- `trim` — prune pulse rows beyond `WINDOW` (hard delete, engine-owned).
- `craft`/`shift`/`fell`/`knot`/`bend`/`snip` — who-attributed core verb
  bodies (put/set/end+lease/tie/tune/cut) shared by Core (sudo) and
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
  (`docs/run/cache.md`).
- `deal` — one cache entry (gens, until, pack).
- `bump` / `stamp` — advance a unit generation at `beat`; snapshot the
  vector for an entry.
- `horizon` — nearest lease `expires_at` inside a pack; entry dies there.
- `bare` — Core builder: cache off (`[cache] kind = "none"`).
- `hold` — cache capacity constant; overflow clears (eviction, never error).
- `involved` — units a tree reads; the invalidation footprint of a cache entry.
- `ship` — idempotently publish the crate family from clean main.
- `estate` — Keel-owned manifest, generation, retention, and cleanup namespace.
- `manifest` — canonical caller model; nominal finite paths, order erased.
- `@estate` — private format, active generation, and physical shape root.
- `@generation` — active/candidate/cleanup manifests and digests.
- `@clock` — permanent named high-water state outside generations.
- `@derivative` — durable logical purge atoms awaiting hook acknowledgement.
- `generation` — one complete manifest expression; exactly one is active.
- `clock` — atomically advance named high-water in the write transaction.
- `format` — private storage format, outside business schema deltas.
- `shape` — canonical physical namespace snapshot checked at bind.
- `attach` / `seed` — bind a catalogued estate / transactionally install one.
- `status` / `vacant` / `token` / `occupied` — state / required / malformed / conflict.
- `frame` — length-delimited canonical rendering of physical catalog rows.
- `drift` — missing, changed, or extra physical object, never schema intent.
- `unsealed` — nonempty namespace without `@estate`; ordinary bind refuses.
- `adopt` — explicit exact-projection seal with allocator reconstruction.
- `projection` — isolated backend realization used to prove an unsealed shape.
- `gone` — one purged canonical path and permanent key.
- `purge` — one generation-keyed at-least-once derivative cleanup event.
- `changed` — requested manifest differs and needs generated evolution.
- `change` — read-only generated plan; callers cannot construct or edit it.
- `step` — one planned nominal path with an act and optional finite check.
- `act` — generated structural operation: add, drop, cast, or alter.
- `denied` — known unlegislated delta, reported by path before allocation.
- `blocked` — generated finite check failed, reported by path before allocation.
- `scalar` — classify retained field deltas into steps or denial.
- `candidate` — out-of-place generation built under `@g<id>:` physical names.
- `transfer` — copy the live world into candidate columns without business verbs.
- `activate` — transactionally exchange stable active names and generation states.
- `contract` — remove pulse events whose nominal path left the active manifest.
- `retain` — minimum cleanup-generation age before collection, or `forever`.
- `sweep` — transactionally collect every eligible cleanup generation and reseal
  the physical shape.
- `shelf` — sparse index path prefix for a crate name.
- `crew` — relation marker (C-M1): the one many2many that is a unit's
  membership roster; grant `who = "<unit> <id>"` admits its live members.
- `bearer` — who-match incl. group expansion through a crew bond.
- `whole` — validate a grant `who` value (id / group / anon / all).
- `cast` — spec builder: which marker a bond carries (Bond/Free/Root/Crew).
- `scopes` — macro parse of a unique scope, one named scalar/ref or a tuple (U4).
- `batch` — Core/Face verb: run a closure of writes as one transaction;
  all-or-nothing, intra-batch visibility, pulse inside the txn
  (`docs/run/txn.md`). Its HTTP face is `POST /batch`, taking a `deeds` list
  (put/set/end/tie/tune/cut, each naming its `unit`/`owner` by key in the
  body) — the colon-free write path, and the write-side mirror of `/query`.
  A `deed` is one entry in that list.
- `begin` / `commit` / `undo` — store transaction primitives; SQLite txn
  state lives on the connection, so per-call locking still shares it.
- `step` — run one transaction control word on the connection.
- `wire` — driver-agnostic SQL seam: `Wire` trait speaks only keel's `Val`
  (Null/Int/Text); `life::Work` is generic over it, never touches a driver
  row type. `run`/`plant`/`rows`/`script` are the four calls.
- `revive` — Wire's fifth call, defaulted to a no-op: re-establish a
  connection that died under the engine. Called from `seize`, so it runs
  only between ops (`docs/run/txn.md` X-9). `pg` tracks whether its last
  call was `sound` and pings only when it was not, so a healthy path pays
  nothing.
- `sql` — the rusqlite `Wire` impl (in adapt::db); `cast`/`lift` convert
  Val <-> rusqlite Value.
- `sheet` — canonical select column order for positional row reads.
- `pg` — the Postgres store (`adapt::pg`, `pg` feature): second Wire impl,
  proving store portability. `dollar`/`lift`/`lean`/`own` bridge Val <-> pg.
- `key` / `stem` — a unit's **identity** rendering and its root segment.
  `Unit::key()` is `{stem}:{name}` when the unit has a root, else the bare
  name, lowercased (`name::key`, in its own `name` module because identity
  is not physical DDL). `stem` is the declared name of the root relation's
  target — `Issue` rooted at `Repo` keys `repo:issue`. The key is what
  appears in `@grant` place, pulse events, pack bag keys, the route path,
  and every plan resolution. The plan and the graph are **keyed by it**, so
  two units may share a declared name once their roots differ (`docs/model/name.md`).
- `table` — a unit's **physical** SQL table name (`Unit::table()`, rendered
  by `ddl::table` as `{stem}_{name}`). A rooted unit's table is `repo_issue`,
  not `issue`; only `has`/`cols` (catalog reads) take a raw table name.
  Identity and table no longer coincide for rooted units — that is the
  point of the split.
- Resolution matches the declared name, its lowercase, or the full key
  **exactly** (never case-folding the caller's input); an ambiguous short
  name is refused with a message that does not name the candidates. A grant
  `place` is resolved to the unit's key at check time, so `Repo` and
  `actor:repo` in a stored grant mean the same unit.
- `grain` — ddl dialect (`Lite` | `Pg`): key column, int type, header.
- `keel-blob` — the default object package (caller space): `blob!(Actor)`
  ships an `Asset` metadata unit; upload/download are capability-gated
  presigned S3 URLs. **Bytes never touch keel** — not even in transit.
- `vault` / `shelf` — blob state + route merge. `stow` (POST /asset:
  gated put + presigned PUT url), `fetch` (GET /asset/{id}: gated see +
  302 to presigned GET url). `object` keys by asset id.
- `like` — query op: case-insensitive substring on a text field (issue search).
- `null` / `missing` — real absence query, spelled `where field is null`.
- pred-subtree (C-16) — a pred grant flows down the root chain; `held` evaluates it against the matching ancestor row.
- `bar` — gate suspension hook (`Gate::bar(field)`): refuses to resolve an operator whose identity row has the named bool field set true (settled gate law).
- `resolve` / `barred` — gate: credential→id, then suspension check.
- C-16 refinement — pred-subtree covers  only; write verbs never descend a pred (no broad-create write escalation).

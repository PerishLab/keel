# Architecture

Keel compiles a caller's resource graph into one canonical model, projects it
onto a store, and exposes the same lifecycle through in-process and HTTP faces.
The engine is the center; macros and optional packages only construct or
compose its public surface.

## Workspace

The workspace contains five published crates.

- `keel-macro` parses `#[resource]`, field, relation, rule, and mode
  declarations and emits the `Resource` implementation.
- `keel` owns the model compiler, storage seam, lifecycle, query engine,
  capability system, estate, cache, pulse stream, and HTTP projection.
- `keel-gate` declares ordinary credential resources and composes resolution,
  suspension, registration, login, logout, and revocation ceremonies.
- `keel-relay` declares webhook resources and consumes the pulse stream under
  ordinary service-operator grants.
- `keel-blob` declares object metadata and composes capability checks with
  presigned object-store URLs. Object bytes do not pass through Keel.

The package crates depend on `keel`; the engine never calls into them. This
keeps authentication, webhook delivery, and byte storage replaceable without
forking resource law.

## Model pipeline

A resource declaration produces a `Spec`: nominal name, mode, scalar fields,
relations, field rules, uniqueness, and relation markers. `Graph::plug`
collects specs. Lifting the complete graph validates names, references,
containment, relation markers, closed enums, and cycles before store access.

The lifted `Plan` contains `Unit`, `Slot`, `Edge`, and engine-owned `Reign`
shapes. It is keyed by full resource identity and is the common input to DDL,
query analysis, HTTP routes, capability root walks, and estate manifests.
Declaration order has no meaning; canonical order and hashes make equivalent
graphs identical.

The logical `Manifest` is the durable canonical expression of the Plan. It
contains business schema only. Engine units, catalog tables, backend dialect,
and private format are storage-format facts rather than business deltas.

## Storage seam

`Wire` is the only lifecycle-to-driver seam. It accepts Keel values and
canonical SQL operations; runtime modules do not read driver row types. The
SQLite adapter is the default closed-loop implementation. The Postgres adapter
implements the same seam with dialect-specific parameters, identity columns,
returns, and catalog inspection.

One engine instance owns one serialized connection. An operation acquires it
once and holds it through every read, check, write, pulse, and commit. A dropped
future marks the seat dirty so the next operation rolls back before proceeding.
If a connection dies, the affected operation fails; revival occurs only
between operations and never replays a statement.

Physical DDL is derived from the Plan and dialect. Business tables carry
stable projected names; engine tables remain estate-wide. Generated SQL quotes
identifiers and uses partial live indexes only as secondary locks. Observable
model semantics remain identical across stores and are held by the scenario
suite.

## Runtime faces

`Core` binds a Plan, Wire, estate policy, cache, optional cleanup hook, and
optional identity-unit designation. It is the in-process possession surface and
can create a `sudo`, `anon`, or operator `Face`.

A `Face` closes every operation over one `Who`. It exposes reads, the six
resource verbs, query, batch, and flow through one capability decision
procedure. A write mints row coverage for its creator only when no live grant
already covers the new row. `Tx` threads the same face and connection
through a batch so earlier deeds are visible to later ones before commit.

Lifecycle modules operate on typed `Cell`, `Row`, and `Tie` values. Business
writes never author engine keys or reign columns. All identity allocation uses
named estate clocks inside the same transaction as the write.

## Query and edge projection

Text queries parse into a `Tree`; callers may also build that tree directly.
Analysis resolves a root unit, typed predicates, direct links, stable root
ordering, limit, cursor, and the count terminal. A normalized digest identifies
the semantic query independently of spelling.

Execution always returns a `Pack`. A normal pack contains a root name and flat
named bags. The root bag holds unit rows. Each requested direct link adds a bond
bag of ties; it does not hydrate target rows. A count tree returns root and
count without bags. Root order and page never apply to bond bags.

HTTP is a projection over the same Core and Face operations. It provides
health, resource CRUD, edge writes, `/query`, and `/batch`; it does not provide
association reads. The caller supplies host, port, prefix, and middleware.
`Operator` is an in-process request extension, never a trusted wire header.

## Capability plane

`@grant` is an engine unit and the authority ledger. Capability checks read its
live rows directly and ground at one meta level. The closed verbs are `see`,
`put`, `set`, `end`, `tie`, and `cut`; `tune` is checked as `tie`. Row, predicate,
subtree, anonymous, universal, and crew coverage are resolved against the same
Plan and lifecycle state.

`@seal` stores the hash of the one bootstrap possession. Sudo enters only
through in-process Core possession or verification of that explicit window;
operator injection cannot express it. Every steady-state caller package uses
ordinary faces and enumerable grants.

The HTTP projector selects sudo, operator, or anonymous faces before running a
route. Unseen rows are filtered or map to 404; refused writes map to 403. Caller
middleware owns 401 because Keel does not authenticate.

## Pulse, relay, and cache

Every committed write appends one thin `@pulse` row inside its transaction.
The row carries sequence, verb, resource or bond path, key, actor, and time;
it carries no business payload. `flow` pulls this total order after a cursor,
checks delivery-time visibility, and refuses a cursor behind the retained
window.

Relay is an ordinary consumer of that stream. It reads Hook resources as a
service operator, applies simple data filters, and delivers at least once. A
held cursor gives retries; successful external effects therefore use the pulse
sequence as an idempotency key.

The in-memory cache stores engine-view packs below capability filtering. Keys
use the query digest; validity uses every involved unit's generation and the
nearest lease horizon. Writes and grants bump generations at the pulse choke
point. Coverage is always recomputed after a cache read, and the write path
never reads cache state.

## Estate lifecycle

An estate is the whole namespace Keel owns. Its private catalog records format,
active generation, canonical manifest rows, physical shape, permanent clocks,
and pending derivative cleanup atoms. Exactly one generation is active; older
generations await cleanup.

Ordinary bind validates the complete requested graph, catalog, active manifest,
and physical snapshot. An exact request attaches. A changed valid manifest
compiles into a closed, read-only `Change`; unknown or unsafe deltas refuse
before candidate allocation.

Evolution builds an out-of-place candidate under generation-qualified names,
copies the live world, rechecks invariants, and atomically exchanges stable
names. The former generation becomes cleanup state. Engine tables and permanent
allocation clocks remain estate-wide.

Bootstrap is separate from bind. It reports vacancy or occupied status, mints
one possession from OS entropy only for vacancy, and transactionally seals the
first estate after the caller has stored that possession. Exact replay is
idempotent; another possession or manifest refuses.

Adoption is also explicit. It builds an isolated projection, requires exact
physical agreement with a nonempty unsealed namespace, reconstructs every
allocator high-water from all rows, and creates the catalog transactionally.
It does not rewrite rows or assert business-semantic validity.

Cleanup drops eligible retired physical generations transactionally and emits
durable logical `Purge` atoms for facts or paths absent from the active world.
An optional typed hook consumes them after commit with at-least-once retry.
Hook failure never rolls back physical cleanup, and acknowledgement removes
only the delivered generation's atoms.

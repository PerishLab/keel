# Estate law

An estate is the whole store namespace Keel owns. Its manifest is the
canonical logical model; its catalog is Keel's private physical truth.
Callers declare the current model and never replay historical Rust types,
SQL, callbacks, or migration chains.

## Identity

Every named model node is nominal. A unit is identified by its full key.
Fields, bonds, and bond fields are identified by their path below that key.
The same path is an update; a changed path is removal plus addition. Keel
never infers a rename from similar shape or data.

Resource mode is nominal truth. Mutable and `frozen` resources are distinct;
changing mode under one full key is denied. A frozen fact admits its complete
payload and point relations only at `put`. Later `set`, `unset`, `tie`, `tune`,
and `cut` refuse, while logical `end` and `lease` remain lifecycle operations.
Generation evolution may copy a frozen fact into a same-name projection
without turning migration into a business mutation.

Manifest order is canonical. Unit, field, bond, bond-field, and unique-scope
declaration order has no meaning. Every kind and constraint is a closed
enum. Duplicate, ambiguous, unknown, or unlegislated shapes fail before
estate access.

## Catalog

The private catalog is model-independent:

| Seat | Meaning |
|------|---------|
| `@estate` | singleton private format, active generation key, and physical shape |
| `@generation` | every active, candidate, and cleanup generation with canonical manifest and digest |
| `@clock` | permanent named high-water values independent of every generation |
| `@derivative` | durable logical purge atoms awaiting caller hook acknowledgement |

Exactly one generation is active and `@estate.active` names it. Every
generation state is one of `active`, `candidate`, or `cleanup`. Every stored
manifest is canonical and matches its digest, including generations that are
not active. Creation time is permanent; retirement time exists only for
cleanup generations and is the retention clock.

The logical manifest excludes engine units. Changes to `@grant`, `@seal`,
`@pulse`, the catalog, or backend projection are Keel format changes, not
business schema deltas.

The physical snapshot covers every table, column, index, constraint,
trigger, view, and sequence visible in the owned namespace. An extra object
is drift just as a missing or altered object is.

## Bind

Bind is fail-closed:

1. Lift and validate the complete requested graph without store access.
2. If the namespace is empty, refuse `vacant` without writing; only the
   explicit bootstrap ceremony may create the first estate.
3. If the catalog exists, require a known format, one canonical manifest
   with a matching digest, and an exact physical snapshot.
4. Attach when the requested manifest equals the active manifest; otherwise
   compile and execute its complete finite change.

Bootstrap validates by the same rules, but has one narrower purpose: install
the plan, caller-supplied genesis seal, catalog, and physical snapshot in one
transaction. Its call direction, custody boundary, replay, and concurrency
law are fixed in `docs/run/bootstrap.md`.

A nonempty namespace without `@estate` is unsealed and is never adopted by
ordinary bind. `.adopt()` is the one explicit exception. Keel builds the
current Plan in an isolated backend projection, compares that complete shape
with the target namespace, and refuses any mismatch as drift. Drift leaves the
target namespace unmodified.

On exact agreement, one transaction creates the private catalog, stamps the
current manifest as generation one, restores generation, unit, bond, scoped
serial, and pulse high-water into `@clock`, and seals the resulting physical
shape. Restoration scans all rows, including ended facts. It never rewrites
business rows, generates a new genesis credential, or claims business-semantic
validity. Any catalog, scan, or seal failure rolls the whole adoption back.

Catalog corruption, unknown format, and physical drift are not migration
requests. Bind never applies opportunistic DDL: a different valid requested
manifest proceeds only through the generated plan and its checks.

## Plan

A valid changed manifest produces a read-only `Change`; callers cannot author
one. It contains deterministic ordered `Step` records:

| Part | Values |
|------|--------|
| path | nominal unit, field, bond, or bond-field path |
| act | add, drop, cast, alter |
| check | none, empty, clear, unique, presence, values, authority |

The check is a finite estate predicate that must succeed before candidate
allocation. Failure is `blocked` with its exact path and check. A denied cast,
serial transition, bond-family transition, or target change is recognized but
unlegislated and refuses by path. Neither case degrades into raw SQL or best
effort.

The initial scalar cast graph is:

| From → to | Rule |
|-----------|------|
| Text → Int/Bool | guarded canonical values |
| Int → Bool | guarded `0` or `1` |
| Link/Int/Bool → Text | total canonical rendering |
| Bool → Int | total |
| Text → Link | denied until canonical Link semantics land |
| every other unequal pair | denied |

Identity copies are not steps. Casts never chain through an intermediate
kind.

Scalar presence is part of the manifest. Adding an optional field is
unconditional and copies null into existing live rows. Adding a required
field requires an empty unit. Required-to-optional is unconditional;
optional-to-required requires the finite `presence` check. Null survives
otherwise legal casts and is excluded from uniqueness conflicts.

Each scalar field may also carry a closed rule: a constant default, a finite
admitted-value set, and inclusive integer bounds. Defaults belong to required
fields; optional omission always means null. Adding a required field or
tightening presence can fill the declared default without an empty/presence
check. Changing only a default affects future puts. Widening an admitted
domain is unconditional; narrowing compiles a `values` check over every live
fact before the candidate exists. The candidate projection repeats these laws
as backend constraints, so the manifest, normal writes, copied facts, and
physical estate cannot disagree.

## Allocation

All identity allocation is a named `@clock` transition inside the calling
write transaction. Unit keys use the unit full key; tie keys use the bond
path; serials use the field path and canonical scope value; pulse and
generation sequences have estate-wide names.

Business tables never infer identity with `max`, row count, backend
autoincrement, or surviving history. Rolling back a write rolls back its
clock transition. Committed gaps are legal, but a value is never reused.
Physical generation cleanup cannot lower or remove a clock.

## Evolution

Evolution is a closed-world compiler from the active canonical manifest to
the requested canonical manifest. It builds and validates an out-of-place
candidate, then atomically activates it. The old generation enters
Keel-owned cleanup. No candidate allocation or estate scan begins until the
entire delta has a known deterministic plan.

The hot path is generation-free. Active business tables always occupy their
stable projected names. Candidate tables use `@g<id>:<table>`; activation
renames the active set into its cleanup generation and the candidate set into
the stable names in one transaction. Engine tables remain estate-wide.

Candidate DDL, live-world copy, checks, name exchange, pulse contraction,
generation state, active pointer, and physical shape seal commit together.
Any error or cancellation rolls back the candidate and its generation clock.

Only the live world crosses generations. Ended facts remain with the
superseded generation. Resource, tie, serial, and pulse high-water marks
remain estate-wide and are never released by physical cleanup.

Business transformations use normal resource ceremony around a schema
change. Private storage-format upgrades remain Keel-owned. Neither gains a
raw SQL or arbitrary callback escape hatch.

## Cleanup

Bind first verifies the sealed physical estate, completes any requested
evolution, and then evaluates cleanup generations against:

```toml
[estate.generation.cleanup]
retain = "30d"
```

`retain` measures age from the generation retirement clock. It accepts an
unsigned count followed by `s`, `m`, `h`, or `d`. `"0s"` makes a retired
generation immediately eligible; `"forever"` explicitly disables automatic
collection. The default is `"30d"`. The environment path is
`KEEL_ESTATE_GENERATION_CLEANUP_RETAIN`.

For every eligible generation, Keel derives the physical `@g<id>:` tables only
from its canonical manifest. It drops those tables, removes the generation
record, recomputes the physical shape, and updates the estate seal in one
transaction. Any failure rolls that cleanup transaction back. Active and
candidate generations are never eligible, and cleanup never changes permanent
clocks.

Before dropping tables, Keel compares the retiring generation with the current
active world by nominal path and permanent key. A row or tie absent from active
becomes a unit or bond purge atom. A removed field, point, or bond field becomes
a path atom only for facts still present in active. Copied live facts and
unchanged paths produce nothing. These atoms enter `@derivative` in the same
transaction as physical cleanup.

## Hook

`.hook(handler)` registers one typed non-estate derivative consumer for that
bind. After estate GC commits, Keel delivers a `Purge` per retired generation.
The event contains the generation as its idempotency key and sorted `Gone`
atoms containing only canonical path and permanent key. It exposes no physical
table, backend, SQL, or removed payload.

Delivery is at-least-once. Success deletes that generation's outbox atoms.
Failure is reported, leaves them durable, and neither rolls back estate GC nor
refuses startup; a later hooked bind retries them even when no new generation
is collected. A crash after external success but before acknowledgement can
repeat an event, so handlers must be idempotent. Without a registered hook,
pending atoms remain untouched.

## Pulse

Pulse is a bounded operational stream, not retained business state.
Activation copies only events whose unit path exists in the candidate
manifest. Contraction advances the stream floor past omitted events; a
consumer behind that floor receives the existing cursor-past-window
refusal. Removed unit descriptors never leak back into the current caller
model.

## Refusals

| Refusal | Meaning |
|---------|---------|
| vacant | an empty namespace requires explicit bootstrap |
| token | a proposed genesis possession has noncanonical shape |
| occupied | bootstrap found an estate it cannot exactly replay |
| unsealed | a nonempty namespace has no Keel catalog |
| unknown | the catalog or manifest is not canonical and recognized |
| format | the private Keel storage format differs |
| drift | physical namespace differs from the sealed snapshot |
| denied | valid known delta has no legislated deterministic plan |
| blocked | a generated finite estate check failed |

Refusals are diagnostic facts. They never trigger repair, adoption,
fallback DDL, or best-effort continuation.

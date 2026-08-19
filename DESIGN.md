# Design

Keel closes the resource-management domain while leaving product identity and
credential policy outside. This boundary lets business callers declare one
model and receive consistent storage, HTTP, authority, transaction, event,
cache, and evolution behavior without re-authoring engine rules.

## Declarative closure

Business code declares resources, fields, relations, and schema rules only.
Engine keys, reign timestamps, estate catalog, authority units, generations,
clocks, and pulses are not business fields. An adapter projects the engine; it
does not become a second modeling API.

Keel accepts runtime values but owns no runtime configuration source. The
caller opens stores, constructs estate and cleanup policy, chooses network
addresses, installs middleware, and keeps possessions. This makes deployment
policy visible at the composition root instead of hiding it inside a library.

Every model shape is nominal and finite. Unknown fields, ambiguous references,
duplicate declarations, cycles, unknown enum values, and unlegislated changes
refuse before side effects. There is no callback, raw SQL, dynamic model, or
best-effort escape hatch through the closure.

## Identity and relations

Resource declaration is flat while identity is a tree. A root relation gives a
unit one containment segment; its full key is `{root}:{name}` and an unrooted
unit keeps its bare name. Keys are derived, lowercased, and never written by the
author. A need for deeper identity is a modeling error rather than permission
to grow a compound name.

Persisted and compiled references always use full keys. A short name is only a
fallible human-input resolution and is refused when ambiguous. Relation target
syntax names Keel resources, not Rust module paths. `:` denotes containment and
`.` denotes possession, so a bag such as `repo:label.tags` remains readable
without schema metadata.

Relations are forward declarations; Keel never invents reverse edges. A
many-to-many association is a bond and may carry business fields belonging to
the pair. It stays a bond while it is a single live pair with no independent
lifecycle. Promote it to a resource when it needs its own edges, workflow,
multiple live occurrences for one pair, or root-level query/order/page. The two
forms never coexist as shadows.

Bond reads remain flat ties. New and updated ties require live endpoints. A
resource may end while the ties it declared are still live, because a bond has
no lifecycle of its own and a row does not answer to the relations it holds; a
resource still cannot end while a live tie points at it from elsewhere.

Containment is the exception, and it is not one: a root relation names the
container a row belongs to, and its identity is derived from that container, so
a contained row cannot outlive it. Ending a row ends the rows rooted in it,
deepest first and in the same transaction, and a lease propagates the same
instant rather than the fact of expiry. Containment is single-parent and
acyclic, so the set is a tree and there is no second reading of what ending a
container means. A caller that wants the contents to survive has declared the
wrong edge: containment is not the way to spell an ordinary reference. Live
pair uniqueness is enforced on the serialized write path; physical indexes are
defense in depth rather than the semantic contract.

A group principal is honored only while its own row is live. Membership carries
authority, so retiring the group revokes every grant that named it in the same
write, rather than leaving the caller to unpick members one at a time and pass
through a half-dissolved group on the way.

## Fields and lifecycle

Scalar fields are required unless declared optional. Optional absence is null,
not empty text. Required fields may declare constant defaults; finite values and
inclusive integer bounds are schema law and apply equally to writes, manifests,
physical projection, and estate evolution.

Uniqueness counts live facts only. Ending a row releases its business unique
key, while scoped serial values are permanent monotonic allocations and are
never reused or renumbered. Check and write occur inside one serialized
transaction; a conflict rejects the entire verb without exposing the competing
row's data.

A frozen resource is immutable from birth. Its full payload and point
relations arrive at put; later mutation and relation editing refuse, while end
and lease remain lifecycle operations. Estate evolution may reproject the same
frozen fact without pretending migration is a business write.

`end` may schedule a future instant. A leased row remains live until that
instant but accepts no new edges. Scheduling requires a currently live row, so
there is no resurrection. The lease write is the observable event; the eventual
time transition is silent and cache validity stops at its horizon.

## Query law

The query language has one root subject. Typed predicates, direct forward
links, root ordering, limit, cursor, and count compile into a Tree. Repeated or
nested links, duplicate bonds, and association order/page refuse rather than
produce ambiguous partial results.

Every query success is a Pack. Normal packs contain a root bag and one flat tie
bag per selected link. Link does not hydrate target resources; clients issue a
second root query for the right keys. Count stands alone and returns no bags.
Selected empty bags are present as empty arrays, and closure overflow is an
error rather than silent truncation.

The same tree and predicate grammar serves query, capability predicates,
subscription filters, digest identity, and cache keys. Keel does not grow a
second policy or filter language beside it.

## Authority law

Authentication ends at one injected `Operator` row id. Keel trusts that
in-process identity and never interprets its credential. The injection channel
cannot express sudo. Passwords, sessions, API keys, OIDC, recovery, and product
identity rules stay in caller space.

Authority is self-hosted in live `@grant` rows. Grant, revoke, and audit are
ordinary put, end, and query operations on that unit. The verb set is closed at
six: `see`, `put`, `set`, `end`, `tie`, and `cut`. No resource or deployment may
add a private verb; ceremonies compose this set.

One bootstrap possession grounds the graph. Keel mints it explicitly from OS
entropy only for a vacant estate, stores only its hash, and journals sudo use.
The caller must keep it before sealing the estate. Losing custody of an occupied
estate is a recovery failure, never permission to mint a replacement.

Row grants cover the row and its root-chain subtree. Predicate grants reuse the
query predicate grammar and fail closed when they no longer parse. Predicate
coverage may descend for `see`, but write predicates never expand down a
subtree. A crew bond can make one resource row a group principal. Grants combine
by union; there are no deny rows, overrides, or precedence rules.

`put` treats a predicate as a postcondition. `set` requires a predicate before
and after, because a predicate describes the row's content and a write must not
falsify it. Row and subtree coverage answers the state the write started in: a
holder may move a row out of their own reach, which is how a thing is handed
over, and afterwards they no longer hold it. Tie and cut require the verb on the
left chain plus visibility of the right.

Creating a row mints its creator full row coverage only when nothing the creator
already holds covers it. Coverage that repeats a live grant is not written, so
the authority ledger measures deliberate delegation and never data volume, and
coverage does not outlive the coverage that authorized it. Anonymous identity
birth remains the one unconditional mint and gives ownership to the newborn row.

Delegation is attenuation: an operator may create or revoke only a grant it
could have issued from its own live coverage. Wildcards remain genesis-only.
Unseen rows behave as absent and never disclose a 403; writes may disclose
refusal. The reverse question “who can see this row?” is not promised as a
simulator.

## Ceremony-only resources

Visibility is a resource boundary, never a field boundary. A field requiring
different visibility belongs to another rooted resource with its own grants.

A veiled resource remains fully governed by Faces but is absent from generic
HTTP and query projection. Credential, invitation, refresh, and similar rows
use this boundary so operators cannot author their own possessions through
generic resource routes. Veil limits the wire rather than the Face; explicit
logout, rotation, and recovery ceremonies still compose ordinary verbs.

The optional Gate package lives on this side of the seam. Its authority is
enumerable as grants, its service face never authors identities, and birth is
an explicit possession-grounded mini-genesis. Resolution primitives and stock
doors are separable so callers can replace ceremonies without retelling the
credential-to-operator contract.

## Transaction law

Every single write is a transaction containing capability checks, invariant
checks, allocation, mutation, pulse emission, and commit. Batch widens that one
commit point across an ordered list of deeds. Later deeds see earlier
uncommitted effects; any failure or cancellation removes every row, grant,
allocation, and pulse from the batch.

A batch carries writes only, runs under one face, and never changes operator
midstream. It returns created resource and tie keys in deed order. It is not a
workflow engine and cannot reference a generated key from an earlier deed in a
static HTTP payload.

The write path never reads cache state. Connection revival happens only
between operations; the operation that meets a dead connection fails and is
never silently replayed.

## Event and cache law

Pulse is a third projection of the write log, not a seventh verb. Events are
thin, totally ordered, and closed over committed verb, path, key, actor, and
time. They contain no business payload. Consumers hydrate current state through
their own visible query, so delivery cannot freeze stale data or bypass current
authority.

Events are post-commit observations. Consumers cannot veto or delay a write.
Delivery checks visibility at delivery time, is at least once, and refuses a
cursor older than the bounded window. Business event names and exactly-once
claims do not enter the engine.

The cache is semantically invisible. It stores only unfiltered engine-view
packs; operator coverage is applied after every read and grant state remains
fresh. Per-unit generations and lease horizons determine validity. Eviction may
discard any entry without effect and can never truncate a result or raise a
capacity error.

Cache validity is per engine instance. Because grant reads share that cache,
more than one instance over one store requires cache off until cross-instance
invalidation exists. Distribution is not implied by a replaceable store.

## Estate law

The estate manifest is the canonical logical model; the private catalog is
Keel's physical truth. Callers declare the desired current model and never
replay historical Rust types, SQL, callbacks, or migration chains. Nominal path
identity makes the same path an update and a changed path removal plus addition;
Keel never infers rename by similarity.

Bind is fail-closed. Empty namespaces require explicit bootstrap. Nonempty
uncatalogued namespaces require explicit exact adoption. Catalog corruption,
unknown format, physical drift, and unknown deltas are diagnostic refusals and
never trigger opportunistic repair.

A changed valid manifest compiles completely before allocation. Each generated
step has a nominal path, a closed structural act, and any finite data check.
Denied conversions and failed checks report the exact path. Casts use one
legislated edge and never chain through convenient intermediates.

Evolution builds a complete out-of-place candidate, copies only the live world,
and atomically exchanges stable names. The old physical generation remains for
retention cleanup; permanent resource, tie, serial, pulse, and generation clocks
survive every generation. Storage-format upgrades and business transformations
remain separate kinds of change.

Cleanup retention is caller-supplied and may be a duration or `forever`.
Eligible generations are removed in isolated transactions after shape
verification. Logical derivative atoms describe only canonical path and
permanent key; typed hooks receive them after commit with at-least-once retry
and must be idempotent.

## Verification boundary

The workspace's unit and in-process scenario suites are the conformance kit.
They hold model compilation, lifecycle, query packs, authority, gate ceremonies,
pulse, cache parity, estate evolution, adoption, cleanup, hooks, SQLite, and
Postgres feature compilation. Keel ships no binary, so the contract is library
and Router behavior rather than process boot.

Store replacement is valid only when the same scenarios cannot distinguish its
observable behavior. Target hydration, reverse edges, association GET, nested
document responses, cross-instance cache coherence, dynamic model loading, and
public bind defaults are outside the current product surface.

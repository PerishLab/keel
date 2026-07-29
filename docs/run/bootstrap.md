# Bootstrap law

Bootstrap is Keel's explicit startup operation surface. It closes estate
genesis without choosing where a possession is delivered or kept.

## Direction

Keel provides two hotspots:

- `Bootstrap::status` distinguishes a verified occupied estate from vacancy.
- `Bootstrap::mint` admits vacancy and obtains one canonical sudo possession
  from OS entropy.
- `Bootstrap::seal` transactionally installs that possession's hash with the
  first estate.

Keel never calls a file writer, webhook, Kubernetes API, vault, environment
convention, or secret provider. The caller chooses custody and calls the
hotspots in this order:

```text
open store and bootstrap
  -> load a previously kept sudo or mint one
  -> durably keep the sudo
  -> seal the estate with that same sudo
  -> continue caller-owned bootstrap through the returned Core
```

Custody failure leaves the store untouched. Seal failure rolls back the
estate and leaves the already kept possession available for retry. Keel never
prints or otherwise exports the possession.

## Surfaces

Ordinary `bind` validates the graph and attaches or evolves a catalogued
estate. It never installs an empty estate. A vacant namespace is a bootstrap
refusal, not an invitation to perform hidden first writes.

`bootstrap(graph, wire)` owns the unopened ceremony. `status` has no store
effect and verifies occupied catalog integrity. `mint` has no store effect and
refuses unless status is vacant. `seal` accepts only the canonical sudo token
shape and returns `Core` only after a transaction or exact replay.

Bootstrap is not a resource, identity, administrator, HTTP route, or
persistent mode. Once it returns `Core`, every subsequent resource effect
uses the ordinary Keel faces.

## State

| Store | Operation | Result |
|-------|-----------|--------|
| empty | status | `vacant`; no writes |
| empty | bind | `vacant`; no writes |
| empty | seal, malformed sudo | `token`; no writes |
| empty | seal, canonical sudo | one transactional estate |
| exact estate | bind | ordinary attach |
| exact estate | status | `occupied`; no writes |
| changed valid graph | bind | ordinary estate evolution |
| exact estate, same sudo | seal | idempotent attach; no writes |
| exact estate, other sudo | seal | `occupied`; no writes |
| other manifest | seal | `occupied`; no evolution |
| nonempty, no catalog | either | `unsealed`; no writes |
| corrupt or drifted | either | existing exact refusal |

An exact replay requires the requested manifest, physical snapshot, catalog,
and sudo hash all to agree. Bootstrap never repairs, adopts, evolves, rotates,
or replaces an occupied estate.

When custody has no sudo, the caller asks `status` before `mint`: occupied
means custody was lost and generation is forbidden. This prevents a fresh,
incorrect artifact from replacing the only recovery path to an existing
estate.

## Failure

The empty check and first install occur inside one backend transaction. At
most one concurrent seal establishes an estate. A contender using the same
possession may retry and observe the exact replay; a contender using another
possession observes `occupied`. No loser overwrites the winner.

A process crash before custody leaves no store write. A crash after custody
but before commit reuses the kept possession. A crash after commit also
reuses it and takes the exact replay path. These are the only supported
recovery edges; generating a new possession on blind retry is caller error.

## Ownership

| Owner | Closed responsibility |
|-------|-----------------------|
| Keel | graph validation, entropy, token shape, transaction, catalog, seal hash, replay |
| Ensign bootstrap | ceremony order, service Actor/grants, identity prerequisites |
| deployment caller | file/webhook/Secret/vault delivery, retention, access, recovery |

The sudo possession and an Ensign OIDC signing key may share orchestration
syntax as named artifacts. They remain separate contracts: Keel owns sudo
meaning and Ensign owns signing meaning; neither gains access to the other's
secret merely because one caller provisions both.

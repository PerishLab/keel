# Transaction law

Settled law for the write path's commit point. Implementation may lag;
behavior that lands must not violate this document.

Every write already runs on a serialized path (single writer). This law
names the boundary that was always implicit: one **commit point** per
write, and a way to widen it across several verbs — `batch`.

## Commit point

- Each single verb (`put` / `set` / `end` / `tie` / `cut` / `set_tie`) is
  its own commit: it opens, checks, writes, emits its pulse, and commits —
  observably atomic, exactly as before. No signature changes.
- A **batch** widens the boundary: a list of verbs commits as one. All land
  or none do. Capability checks, mint, unique/serial checks, and pulse
  emission for every verb happen inside the one transaction.
- The write path never reads the cache (`docs/cache.md` H-5) — inside a
  batch too.

## Batch

```text
batch([Deed…]) -> ids   -- all-or-nothing; writes only; no reads returned
```

- **Intra-batch visibility**: verb N sees the uncommitted effects of verbs
  1..N-1. A team rooted at an org created earlier in the same batch passes
  its capability and ref-liveness checks. This is the reason batch exists.
- **All-or-nothing**: any failure (capability, unique, missing ref, engine)
  rolls back the whole transaction. No partial rows, no minted grants, no
  emitted pulses survive a rolled-back batch.
- **Writes only**: a batch carries verbs, not queries. It returns the ids
  of its `put`/`tie` deeds in order.
- **Operator-scoped**: a batch runs under one face; every verb is checked
  under that operator (or sudo). No verb escapes coverage.
- Pulse emission is part of the transaction: a rolled-back batch emits
  nothing; a committed batch emits every verb's event in order. This closes
  the write-then-emit atomicity gap that single verbs still carry as debt
  (see § single-verb note).

## Serialization (v0.2.0)

The async engine makes the concurrency contract explicit:

- The engine serializes all SQL on **one connection**; each face op
  acquires it once at entry and holds it for the op's whole duration.
- Every write op is one transaction — a single verb is a batch of one.
  The old check-then-insert race (unique/serial probes vs insert under
  concurrent handlers) is closed structurally: no other SQL can land
  between a write op's checks and its commit.
- No other SQL can interleave inside an open transaction — a
  transaction owns the connection BEGIN→COMMIT, so uncommitted state is
  never observable outside its own op.
- **Cancellation**: a future dropped mid-transaction (client
  disconnect) leaves the connection marked dirty; the next op issues
  ROLLBACK before proceeding. A cancelled write is a rolled-back write.

## Mechanism (default engine, non-normative)

One `tokio::Mutex` guards the connection. A batch opens one
transaction, threads the same `&mut` connection through every verb and
every capability read, then commits or rolls back once. Capability
reads inside a batch see uncommitted rows because they share the
connection — there is no second lock to reenter. A failed or cancelled
transaction also drops the query cache (uncommitted packs must not
survive a rollback).

## Single-verb note

Resolved in v0.2.0: single verbs run inside their own transaction, so
the write-then-emit sequence commits atomically — the crash-between-
statements event-loss debt is closed.

## Settled package

| Id | Choice |
|----|--------|
| X-1 | One commit point per verb; single-verb behavior unchanged |
| X-2 | `batch` widens the boundary across verbs; all-or-nothing |
| X-3 | Intra-batch visibility: verb N sees verbs 1..N-1 uncommitted |
| X-4 | Batch carries write verbs only; returns put/tie ids in order |
| X-5 | Operator-scoped: every verb checked under the one face |
| X-6 | Pulse emission is inside the transaction; rollback emits nothing |
| X-7 | One connection, op-scoped hold; every write op is one transaction |
| X-8 | Cancellation-safe: a dropped mid-txn op rolls back before the next op runs |

## Must not

- A batch that commits partially.
- Reads or queries inside a batch.
- A verb inside a batch escaping its face's coverage.
- Cache reads on the write path, batch included.
- Cross-face batches (one operator per batch).

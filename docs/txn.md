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

## Mechanism (default engine, non-normative)

The store gains a transaction-scoped connection view. Single verbs open and
commit their own transaction (behavior identical to today). A batch opens
one transaction, threads the same connection through every verb and every
capability read, then commits or rolls back once. Capability reads inside a
batch see uncommitted rows because they share the connection — there is no
second lock to reenter.

## Single-verb note

Single verbs keep their current shape. Their write-then-emit sequence is
two statements on one connection; a crash between them can still lose an
event (recorded debt). Wrapping every single verb in an explicit
transaction to close that gap is a permitted engine improvement, not a law
change — the observable contract is already "atomic per verb".

## Settled package

| Id | Choice |
|----|--------|
| X-1 | One commit point per verb; single-verb behavior unchanged |
| X-2 | `batch` widens the boundary across verbs; all-or-nothing |
| X-3 | Intra-batch visibility: verb N sees verbs 1..N-1 uncommitted |
| X-4 | Batch carries write verbs only; returns put/tie ids in order |
| X-5 | Operator-scoped: every verb checked under the one face |
| X-6 | Pulse emission is inside the transaction; rollback emits nothing |

## Must not

- A batch that commits partially.
- Reads or queries inside a batch.
- A verb inside a batch escaping its face's coverage.
- Cache reads on the write path, batch included.
- Cross-face batches (one operator per batch).

# Unique law

Settled storage contract for engine-enforced write invariants. Implementation
may lag; behavior that lands must not violate this document.

One principle covers the family: **all write verbs execute on a serialized
write path**. The law fixes observable semantics; the mechanism (single
connection, partial index, transaction shape) belongs to the engine and may
differ per store, provided the scenario suite cannot tell.

## The family

| Id | Invariant |
|----|-----------|
| U1 | at most one **live** tie per `(left, right)` per bond; second rejected |
| U2 | `unique` field: at most one live row per value, engine-wide |
| U3 | `unique = <relation>` field: at most one live row per value **within the parent row** |
| U4 | composite unique: one live row per value tuple (grammar open, see S1) |
| U5 | scoped serial: per-parent monotonic allocation at `put`; never reused |

## Liveness rule

Uniqueness counts **live rows only**. An `end`ed row releases its key; a new
live row may take it. History keeps the old row untouched — uniqueness is a
statement about the present, never about the past.

Serials are the one exception: an ended row keeps its serial and the number
is never reallocated. Gaps are legal and meaningless.

## Write-path semantics

- Check and write are **one atomic step** on the serialized path. No caller
  can observe the window between them.
- Violations reject the whole verb: no partial writes, no renumbering, no
  silent retry that changes outcome.
- HTTP maps unique conflicts to **409 Conflict**; the body names the field
  or bond, never the conflicting row's content (that would leak past `see`).
- Serial allocation happens inside the same step; a rejected `put` allocates
  nothing.

## Mechanism notes (default engine, non-normative)

- SQLite single-connection writes are the base serialization.
- A partial unique index (`WHERE` live) is a legal mechanism and a welcome
  second lock; plain DDL `UNIQUE` is not (it would block key reuse after
  `end`, violating the liveness rule). Leased rows (`docs/lease.md`) escape
  the index; the serialized engine check is the contract (L5).
- Check-then-insert is legal **only** inside the serialized step.

## Conformance

A replacement engine must pass the scenario suite's contention acts when
they exist (`docs/verify.md`); until then, the sequential acts plus this
document are the contract.

## Must not

- Plain DDL `UNIQUE` on business fields (blocks reuse after `end`).
- Uniqueness across dead rows, or checks that count ended rows.
- Serial reuse, renumbering, or per-engine serial semantics.
- Caller-observable check/write windows.
- Conflict errors that reveal row content beyond the key name.
- A second uniqueness vocabulary outside `unique` / `unique = rel` /
  composite / serial.

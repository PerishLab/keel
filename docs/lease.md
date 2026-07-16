# Lease law

Settled law completing the `end` verb with a scheduled instant. Implementation
may lag; behavior that lands must not violate this document.

Time becomes a way to die without becoming a new verb: `end` was always the
engine verb that writes `expires_at`; it now takes an optional instant.

```text
end(key)      -- die now (unchanged)
end(key, at)  -- die at `at`; at >= now
```

The verb set stays closed at six. A scheduled end is the **same verb** —
capability coverage, HTTP mapping, and journal treatment are `end`'s.

## The five clauses

| Id | Law |
|----|-----|
| L1 | `end` takes an optional instant; `at >= now`; default is now |
| L2 | **No resurrection**: scheduling (and re-scheduling) requires a live row; a dead row stays dead |
| L3 | K3 checks at **schedule time**; a leased row (live, `expires_at` set) accepts **no new edges** — neither ties nor incoming refs |
| L4 | Death is **pre-observed**: the lease write is the event; the transition at `at` is silent but fully determined by it. Caches bound entry validity by the nearest lease horizon (`docs/trigger.md`) |
| L5 | The unique second lock covers NULL-live rows only; the serialized engine check remains the contract (`docs/unique.md`) |

## Semantics

- Renewal = `end(key, later)` while live. Moving the instant earlier is
  equally legal; `end(key, now)` is revocation.
- A leased row is **live** until its instant: it appears in `live` slices,
  holds its unique keys, and satisfies existing edges. Only *new* edges are
  refused (L3) — the row is dying, nothing may come to depend on it.
- L3 with schedule-time K3 keeps the invariant sound end to end: no live
  edges at scheduling, no new edges after, hence no orphan edges at death.
- Reign stays engine-owned: no business field ever carries row death.
  Workflow states (`closed`, `archived`) remain business fields (F13);
  lease is for rows whose existence itself is temporal (sessions,
  reservations, offers).

## Open

- HTTP projection of the instant (DELETE with a body? PATCH seat?) — until
  settled, lease is a Core/Face surface only.
- Tie-lease propagation (edges inheriting `min` of endpoint leases) — a
  later relaxation of L3's strictness.

## Must not

- A resurrection path, under any name.
- New edges to a leased row, including many2one refs.
- Business fields that encode row death (that is reign's seat).
- A seventh verb; lease rides `end` or does not exist.
- Silent unique-lock claims over leased rows (L5 honesty).

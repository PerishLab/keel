# Cache law

Settled law for the engine-integrated cache. Implementation may lag;
behavior that lands must not violate this document.

One sentence governs everything: **the cache is semantically invisible** —
apart from latency, no observable behavior may differ with the cache on or
off. Correctness is not a knob, so there is no control plane: no TTLs, no
invalidation API, no cache headers, no business surface at all.

## Seat

`keel.toml`:

```toml
[cache]
kind = "memory"
# kind = "none"
```

Default is `memory` (invisibility makes on the honest default); `none`
exists for conformance diffing and debugging. Capacity is an engine
constant (C0 seat).

## Placement: under coverage

Cached entries are **engine-view packs** keyed by query `digest`. Operator
coverage (`see` filtering) is applied **after** every cache read and is
never cached. Consequences, by construction:

- No cross-operator leak is possible: no filtered pack is ever stored.
- Revocation is immediate: grant reads stay fresh on every read.
- One entry serves every operator; the coverage pass is the only
  per-operator cost.

## Validity: generations and horizons

Every write verb bumps a per-unit **generation** at the same choke point
that emits the pulse (`beat`); bond writes bump the owning unit. An entry
records the generations of every unit its tree involves (root, link
targets, `has`/`some` targets) at fill time and is valid only while all
match — invalidation is exact, not timed.

Leases are the one time-driven death (`docs/lease.md` L4): an entry also
records the nearest `expires_at` horizon among the rows and ties it holds,
and dies at that instant. A leased row can therefore never be served past
its death.

## Write path

The write path never reads the cache. Unique checks, serial allocation,
ref liveness, coverage checks — all read engine-fresh state, always.

## Eviction is not truncation

C0 forbids silently truncated *results*; eviction only discards *entries*
(they recompute). Evicting is always legal; the engine may clear the whole
cache at any time without observable effect. Capacity overflow evicts, it
never errors.

## Conformance

The scenario suite must pass byte-equal with `kind = "memory"` and
`kind = "none"`. Any observable divergence is a fault in the cache, never
acceptable behavior. Validity is scoped to one engine instance (single
writer); multi-instance coherence is parked with distribution.

## Settled package

| Id | Choice |
|----|--------|
| H-1 | Semantically invisible; no control plane, no TTL, no invalidation API |
| H-2 | Placement under coverage: engine-view packs only; filtering never cached |
| H-3 | Key = digest; validity = per-unit generation vector, bumped at `beat` |
| H-4 | Lease horizon bounds entry validity (L4) |
| H-5 | Write path never reads cache |
| H-6 | Eviction invisible and always legal; capacity is an engine constant |
| H-7 | Conformance = scenario suite byte-equal on/off; per-instance validity |

## Open

| Id | Question |
|----|----------|
| H-M1 | Generation granularity below unit level (row/pred) if profiling demands |
| H-M2 | Eviction policy beyond clear-all (LRU seat) |

## Must not

- Any caller-facing cache control: headers, TTLs, flush endpoints, hints.
- Caching coverage-filtered packs, or serving one operator's filter to
  another.
- Write-path reads through the cache, under any name.
- Serving a leased row past its horizon.
- Timed expiry as a correctness mechanism (horizons are lease facts, not
  apologies).
- Errors on capacity (evict instead).

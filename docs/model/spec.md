# Stage law

Settled delivery contract: what each stage of keel ships and what proves it
shipped. Stages are topological, not calendar — a stage is done when its
gate is green, never when a date arrives. Laws may still be amended when
implementation finds friction; behavior that lands must not violate the
laws in force, and no stage begins against unlanded law.

## Delivery grammar

Every stage delivers the same triple:

| Part | Meaning |
|------|---------|
| laws | documents settled or amended in `docs/` |
| surface | engine / package capability that lands in code |
| scenario | a new or extended L2 act proving the surface |

Acceptance is exactly `runseal :guard` green including the stage's
scenario. The scenario suite is the conformance kit (`docs/run/verify.md`);
each stage's act is the executable form of its promise.

**Flagship scenario `:forge`**: a thin Forgejo slice (Actor / Repo / Issue,
then authority, then credentials, then webhooks) growing one act per
stage. Its final form is the executable proof that keel absorbs a real
system's state and authority plane.

Micro-seats listed as *open inside* settle during their stage by the
dream-code method: write the ideal caller line first, legislate the
friction, then implement.

## Stages

### S0 — legislation closeout

| Part | Content |
|------|---------|
| laws | U1 storage contract (one txn family: live-pair unique, scoped unique, scoped serial); bond→unit promotion law (Review / Reaction exemplars) |
| surface | none |
| scenario | none (docs only; guard green) |

### S1 — data completeness

| Part | Content |
|------|---------|
| laws | vocabulary delta: relation kinds spelled `many2many` / `many2one` / `one2one` (+ `opt`); `one2many` recorded as deliberately absent |
| surface | atoms `int` / `bool`; the three relation kinds; global / scoped unique; scoped serial; `count` terminal (composite unique settles at need, first act that models a reaction-shaped unit) |
| scenario | `:forge` act 1 — the slice declares; repo name unique per owner; issue index per repo; star count |

Open inside: serial txn mechanics (under U1 law), text-match pred if the
act needs search.

### S2 — capability

| Part | Content |
|------|---------|
| laws | `docs/run/capability.md` in force; amendments as friction demands |
| surface | `of(operator)` / `sudo()` faces; `@grant` unit; root chain + subtree coverage; uniform six-verb check; mint; attenuation; `@me`; `anon` / `all`; genesis token ceremony; sudo journal; 404 semantics |
| scenario | `:forge` act 2 — six-row seed policy runs; authority matrix (author edits own issue, stranger reads 404, writer 403 outside coverage, transfer moves the subtree) |

Open inside: C-M1 membership grammar, C-M2 chain designation grammar,
C-9 `batch`, C-P injection cost.

### S3 — gate

| Part | Content |
|------|---------|
| laws | capability.md § gate in force |
| surface | default credential package: `Token` / `Session` / `Pass` units + projection middleware; in-process operator injection |
| scenario | `:forge` act 3 — register / login / api-key / logout / password change / revoke over HTTP; gate authority enumerable as its grant rows; registration grounds as mini-genesis |

Open inside: session expiry riding reign lifecycle.

### S4 — trigger + relay

| Part | Content |
|------|---------|
| laws | `docs/run/trigger.md` in force |
| surface | ordered post-commit stream with cursor; coverage-checked delivery; `relay` package (`Hook` unit + consumer) |
| scenario | webhook act — delivery bound by `see`; cursor past window errors; `@grant` change events observable |

Open inside: T-M1 window seat, T-M2 naming, T-M3 filter designation,
T-M4 end-event hydration.

### S5 — cache

| Part | Content |
|------|---------|
| laws | cache law (to draft at stage entry): semantically invisible; key = digest × coverage × generations; write path never reads cache; eviction ≠ truncation |
| surface | engine-integrated cache; caller-supplied switch at bind |
| scenario | no new act — **every** prior scenario byte-equal with cache on and off |

Open inside: generation granularity, eviction policy.

### S6 — estate

Estate evolution begins under `docs/run/estate.md`.

| Part | Content |
|------|---------|
| laws | nominal manifest identity; exact bind; closed delta compiler; out-of-place activation; Keel-owned cleanup |
| surface | canonical manifest, estate/generation/clock/derivative catalog, permanent allocators, physical drift seal, generated evolution plan, configured cleanup retention, typed purge hook |
| scenario | estate act — fresh seal, exact reopen, drift refusal, additive evolution, contraction, cleanup |

Open inside: the finite validator and cast matrices.

## Sequencing

The chain is forced, not chosen: root chain needs `many2one` (S1→S2); gate
needs mint and faces (S2→S3); relay needs the stream and a service
operator (S2+S4); the cache key needs coverage fingerprints (S2→S5). Small
independent seats (text-match, order-by-aggregate) may land in any stage
after S1 when a scenario needs them — never before.

## Must not

- Begin a stage against unlanded law.
- Advance past a red or skipped scenario.
- Reference unimplemented law from landed code across a stage boundary.
- Calendar commitments anywhere in this document.
- Distribution work in any stage (parked by decision).
- Grow `:forge` outside the stage that owns the act.
- Change stage boundaries except by an explicit amendment commit.

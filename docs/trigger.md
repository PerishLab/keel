# Trigger law

Settled product law for the engine's third projection. Implementation may
lag; behavior that lands must not violate this document.

The engine surface projects three ways: **store** (state), **HTTP**
(synchronous request/response), **trigger** (asynchronous post-commit
broadcast). Trigger is an adaptor, not a verb: it adds no write surface and
no engine behavior — it is the public face of the write log.

## Event shape (closed, thin)

An event is a verb landing on a row. The schema is closed by verb closure
and never grows with business models:

```text
event
  seq       -- total order, monotonic
  verb      -- put | set | end | tie | cut
  unit      -- unit name (bond events carry the bond path)
  id        -- row or tie id
  operator  -- acting operator row id (sudo marked as sudo)
  at        -- commit time
```

- **No business event types, ever.** "Issue closed" is not an event kind;
  it is a `set` event whose consumer reads the row.
- **Thin events**: no field data, no row payload. Consumers hydrate through
  an ordinary query under their own operator — the H0 instinct applied to
  time. Fat payloads would freeze stale data into deliveries and dodge the
  read check.
- `see` never emits events (reads are not occurrences).

## Post-commit, observe-only

Triggers observe; they never participate:

- Emission is strictly **after commit**. No consumer can veto, delay, or
  mutate a write. Refusal is capability's job, before the write.
- The engine never invokes business logic in the write path. This is the
  explicit immunity to the SQL-trigger / ORM-callback disease: hidden
  control flow inside writes.
- A slow or dead consumer affects delivery lag only — never write latency.

## Trigger is a read surface

Coverage law (`docs/capability.md`) applies in full: an event about row X
is delivered to an operator's subscription only if that operator can `see`
X — checked **at delivery time**, consistent with revocation immediacy. No
event escapes coverage; a broadcast channel that skipped this check would
be the universal authority bypass.

Corollary: `@grant` writes are ordinary put/end events on `@grant` —
permission-change subscriptions exist for free, governed like any other,
visible only to operators who can see those grant rows.

## Engine primitive vs caller space

The engine owns exactly one primitive: an **ordered stream with a cursor**.
Consumers pull from a position; the engine retains a bounded replay window;
a cursor older than the window is an **error, not a silent skip** (C0
spirit).

Everything else is caller space. keel may ship **`relay`**, the default
webhook package, on the gate pattern (`docs/capability.md` § gate):
recursive resource definitions (a `Hook` unit — url, events, active) plus
one trigger consumer that delivers matching events outward. Caller space,
enumerable grant authority, replace or omit freely.

## Delivery semantics

- **At-least-once**, `seq` as the idempotency key; consumers are expected
  idempotent. Exactly-once is not promised — no adaptor may claim it.
- Order within the stream is total (`seq`); delivery order across retries
  is not re-promised.
- Subscription filtering reuses the query pred grammar — no second filter
  language.

## Unification note

The write log has one spine and multiple consumers: trigger delivery,
engine cache invalidation (internal), and — archived, not planned —
replication would be a third. One investment, several payoffs; none of
them changes modeling.

## Settled package

| Id | Choice |
|----|--------|
| T-1 | Trigger is the third projection: adaptor, not verb |
| T-2 | Closed thin event shape; no business event types; no payloads |
| T-3 | Post-commit observe-only; consumers never veto or delay writes |
| T-4 | Read surface: coverage checked at delivery time |
| T-5 | Total order via `seq`; single-writer makes it free |
| T-6 | `relay` default webhook package in caller space (gate pattern) |
| T-7 | At-least-once + cursor pull; window overflow errors (C0 spirit) |

## Open

| Id | Question |
|----|----------|
| T-M1 | Retention / replay window size and seat (engine constant vs toml) |
| T-M2 | Adaptor module naming (single word) |
| T-M3 | Subscription filter designation grammar on the stream API |
| T-M4 | `end`-event hydration vs history-read semantics (open elsewhere) |

## Must not

- Synchronous triggers, or any consumer participation in the write path.
- Business event types or per-domain event schemas.
- Fat event payloads carrying row data.
- Delivery that bypasses coverage, including to `relay`.
- A second filter or subscription language beside the pred grammar.
- Exactly-once delivery claims.
- Silent cursor skips past the replay window.
- Events for reads.

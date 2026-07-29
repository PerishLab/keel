# Cold-start verification boundary

Contract: prove the closed loop under the frozen strategy. Passing means cold
start is done; failures mean cold start is not closed.

## Layers

| Layer | What | Gate |
|-------|------|------|
| L1 | unit tests (data/bond/query/cap/pulse/estate) | CI + `runseal :guard` |
| L2 | scenario tests (`keel` route, `keel-gate` forge) | CI + `runseal :guard` |
| L3 | static discipline (fmt/clippy/deno/ectropy) | `runseal :guard` |

L2 drives the axum `Router` in process. It holds the REST surface, the `/query`
DSL, the authority matrix, the gate doors, and relay delivery against a real
local listener. It does not boot a separate process, so it does not cover cold
start of a shipped binary; keel ships no binary.

## Must pass (in)

- Business-only resource DSL; reign columns engine-owned
- Ordinary bind refuses an empty store without writes
- Explicit bootstrap custody-before-seal; transactional install and exact replay
- Malformed, conflicting, failed, and concurrent bootstrap paths fail closed
- Bootstrap status prevents mint against an occupied estate
- Gate rise is write-free; explicit seed and ready delimit package bootstrap
- Unsealed, corrupt, changed, and physically drifted estates fail closed
- Generated additive/contraction/cast evolution with atomic rollback
- Live fact, point relation, tie, key, and serial preservation across generations
- Configured retained-generation GC with immediate, forever, and atomic rollback cases
- Explicit exact-projection adoption with allocator restoration and atomic rollback
- Durable typed derivative hooks with logical diff, acknowledgement, and retry
- sqlite wire + put/live/end + tie/ties/cut on Core
- Optional scalar null, explicit unset, null query, and presence-guarded evolution
- Required constant defaults, finite values, integer bounds, and narrowing checks
- Frozen fact write refusal, lifecycle end, and resource-mode delta denial
- `live` == root rows of `query("from Unit")` pack; scalar `where` +
  stable multi-field order/page
- `where id` / `order by id` on engine key; `id in ("…")` for H0 follow-up loads
- `POST /query` always pack `{root,bags}`; optional `link` → bond bags (H0)
- REST: resource CRUD + edge write (tie/cut); **no** association GET
- guard green without docker daemon
- scenario coverage holds without a shipped binary

## Must not require (out)

- target hydrate from `link` (H0 only); nested GraphQL document responses
- association REST, reverse edges, reverse-query privilege
- order/page on non-root bags
- postgres execution in the default guard (feature compilation remains required)
- pagination/cache performance; keyset beyond id cursor
- end cascading ties, business unique policy
- dynamic model load, public bind on 0.0.0.0

## Commands

```sh
runseal :guard              # L1 + L2 + L3
cargo test -p keel --test route
cargo test -p keel-gate --test forge
```

## Pass rule

All L1 and L2 tests green, and this document matches behavior (no false claims
of filter/page/capability support).

# Cold-start verification boundary

Contract: prove the closed loop under the frozen strategy. Passing means cold
start is done; failures mean cold start is not closed.

## Layers

| Layer | What | Gate |
|-------|------|------|
| L1 | unit tests (data/bond/query/cap/pulse/estate) | CI + `runseal :guard` |
| L2 | process smoke + course scenario | `runseal :smoke` / `:course` (from `:guard`) |
| L3 | static discipline (fmt/clippy/deno/ectropy) | `runseal :guard` |

## Must pass (in)

- Business-only resource DSL; reign columns engine-owned
- Empty-store transactional seal; exact canonical reopen
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
runseal :guard    # L1 + L3 + L2 smoke + course
runseal :smoke    # thin L2
runseal :course   # student/course scenario L2
```

## Pass rule

All L1 tests green, `:smoke` green, and this document matches behavior (no
false claims of filter/page/capability support).

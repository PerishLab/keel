# Cold-start verification boundary

Contract: prove the closed loop under the frozen strategy. Passing means cold
start is done; failures mean cold start is not closed.

## Layers

| Layer | What | Gate |
|-------|------|------|
| L1 | unit tests (smoke/life/bond/miss/config/query) | CI + `runseal :guard` |
| L2 | process smoke (`keel-api` REST + `/query`) | `runseal :smoke` (also from `:guard`) |
| L3 | static discipline (fmt/clippy/deno/negentropy) | `runseal :guard` |

## Must pass (in)

- Business-only resource DSL; reign columns engine-owned
- sqlite wire + put/live/end + tie/ties/cut on Core
- `live` == root rows of `query("from Unit")` pack; scalar `where` + order/page
- `where id` / `order by id` on engine key; `id in ("…")` for H0 follow-up loads
- `POST /query` always pack `{root,bags}`; optional `link` → bond bags (H0)
- REST: health, list/create/get-one/delete; **no** association routes
- guard green without docker daemon

## Must not require (out)

- target hydrate from `link` (H0 only); nested GraphQL document responses
- association REST, reverse edges, reverse-query privilege
- order/page on non-root bags
- field validation, capability, identity, postgres
- pagination/cache performance; multi-field order; keyset beyond id cursor
- end cascading ties, business unique policy
- dynamic model load, public bind on 0.0.0.0

## Commands

```sh
runseal :guard    # L1 + L3 + L2
runseal :smoke    # L2 only
```

## Pass rule

All L1 tests green, `:smoke` green, and this document matches behavior (no
false claims of filter/page/capability support).

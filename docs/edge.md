# Edge navigation law

Settled product law for how query grows association reads. Implementation may
lag; behavior that lands must not violate this document.

## Return shape (flat bags)

Edge reads return a **pack**: named flat **bags**, not a nested document tree.

- One bag per unit kind involved (e.g. `Student`, `Class`).
- One bag per selected bond, named `Owner.bond` (e.g. `Student.classes`).
- Bond bags are honest **tie** projections (`id`, `left`, `right`, reign), not
  embedded target rows.
- Clients assemble trees from bags by id. The engine does not nest.

REST stays free of association routes. Edge **read** lives only in query DSL.
Edge **write** stays on Core (`tie` / `cut`).

## Single subject (root only order / page)

Only the **root** unit of a tree (the `from` unit) may take:

- `order by`
- `limit`
- `after`

Bond bags and target bags **must not** accept order/page in the DSL.

Rationale: `limit` / `after` need exactly one page subject. Nested or per-edge
page creates conflicting cursors and forces connection-shaped semantics that
contradict flat bags and 4NF n2m. That path is rejected for keel.

Root order defines the main bag sequence and thus `after`. Non-root bags use
**engine-fixed stable order** (implementation detail; not user-authored).

## Closure, not a second feed

When a tree **link**s a bond, non-root bags are the **live closure** of the
current root bag:

- Root empty ⇒ bond and target bags empty.
- Bond bag = live ties whose `left` is in the root id set (forward bond only).
- Target bag = live target rows referenced by those ties (when hydrate is on).

Closure is "complete for this root page", not "another independently paged
feed". Deep graphs use **another top-level query** (`from` the next unit), not
nested link chains in one tree.

## Depth and direction

- **Depth 1** per tree: root + direct bonds only. No `link` of a `link`.
- **Forward bonds only** as declared on the root unit. No reverse generation.
- Multiple `link`s on one root are allowed; each is depth-1 off the same root.

## Safety valve

If a closure would exceed an engine hard cap, the query **errors**. Silent
truncation of bond/target bags is forbidden (false completeness).

Caps are engine policy, not user `limit` on non-root bags.

## Not settled here (open until chosen)

- DSL keyword and exact pack JSON shape.
- Whether target unit rows hydrate by default or only when named.
- Numeric cap values and where they are configured.
- Edge-as-predicate (`where` over bonds) vs expand-only `link` (may coexist).

## Must not

- Nested GraphQL-style response trees as the primary edge delivery.
- Order/page on bond or target bags.
- Association REST routes.
- Engine-invented reverse edges.
- Silent partial closures.

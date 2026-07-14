# Bond attributes law

Settled: n2m associations may carry **business fields** on the join relation.
This is 4NF-correct: attributes of the pair, not an independent multivalued
fact. It is an engine expressiveness gap closed by design — not a fork against
normalization.

Enrollment-as-named-unit remains a **later promotion** when the association
must be a query root with its own lifecycle/workflow. Default path is bond
attrs on `#[relation(..., n2m)]`.

## Relational shape

```text
join table = owner_bond
  id              -- engine key (tie id)
  {owner}_id      -- left
  {target}_id     -- right
  <bond fields…>  -- business attrs (optional set may be empty)
  expires_at, created_at, updated_at
```

- Key of the association fact: live unique on `(left, right)` (see open U*).
- Reign stays engine-owned; never business fields.
- Pure n2m (zero bond fields) remains valid.

## Macro intent (normative direction)

Declare fields on the relation, not as a second resource:

```rust
// illustrative; exact attribute grammar is an open micro-decision
#[relation(Course, n2m)]
#[field(string)] // or relation-scoped field attrs — see open M*
grade: string,
courses: Course,
```

Open: precise macro surface (**M1** below). Semantic requirement is only:
bond name + target + kind + zero-or-more business slots.

## Wire / pack (H0 preserved)

`link courses` still yields bond bag only. Items become **fat ties**:

```json
{
  "id": 9,
  "left": 1,
  "right": 3,
  "grade": "A",
  "expires_at": null,
  "created_at": 0,
  "updated_at": 0
}
```

- Same bag key `{root_table}.{bond}`.
- Business keys are bond field names; never nested target rows (H0).
- Clients still hydrate targets via `from Course where id in (…)`.

## Write surface (direction)

| Path | Meaning |
|------|---------|
| `POST /{unit}/{id}/{bond}` | `tie` — body `{ "right", …attrs }` |
| partial update on live tie | Core `set` on tie (name TBD) + HTTP PATCH on tie path |
| `DELETE …/{bond}/{tie}` | `cut` (unchanged) |

Empty attrs on a field-less bond: body may be only `{ "right" }`.

## Settled with this choice

| Topic | Choice |
|-------|--------|
| D1 association data | **Bond attrs** on n2m join (not Enrollment unit first) |
| 4NF | Satisfied: attrs of the pair on the association relation |
| Pack / H0 / root-only page | Unchanged |
| No auto reverse | Unchanged |
| No association GET | Unchanged |

## Remaining decision blockers

Re-scoped after choosing bond attrs. Only these need a product call before
implementation freezes.

### M1 — Macro grammar

How do business slots attach to a relation in the DSL?

| Option | Sketch |
|--------|--------|
| **M1a** | Field-like items before the relation field, attributed to next `#[relation]` |
| **M1b** | Nested: `#[relation(Course, n2m, fields(grade = string))]` |
| **M1c** | Separate attr: `#[bond(fields…)]` on the relation field |

Does not change storage; only authoring surface.

### W1 — Write verbs for attrs

| Option | Behavior |
|--------|----------|
| **W1a** | Attrs only at `tie` time; changes require cut+tie (harsh) |
| **W1b** | `tie` may set attrs; live **set on tie** (partial) + PATCH HTTP (recommended) |
| **W1c** | Full replace only on tie (PUT) |

### D3 — Edge predicates on root (still open)

Bond attrs make predicates more valuable, but still a separate slice.

| Option | Behavior |
|--------|----------|
| **P0** | Defer all edge `where` |
| **P1** | `courses has "<right-id>"` only |
| **P1+P2** | + `courses some (grade = "A")` / target field some |

Still: predicates filter **root** only; no order/page on bond bags.

### D4 — `end(target)` vs live ties (still open)

| Option | Behavior |
|--------|----------|
| **K0** | No change (ties may point at ended targets) |
| **K1** | `link` omits ties whose right is not live (read filter) |
| **K2** | Cascade cut on end(target) |
| **K3** | Reject end(target) while live ties exist |

### D5 — Live uniqueness / re-enroll (still open; likely bug today)

DDL `UNIQUE(left,right)` + soft `cut` can block re-tie after drop.

| Option | Behavior |
|--------|----------|
| **U1** | Drop hard UNIQUE; engine enforces at most one **live** pair |
| **U2** | `cut` hard-deletes join row |
| **U3** | Re-`tie` resurrects soft-cut row (clear expires, optional attr reset) |

### D2 — Roster / reverse (deprioritized)

With P1, “students who take course X” is filter-on-root, not reverse bond.

| Option | Behavior |
|--------|----------|
| **R0** | No reverse; use P1/scan (default until scale hurts) |
| **R3** | Later: explicit mirror entrance, **one** physical join |

**R2** (second join table for reverse) remains **must not**.

## Suggested default package (for when you want a single vote)

If no further taste calls:

| Id | Default |
|----|---------|
| M1 | **M1b** nested relation fields (local, explicit) |
| W1 | **W1b** tie + set-on-tie |
| D3 | **P1** first; P2 after attrs ship |
| D4 | **K1** read filter |
| D5 | **U1** live-unique (keep soft-cut symmetry with row `end`) |
| D2 | **R0** until needed |

## Must not

- Treat bond attrs as violating 4NF or requiring Enrollment first.
- Auto-generate reverse bonds or second join tables for mirrors.
- Order/page on bond bags.
- Silent truncate of fat bond bags.
- Put reign columns in business bond fields.

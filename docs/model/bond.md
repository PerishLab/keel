# Bond attributes law

Settled: many2many associations may carry **business fields** on the join relation.
This is 4NF-correct: attributes of the pair, not an independent multivalued
fact. It is an engine expressiveness gap closed by design — not a fork against
normalization.

Enrollment-as-named-unit remains a **later promotion** when the association
must be a query root with its own lifecycle/workflow. Default path is bond
attrs on `#[relation(..., many2many)]`.

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
- Pure many2many (zero bond fields) remains valid.

## Macro intent (normative direction)

Declare fields on the relation, not as a second resource:

```rust
// illustrative; exact attribute grammar is an open micro-decision
#[relation(Course, many2many)]
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
| D1 association data | **Bond attrs** on many2many join (not Enrollment unit first) |
| 4NF | Satisfied: attrs of the pair on the association relation |
| Pack / H0 / root-only page | Unchanged |
| No auto reverse | Unchanged |
| No association GET | Unchanged |

## Settled package (adopted)

| Id | Choice |
|----|--------|
| M1 | **M1b** — `#[relation(Course, many2many, grade = string)]` |
| W1 | **W1b** — attrs on `tie`; `tune` + HTTP PATCH on tie |
| D3 | **P1+P2** — `has` + `some (field op val)` on bond or target |
| D4 | **K1+K3** — link hides dead targets; `end` rejects while live ties remain (in **or** out) |
| D5 | **U1** — live-pair unique on the serialized write path (`docs/unique.md`) |
| D2 | **R0** — no reverse; roster via P1 / scan |

### P2 surface

```text
from Student where courses some (grade = "A")
from Student where courses some (code = "CS101")
from Student where courses some (id = "3")
```

- Outer field is bond name; inner is bond attr, target field, or `id` (right key).
- Only live ties; target-field/`id` match requires live target (same spirit as K1).
- Still filters **root** only; no order/page on bonds.

### K3 surface

`end` on a unit fails with `live ties remain` when any live many2many still references
it as **right (inbound)** or **left (outbound)**. Cut those ties first, then
end. HTTP maps this to **409 Conflict**. K1 remains for read filtering.

### Live ends on tie (A)

`tie` and `tune` require both endpoints live:

- `left not live` / `right not live` (HTTP **400**)
- aligns write path with K1 (no new edges to retired rows)

## Promotion law (bond → unit)

Default stays bond attrs. Promote the association to a named unit when
**any** hard trigger holds:

| Trigger | Test |
|---------|------|
| further edges | something must attach to the association itself (e.g. line comments on a review) |
| workflow | the association owns state transitions (e.g. review approve/reject) |
| multiplicity | the same pair must exist more than once live — live-unique on `(left, right)` is wrong for the fact (e.g. reactions: one actor, one issue, many emoji) |
| root need | the association must be ordered, paged, or filtered as the query subject (bond bags never order/page) |

Exemplars: a review (edges + workflow → unit), a reaction (multiplicity →
unit), an enrollment grade (no trigger → bond attr).

Promotion is a modeling change, not an engine feature: the unit declares
`many2one` to both former endpoints and lives under normal law. No dual
form — a promoted association must not keep a shadow bond.

### Deferred

- R3: shared-arc reverse entrance
- K2 cascade cut on end

## Must not

- Treat bond attrs as violating 4NF or requiring Enrollment first.
- Auto-generate reverse bonds or second join tables for mirrors.
- Order/page on bond bags.
- Silent truncate of fat bond bags.
- Put reign columns in business bond fields.

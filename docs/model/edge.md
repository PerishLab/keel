# Edge navigation law

Settled product law for how query grows association reads. Implementation may
lag; behavior that lands must not violate this document.

## Return shape (flat bags)

Edge reads return a **pack**: named flat **bags**, not a nested document tree.

- One bag per unit kind involved (root always; targets only if a future hydrate
  mode adds them — current mode is **H0**, bond bags only).
- One bag per selected bond.
- Bond bags are honest **tie** projections, not embedded target rows.
- Clients assemble trees from bags by id. The engine does not nest.

Edge **read** lives only in query DSL (no association GET). Edge **write** is
Core (`tie` / `cut`) and may be projected over HTTP:

| Method | Path | Core |
|--------|------|------|
| `POST` | `{prefix}/{unit}/{id}/{bond}` | `tie` body `{"right": <id>}` → `{"id": <tie>}` |
| `DELETE` | `{prefix}/{unit}/{id}/{bond}/{tie}` | `cut` (tie must belong to that left) |

`GET …/{bond}` remains **404** (reads stay in `/query` + `link`).

## Wire shape (settled)

### Always pack (A)

Every successful `POST {prefix}/query` returns a pack **object**, including
when the DSL has no `link`:

```json
{
  "root": "student",
  "bags": {
    "student": [ /* row */ ]
  }
}
```

There is no array-shaped `/query` success body. REST `GET /{unit}` remains an
array of rows (different surface).

Request body stays `{"q":"<dsl>"}`.

### Pack JSON

| Field | Rule |
|-------|------|
| `root` | Root unit **table** name (`ddl::table`), ascii lower. |
| `bags` | Object map; keys are bag names; values are arrays. |
| unit bag key | Table name of that unit (`student`). |
| bond bag key | `{root_table}.{bond}` as declared on the root unit (`student.classes`). |
| missing key | Only bags that this tree produces appear. No placeholder nulls. |
| empty bag | Present as `[]` when the tree selected that bag and the set is empty. |

Root bag is **always** present (may be `[]`). Bond bags appear only for each
`link` on the tree.

### Hydrate (H0)

`link` produces the **bond bag only**. No target unit bag is added by expand.

Clients that need target rows issue a second top-level query, e.g.
`from Class where id in ("3", "7")` using engine-key `id` preds.

### Row item (unit bag)

Same projection as today's REST list row:

```json
{
  "id": 1,
  "nickname": "ada",
  "avatar": "https://a.example/a",
  "expires_at": null,
  "created_at": 0,
  "updated_at": 0
}
```

### Tie item (bond bag)

```json
{
  "id": 9,
  "left": 1,
  "right": 3,
  "expires_at": null,
  "created_at": 0,
  "updated_at": 0
}
```

- `left`: root-side key (owner row id).
- `right`: target-side key.
- No embedded target fields.

### Non-root bag order (engine-fixed)

| Bag | Stable order |
|-----|----------------|
| bond bag | `id` ascending |

Not expressible in DSL. Root bag order follows root `order by` / default id.

### Core / Rust sketch (normative intent)

- `Pack { root, bags }` is the query result type on the HTTP path.
- `Core::live` may keep returning `Vec<Row>` as sugar (single unit, no pack).
- `Core::query` / HTTP `/query` always produce pack on success.

### Digest

`digest` must include every `link` bond name in stable order.

## DSL shape (settled, expand path)

Clause order (fixed, case-insensitive keywords):

```text
from <Unit>
[where <pred> (and <pred>)*]
[link <bond>]+
[order by <field> [asc|desc]]
[limit <n>]
[after "<id>"]
```

- `link <bond>`: forward edge name on the root unit.
- Repeat `link` for multiple bonds. Duplicate same bond → error.
- Unknown bond → error. Nested bond path → error.
- `order` / `limit` / `after` bind **only** the root.

Example:

```text
from Student
where nickname != "zoe"
link classes
order by nickname asc
limit 10
after "3"
```

Example pack under **H0**:

```json
{
  "root": "student",
  "bags": {
    "student": [
      {
        "id": 4,
        "nickname": "ada",
        "avatar": "https://a.example/a",
        "expires_at": null,
        "created_at": 1,
        "updated_at": 1
      }
    ],
    "student.classes": [
      {
        "id": 9,
        "left": 4,
        "right": 3,
        "expires_at": null,
        "created_at": 1,
        "updated_at": 1
      }
    ]
  }
}
```

## Count terminal

`from <Unit> [where …] count` returns a count pack — no bags:

```json
{ "root": "student", "count": 3 }
```

- Counts live root rows passing the preds; `has` / `some` apply.
- **Count stands alone**: combining with `link` / `order` / `limit` /
  `after` is an error.
- No group-by, no further aggregates; `digest` includes the terminal.

## Single subject (root only order / page)

Only the **root** unit of a tree (the `from` unit) may take:

- `order by`
- `limit`
- `after`

Bond bags **must not** accept order/page in the DSL.

## Closure, not a second feed

When a tree **link**s a bond, bond bags are the **live closure** of the current
root bag:

- Root empty ⇒ linked bond bags are `[]`.
- Bond bag = live ties whose `left` is in the root id set (forward bond only).

Closure is complete for this root page (subject to hard cap). Deep graphs use
another top-level query, not nested links.

## Depth and direction

- **Depth 1** per tree: root + direct bonds only.
- **Forward bonds only**. No reverse generation.
- Multiple distinct `link`s on one root are allowed.

## Safety valve (settled seat: C0)

If a closure would exceed an engine hard cap, the query **errors**. Silent
truncation is forbidden.

- **Seat**: engine constants (**C0**), not `keel.toml` for v1.
- **Scope**: max live ties loaded per `link` bag in one request (exact number
  is an impl constant; document in code + raise later if needed).
- Caps are not user `limit` on bond bags.

## Settled decisions

| Id | Choice |
|----|--------|
| Response | **A** — always pack on `/query` |
| Hydrate | **H0** — bond bags only; no target unit bag from `link` |
| Cap seat | **C0** — engine constants; error on overflow |
| Count | terminal, stands alone, count pack without bags |
| Like | `field like "frag"` case-insensitive substring on a text field |
| Association data | **Bond attrs** on many2many join — see `docs/model/bond.md` |

## Bond suite (see `docs/model/bond.md`)

Adopted: M1b attrs, W1b tune, P1 `has`, K1 live-target filter, U1
live-unique, R0 no reverse.

## Must not

- Nested GraphQL-style response trees as the primary edge delivery.
- Order/page on bond bags.
- Association REST **reads** (GET on bonds).
- Engine-invented reverse edges.
- Silent partial closures.
- Dual bag naming schemes (always table / `{table}.{bond}`).
- Array success body on `/query` (always pack).
- Target unit bags from `link` under H0.

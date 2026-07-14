# Edge navigation law

Settled product law for how query grows association reads. Implementation may
lag; behavior that lands must not violate this document.

## Return shape (flat bags)

Edge reads return a **pack**: named flat **bags**, not a nested document tree.

- One bag per unit kind involved.
- One bag per selected bond.
- Bond bags are honest **tie** projections, not embedded target rows.
- Clients assemble trees from bags by id. The engine does not nest.

REST stays free of association routes. Edge **read** lives only in query DSL.
Edge **write** stays on Core (`tie` / `cut`).

## Wire shape (settled)

### Pack JSON

`POST {prefix}/query` body stays `{"q":"<dsl>"}`. Response object:

```json
{
  "root": "student",
  "bags": {
    "student": [ /* row */ ],
    "student.classes": [ /* tie */ ],
    "class": [ /* row */ ]
  }
}
```

| Field | Rule |
|-------|------|
| `root` | Root unit **table** name (`ddl::table`), ascii lower. |
| `bags` | Object map; keys are bag names; values are arrays. |
| unit bag key | Table name of that unit (`student`, `class`). |
| bond bag key | `{root_table}.{bond}` as declared on the root unit (`student.classes`). |
| missing key | Only bags that this tree produces appear. No placeholder nulls. |
| empty bag | Present as `[]` when the tree selected that bag and the set is empty. |

Root bag is **always** present (may be `[]`). Bond bags appear only for each
`link` on the tree. Target unit bags appear only under the hydrate rule (see
decision blockers).

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

- `id`: engine key (`i64`).
- business fields: strings (current cell encoding).
- reign: `expires_at` / `created_at` / `updated_at` (null or number).

### Tie item (bond bag)

Honest `Tie` projection:

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
- No embedded target fields. Client joins `right` → target unit bag `id`.

### Non-root bag order (engine-fixed)

| Bag | Stable order |
|-----|----------------|
| bond bag | `id` ascending |
| target unit bag | `id` ascending |

Not expressible in DSL. Root bag order follows root `order by` / default id.

### Core / Rust sketch (normative intent)

Wire is the contract. Engine types should mirror it (names may track vocabulary):

- `Pack { root: String, bags: … }`
- unit bag → `Vec<Row>`
- bond bag → `Vec<Tie>`
- `query` / `ask` with `link` return `Pack` (exact Rust API is an impl detail
  as long as wire matches).

### Digest

`digest` must include every `link` bond name in stable order so identity/cache
cannot ignore expansion. Exact digest string is impl detail; omitting links is
a bug.

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

- `link <bond>`: `bond` is an ident naming a **forward** edge on the root unit.
- Repeat `link` for multiple bonds (`link classes link …`). No `link a.b`.
- Unknown bond → error. Bond on non-root → error (depth 1).
- `order` / `limit` / `after` after any `link` still bind **only** the root.
- Writing order/page tokens after a bond name as if paging the bond → parse
  error (trailing / wrong subject), never silent ignore.

Example:

```text
from Student
where nickname != "zoe"
link classes
order by nickname asc
limit 10
after "3"
```

Example pack (hydrate assumed on for illustration only):

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
    ],
    "class": [
      {
        "id": 3,
        "title": "algo",
        "expires_at": null,
        "created_at": 1,
        "updated_at": 1
      }
    ]
  }
}
```

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
engine-fixed stable order above.

## Closure, not a second feed

When a tree **link**s a bond, non-root bags are the **live closure** of the
current root bag:

- Root empty ⇒ linked bond bags (and any target bags) are `[]`.
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

Caps are engine policy, not user `limit` on non-root bags. Default numbers and
config seat are not settled (see blockers).

## Decision blockers (stop here)

These are product forks. Do not invent a default in code without an answer.

### 1. Response mode when there is no `link`

Today `/query` returns a **JSON array** of rows. Pack is an **object**.

| Option | Behavior |
|--------|----------|
| **A. Always pack** | Every `/query` returns `{root, bags}`. Root-only query ⇒ `bags` has only the root unit bag. Breaking for current clients/smoke. |
| **B. Dual** | No `link` ⇒ keep array (compat). Any `link` ⇒ pack object. Two shapes forever. |

Shape of pack itself is settled above either way. **This blocker is only about
compat vs one wire type.**

### 2. Target unit hydrate

When `link classes` runs, is the target unit bag included?

| Option | Behavior |
|--------|----------|
| **H0** | Bond bag only. Client loads targets with a second `from Class where id in (…)`. |
| **H1** | Always hydrate live targets for every `link` (bag key = target table). |
| **H2** | Explicit only, e.g. `link classes into Class` / `link classes with Class` (keyword TBD once H2 wins). |

Illustrative pack above assumed **H1**. Law allows any of H0–H2; pick one.

### 3. Closure hard cap

| Option | Behavior |
|--------|----------|
| **C0** | Fixed constants in engine (e.g. max ties per request). |
| **C1** | `keel.toml` policy section. |
| **C2** | Defer caps until a real blow-up; still error API must exist before silent truncate is possible. |

Numbers and seat wait on this choice. **Error-on-overflow** is already law.

### 4. Edge as where-predicate (not shape)

`where classes …` as exists/filter on root is **out of shape scope**. May land
later beside `link`. Does not change pack layout. **Not required to start
implementing expand `link`.**

## Must not

- Nested GraphQL-style response trees as the primary edge delivery.
- Order/page on bond or target bags.
- Association REST routes.
- Engine-invented reverse edges.
- Silent partial closures.
- Dual bag naming schemes (always table / `{table}.{bond}`).

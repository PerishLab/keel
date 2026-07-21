# Capability law — modeling & veil

## Modeling law

A visibility boundary must be a **unit boundary**. Fields never carry their
own visibility; a field that needs different visibility than its row is a
different unit with its own root chain. No field-level grants, ever.

## Veil (ceremony-only units)

A unit marked **`veil`** (`#[resource(veil)]`) is kept in the graph and
governed by the engine, but is **removed from the generic HTTP
projection**: no `/{unit}` routes and no `/query` reach it. It is written
and read only by app code through the six Face verbs — its *ceremonies*.

The reason is a real composition hazard: generic projection and root-chain
subtree grants are each correct alone, but together they let an operator
who holds a grant over its own subtree **author its own credential rows**
(mint a bearer token, set a password hash) through the generic routes,
bypassing the ceremony that is supposed to be the only writer. Veil is the
seam: a credential unit (session, token, password, invite, refresh) roots
at its owner for lifecycle, yet is authored only by ceremony.

- Veil bounds the **wire**, not the Face. Root-chain self-service still
  holds *through ceremonies*: logout ends the session, a rotate replaces a
  token — the app exposes those, the generic `PUT /Session` does not.
- Veil is not visibility. A veiled unit the operator could `see` is still
  invisible on the wire; a non-veiled unit still obeys grant coverage. The
  two boundaries compose.
- Engine units (`@grant`, `@seal`, `@pulse`) are never veiled: `@grant` is
  deliberately writable on the wire (that is how grant/revoke happen).

## Settled package

| Id | Choice |
|----|--------|
| C-B | Bootstrap: `@grant` engine unit, self-governed; verbs closed at six |
| C-G | Genesis: super admin token, unique window, pre-identity, journaled |
| C-L | Lockout: recoverable via window; guardrail courtesy, not law |
| C-1 | Root chain + subtree grants |
| C-M2 | Chain designation: bare `root` marker on one `many2one`; ≤1 per unit; implies required |
| C-M1 | Group operator: `crew` marker on a many2many; grant `who = "<unit> <id>"` expands live membership |
| C-2 | `@me` — sole context variable in pred scopes |
| C-3 | Write scopes: postcondition on `put`, pre+post on `set` |
| C-4 | Attenuation; no deny; no amplification |
| C-5 | Wildcards genesis-only |
| C-6 | Grant OR-injection engine-internal; DSL stays and-only |
| C-7 | No-see reads as 404; 403 writes only |
| C-8 | Audit asymmetry documented; no simulator promised |
| C-10 | `tie` = verb on left chain + `see` on right |
| C-11 | `anon` / `all` pseudo-operators |
| C-12 | Visibility boundary = unit boundary |
| C-14 | Mint to creator; identity-unit anon put mints to row |
| C-15 | Uniform check across all six verbs, `put` included |
| C-16 | `see` pred scopes cover the subtree via the matching ancestor; write preds never descend |
| C-D | `gate` default credential package: caller space, enumerable grant authority, possession only at identity birth |
| C-17 | `allows`: enforcement-point probe; one decision procedure with the verbs; reading not ticket; fail-closed on unwritten data |
| C-18 | `veil`: credential units off the generic projection; ceremony-written via Faces, never authorable on the wire; bounds the wire not the Face |

## Open

| Id | Question |
|----|----------|

| C-9 | `batch`: composite multi-verb atomicity (org+team+member+grant) |
| C-P | Injection cost; materialized root column as known mitigation |

## Must not

- A second policy language, condition grammar, or context variable beyond
  `@me`.
- Deny rules, overrides, or precedence between grants.
- A second wire path to sudo, or sudo verbs escaping the journal.
- Grants that amplify beyond the granter's coverage.
- Field-level visibility or field-level grants.
- 403 on refused reads.
- Admin REST routes beside the six verbs on `@grant`.
- Per-descendant grant choreography where a root grant is the fact.
- A grant with an unparseable pred granting anything (must fail closed).
- Engine checks that bypass `@grant` semantics outside the one grounded
  meta level.
- A probe that grants: no reservation, ticket, or lock behind `allows`.
- A second decision procedure for `allows` beside the one the verbs run.
- A credential unit (session, token, password, refresh, invite) authorable
  through the generic projection instead of a ceremony — it must be veiled.
- A credential package holding wildcard or sudo for steady-state
  operation (possession is for identity birth only).


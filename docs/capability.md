# Capability law

Settled product law for how keel grows authority. Implementation may lag;
behavior that lands must not violate this document. Depends on the
`many2one` relation kind for root chains (§ root chain); until many2one
lands, every row is its own root and only row/pred/all scopes apply.

## Bootstrap (self-hosting)

Grants are rows of an engine unit **`@grant`**, governed by the same grant
machinery as every other unit. There is no second administration surface:

| Admin act | Is exactly |
|-----------|------------|
| grant | `put` on `@grant` |
| revoke | `end` on `@grant` |
| audit | `query` from `@grant` |

The verb set is **closed at six** and engine-owned:

```text
see  put  set  end  tie  cut
```

`see` is the read verb (`live` / `query` / GET). `set_tie` checks as `tie`.
No verb is ever added per unit or per deployment.

## Genesis (the window)

One credential exists outside the grant system: the **super admin token** —
the single axiom of an otherwise self-hosted authority graph.

- Minted at first `bind`, surfaced exactly once, stored only as hash.
- **Pre-identity**: it resolves to the sudo face directly, never to a row of
  the identity unit.
- **Unique window**: the only wire path to sudo, and the only credential
  the engine itself ever parses (operator injection cannot express sudo —
  see § identity). In-process, the holder of `bind` is sudo by possession
  (`Core::sudo()`); that is the honest trust boundary, not a second window.
- Dormant by ceremony: its first acts create a real admin operator and its
  grants; thereafter it is set aside. Rotatable via sudo.
- Every sudo verb is journaled. Sudo use is loud by design.

Lockout is recoverable through the window by definition, so no last-admin
invariant is law. An engine may refuse to `end` the final wildcard grant as
a courtesy guardrail.

## Grounding (one meta level)

Checks consult `@grant` without checking access to `@grant`. The metacircle
grounds out at exactly one level: engine-internal reads of the grant table
carry ambient authority; **user-facing** reads and writes of `@grant` are
governed like any unit.

This clause fixes semantics only. The default engine implements it by direct
reads; a replacement engine may use any mechanism (e.g. store-native row
security) provided observable behavior is identical. Modeling never moves
with the engine. The scenario suite is the conformance boundary
(`docs/verify.md`).

## Identity

Identity is decoupled: **keel never authenticates.** Caller middleware
resolves credentials — api key, session, user id, mTLS; keel does not know
or care which — and injects the resulting **operator**. keel consumes it.

- One designated **identity unit** per deployment, named in `keel.toml`:

```toml
[identity]
unit = "Actor"
```

- An **operator** is a live row id of the identity unit — nothing else. The
  injection channel carries row ids only and is therefore structurally
  incapable of expressing sudo. Injection is **in-process by definition** —
  never a wire header; a deployment that moves resolution behind the wire
  owns that trust hop itself.
- An injected operator is trusted by definition; credential quality is the
  caller's responsibility. Core enters through `Core::of(operator)` and
  never sees passwords, tokens, sessions, or headers.
- Credential rows (tokens, keys) are ordinary business units with **no
  engine designation**; self-service management falls out of mint and root
  chain, not special routes.
- No injection = `anon`. Any injected operator matches `all`. Grants to
  them are ordinary `@grant` rows. A grant to `anon` covers operators too —
  what is public to strangers is public to members.
- **Group operator** (C-M1 settled): a many2many bond marked `crew` names a
  unit's membership roster. A grant's `who` of the form `<unit> <id>` (e.g.
  `team 5`) admits any operator that is a live member of row `id` via that
  unit's crew bond. At most one crew bond per unit.

## Gate (default credential package)

keel may ship **`gate`**, the optional default credential package:
recursive resource definitions (`Token`, `Session`, `Pass` — plain
business units) plus one projection middleware. It stands in caller space
and carries no engine privilege; replacing or omitting it is not a fork of
the law.

- **Authority is enumerable.** Gate runs as a minted service operator
  whose entire power is ordinary grant rows (`see` on its credential units
  and the identity unit, `put` on the session unit).
  `from @grant where who = <gate>` is the package's complete audit.
- **Two grounding floors.** Steady-state resolution (credential →
  operator) is grant-grounded: an ordinary governed query. There is no
  cycle — `of(id)` never involves a credential, so verification never
  recursively authenticates. Identity **birth** (registration) is
  possession-grounded: writing the newborn's credential rows as the
  newborn is a mini-genesis, grounded at the same axiom as system genesis.
- Registration rides the **anon face** so mint goes to the created row
  (§ mint); a credential package must never create identity rows through
  its own face.
- Self-service is root chain, not gate code: logout = `end` own session;
  password change = `set` own credential row; revocation = `end` own
  token, effective immediately.
- Suspension is business state on the identity row, enforced by gate
  refusing to resolve operators for suspended rows — never engine `end`.
- **Resolution and ceremony are separable surfaces.** `screen` is the
  pass layer alone (credential → operator, bar honored); `wall` adds
  the stock doors. A caller with its own ceremonies (invitation join,
  password floor) takes `screen` and keeps gate's grounding intact.
- **Bearer secrets from the OS CSPRNG.** Sessions and tokens draw 256
  bits from `getrandom`, never a hash-table hasher. Session cookies are
  `HttpOnly; SameSite=Lax; Path=/`; `.secure()` adds `Secure` behind
  TLS. Credential units should be `veil`ed (C-18) — no wire authoring.

## Grant shape

```text
@grant
  id            -- engine key
  who           -- operator row id | group row id | anon | all
  verb          -- one of the six, or * (genesis only)
  unit          -- unit name, or * (genesis only)
  scope         -- all | row <id> | pred "<where-pred>"
  expires_at, created_at, updated_at
```

- Scope preds reuse the query where-grammar — **no second policy language**.
- Exactly one context variable exists in pred position: **`@me`**, the
  checking operator's row id.
- A pred that no longer parses against the current schema grants **nothing**
  (fail closed).
- Wildcard `verb` / `unit` exist for genesis (site admin) only; business
  policy names its units.

## Root chain

Every unit derives its **root chain**: the designated `many2one` relation
walked upward until it reaches the identity unit. A grant on a row covers the row
**and its entire subtree**. Checks walk the chain up; any covering grant
suffices.

- Chain designation (C-M2 settled): the bare `root` marker on one
  `many2one` — `#[relation(Repo, many2one, root)]`. At most one per unit;
  none makes the row its own root; `root` implies required (never `opt`).
- Subtree coverage is what keeps policy quantified: authority is granted at
  the root (an org, a repo), never choreographed per descendant row.
- Read-path visibility never needs path predicates; the chain walk is
  engine-internal. Pred scopes with a `many2one` hop remain legal only in
  **write** position (one hop).
- **Pred scopes cover the subtree for `see` only** (C-16): a `see` pred
  grant covers every descendant whose root chain passes through a row
  matching the pred — evaluated against that ancestor row. `see Repo pred
  visibility = "public"` thus makes every issue, comment, and pull of a
  public repo visible, and retracts the moment the repo turns private.
  **Write verbs never inherit pred coverage down the subtree** — a broad
  create-pred (e.g. `put Actor pred kind = "org"`) must not become
  write-anywhere; subtree writes require a row-scope grant (mint). Pred
  scopes still cover writes when the pred matches the row itself.

## Check semantics

Uniform for all six verbs: a verb on a row is allowed iff some live grant
covers `(who ∋ operator, verb, unit, row)` via row/subtree scope or a
satisfied pred scope.

- **`put`**: the incoming row's relations determine its root before insert;
  pred scopes act as **postconditions** on the incoming row.
- **`set`**: pred scopes must hold **before and after** the write.
- **`tie` / `cut`**: require the verb on the **left** row's chain plus `see`
  on the right row. `set_tie` checks as `tie`.
- **`see` on query**: the engine conjoins the operator's coverage into the
  plan. Multiple applicable grants combine as **OR** — engine-internal only;
  the DSL keeps its and-only surface.

## Mint

`put` mints a full-verb row grant on the created row to the **creator**.
Single exception: an `anon` put on the identity unit mints to the **created
row** (a new operator owns itself; there is no creator to own it).

## Attenuation

An operator may `put` on `@grant` only what its own live coverage already
includes — grants can narrow, never amplify. Sudo is exempt (it is the
axiom). No deny rows exist in any form: absence of a grant is the only
refusal.

Attenuation is the **whole** gate on grant writes: delegation is inherent
to holding, never a separate permission. Symmetrically, an operator may
`end` a grant row it could have issued. Wildcard-unit grants remain
genesis-only (attenuation refuses them for operators).

## Read refusal is absence

A row the operator cannot `see` does not exist for it:

- `GET /{unit}/{id}` → **404**, never 403.
- Query results silently exclude it (this is coverage, not truncation — C0
  is about caps, not authority).
- **403** is reserved for refused writes; **401** belongs to caller
  middleware (keel itself never authenticates).
- The trigger adaptor is a read surface under this same law: no event is
  delivered outside `see` coverage (`docs/trigger.md`).

## Audit asymmetry

`check(who, verb, unit, row)` is a cheap point query. The reverse question —
"who can see this row" — is satisfiability over pred grants, not lookup.
The engine ships `check` and honest `@grant` enumeration; it does not
promise a reverse audit or simulator.

## Enforcement points (the probe)

Bytes may live outside keel while their authority lives inside — blob
objects (`keel-blob`), git repositories, any app-owned byte plane. The
app code serving those bytes is an **enforcement point**: it must ask
what an operator may do without doing it.

`Face::allows(verb, unit, key) -> bool` is that question: **would this
face's operator be authorized for `verb` on this row, now.** It is the
read face of the capability system — the operator-side projection of
`check` (the audit side keeps its explicit `who`). One decision
procedure serves the six verbs, `check`, and `allows`; a divergence
between them is a bug in law, not a tuning knob.

- `allows` is **not a seventh verb**. It takes a verb as an argument,
  performs nothing, mints nothing, journals nothing, and needs no grant
  to call: the caller learns only what its own attempts would already
  reveal (404 / 403 / success).
- The answer is a **reading, not a ticket**: true at evaluation time,
  conferring nothing. An enforcement point that stretches an answer
  across time (a presigned URL, a session) owns that window — keel
  promised the instant, not the interval.
- **The probe never speculates about unwritten data.** For `put`, `key`
  anchors the prospective root and the probe answers from row and
  subtree scopes; a pred scope whose truth depends on the candidate row
  contributes `false` (fail closed). `set` probes answer the
  precondition half only.

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

# Name law

Settled contract for how a unit is identified. Implementation may lag;
behavior that lands must not violate this document.

One principle covers the family: **a unit's declaration is flat, and its
identity is a tree**. The tree is not a new structure. It is the root
chain, which capability law already derives for every unit and already
walks for every check. Identity simply stops discarding it.

## Two spaces

- **Declaration is flat.** Units never nest syntactically. Every
  `#[resource]` is top level, and containment is declared as a `many2one`
  root inside the struct, never as enclosure around it. Hierarchy stays
  data, so `transfer` can move a subtree by setting one field.
- **Identity is a tree.** A unit's key is its name under its root. Nothing
  new is declared to obtain it; the key is derived from what the root
  relation already says.

## The key

- `key := {root}:{name}`, lowercased, where `root` is the declared name of
  the root relation's target. A unit with no root keeps a bare `{name}`.
- The key is **derived, never written**. The author writes one atom, and
  the word law applies to that atom alone.
- **Depth is exactly one segment of scope.** A key has at most two parts.

## Depth is a detector

A plan that would need a third segment contains two same-named units under
two same-named units. That is a modeling error, and bind refuses it rather
than growing the name to accommodate it. The bound is not a ceiling the
engine hopes never to meet; meeting it is the report.

This is the same shape as lowercasing: `table()` is lossless for every name
the word law permits, so a run-together table name is a compound that
escaped. Here, an unrepresentable key is a model that escaped.

## Two relations, two separators

keel has exactly two structural relations, and each owns a separator:

- `:` — **containment**, a unit under its root: `repo:label`.
- `.` — **possession**, a bond of a unit: `student.courses`.

A pack bag key is therefore `repo:label.tags`, and reads without a schema.
One separator serving both relations would recreate, one scale smaller,
the overload this law exists to remove.

`/` is unavailable: the route grammar's second segment is an id.

## The full key is the only identity

- Everything **persisted or compiled** holds the full key — `@grant` place,
  pulse rows, relation targets, and call sites in a host language.
- A short name is **not a shorter identity**. It is an explicit, fallible
  resolution from human input — query text, a URL path, a hand-typed grant
  — into a key.
- The engine **never widens a short name implicitly**. Nothing stored can
  turn ambiguous as the schema grows, because nothing stored was ever
  short.
- Ambiguity is refused **at resolution**, never at check time. A short name
  that resolves to more than one unit is an error at the door, and an error
  never falls through to a match.

## Relation targets

A `#[relation(...)]` target is a **keel reference, not a Rust path**. A bare
`Name` resolves like any short name — fine while it is unambiguous, refused
once two units share it. To bind a specific one, qualify it with its root:
`#[relation(Repo::Label, …)]` renders the target key `repo:label`. The
`Root::Name` form is exactly two segments; a longer or module-qualified path
(`crate::model::Label`) is not a qualifier and will not resolve — the target
is a name in keel's space, never an import path.

## Renderings

One declared name, several renderings, each derived and each legal in its
own alphabet:

| Contract | Rendering | Example |
|----------|-----------|---------|
| Host type | the declared name | `Label` |
| Identity key | `{root}:{name}` | `repo:label` |
| SQL table | the key, `:` written `_` | `repo_label` |
| URL segment | the key | `/repo:label/{id}` |
| Capability, pulse | the key | `repo:label` |

A bond name is one atom, so the only `_` a table name carries comes from
its own scope. `repo_label_tags` decomposes exactly one way.

## Conformance

The scenario suite must carry two units of the same name under different
roots: both addressable, both separately grantable, a grant on one invisible
to the other, and an ambiguous short name refused at resolution.

## Settled package

| Id | Choice |
|----|--------|
| N-1 | Declaration is flat; identity is a tree derived from the root chain |
| N-2 | `key := {root}:{name}`; bare name when the unit has no root |
| N-3 | The key is derived from the declaration, never written by the author |
| N-4 | Scope is exactly one segment; a deeper need is a modeling error, refused at bind |
| N-5 | `:` is containment, `.` is possession; neither separator serves both |
| N-6 | Persisted and compiled identity is always the full key |
| N-7 | A short name is an explicit fallible resolution, never an implicit widening |
| N-8 | Ambiguity is refused at resolution and never falls through to a match |

## Must not

- A unit name that spells its own scope — a compound standing in for a path.
- Implicit widening of a short name, anywhere.
- A stored or compiled identity that is not a full key.
- One separator carrying both containment and possession.
- Scope deeper than one segment, or a name grown to dodge N-4.
- A resolution failure that resolves to something anyway.
- Case-folding a caller's name on the way in. Resolution matches the
  declared name or its key exactly; `/ACTOR` is not `Actor`.

# Keel v0.10.1

A null no longer carries a type into Postgres. `Val::Null` bound
`None::<i64>`, so the driver declared that parameter `int8` when it prepared
the statement, and Postgres cached the statement together with those types.
The first row to leave a column null therefore fixed it as `int8` for every
later execution, and the next row that supplied text sent four bytes where
eight had been declared:

```
INSERT INTO "@unit_@field" (...) VALUES ($1..$14)
ERROR:  insufficient data left in message
CONTEXT:  unnamed portal parameter $6
```

A null now binds a value that produces OID zero. The type stays unspecified,
Postgres infers it from the target column, and the first prepare reads the
type off the schema rather than off whichever row happened to arrive first.

The binding is as old as the adaptor and is not a v0.10.0 regression. What
v0.10.0 changed is that the manifest is written as rows, so `@field.serial` —
null for most fields and text for a scoped serial — puts the poisoning
sequence directly in the boot path. Any model carrying a `serial` field could
not bootstrap on Postgres at all.

The reach is wider than bootstrap. An ordinary model whose optional text
field is absent from the first row and present in the second failed the same
way, on the ordinary write path. Two tests hold both halves: `scoped`
bootstraps a model with a scoped serial, `spare` writes an optional field
after a row that omitted it. The pg suite carried neither before, which is
why the engine shipped with this.

Sqlite is untouched. It declares no parameter types, so its null binding was
never able to fix a column.

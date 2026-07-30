# Migrating to v0.10.1

Nothing is asked of a caller. The public surface is unchanged, the estate
format stays at eleven, and no store needs rebuilding.

If you run on Postgres, upgrade. Before this release a nullable text column
was fixed to `int8` by the first row that left it null, and every later row
that supplied text for it failed with `insufficient data left in message`.
The failure is a refusal rather than a corruption — the write does not land —
but it can appear long after the schema was created, on the first row that
happens to fill a column its predecessors left empty.

If you run only on Sqlite, this release changes nothing you can observe.

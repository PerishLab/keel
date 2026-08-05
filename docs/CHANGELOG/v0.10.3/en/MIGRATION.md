# Migrating to Keel v0.10.3

Nothing is asked of a caller who declares no closure. The public surface
only grows: `closure` is a new modifier, and code that does not use it
compiles and behaves exactly as it did on v0.10.2.

The stored format moves from eleven to twelve, so an existing store does
evolve on first open. That evolution is the ordinary one Keel already
performs; it now also refreshes derived relations inside the same
transaction, so there is no separate step to run and no window where a
store carries a schema without its closures. No store needs rebuilding and
no data is discarded.

If you adopt `closure` on an existing bond, two behaviours are worth
knowing before you do. Any tie that would close a cycle is refused from
that point on, so a graph that currently contains cycles will start
refusing the writes that create them — check that your data is acyclic
first, because the failure surfaces at write time rather than at
declaration time. And the generated `<bond>_closure` relation is engine
owned: writing to it directly is refused, so anything that previously
maintained a hand-rolled reachability table must stop writing to the name
the derive now claims.

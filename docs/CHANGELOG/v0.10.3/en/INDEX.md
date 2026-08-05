# Keel v0.10.3

## Declared relations can carry their own closure

A self-referential many-to-many bond can now be declared `closure`. The
derive generates a second relation beside it, named `<bond>_closure`, and
the engine keeps that relation equal to the non-reflexive transitive
reachability of the direct ties:

```rust
#[bond(many2many, target = "Part", closure)]
contains: Vec<Part>,
```

Reading `contains` answers which parts are held directly. Reading
`contains_closure` answers which parts are reachable at any depth, without
a recursive query and without a second round trip.

The modifier is many2many only. Declaring it on any other cardinality is a
compile error, in the same place `crew` already refuses one.

## The closure is maintained, not recomputed

Every path that can change reachability maintains the closure inside the
same transaction that changed the direct tie: linking, cutting, batch
writes, schema evolution, query reads, and the operator Faces. A rollback
takes the closure back with it, so a reader never sees reachability that
disagrees with the ties it was derived from.

Two things are refused rather than allowed to drift. A tie that would close
a cycle is refused, because transitive reachability over a cycle has no
non-reflexive answer. And a direct write to a `<bond>_closure` relation is
refused as engine owned, so the derived relation cannot be edited into
disagreeing with its source.

## Estate format twelve

The stored format moves from eleven to twelve. Evolution now refreshes
derived relations as part of the same migration that reshapes the schema,
so a store carrying declared closures arrives consistent rather than
needing a separate pass.

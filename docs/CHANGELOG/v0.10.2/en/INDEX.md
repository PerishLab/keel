# Keel v0.10.2

## Keel compiles in the release profile again

`Manifest::lift` asserts round-trip stability through `debug_assert!`. That
macro compiles its call away when assertions are off, but it still
type-checks the expression in every profile — and `mirrors` was declared
`#[cfg(debug_assertions)]`. In a release build the call survived and the
method did not:

```
error[E0599]: no method named `mirrors` found for struct `Manifest`
  keel/src/model/manifest/mod.rs:89
```

v0.10.0 and v0.10.1 cannot be built in release at all. The gate is gone;
the method costs nothing in release because the macro already removes the
call, and dead-code analysis still counts it used.

## The guard now type-checks that profile

Nothing here had ever compiled it. Guard runs `cargo test` and clippy
`--all-targets`, both debug. The release lane publishes crates, and
`cargo publish` verifies in debug too. Keel is a library, so no lane of its
own was ever going to reach a release build — the first one that did was a
downstream Dockerfile, two versions later.

Guard gains `cargo check --release`. The defect class is a type error under
a different `cfg`, so a check is enough, and it costs seconds where a build
costs minutes.

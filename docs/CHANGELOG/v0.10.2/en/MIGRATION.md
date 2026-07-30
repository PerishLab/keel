# Migrating to Keel v0.10.2

Nothing is asked of a caller. The public surface is unchanged, the estate
format stays at eleven, and no store needs rebuilding.

Upgrade if you build Keel in the release profile — a container image, a
packaged binary, anything past `cargo build`. On v0.10.0 and v0.10.1 that
build fails outright with `no method named mirrors found for struct
Manifest`. A debug build, which is what tests and local runs use, was never
affected, so the failure appears first at packaging time.

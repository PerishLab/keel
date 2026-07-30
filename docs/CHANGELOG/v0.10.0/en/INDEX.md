# Keel v0.10.0

Keel is a library and owns no configuration surface. `Config`, `load`, `NAME`,
`Listen`, `Cache`, `Hold`, `Identity`, and the adaptor store fragments are
gone. The caller constructs every runtime value and hands it over: an `Estate`
to `bind`, a host, port, and prefix to `listen`, and its own opened store.
Keel reads no file and no environment variable.

The demo binaries and the repository-rooted `keel.toml` go with them. Keel
ships no binary. The scenarios those binaries carried are now Rust tests
against the router, the authority matrix, the gate doors, and a live relay,
so the same surface is held by assertions rather than by a process nobody
deployed.

Keel depends on Plumb nowhere. The vocabulary Keel needs is Keel's own.

The schema is rows. `@unit`, `@bond`, `@field`, `@scope`, and `@value` are
engine units carrying the manifest of each generation; a unit row names its
generation and the rest hang from it. Bootstrap, adoption, and evolution write
them, and the ordinary put path refuses them the way it refuses `@pulse`.
Bind gathers the active generation and holds it against its stored digest,
which is a check on content rather than on format.

A plan no longer has to come from compile-time types. `Graph::add` takes a
`Spec` built at runtime, and `Graph::read` rehydrates the manifest Keel wrote,
so a schema can enter the engine as data.

Estate format moves from nine to eleven.

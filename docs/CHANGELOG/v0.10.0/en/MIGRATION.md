# Migrating to v0.10.0

## Every store must be rebuilt

Estate format moves from nine to eleven. A store on an older format refuses to
open with `estate format 9 needs 11`, and there is no upgrade path: the engine
unit tables are created at bootstrap and never participate in generation
migration, so the new schema rows cannot be grafted onto an existing estate.

Drop the namespace and bootstrap it again. Nothing else in this release can be
adopted without doing so first.

## If you read `keel.toml`

Declare your own configuration surface, in your own format, under your own
environment prefix. Keel discovers no file and reads no environment variable,
so `keel::config::NAME` is gone and the filename is yours to choose.

`Estate`, `Generation`, `Cleanup`, and `Retain` remain Keel vocabulary and
still derive `Deserialize`. Carry them inside your own file if you want them
there, then hand the value to `bind`:

```rust
let core = keel::bind(graph, store).estate(&estate).await?;
```

## If you used `keel::config::Hold`

It selected between a core and `core.bare()`. Call `.bare()` on your own
condition instead.

## If you used `keel::config::Listen`

`listen` takes host, port, and prefix as arguments:

```rust
keel::listen(core.share(), "127.0.0.1", 3000, "").await?;
```

## If you used `adapt::db::Store` or `adapt::pg::Store`

Those fragments carried a store's own configuration. Open the store yourself
with `Sqlite::memory`, `Sqlite::file`, or `Postgres::at`, and hand the opened
store to `bind`.

## If you depended on Keel pulling in Plumb

It no longer does. Declare Plumb yourself if you use it; the two versions are
now independent.

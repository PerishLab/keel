# keel

Data model description engine. Business code defines **resources, fields, and
relations only**. Control fields and control capabilities stay inside the
engine. HTTP and store adaptors project the engine surface — they are not a
business authoring API.

Canonical source: [PerishLab/keel](https://git.perish.top/PerishLab/keel).

## Cold start

```rust
use keel::adapt::db::Sqlite;
use keel::atom::string;
use keel::resource;
use keel::{Graph, bind};

#[resource]
struct Course {
    #[field(string)]
    code: string,
    #[field(string)]
    title: string,
}

#[resource]
struct Student {
    #[field(string)]
    no: string,
    #[field(string)]
    name: string,
    #[relation(Course, many2many)]
    courses: Course,
}

fn main() {
    let mut graph = Graph::new();
    graph.plug::<Course>().plug::<Student>();
    let core = bind(graph, Sqlite::memory()).expect("bind");
    let _id = core
        .put("Student", &[("no", "S01"), ("name", "ada")])
        .expect("put");
}
```

## Engine face (`Core`)

| Method | Meaning |
|--------|---------|
| `put` / `set` / `live` / `end` | insert, partial update, live list, soft-end |
| `unset` | clear explicitly optional scalar or single-relation fields to null |
| `query` / `ask` | text → AST → run; `ask` takes `Tree` directly |
| AST | `from` + where + `link` + order/page → always **pack** |
| `tie` / `ties` / `cut` | many2many write on Core; read via `link` bond bags (H0) |
| edge law | flat pack; root-only order/page — `docs/model/edge.md` |

Scalar fields are required by default. `#[field(string, opt)]` makes absence a
real null state: omit it on `put`, supply a value through `set`, and clear it
explicitly through `unset`. Empty text remains ordinary business data. Query
absence with `where field is null` or `form("Unit").missing("field")`.

Field laws are schema, not runtime configuration:

```rust
#[field(string, default = "queued", values = ("done", "queued"))]
state: string,
#[field(int, opt, min = 1)]
remind_every_seconds: int,
```

`default` fills an omitted required field. `values` is a finite admitted set;
`min` and `max` are inclusive integer bounds. Keel enforces the same law in
writes, physical projection, manifests, and generation evolution.

`#[resource(frozen)]` declares an immutable-from-birth fact. It may be put and
later ended or leased, but `set`, `unset`, and set-relation mutation refuse.
Schema generations may still re-express the same frozen resource.

| `serve` / `listen` | axum (`http` feature) |

### HTTP surface

Native REST per resource (**no association queries**):

| Method | Path | Engine |
|--------|------|--------|
| `GET` | `{prefix}/health` | liveness |
| `GET` | `{prefix}/{unit}` | `live` |
| `GET` | `{prefix}/{unit}/{id}` | one live row by id |
| `POST` | `{prefix}/{unit}` | `put` |
| `PATCH` | `{prefix}/{unit}/{id}` | `set` (partial body) |
| `DELETE` | `{prefix}/{unit}/{id}` | `end` |
| `POST` | `{prefix}/{unit}/{id}/{bond}` | `tie` body `{"right": id}` |
| `DELETE` | `{prefix}/{unit}/{id}/{bond}/{tie}` | `cut` |
| `POST` | `{prefix}/query` | body `{"q":…}` → always pack |

The prefix passed to `app`/`listen` is the api prefix (default empty).

## Runtime values

Keel is a library and reads no configuration file. The caller supplies every
runtime value:

```rust
let estate = keel::config::Estate::default();
let core = keel::bind(graph, store).estate(&estate).await?;
keel::listen(core.share(), "127.0.0.1", 3000, "").await?;
```

`Estate`, `Generation`, `Cleanup`, and `Retain` are keel vocabulary; they derive
`Deserialize`, so a caller may carry them inside its own config file under its
own environment prefix. Keel never discovers that file.

```rust
custody.keep(&sudo)?;
let core = boot.seal(&sudo).await?;
```

`custody` belongs to the caller; Keel performs no file, webhook, Secret, or
vault delivery and never prints sudo.

Cleanup retention accepts `<digits>s`, `m`, `h`, or `d`; `"0s"` makes retired
generations immediately eligible and `"forever"` disables automatic
collection.

An existing nonempty namespace without an estate seal is refused by ordinary
bind. `bind(graph, store).adopt().await` is the explicit exception: it succeeds
only when the namespace exactly matches Keel's generated physical projection,
then seals it and reconstructs every permanent allocator high-water.

`bind(graph, store).hook(handler).await` registers one typed derivative cleanup
consumer. Keel commits estate GC first, durably retains logical `path + key`
events, and retries failed or unacknowledged delivery on later hooked binds.

## Operating

```sh
runseal :init
runseal :guard    # unit + scenario tests + static discipline
cargo test -p keel --test route
cargo test -p keel-gate --test forge
```

Scenario notes: `docs/run/scenario.md`.

Cold-start verification boundary: `docs/run/verify.md`.

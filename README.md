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
    #[relation(Course, n2m)]
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
| `query` / `ask` | text → AST → run; `ask` takes `Tree` directly |
| AST | `from` + where + `link` + order/page → always **pack** |
| `tie` / `ties` / `cut` | n2m write on Core; read via `link` bond bags (H0) |
| edge law | flat pack; root-only order/page — `docs/edge.md` |
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

`listen.prefix` in `keel.toml` is the api prefix (default empty).

## Runtime config (`keel.toml`)

Repo-rooted, negentropy-style. Missing file uses the same defaults:

```toml
[listen]
host = "127.0.0.1"
port = 3000
prefix = ""
# prefix = "/api"

[store]
kind = "memory"
# kind = "file"
# path = ".local/keel.sqlite"
```

```rust
let cfg = keel::config::load(".");
let store = cfg.open()?;
let core = bind(graph, store)?;
// listen(core.share(), &cfg.listen.host, cfg.listen.port).await?;
```

`keel-api [ROOT]` loads `ROOT/keel.toml` (default `ROOT=.`). CLI does not
re-express policy keys — change the file.

## Local process (sidecar)

```sh
# requires sidecar CLI installed
sidecar start --config sidecar.toml
# health: http://127.0.0.1:3000/health
sidecar stop --config sidecar.toml
```

`keel-api` is a demo binary (Student/Course selection + store/listen from keel.toml).

## Operating

```sh
runseal :init
runseal :guard    # unit tests + smoke + course scenario
runseal :smoke    # thin L2
runseal :course   # classic enroll/drop/schedule L2
cargo run -p keel-api --locked
```

Scenario notes: `docs/scenario.md`.

Cold-start verification boundary: `docs/verify.md`.

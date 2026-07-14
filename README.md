# keel

Data model description engine. Business code defines **resources, fields, and
relations only**. Control fields and control capabilities stay inside the
engine. HTTP and store adaptors project the engine surface — they are not a
business authoring API.

Canonical source: [PerishLab/keel](https://git.perish.top/PerishLab/keel).

## Cold start

```rust
use keel::adapt::db::Sqlite;
use keel::atom::{string, url};
use keel::resource;
use keel::{Graph, bind};

#[resource]
struct Class {
    #[field(string)]
    title: string,
}

#[resource]
struct Student {
    #[field(string)]
    nickname: string,
    #[field(url)]
    avatar: url,
    #[relation(Class, n2m)]
    classes: Class,
}

fn main() {
    let mut graph = Graph::new();
    graph.plug::<Class>().plug::<Student>();
    let core = bind(graph, Sqlite::memory()).expect("bind");
    let _id = core
        .put(
            "Student",
            &[("nickname", "ada"), ("avatar", "https://a.example/a")],
        )
        .expect("put");
}
```

## Engine face (`Core`)

| Method | Meaning |
|--------|---------|
| `put` / `live` / `end` | resource rows; `live` builds AST then runs |
| `query` / `ask` | text → AST → run; `ask` takes `Tree` directly |
| AST | `from` + live + scalar `where` (`=` `!=` `<` `<=` `>` `>=` `in`) + order/page |
| `tie` / `ties` / `cut` | n2m edges (Core only for now; not default REST) |
| edge law | flat multi-bag pack; root-only order/page — `docs/edge.md` |
| `serve` / `listen` | axum (`http` feature) |

### HTTP surface

Native REST per resource (**no association queries**):

| Method | Path | Engine |
|--------|------|--------|
| `GET` | `{prefix}/health` | liveness |
| `GET` | `{prefix}/{unit}` | `live` |
| `POST` | `{prefix}/{unit}` | `put` |
| `DELETE` | `{prefix}/{unit}/{id}` | `end` |
| `POST` | `{prefix}/query` | body `{"q":"from Student"}` → DSL |

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

`keel-api` is a demo binary (Student/Class + store/listen from keel.toml).

## Operating

```sh
runseal :init
runseal :guard    # unit tests + cold-start HTTP smoke
runseal :smoke    # L2 only: boot keel-api, REST + /query
cargo run -p keel-api --locked
```

Cold-start verification boundary: `docs/verify.md`.

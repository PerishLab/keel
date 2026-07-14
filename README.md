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
| `put` / `live` / `end` | resource rows; `live` = effective slice |
| `tie` / `ties` / `cut` | n2m edges; no reverse edges generated |
| `serve` / `listen` | axum on `127.0.0.1:3000` by default (`http` feature) |

HTTP maps to the same face (`DELETE` → `end`, list → `live`, no pagination).

## Local process (sidecar)

```sh
# requires sidecar CLI installed
sidecar start --config sidecar.toml
# health: http://127.0.0.1:3000/health
sidecar stop --config sidecar.toml
```

`keel-api` is a demo binary (Student/Class + memory sqlite + axum).

## Operating

```sh
runseal :init
runseal :guard
cargo run -p keel-api --locked
```

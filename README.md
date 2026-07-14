# keel

Data model description engine. Business code defines **resources, fields, and
relations only**. Control fields and control capabilities (expire, create,
update, query, migration, …) stay inside the engine and are never opened to
callers.

Canonical source: [PerishLab/keel](https://git.perish.top/PerishLab/keel).

## Cold start

Pure data layer: model graph → sealed reign → http/db adapt ports. No
capability, auth, or identity product surface yet.

```rust
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
    let _core = bind(
        graph,
        keel::adapt::http::Utopia,
        keel::adapt::db::Postgres,
    )
    .expect("bind");
}
```

## Shape

- `crates/keel` — graph, plan, sealed reign, adapt ports
- `crates/macro` — `#[resource]` / `#[field]` / `#[relation]`
- control plane (reign) is engine-only: expires, created, updated always applied in plan

## Operating

```sh
runseal :init
runseal :guard
```

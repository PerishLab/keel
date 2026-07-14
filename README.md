# keel

Data model description engine. Business code defines **resources, fields, and
relations only**. Control fields and control capabilities (expire, create,
update, query, migration, …) stay inside the engine and are never opened to
callers.

Canonical source: [PerishLab/keel](https://git.perish.top/PerishLab/keel).

## Cold start

Pure data layer: model graph → sealed reign → http/db adapt ports. No
capability, auth, or identity product surface yet.

The first db adapt is **sqlite** (in-process, closed loop). Other stores
(e.g. postgres) are later, independent adapt implementations — not cold-start
infrastructure.

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
    let db = keel::adapt::db::Sqlite::memory();
    let _core = bind(graph, keel::adapt::http::Utopia, &db).expect("bind");
}
```

`Sqlite::wire` opens an in-memory (or file) database and applies engine DDL:
resource tables, n2m join tables, and reign columns (`id`, `expires_at`,
`created_at`, `updated_at`).

Engine-internal row lifecycle (adapt surface, not business API):

- `put` — insert business cells; engine stamps created/updated, expires null
- `live` — rows in the effective slice (`expires_at` null or in the future)
- `end` — set `expires_at` to now (soft end via reign, not a business delete field)

No business create/update/query/migration API.

## Shape

- `crates/keel` — graph, plan, ddl, sealed reign, adapt ports
- `crates/macro` — `#[resource]` / `#[field]` / `#[relation]`
- control plane (reign) is engine-only and always present in DDL
- `adapt::db::Sqlite` — closed-loop db adapt (`memory` / `file`)

## Operating

```sh
runseal :init
runseal :guard
```

use axum::extract::{Request, State};
use axum::middleware::{self, Next};
use axum::response::Response;
use keel::adapt::db::Sqlite;
use keel::atom::{int, string};
use keel::config;
use keel::resource;
use keel::{Cell, Core, Graph, Operator, app, bind};
use std::env;
use std::path::Path;
use std::sync::Arc;

#[resource]
struct Actor {
    #[field(string, unique)]
    login: string,
    #[relation(Repo, many2many)]
    stars: Repo,
}

#[resource]
struct Repo {
    #[field(string, unique = owner)]
    name: string,
    #[field(string)]
    visibility: string,
    #[relation(Actor, many2one, root)]
    owner: Actor,
}

#[resource]
struct Issue {
    #[field(string)]
    title: string,
    #[field(serial, scope = repo)]
    index: int,
    #[field(bool)]
    closed: bool,
    #[relation(Repo, many2one, root)]
    repo: Repo,
    #[relation(Actor, many2one)]
    author: Actor,
}

#[tokio::main]
async fn main() {
    let root = env::args().nth(1).unwrap_or_else(|| ".".into());
    let cfg = config::load(Path::new(&root));
    let store = match cfg.open() {
        Ok(store) => store,
        Err(err) => {
            eprintln!("forge: config: {err}");
            std::process::exit(1);
        }
    };
    let mut graph = Graph::new();
    graph.plug::<Actor>().plug::<Repo>().plug::<Issue>();
    let core = match bind(graph, store) {
        Ok(core) => core.share(),
        Err(err) => {
            eprintln!("forge: bind: {err}");
            std::process::exit(1);
        }
    };
    let router = app(core.clone(), &cfg.listen.prefix)
        .layer(middleware::from_fn_with_state(core.clone(), gate));
    let addr = format!("{}:{}", cfg.listen.host, cfg.listen.port);
    let bound = match tokio::net::TcpListener::bind(&addr).await {
        Ok(bound) => bound,
        Err(err) => {
            eprintln!("forge: bind {addr}: {err}");
            std::process::exit(1);
        }
    };
    eprintln!("keel: ready on http://{addr}");
    if let Err(err) = axum::serve(bound, router).await {
        eprintln!("forge: {err}");
        std::process::exit(1);
    }
}

async fn gate(State(core): State<Arc<Core<Sqlite>>>, mut req: Request, next: Next) -> Response {
    let login = req
        .headers()
        .get("x-login")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    if let Some(login) = login
        && let Some(key) = whom(&core, &login)
    {
        req.extensions_mut().insert(Operator(key));
    }
    next.run(req).await
}

fn whom(core: &Core<Sqlite>, login: &str) -> Option<i64> {
    let rows = core.live("Actor").ok()?;
    rows.iter()
        .find(|row| row.cells().get("login").map(Cell::text) == Some(login))
        .map(|row| row.key())
}

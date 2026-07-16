use keel::atom::{int, string};
use keel::config;
use keel::resource;
use keel::{Graph, bind, listen};
use std::env;
use std::path::Path;

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
    #[relation(Actor, many2one)]
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
    #[relation(Repo, many2one)]
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
    if let Err(err) = listen(core, &cfg.listen.host, cfg.listen.port, &cfg.listen.prefix).await {
        eprintln!("forge: {err}");
        std::process::exit(1);
    }
}

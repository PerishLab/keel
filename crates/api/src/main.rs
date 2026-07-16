use keel::atom::string;
use keel::config;
use keel::resource;
use keel::{Graph, bind, listen};
use std::env;
use std::path::Path;

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
    #[relation(Course, many2many, grade = string)]
    courses: Course,
}

#[tokio::main]
async fn main() {
    let root = env::args().nth(1).unwrap_or_else(|| ".".into());
    let cfg = config::load(Path::new(&root));
    let store = match cfg.open() {
        Ok(store) => store,
        Err(err) => {
            eprintln!("keel-api: config: {err}");
            std::process::exit(1);
        }
    };
    let mut graph = Graph::new();
    graph.plug::<Course>().plug::<Student>();
    let core = match bind(graph, store) {
        Ok(core) => core.share(),
        Err(err) => {
            eprintln!("keel-api: bind: {err}");
            std::process::exit(1);
        }
    };
    if let Err(err) = listen(core, &cfg.listen.host, cfg.listen.port, &cfg.listen.prefix).await {
        eprintln!("keel-api: {err}");
        std::process::exit(1);
    }
}

use keel::adapt::db::Sqlite;
use keel::atom::{string, url};
use keel::resource;
use keel::{Graph, bind, serve};

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

#[tokio::main]
async fn main() {
    let mut graph = Graph::new();
    graph.plug::<Class>().plug::<Student>();
    let core = bind(graph, Sqlite::memory()).expect("bind").share();
    if let Err(err) = serve(core).await {
        eprintln!("keel-api: {err}");
        std::process::exit(1);
    }
}

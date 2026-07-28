use keel::adapt::db::Sqlite;
use keel::atom::{int, string};
use keel::{Cell, Graph, bind, resource};

#[resource]
struct Job {
    #[field(string, default = "queued", values = ("done", "queued"))]
    state: string,
    #[field(int, default = 0, min = 0, max = 2)]
    attempts: int,
    #[field(int, opt, min = 1)]
    reminder: int,
}

#[resource]
struct Broken {
    #[field(int, opt, default = 1)]
    count: int,
}

#[tokio::test]
async fn field() {
    let mut graph = Graph::new();
    graph.plug::<Job>();
    let core = bind(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind");

    let key = core.put("Job", &[]).await.expect("defaults");
    let row = core.live("Job").await.expect("live").remove(0);
    assert_eq!(row.cell("state"), Some(&Cell::Text("queued".into())));
    assert_eq!(row.cell("attempts"), Some(&Cell::Int(0)));
    assert!(!row.cells().contains_key("reminder"));

    assert!(core.put("Job", &[("state", "unknown")]).await.is_err());
    assert!(core.set("Job", key, &[("attempts", "3")]).await.is_err());
    assert!(core.set("Job", key, &[("reminder", "0")]).await.is_err());
    core.set(
        "Job",
        key,
        &[("state", "done"), ("attempts", "2"), ("reminder", "1")],
    )
    .await
    .expect("valid");
}

#[tokio::test]
async fn shape() {
    let mut graph = Graph::new();
    graph.plug::<Broken>();
    match bind(graph, Sqlite::memory().await.expect("db")).await {
        Err(err) => assert!(err.to_string().contains("optional field")),
        Ok(_) => panic!("expected optional default refusal"),
    }
}

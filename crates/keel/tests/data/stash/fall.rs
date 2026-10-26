use keel::adapt::db::Sqlite;
use keel::atom::string;
use keel::resource;
use keel::{Core, Graph};
use std::time::{SystemTime, UNIX_EPOCH};

#[resource]
struct Org {
    #[field(string)]
    name: string,
}

#[resource]
struct Repo {
    #[field(string)]
    name: string,
    #[relation(Org, many2one, root)]
    org: Org,
}

#[resource]
struct Commit {
    #[field(string)]
    name: string,
    #[relation(Repo, many2one, root)]
    repo: Repo,
}

const UNITS: [&str; 3] = ["Org", "Repo", "Commit"];

fn tick() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

async fn grow() -> (Core<Sqlite>, i64) {
    let mut graph = Graph::new();
    graph.plug::<Org>().plug::<Repo>().plug::<Commit>();
    let core = crate::support::boot(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind");
    let org = core.put("Org", &[("name", "lab")]).await.expect("org");
    let repo = core
        .put("Repo", &[("name", "keel"), ("org", &org.to_string())])
        .await
        .expect("repo");
    core.put("Commit", &[("name", "c"), ("repo", &repo.to_string())])
        .await
        .expect("commit");
    (core, org)
}

async fn sizes(core: &Core<Sqlite>) -> Vec<usize> {
    let mut out = Vec::new();
    for unit in UNITS {
        let live = core.live(unit).await.expect("live").len();
        let pack = core.query(&format!("from {unit}")).await.expect("query");
        assert_eq!(live, pack.rows().len(), "{unit}");
        out.push(live);
    }
    out
}

#[tokio::test]
async fn ended() {
    let (core, org) = grow().await;
    assert_eq!(sizes(&core).await, vec![1, 1, 1]);
    core.end("Org", org).await.expect("end");
    assert_eq!(sizes(&core).await, vec![0, 0, 0]);
}

#[tokio::test]
async fn leased() {
    let (core, org) = grow().await;
    assert_eq!(sizes(&core).await, vec![1, 1, 1]);
    core.lease("Org", org, tick() + 2).await.expect("lease");
    assert_eq!(sizes(&core).await, vec![1, 1, 1]);
    std::thread::sleep(std::time::Duration::from_secs(3));
    assert_eq!(sizes(&core).await, vec![0, 0, 0]);
}

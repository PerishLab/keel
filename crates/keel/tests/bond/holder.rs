use keel::adapt::db::Sqlite;
use keel::atom::string;
use keel::resource;
use keel::{Core, Ends, Graph};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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
struct Team {
    #[field(string)]
    name: string,
    #[relation(Org, many2one, root)]
    org: Org,
    #[relation(Repo, many2many)]
    repos: Repo,
}

#[resource]
struct Pin {
    #[field(string)]
    name: string,
    #[relation(Org, many2one, root)]
    org: Org,
    #[relation(Repo, many2one)]
    repo: Repo,
}

#[resource]
struct Fan {
    #[field(string)]
    name: string,
    #[relation(Repo, many2many)]
    repos: Repo,
}

async fn boot() -> Core<Sqlite> {
    let mut graph = Graph::new();
    graph
        .plug::<Org>()
        .plug::<Repo>()
        .plug::<Team>()
        .plug::<Pin>()
        .plug::<Fan>();
    crate::support::boot(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind")
}

async fn seed(core: &Core<Sqlite>) -> (i64, i64, i64) {
    let org = core.put("Org", &[("name", "lab")]).await.expect("org");
    let o = org.to_string();
    let repo = core
        .put("Repo", &[("name", "keel"), ("org", &o)])
        .await
        .expect("repo");
    let team = core
        .put("Team", &[("name", "core"), ("org", &o)])
        .await
        .expect("team");
    let ends = Ends {
        left: team,
        right: repo,
    };
    core.tie("Team", "repos", ends, &[]).await.expect("tie");
    (org, repo, team)
}

#[tokio::test]
async fn holder() {
    let core = boot().await;
    let (_, repo, team) = seed(&core).await;
    core.end("Team", team).await.expect("end holder");
    core.end("Repo", repo).await.expect("end target");
    assert!(core.live("Repo").await.expect("live").is_empty());
}

#[tokio::test]
async fn cascade() {
    let core = boot().await;
    let (org, repo, _) = seed(&core).await;
    let (o, r) = (org.to_string(), repo.to_string());
    core.put("Pin", &[("name", "pin"), ("org", &o), ("repo", &r)])
        .await
        .expect("pin");
    core.end("Org", org).await.expect("end root");
    for unit in ["Org", "Repo", "Team", "Pin"] {
        assert!(core.live(unit).await.expect("live").is_empty(), "{unit}");
    }
}

#[tokio::test]
async fn outside() {
    let core = boot().await;
    let (org, repo, _) = seed(&core).await;
    let fan = core.put("Fan", &[("name", "ada")]).await.expect("fan");
    let ends = Ends {
        left: fan,
        right: repo,
    };
    core.tie("Fan", "repos", ends, &[]).await.expect("tie");
    let refused = core.end("Org", org).await.expect_err("held from outside");
    assert!(
        refused.to_string().contains("live ties remain"),
        "{refused}"
    );
    assert_eq!(core.live("Team").await.expect("live").len(), 1);
    assert_eq!(core.ties("Fan", "repos", fan).await.expect("ties").len(), 1);
}

#[tokio::test]
async fn leased() {
    let core = boot().await;
    let (_, repo, team) = seed(&core).await;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    core.lease("Team", team, now + 2).await.expect("lease");
    let held = core.end("Repo", repo).await.expect_err("holder still live");
    assert!(held.to_string().contains("live ties remain"), "{held}");
    tokio::time::sleep(Duration::from_secs(3)).await;
    core.end("Repo", repo).await.expect("end target");
}

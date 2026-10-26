use keel::adapt::Error;
use keel::adapt::db::Sqlite;
use keel::atom::string;
use keel::ddl::Grain;
use keel::resource;
use keel::wire::{Val, Wire};
use keel::{Core, Ends, Graph};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

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
struct Branch {
    #[field(string)]
    name: string,
    #[relation(Repo, many2one, root)]
    repo: Repo,
}

#[resource]
struct Commit {
    #[field(string)]
    name: string,
    #[relation(Branch, many2one, root)]
    branch: Branch,
}

#[resource]
struct Note {
    #[field(string)]
    name: string,
    #[relation(Commit, many2one)]
    commit: Commit,
}

#[resource]
struct Star {
    #[field(string)]
    name: string,
    #[relation(Branch, many2many)]
    branches: Branch,
}

const UNITS: [&str; 4] = ["Org", "Repo", "Branch", "Commit"];

struct Count<W> {
    wire: W,
    seen: Arc<AtomicUsize>,
}

impl<W: Wire> Wire for Count<W> {
    fn grain(&self) -> Grain {
        self.wire.grain()
    }

    async fn run(&mut self, sql: &str, args: &[Val]) -> Result<u64, Error> {
        self.seen.fetch_add(1, Ordering::SeqCst);
        self.wire.run(sql, args).await
    }

    async fn plant(&mut self, sql: &str, args: &[Val]) -> Result<i64, Error> {
        self.seen.fetch_add(1, Ordering::SeqCst);
        self.wire.plant(sql, args).await
    }

    async fn rows(&mut self, sql: &str, args: &[Val]) -> Result<Vec<Vec<Val>>, Error> {
        self.seen.fetch_add(1, Ordering::SeqCst);
        self.wire.rows(sql, args).await
    }

    async fn script(&mut self, sql: &str) -> Result<(), Error> {
        self.seen.fetch_add(1, Ordering::SeqCst);
        self.wire.script(sql).await
    }
}

fn graph() -> Graph {
    let mut graph = Graph::new();
    graph
        .plug::<Org>()
        .plug::<Repo>()
        .plug::<Branch>()
        .plug::<Commit>()
        .plug::<Note>()
        .plug::<Star>();
    graph
}

struct Lab<W: Wire> {
    core: Core<W>,
}

async fn boot() -> Lab<Sqlite> {
    lab(Sqlite::memory().await.expect("db")).await
}

async fn lab<W: Wire>(wire: W) -> Lab<W> {
    let core = crate::support::boot(graph(), wire).await.expect("bind");
    Lab { core }
}

impl<W: Wire> Lab<W> {
    async fn grow(&self, name: &str, width: usize) -> (i64, Vec<i64>) {
        let org = self.core.put("Org", &[("name", name)]).await.expect("org");
        let mut branches = Vec::new();
        for _ in 0..width {
            let repo = self
                .core
                .put("Repo", &[("name", "repo"), ("org", &org.to_string())])
                .await
                .expect("repo");
            branches.extend(self.fork(repo, width).await);
        }
        (org, branches)
    }

    async fn fork(&self, repo: i64, width: usize) -> Vec<i64> {
        let mut out = Vec::new();
        for _ in 0..width {
            let branch = self
                .core
                .put("Branch", &[("name", "main"), ("repo", &repo.to_string())])
                .await
                .expect("branch");
            self.seal(branch, width).await;
            out.push(branch);
        }
        out
    }

    async fn seal(&self, branch: i64, width: usize) -> i64 {
        let mut last = 0;
        for _ in 0..width {
            last = self
                .core
                .put("Commit", &[("name", "c"), ("branch", &branch.to_string())])
                .await
                .expect("commit");
        }
        last
    }

    async fn star(&self, branch: i64) -> i64 {
        let star = self
            .core
            .put("Star", &[("name", "ada")])
            .await
            .expect("star");
        let ends = Ends {
            left: star,
            right: branch,
        };
        self.core
            .tie("Star", "branches", ends, &[])
            .await
            .expect("tie");
        star
    }

    async fn sizes(&self) -> Vec<usize> {
        let mut out = Vec::new();
        for unit in UNITS {
            out.push(self.core.live(unit).await.expect("live").len());
        }
        out
    }
}

#[tokio::test]
async fn deep() {
    let lab = boot().await;
    let (org, _) = lab.grow("lab", 3).await;
    lab.grow("other", 2).await;
    lab.core.end("Org", org).await.expect("end root");
    assert_eq!(lab.sizes().await, vec![1, 2, 4, 8]);
}

#[tokio::test]
async fn referred() {
    let lab = boot().await;
    let (org, branches) = lab.grow("lab", 2).await;
    let commit = lab.seal(branches[branches.len() - 1], 1).await.to_string();
    lab.core
        .put("Note", &[("name", "see"), ("commit", &commit)])
        .await
        .expect("note");
    let refused = lab
        .core
        .end("Org", org)
        .await
        .expect_err("referred from outside");
    assert!(
        refused.to_string().contains("live ties remain"),
        "{refused}"
    );
    assert_eq!(lab.sizes().await, vec![1, 2, 4, 9]);
}

#[tokio::test]
async fn starred() {
    let lab = boot().await;
    let (org, branches) = lab.grow("lab", 2).await;
    lab.star(branches[0]).await;
    let refused = lab
        .core
        .end("Org", org)
        .await
        .expect_err("tied from outside");
    assert!(
        refused.to_string().contains("live ties remain"),
        "{refused}"
    );
    assert_eq!(lab.sizes().await, vec![1, 2, 4, 8]);
}

#[tokio::test]
async fn unstarred() {
    let lab = boot().await;
    let (org, branches) = lab.grow("lab", 2).await;
    let star = lab.star(branches[0]).await;
    lab.core.end("Star", star).await.expect("end holder");
    lab.core.end("Org", org).await.expect("end root");
    assert_eq!(lab.sizes().await, vec![0, 0, 0, 0]);
}

async fn spent(width: usize) -> usize {
    let seen = Arc::new(AtomicUsize::new(0));
    let wire = Count {
        wire: Sqlite::memory().await.expect("db"),
        seen: seen.clone(),
    };
    let lab = lab(wire).await;
    let (org, _) = lab.grow("lab", width).await;
    seen.store(0, Ordering::SeqCst);
    lab.core.end("Org", org).await.expect("end root");
    seen.load(Ordering::SeqCst)
}

#[tokio::test]
async fn flat() {
    let few = spent(1).await;
    let many = spent(6).await;
    assert_eq!(
        few, many,
        "1 row per layer took {few}, 6 per parent took {many}"
    );
}

use keel::adapt::db::Sqlite;
use keel::atom::string;
use keel::ddl::Grain;
use keel::resource;
use keel::wire::{Val, Wire};
use keel::{Graph, bind};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[resource]
pub struct Org {
    #[field(string)]
    name: string,
}

#[resource]
pub struct Repo {
    #[field(string)]
    name: string,
    #[relation(Org, many2one, root)]
    org: Org,
}

#[derive(Default)]
pub struct Tally {
    pub reads: AtomicUsize,
    pub grants: AtomicUsize,
}

pub struct Count<I: Wire> {
    inner: I,
    tally: Arc<Tally>,
}

impl<I: Wire> Wire for Count<I> {
    fn grain(&self) -> Grain {
        self.inner.grain()
    }
    async fn run(&mut self, sql: &str, args: &[Val]) -> Result<u64, keel::adapt::Error> {
        self.inner.run(sql, args).await
    }
    async fn plant(&mut self, sql: &str, args: &[Val]) -> Result<i64, keel::adapt::Error> {
        self.inner.plant(sql, args).await
    }
    async fn rows(&mut self, sql: &str, args: &[Val]) -> Result<Vec<Vec<Val>>, keel::adapt::Error> {
        self.tally.reads.fetch_add(1, Ordering::Relaxed);
        if sql.contains("grant") {
            self.tally.grants.fetch_add(1, Ordering::Relaxed);
        }
        self.inner.rows(sql, args).await
    }
    async fn script(&mut self, sql: &str) -> Result<(), keel::adapt::Error> {
        self.inner.script(sql).await
    }
}

async fn stand(graph: Graph, tally: Arc<Tally>) -> keel::Core<Count<Sqlite>> {
    let wire = Count {
        inner: Sqlite::memory().await.expect("db"),
        tally,
    };
    bind(graph, wire).await.expect("bind")
}

#[tokio::test]
async fn flat() {
    let tally = Arc::new(Tally::default());
    let mut graph = Graph::new();
    graph.plug::<Org>();
    let core = stand(graph, tally.clone()).await;
    for i in 0..40 {
        core.put("Org", &[("name", &format!("o{i}")[..])])
            .await
            .expect("put");
    }
    core.put(
        "@grant",
        &[
            ("who", "all"),
            ("verb", "see"),
            ("unit", "Org"),
            ("scope", r#"pred name != "zzz""#),
        ],
    )
    .await
    .expect("grant");

    tally.reads.store(0, Ordering::Relaxed);
    tally.grants.store(0, Ordering::Relaxed);
    let pack = core.of(1).query("from Org").await.expect("q");

    assert_eq!(pack.rows().len(), 40);
    assert_eq!(
        tally.grants.load(Ordering::Relaxed),
        1,
        "the grant table is read once per operation, not once per row"
    );
    assert_eq!(
        tally.reads.load(Ordering::Relaxed),
        2,
        "one row scan plus one grant scan; a read per row is the regression this pins"
    );
}

#[tokio::test]
async fn bared() {
    let tally = Arc::new(Tally::default());
    let mut graph = Graph::new();
    graph.plug::<Org>();
    let wire = Count {
        inner: Sqlite::memory().await.expect("db"),
        tally: tally.clone(),
    };
    let core = bind(graph, wire).await.expect("bind").bare();
    core.put("Org", &[("name", "lab")]).await.expect("org");
    core.put(
        "@grant",
        &[
            ("who", "all"),
            ("verb", "see"),
            ("unit", "Org"),
            ("scope", "all"),
        ],
    )
    .await
    .expect("grant");

    tally.grants.store(0, Ordering::Relaxed);
    core.of(1).query("from Org").await.expect("one");
    let first = tally.grants.load(Ordering::Relaxed);
    core.of(1).query("from Org").await.expect("two");
    let second = tally.grants.load(Ordering::Relaxed);

    assert!(first > 0);
    assert_eq!(
        second - first,
        first,
        "bare() must hold no authority: a second op re-reads the grants"
    );
}

#[tokio::test]
async fn rooted() {
    let tally = Arc::new(Tally::default());
    let mut graph = Graph::new();
    graph.plug::<Org>().plug::<Repo>();
    let core = stand(graph, tally.clone()).await;
    let org = core.put("Org", &[("name", "lab")]).await.expect("org");
    for i in 0..40 {
        core.put(
            "Repo",
            &[
                ("name", &format!("r{i}")[..]),
                ("org", &org.to_string()[..]),
            ],
        )
        .await
        .expect("put");
    }
    core.put(
        "@grant",
        &[
            ("who", "all"),
            ("verb", "see"),
            ("unit", "Org"),
            ("scope", r#"pred name = "lab""#),
        ],
    )
    .await
    .expect("grant");

    tally.reads.store(0, Ordering::Relaxed);
    tally.grants.store(0, Ordering::Relaxed);
    let pack = core.of(1).query("from Repo").await.expect("q");

    assert_eq!(pack.rows().len(), 40);
    assert_eq!(tally.grants.load(Ordering::Relaxed), 1);
    assert_eq!(
        tally.reads.load(Ordering::Relaxed),
        42,
        "one row scan plus one root-chain read per row; the grant scan is hoisted"
    );
}

#[tokio::test]
async fn fresh() {
    let tally = Arc::new(Tally::default());
    let mut graph = Graph::new();
    graph.plug::<Org>();
    let core = stand(graph, tally.clone()).await;
    core.put("Org", &[("name", "lab")]).await.expect("org");

    async fn seen(core: &keel::Core<Count<Sqlite>>) -> usize {
        core.of(1).query("from Org").await.expect("q").rows().len()
    }
    assert_eq!(seen(&core).await, 0);

    let deed = core
        .put(
            "@grant",
            &[
                ("who", "all"),
                ("verb", "see"),
                ("unit", "Org"),
                ("scope", "all"),
            ],
        )
        .await
        .expect("grant");
    assert_eq!(seen(&core).await, 1);

    core.end("@grant", deed).await.expect("revoke");
    assert_eq!(seen(&core).await, 0);
}

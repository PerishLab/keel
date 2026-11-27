use super::*;
use crate::{Core, Graph, Resource, bootstrap};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::Duration;
use tokio::sync::oneshot;

struct Item;

impl Resource for Item {
    fn name() -> &'static str {
        "Item"
    }
    fn spec() -> crate::spec::Spec {
        crate::spec::Spec::build("Item")
            .field("name", crate::atom::Kind::Text)
            .seal()
    }
}

enum Edge {
    Begin,
    Commit,
    Rollback,
}

struct Gate {
    armed: Arc<AtomicBool>,
    entered: oneshot::Receiver<()>,
    release: mpsc::Sender<()>,
}

async fn fixture(edge: Edge) -> (Core<Sqlite>, Gate) {
    let mut wire = Sqlite::memory().await.expect("wire");
    let armed = Arc::new(AtomicBool::new(false));
    let flag = armed.clone();
    let (tx, entered) = oneshot::channel();
    let (release, rx) = mpsc::channel();
    let mut tx = Some(tx);
    let mut wait = move || {
        if flag.swap(false, Ordering::SeqCst) {
            tx.take()
                .expect("one rendezvous")
                .send(())
                .expect("entered");
            rx.recv_timeout(Duration::from_secs(3)).expect("release");
        }
    };
    let mut handle = wire.conn.lock_handle().await.expect("handle");
    match edge {
        Edge::Begin => handle.set_progress_handler(1, move || {
            wait();
            true
        }),
        Edge::Commit => handle.set_commit_hook(move || {
            wait();
            true
        }),
        Edge::Rollback => handle.set_rollback_hook(wait),
    }
    drop(handle);
    let mut graph = Graph::new();
    graph.plug::<Item>();
    let mut boot = bootstrap(graph, wire).expect("bootstrap");
    let token = boot.mint().await.expect("mint");
    let core = boot.seal(&token).await.expect("seal");
    (
        core,
        Gate {
            armed,
            entered,
            release,
        },
    )
}

async fn cancel<F: std::future::Future>(future: F, gate: Gate) {
    let mut future = Box::pin(future);
    gate.armed.store(true, Ordering::SeqCst);
    tokio::select! {
        _ = &mut future => panic!("operation completed before rendezvous"),
        entered = tokio::time::timeout(Duration::from_secs(2), gate.entered) => {
            entered.expect("bounded rendezvous").expect("native worker entered");
        }
    }
    drop(future);
    gate.release.send(()).expect("release worker");
}

async fn check(core: &Core<Sqlite>, rows: usize) {
    assert_eq!(
        core.query("from Item")
            .await
            .expect("recovered query")
            .rows()
            .len(),
        rows
    );
    assert_eq!(
        core.query("from Item")
            .await
            .expect("cached query")
            .rows()
            .len(),
        rows
    );
    core.put("Item", &[("name", "next")])
        .await
        .expect("next write");
    assert_eq!(
        core.query("from Item")
            .await
            .expect("updated query")
            .rows()
            .len(),
        rows + 1
    );
}

#[tokio::test]
async fn begin() {
    let (core, gate) = fixture(Edge::Begin).await;
    cancel(core.put("Item", &[("name", "cancelled")]), gate).await;
    check(&core, 0).await;
}

#[tokio::test]
async fn commit() {
    let (core, gate) = fixture(Edge::Commit).await;
    let work = core.batch(async |tx| {
        tx.put("Item", &[("name", "committed")]).await?;
        assert_eq!(tx.query("from Item").await?.rows().len(), 1);
        Ok(())
    });
    cancel(work, gate).await;
    check(&core, 1).await;
}

#[tokio::test]
async fn rollback() {
    let (core, gate) = fixture(Edge::Rollback).await;
    let work = core.batch(async |tx| {
        tx.put("Item", &[("name", "rolled back")]).await?;
        assert_eq!(tx.query("from Item").await?.rows().len(), 1);
        Err::<(), _>(Error::Adapt("rollback".to_string()))
    });
    cancel(work, gate).await;
    check(&core, 0).await;
}

#[tokio::test]
async fn body() {
    let (core, _) = fixture(Edge::Rollback).await;
    let work = core.batch(async |tx| {
        tx.put("Item", &[("name", "cancelled")]).await?;
        assert_eq!(tx.query("from Item").await?.rows().len(), 1);
        std::future::pending::<()>().await;
        Ok(())
    });
    assert!(
        tokio::time::timeout(Duration::from_millis(50), work)
            .await
            .is_err()
    );
    check(&core, 0).await;
}

#[tokio::test]
async fn strict() {
    let mut wire = Sqlite::memory().await.expect("wire");
    wire.script("BEGIN").await.expect("begin");
    assert!(wire.script("BEGIN").await.is_err());
    wire.script("ROLLBACK").await.expect("rollback");
    wire.script("BEGIN").await.expect("usable");
    wire.script("COMMIT").await.expect("commit");
}

#[tokio::test]
async fn custody() {
    let (core, _) = fixture(Edge::Rollback).await;
    core.put("Item", &[("name", "retained")])
        .await
        .expect("retained");
    assert!(core.of(7).live("Item").await.expect("before").is_empty());
    let work = core.batch(async |tx| {
        tx.grant(crate::Grant::new("all", "see", "Item", "all"))
            .await?;
        assert_eq!(tx.query("from Item").await?.rows().len(), 1);
        std::future::pending::<()>().await;
        Ok(())
    });
    assert!(
        tokio::time::timeout(Duration::from_millis(50), work)
            .await
            .is_err()
    );
    assert!(
        core.of(7)
            .live("Item")
            .await
            .expect("recovered custody")
            .is_empty()
    );
    check(&core, 1).await;
}

async fn pair() -> (tempfile::TempDir, Sqlite, Sqlite) {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("keel.sqlite");
    let mut first = Sqlite::file(&path).await.expect("first");
    first
        .script("CREATE TABLE note (id INTEGER PRIMARY KEY, body TEXT NOT NULL)")
        .await
        .expect("table");
    let second = Sqlite::file(&path).await.expect("second");
    (dir, first, second)
}

async fn append(wire: &mut Sqlite, body: &str) -> Result<(), Error> {
    wire.script("BEGIN").await?;
    let out = async {
        wire.rows("SELECT count(*) FROM note", &[]).await?;
        wire.run(
            "INSERT INTO note (body) VALUES (?)",
            &[Val::Text(body.into())],
        )
        .await?;
        wire.script("COMMIT").await
    }
    .await;
    if out.is_err() {
        let _ = wire.script("ROLLBACK").await;
    }
    out
}

#[tokio::test]
async fn upgrade() {
    let (_dir, mut first, mut second) = pair().await;
    first.script("BEGIN").await.expect("begin");
    first
        .run("INSERT INTO note (body) VALUES ('held')", &[])
        .await
        .expect("held");
    let late = tokio::spawn(async move {
        let out = append(&mut second, "late").await;
        (second, out)
    });
    tokio::time::sleep(Duration::from_millis(200)).await;
    first.script("COMMIT").await.expect("commit");
    let (mut second, out) = tokio::time::timeout(Duration::from_secs(10), late)
        .await
        .expect("bounded")
        .expect("joined");
    out.expect("a read-then-write batch waits for the writer instead of failing with code 5");
    let rows = second
        .rows("SELECT body FROM note ORDER BY id", &[])
        .await
        .expect("rows");
    assert_eq!(rows.len(), 2);
}

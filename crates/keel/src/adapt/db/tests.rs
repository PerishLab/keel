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
        tx.put(
            "@grant",
            &[
                ("who", "all"),
                ("verb", "see"),
                ("unit", "Item"),
                ("scope", "all"),
            ],
        )
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

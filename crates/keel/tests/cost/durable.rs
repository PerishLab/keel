use crate::world::*;
use crate::write::{each, open};
use keel::adapt::db::Sqlite;
use keel::{Core, Graph};
use std::path::{Path, PathBuf};
use std::time::Instant;

fn seat(count: usize) -> PathBuf {
    std::env::temp_dir().join(format!("keel-cost-{}-{count}.db", std::process::id()))
}

async fn disk(path: &Path) -> Core<Sqlite> {
    let mut graph = Graph::new();
    graph.plug::<Flat>().plug::<Deep>().plug::<Deeper>();
    crate::support::boot(graph, Sqlite::file(path).await.expect("db"))
        .await
        .expect("bind")
}

async fn memory() -> Core<Sqlite> {
    let mut graph = Graph::new();
    graph.plug::<Flat>().plug::<Deep>().plug::<Deeper>();
    crate::support::boot(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind")
}

async fn alone(core: &Core<Sqlite>, count: usize, note: &str) -> f64 {
    let face = core.of(1);
    let started = Instant::now();
    for spot in 0..count {
        let held = format!("{note} {spot}");
        face.put("Flat", &[("note", held.as_str())])
            .await
            .expect("put");
    }
    each(started, count)
}

async fn together(core: &Core<Sqlite>, count: usize) -> f64 {
    let face = core.of(1);
    let started = Instant::now();
    face.batch(async |tx| {
        for spot in 0..count {
            let held = format!("batch {spot}");
            tx.put("Flat", &[("note", held.as_str())]).await?;
        }
        Ok(())
    })
    .await
    .expect("batch");
    each(started, count)
}

#[tokio::test]
#[ignore]
async fn fsync() {
    for count in [100usize, 400] {
        let held = seat(count);
        let _ = std::fs::remove_file(&held);
        let core = memory().await;
        open(&core).await;
        let bare = alone(&core, count, "memory").await;
        let core = disk(&held).await;
        open(&core).await;
        let kept = alone(&core, count, "disk").await;
        let grouped = together(&core, count).await;
        drop(core);
        let _ = std::fs::remove_file(&held);
        println!(
            "{count:>4} rows  memory {bare:.2}ms  durable {kept:.2}ms  batched {grouped:.2}ms  durability {:.2}ms",
            kept - grouped
        );
    }
}

#[path = "support/mod.rs"]
mod support;

use std::fs;
use std::sync::Arc;
use std::time::Duration;
use support::{batch, everything, store, temp};

const FAR: u64 = u64::MAX / 2;

#[tokio::test]
async fn idempotent() {
    let home = temp("idempotent");
    let (_core, store) = store(&home).await;
    store.append("t1", batch(1, 1)).await.expect("first");
    store.append("t1", batch(1, 1)).await.expect("again");
    assert_eq!(everything(&store).await.len(), 3);
    store.seal(FAR).await.expect("seal");
    store.append("t1", batch(1, 1)).await.expect("after seal");
    store.append("t2", batch(2, 1)).await.expect("second");
    assert_eq!(everything(&store).await.len(), 6);
}

#[tokio::test]
async fn persisted() {
    let home = temp("persisted");
    let expected = {
        let (_core, store) = store(&home).await;
        store.append("t1", batch(1, 1)).await.expect("sealed");
        store.seal(FAR).await.expect("seal");
        store.append("t2", batch(2, 1)).await.expect("open");
        everything(&store).await
    };
    let (_core, store) = store(&home).await;
    assert_eq!(everything(&store).await, expected);
    assert_eq!(expected.len(), 6);
}

#[tokio::test]
async fn torn() {
    let home = temp("torn");
    {
        let (_core, store) = store(&home).await;
        store.append("t1", batch(1, 1)).await.expect("first");
        store.append("t2", batch(2, 1)).await.expect("second");
    }
    let segment = files(&home.join("column/open"))[0].clone();
    let length = fs::metadata(&segment).expect("segment").len();
    fs::OpenOptions::new()
        .write(true)
        .open(&segment)
        .and_then(|file| file.set_len(length - 3))
        .expect("tear");
    let (_core, store) = store(&home).await;
    let found = everything(&store).await;
    assert_eq!(found, vec!["r1-0", "r1-1", "r1-2"]);
    store.append("t3", batch(3, 1)).await.expect("after tear");
    assert_eq!(everything(&store).await.len(), 6);
}

#[tokio::test]
async fn orphans() {
    let home = temp("orphans");
    let part = {
        let (_core, store) = store(&home).await;
        store.append("t1", batch(1, 1)).await.expect("append");
        store.seal(FAR).await.expect("seal");
        files(&home.join("column/parts"))[0].clone()
    };
    let directory = part.parent().expect("scope").to_path_buf();
    fs::write(directory.join("00000000000000000099.parquet"), b"orphan").expect("orphan");
    fs::write(
        directory.join("00000000000000000098.parquet.staging"),
        b"staging",
    )
    .expect("staging");
    let (_core, store) = store(&home).await;
    assert_eq!(files(&home.join("column/parts")), vec![part]);
    assert_eq!(everything(&store).await.len(), 3);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn retained() {
    let home = temp("retained");
    let (_core, store) = store(&home).await;
    let store = Arc::new(store);
    store.append("old", batch(1, 1)).await.expect("old");
    store.append("new", batch(2, 2)).await.expect("new");
    store.seal(FAR).await.expect("seal");
    let reader = store.clone();
    let scan = tokio::spawn(async move {
        let mut found = 0;
        reader
            .scan(&keel_column::Filter::default(), &mut |_| {
                if found == 0 {
                    std::thread::sleep(Duration::from_millis(300));
                }
                found += 1;
                Ok(())
            })
            .await
            .map(|()| found)
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    store.retain(2, 0).await.expect("retain");
    store.sweep(0).await.expect("sweep within grace");
    assert_eq!(scan.await.expect("join").expect("scan in flight"), 6);
    assert_eq!(everything(&store).await, vec!["r2-0", "r2-1", "r2-2"]);
    assert_eq!(files(&home.join("column/parts")).len(), 2);
    store.sweep(FAR).await.expect("sweep");
    assert_eq!(files(&home.join("column/parts")).len(), 1);
}

#[tokio::test]
async fn exclusive() {
    let home = temp("exclusive");
    let (core, _store) = store(&home).await;
    let second = keel_column::Store::open(core, support::table(), &home.join("column")).await;
    assert!(second.is_err());
}

fn files(area: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut found: Vec<_> = fs::read_dir(area)
        .expect("area")
        .flat_map(|scope| fs::read_dir(scope.expect("scope").path()).expect("files"))
        .map(|file| file.expect("file").path())
        .collect();
    found.sort();
    found
}

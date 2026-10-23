mod cases;
#[path = "../support/mod.rs"]
mod support;

use arrow_array::{BinaryArray, RecordBatch, StringArray, UInt64Array};
use keel_column::{Filter, Query, Record};
use support::*;

fn selected() -> Filter {
    Filter {
        from: Some(2),
        to: Some(9),
        keys: vec![("trace".into(), "trace-2".into())],
    }
}

#[tokio::test]
async fn mixed() {
    let home = tempfile::tempdir().unwrap();
    let (core, store) = store(home.path()).await;
    for index in 0..6 {
        store
            .append(&index.to_string(), batch(index, 0))
            .await
            .unwrap();
    }
    store.seal(u64::MAX).await.unwrap();
    for index in 6..12 {
        store
            .append(&index.to_string(), batch(index, 0))
            .await
            .unwrap();
    }
    let mut raw = Vec::new();
    store
        .scan(&selected(), &mut |bytes| {
            raw.push(bytes.to_vec());
            Ok(())
        })
        .await
        .unwrap();
    let mut found = Vec::new();
    let mut query = Query::select(selected(), &["raw", "trace", "at"]);
    query.batch = 2;
    let stats = store
        .query(&query, &mut |batch| {
            check(&batch, &mut found);
            Ok(())
        })
        .await
        .unwrap();
    assert_eq!(raw, found);
    assert_eq!(stats.matched, 6);
    assert_eq!(stats.rows, 6);
    assert_eq!(stats.batches, 4);
    assert_eq!(stats.files, 1);
    let count = store
        .query(&Query::count(selected()), &mut |_| {
            panic!("count emitted a batch")
        })
        .await
        .unwrap();
    assert_eq!(count.matched, stats.matched);
    assert_eq!((count.rows, count.batches, count.memory), (0, 0, 0));
    drop(store);
    let reopened = keel_column::Store::open(core, table(), &home.path().join("column"))
        .await
        .unwrap();
    assert_eq!(
        reopened
            .query(&Query::count(selected()), &mut |_| unreachable!())
            .await
            .unwrap()
            .matched,
        6
    );
}

fn check(batch: &RecordBatch, found: &mut Vec<Vec<u8>>) {
    assert!(batch.num_rows() <= 2);
    assert_eq!(batch.schema().field(0).name(), "raw");
    let raw = batch
        .column(0)
        .as_any()
        .downcast_ref::<BinaryArray>()
        .unwrap();
    let trace = batch
        .column(1)
        .as_any()
        .downcast_ref::<StringArray>()
        .unwrap();
    let time = batch
        .column(2)
        .as_any()
        .downcast_ref::<UInt64Array>()
        .unwrap();
    for index in 0..batch.num_rows() {
        assert_eq!(trace.value(index), "trace-2");
        assert!((2..9).contains(&time.value(index)));
        found.push(raw.value(index).to_vec());
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn view() {
    let home = tempfile::tempdir().unwrap();
    let (_, store) = store(home.path()).await;
    let store = std::sync::Arc::new(store);
    store.append("sealed", batch(1, 0)).await.unwrap();
    store.seal(u64::MAX).await.unwrap();
    store.append("open", batch(2, 0)).await.unwrap();
    let (signal, ready) = tokio::sync::oneshot::channel();
    let (finish, written) = std::sync::mpsc::channel();
    let writer = store.clone();
    let task = tokio::spawn(async move {
        ready.await.unwrap();
        writer.append("later", batch(3, 0)).await.unwrap();
        writer.seal(u64::MAX).await.unwrap();
        writer.retain(WIDTH, 0).await.unwrap();
        finish.send(()).unwrap();
    });
    let mut signal = Some(signal);
    let query = Query::select(Filter::default(), &["at"]);
    let stats = store
        .query(&query, &mut move |_| {
            if let Some(signal) = signal.take() {
                signal.send(()).unwrap();
                written
                    .recv_timeout(std::time::Duration::from_secs(10))
                    .unwrap();
            }
            Ok(())
        })
        .await
        .unwrap();
    task.await.unwrap();
    assert_eq!((stats.matched, stats.rows), (6, 6));
    assert_eq!(
        store
            .query(&Query::count(Filter::default()), &mut |_| unreachable!())
            .await
            .unwrap()
            .matched,
        0
    );
}

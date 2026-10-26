use super::*;
use std::sync::{Arc, mpsc};
use std::time::Duration;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn captured() {
    let home = tempfile::tempdir().unwrap();
    let core = estate(home.path()).await;
    let store = Arc::new(
        Store::open(core, model(Kind::Text), &home.path().join("column"))
            .await
            .unwrap(),
    );
    store
        .write(
            "old",
            vec![
                row(1, "left", Value::Text("x".into())),
                row(10, "right", Value::Text("x".into())),
            ],
        )
        .await
        .unwrap();
    store.seal(u64::MAX).await.unwrap();
    store
        .write("open", vec![row(2, "left", Value::Text("later".into()))])
        .await
        .unwrap();
    let (signal, ready) = tokio::sync::oneshot::channel();
    let (done, finished) = mpsc::channel();
    let writer = store.clone();
    let task = tokio::spawn(async move {
        ready.await.unwrap();
        writer
            .write(
                "later",
                vec![
                    row(3, "left", Value::Text("x".into())),
                    row(20, "right", Value::Text("later".into())),
                ],
            )
            .await
            .unwrap();
        writer.seal(u64::MAX).await.unwrap();
        writer.retain(1, 0).await.unwrap();
        done.send(()).unwrap();
    });
    let mut signal = Some(signal);
    let mut query = query();
    query.batch = 1;
    let stats = store
        .join(&query, &mut move |batch| {
            let id = batch
                .left
                .column(0)
                .as_any()
                .downcast_ref::<UInt64Array>()
                .unwrap()
                .value(0);
            assert_eq!(batch.present.value(0), id == 1);
            if let Some(signal) = signal.take() {
                signal.send(()).unwrap();
                finished.recv_timeout(Duration::from_secs(10)).unwrap();
            }
            Ok(())
        })
        .await
        .unwrap();
    task.await.unwrap();
    assert_eq!((stats.pairs, stats.missing, stats.rows), (1, 1, 2));
    assert_eq!(
        store
            .join(&query, &mut |_| unreachable!())
            .await
            .unwrap()
            .rows,
        0
    );
}

#[tokio::test]
async fn projection() {
    let home = tempfile::tempdir().unwrap();
    let core = estate(home.path()).await;
    let store = Store::open(core, model(Kind::Text), &home.path().join("column"))
        .await
        .unwrap();
    let mut seed = 17u64;
    let mut rows = vec![
        row(1, "left", Value::Text("x".into())),
        row(10, "right", Value::Text("x".into())),
    ];
    for row in &mut rows {
        row.record.raw = (0..1024 * 1024)
            .map(|_| {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                seed as u8
            })
            .collect();
    }
    store.write("all", rows).await.unwrap();
    let (found, open) = read(&store, &query()).await;
    assert_eq!(found, vec![(1, Some(10), true)]);
    assert!(open.scan.memory < 1024);
    store.seal(u64::MAX).await.unwrap();
    let (_, joined) = read(&store, &query()).await;
    let raw = store
        .query(
            &keel_column::Query::select(Filter::default(), &["raw"]),
            &mut |_| Ok(()),
        )
        .await
        .unwrap();
    assert!(joined.scan.bytes * 10 < raw.bytes, "{joined:?} / {raw:?}");
    eprintln!("join={joined:?} raw={raw:?}");
}

use super::*;

#[test]
fn virtuals() {
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join("old.parquet");
    let mut seed = 17u64;
    let records = (0..2048)
        .map(|time| {
            let mut row = record(time);
            row.raw = (0..2048)
                .map(|_| {
                    seed ^= seed << 13;
                    seed ^= seed >> 7;
                    seed ^= seed << 17;
                    seed as u8
                })
                .collect();
            row
        })
        .collect();
    table().write(&path, records).unwrap();
    let new = table().field("code", Kind::Int).unwrap();
    let raw = new
        .query(
            &path,
            &Query::select(Filter::default(), &["raw"]),
            &mut |_| Ok(()),
        )
        .unwrap();
    let mut query = Query::select(Filter::default(), &["code"]);
    query.batch = 73;
    let stats = new
        .query(&path, &query, &mut |batch| {
            assert!(batch.num_rows() <= 73);
            assert_eq!(batch.column(0).null_count(), batch.num_rows());
            assert_eq!(batch.column(0).data_type(), &arrow_schema::DataType::Int64);
            Ok(())
        })
        .unwrap();
    assert_eq!(stats.matched, raw.matched);
    assert_eq!(stats.rows, 2048);
    assert!(stats.bytes * 10 < raw.bytes, "{stats:?} / {raw:?}");
    eprintln!("virtual={stats:?} raw={raw:?}");
    query.limit.bytes = stats.bytes - 1;
    assert!(new.query(&path, &query, &mut |_| Ok(())).is_err());
    query.limit.bytes = raw.bytes;
    query.limit.rows = 2047;
    assert!(new.query(&path, &query, &mut |_| Ok(())).is_err());
}

#[tokio::test]
async fn budgets() {
    let home = tempfile::tempdir().unwrap();
    let core = estate(home.path()).await;
    let store = Store::open(core, model(), &home.path().join("column"))
        .await
        .unwrap();
    let mut values = values(0);
    values[3] = Value::Text("x".repeat(1024 * 1024));
    let mut row = record(0);
    let mut seed = 17u64;
    row.raw = (0..1024 * 1024)
        .map(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed as u8
        })
        .collect();
    store.write("one", vec![row.values(values)]).await.unwrap();
    let mut query = Query::select(Filter::default(), &["code"]);
    query.limit.memory = 64;
    let small = store.query(&query, &mut |_| Ok(())).await.unwrap();
    assert_eq!(small.rows, 1);
    assert!(small.memory < 64);
    query.projection = keel_column::Projection::Columns(vec!["name".into()]);
    assert!(store.query(&query, &mut |_| unreachable!()).await.is_err());
    let count = store
        .query(&Query::count(Filter::default()), &mut |_| unreachable!())
        .await
        .unwrap();
    assert_eq!(count.memory, 0);
    store.seal(u64::MAX).await.unwrap();
    let scalar = store
        .query(
            &Query::select(Filter::default(), &["code"]),
            &mut |_| Ok(()),
        )
        .await
        .unwrap();
    let raw = store
        .query(&Query::select(Filter::default(), &["raw"]), &mut |_| Ok(()))
        .await
        .unwrap();
    assert!(scalar.bytes < raw.bytes);
    assert_eq!(scalar.matched, count.matched);
}

#[tokio::test]
async fn seals() {
    let home = tempfile::tempdir().unwrap();
    let core = estate(home.path()).await;
    let table = table().field("data", Kind::Bytes).unwrap();
    let store = Store::open(core, table, &home.path().join("column"))
        .await
        .unwrap();
    let mut row = record(0);
    row.raw.clear();
    store
        .write(
            "large",
            vec![row.values(vec![Value::Bytes(vec![0; 16 * 1024 * 1024])])],
        )
        .await
        .unwrap();
    store.seal(0).await.unwrap();
    let count = store
        .query(&Query::count(Filter::default()), &mut |_| unreachable!())
        .await
        .unwrap();
    assert_eq!((count.files, count.matched), (1, 1));
}

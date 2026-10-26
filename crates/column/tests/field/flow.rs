use super::*;

#[tokio::test]
async fn durable() {
    let home = tempfile::tempdir().unwrap();
    let core = estate(home.path()).await;
    let root = home.path().join("column");
    let store = Store::open(core.clone(), model(), &root).await.unwrap();
    let rows = (0..3)
        .map(|time| record(time).values(values(time)))
        .collect();
    store.write("typed", rows).await.unwrap();
    store.append("raw", vec![record(3)]).await.unwrap();
    store
        .write("partial", vec![record(4).values(vec![Value::Bool(false)])])
        .await
        .unwrap();
    store
        .write("null", vec![record(5).values(vec![Value::Null; 5])])
        .await
        .unwrap();
    let expected = read(&store).await;
    for time in 0..3 {
        assert_eq!(expected[&time], (record(time).raw, values(time)));
    }
    assert_eq!(expected[&3].1, vec![Value::Null; 5]);
    assert_eq!(
        expected[&4].1,
        vec![
            Value::Bool(false),
            Value::Null,
            Value::Null,
            Value::Null,
            Value::Null
        ]
    );
    assert_eq!(expected[&5].1, vec![Value::Null; 5]);
    store.seal(u64::MAX).await.unwrap();
    assert_eq!(read(&store).await, expected);
    store
        .write("typed", vec![record(0).values(values(0))])
        .await
        .unwrap();
    assert_eq!(read(&store).await, expected);
    let mut raw = Vec::new();
    store
        .scan(&Filter::default(), &mut |bytes| {
            raw.push(bytes.to_vec());
            Ok(())
        })
        .await
        .unwrap();
    raw.sort();
    let mut original: Vec<_> = expected.values().map(|(raw, _)| raw.clone()).collect();
    original.sort();
    assert_eq!(raw, original);
    drop(store);
    drop(core);
    let core = estate(home.path()).await;
    assert_eq!(
        read(&Store::open(core, model(), &root).await.unwrap()).await,
        expected
    );
}

#[tokio::test]
async fn evolves() {
    let home = tempfile::tempdir().unwrap();
    let (core, store) = store(home.path()).await;
    store.append("sealed", vec![record(0)]).await.unwrap();
    store.seal(u64::MAX).await.unwrap();
    store.append("open", vec![record(1)]).await.unwrap();
    drop(store);
    let root = home.path().join("column");
    let mut before = files(&root);
    before.remove(&root.join("model.json"));
    let first = table().field("ok", Kind::Bool).unwrap();
    assert!(
        Store::open(core.clone(), first.clone(), &root)
            .await
            .is_err()
    );
    let store = Store::evolve(core.clone(), first.clone(), &root)
        .await
        .unwrap();
    let mut after = files(&root);
    after.remove(&root.join("model.json"));
    assert_eq!(before, after);
    store
        .write("first", vec![record(2).values(vec![Value::Bool(true)])])
        .await
        .unwrap();
    store.seal(u64::MAX).await.unwrap();
    drop(store);
    let mut before = files(&root);
    before.remove(&root.join("model.json"));
    let store = Store::evolve(core.clone(), model(), &root).await.unwrap();
    let mut after = files(&root);
    after.remove(&root.join("model.json"));
    assert_eq!(before, after);
    let old = read(&store).await;
    assert_eq!(old[&0].1, vec![Value::Null; 5]);
    assert_eq!(old[&1].1, vec![Value::Null; 5]);
    assert_eq!(old[&2].1[0], Value::Bool(true));
    assert_eq!(&old[&2].1[1..], vec![Value::Null; 4].as_slice());
    store
        .write("new", vec![record(3).values(values(3))])
        .await
        .unwrap();
    let expected = read(&store).await;
    assert_eq!(expected[&3].1, values(3));
    store.seal(u64::MAX).await.unwrap();
    assert_eq!(read(&store).await, expected);
    drop(store);
    assert!(Store::open(core.clone(), first, &root).await.is_err());
    let store = Store::evolve(core.clone(), model(), &root).await.unwrap();
    assert_eq!(read(&store).await, expected);
    drop(store);
    assert_eq!(
        read(&Store::open(core, model(), &root).await.unwrap()).await,
        expected
    );
}

#[test]
fn direct() {
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join("part.parquet");
    let table = model();
    let text = table.describe().unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&text).unwrap()["format"],
        2
    );
    assert_eq!(Table::parse(&text).unwrap(), table);
    assert_eq!(table.fields().len(), 5);
    assert!(
        table.schema().fields()[4..]
            .iter()
            .all(|field| field.is_nullable())
    );
    let nulls = vec![
        Value::Bool(false),
        Value::Int(i64::MAX),
        Value::Uint(0),
        Value::Text(String::new()),
        Value::Bytes(Vec::new()),
    ];
    table.rows(&path, vec![record(0).values(nulls)]).unwrap();
    let stats = table
        .query(
            &path,
            &Query::select(
                Filter::default(),
                &["at", "raw", "ok", "code", "size", "name", "data"],
            ),
            &mut |batch| {
                assert_eq!(value(&batch, 2, 0), Value::Bool(false));
                assert_eq!(value(&batch, 3, 0), Value::Int(i64::MAX));
                assert_eq!(value(&batch, 4, 0), Value::Uint(0));
                assert_eq!(value(&batch, 5, 0), Value::Text(String::new()));
                assert_eq!(value(&batch, 6, 0), Value::Bytes(Vec::new()));
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(stats.matched, 1);
    assert!(
        super::table()
            .query(
                &path,
                &Query::count(Filter::default()),
                &mut |_| unreachable!()
            )
            .is_err()
    );
    let path = home.path().join("legacy.parquet");
    table.write(&path, vec![record(0)]).unwrap();
    table
        .query(
            &path,
            &Query::select(Filter::default(), &["data", "ok"]),
            &mut |batch| {
                assert_eq!(batch.column(0).null_count(), 1);
                assert_eq!(batch.column(1).null_count(), 1);
                Ok(())
            },
        )
        .unwrap();
}

#[tokio::test]
async fn predecessor() {
    let home = tempfile::tempdir().unwrap();
    let core = estate(home.path()).await;
    let root = home.path().join("column");
    let path = root.join("open/k636f6e636f7264-0/00000000000000000001.seg");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let bytes = vec![
        41, 0, 0, 0, 155, 208, 130, 229, 1, 1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 7, 0, 0, 0, 99,
        111, 110, 99, 111, 114, 100, 7, 0, 0, 0, 116, 114, 97, 99, 101, 45, 49, 2, 0, 0, 0, 0, 255,
        7, 0, 0, 0, 51, 119, 101, 59, 0, 108, 101, 103, 97, 99, 121,
    ];
    std::fs::write(&path, &bytes).unwrap();
    std::fs::write(root.join("model.json"), r#"{"format":1,"time":"at","keys":["producer","trace"],"sort":["trace"],"grain":{"key":"producer","width":1000000}}"#).unwrap();
    let store = Store::evolve(core.clone(), model(), &root).await.unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert_eq!(read(&store).await[&1], (vec![0, 255], vec![Value::Null; 5]));
    store.append("legacy", vec![record(1)]).await.unwrap();
    assert_eq!(read(&store).await[&1].0, vec![0, 255]);
    store
        .write("typed", vec![record(2).values(values(2))])
        .await
        .unwrap();
    store.seal(u64::MAX).await.unwrap();
    drop(store);
    let store = Store::open(core, model(), &root).await.unwrap();
    let found = read(&store).await;
    assert_eq!(found[&1], (vec![0, 255], vec![Value::Null; 5]));
    assert_eq!(found[&2], (record(2).raw, values(2)));
}

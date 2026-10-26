use super::*;

fn expected(left: &[(u64, Value)], right: &[(u64, Value)], mode: join::Mode) -> Vec<Pair> {
    let mut result = Vec::new();
    for (id, key) in left {
        let mut matches = 0;
        for (other, candidate) in right {
            if !matches!(key, Value::Null) && key == candidate {
                result.push((*id, Some(*other), true));
                matches += 1;
            }
        }
        if matches == 0 && mode == join::Mode::Left {
            result.push((*id, None, false));
        }
    }
    result.sort();
    result
}

#[tokio::test]
async fn types() {
    let cases = vec![
        (Kind::Bool, Value::Bool(false), Value::Bool(true)),
        (Kind::Int, Value::Int(i64::MIN), Value::Int(i64::MAX)),
        (Kind::Uint, Value::Uint(0), Value::Uint(u64::MAX)),
        (
            Kind::Text,
            Value::Text(String::new()),
            Value::Text("你好".into()),
        ),
        (
            Kind::Bytes,
            Value::Bytes(Vec::new()),
            Value::Bytes(vec![0, 255]),
        ),
    ];
    for (kind, first, second) in cases {
        let home = tempfile::tempdir().unwrap();
        let core = estate(home.path()).await;
        let store = Store::open(core, model(kind), &home.path().join("column"))
            .await
            .unwrap();
        let left = vec![
            (1, first.clone()),
            (1, first.clone()),
            (2, first.clone()),
            (3, Value::Null),
            (4, second.clone()),
        ];
        let right = vec![
            (10, first.clone()),
            (10, first),
            (11, Value::Null),
            (12, second),
        ];
        let rows = left
            .iter()
            .map(|(id, key)| row(*id, "left", key.clone()))
            .chain(right.iter().map(|(id, key)| row(*id, "right", key.clone())))
            .collect();
        store.write("all", rows).await.unwrap();
        for mode in [join::Mode::Left, join::Mode::Inner] {
            let mut query = query();
            query.mode = mode;
            query.batch = 2;
            let (found, stats) = read(&store, &query).await;
            assert_eq!(found, expected(&left, &right, mode));
            assert_eq!(stats.left, join::Input { rows: 5, nulls: 1 });
            assert_eq!(stats.right, join::Input { rows: 4, nulls: 1 });
            assert_eq!((stats.indexed, stats.pairs, stats.missing), (3, 7, 1));
            assert_eq!(stats.rows as usize, found.len());
        }
        let mut selected = query();
        selected
            .left
            .when
            .push(join::Condition::Equal("left".into(), left[0].1.clone()));
        selected
            .right
            .when
            .push(join::Condition::Equal("right".into(), right[0].1.clone()));
        let (found, stats) = read(&store, &selected).await;
        assert_eq!(found.len(), 6);
        assert_eq!(stats.left.rows, 3);
        assert_eq!(stats.right.rows, 2);
        store.seal(u64::MAX).await.unwrap();
        assert_eq!(
            read(&store, &query()).await.0,
            expected(&left, &right, join::Mode::Left)
        );
    }
}

#[tokio::test]
async fn presence() {
    let home = tempfile::tempdir().unwrap();
    let core = estate(home.path()).await;
    let store = Store::open(core, model(Kind::Text), &home.path().join("column"))
        .await
        .unwrap();
    store
        .write(
            "all",
            vec![
                row(1, "left", Value::Text("x".into())),
                row(2, "left", Value::Null),
                row(10, "right", Value::Text("x".into())),
            ],
        )
        .await
        .unwrap();
    let mut query = query();
    query.right.columns = vec!["value".into()];
    query
        .left
        .when
        .push(join::Condition::Missing("value".into()));
    query.right.when.push(join::Condition::Equal(
        "trace".into(),
        Value::Text("trace-0".into()),
    ));
    let mut result = BTreeMap::new();
    let stats = store
        .join(&query, &mut |batch| {
            assert_eq!(batch.right.column(0).null_count(), batch.right.num_rows());
            for index in 0..batch.left.num_rows() {
                let id = batch
                    .left
                    .column(0)
                    .as_any()
                    .downcast_ref::<UInt64Array>()
                    .unwrap()
                    .value(index);
                result.insert(id, batch.present.value(index));
            }
            Ok(())
        })
        .await
        .unwrap();
    assert_eq!(result, BTreeMap::from([(1, true), (2, false)]));
    assert_eq!((stats.pairs, stats.missing), (1, 1));
}

#[tokio::test]
async fn historical() {
    let home = tempfile::tempdir().unwrap();
    let (core, store) = store(home.path()).await;
    store
        .append(
            "old",
            vec![
                row(1, "left", Value::Null).record,
                row(10, "right", Value::Null).record,
            ],
        )
        .await
        .unwrap();
    store.seal(u64::MAX).await.unwrap();
    store
        .append(
            "open",
            vec![
                row(2, "left", Value::Null).record,
                row(11, "right", Value::Null).record,
            ],
        )
        .await
        .unwrap();
    drop(store);
    let root = home.path().join("column");
    let store = Store::evolve(core.clone(), model(Kind::Text), &root)
        .await
        .unwrap();
    store
        .write(
            "new",
            vec![
                row(3, "left", Value::Text("x".into())),
                row(20, "right", Value::Text("x".into())),
            ],
        )
        .await
        .unwrap();
    store.seal(u64::MAX).await.unwrap();
    store
        .write(
            "late",
            vec![
                row(4, "left", Value::Text("none".into())),
                row(21, "right", Value::Text("x".into())),
            ],
        )
        .await
        .unwrap();
    let expected = vec![
        (1, None, false),
        (2, None, false),
        (3, Some(20), true),
        (3, Some(21), true),
        (4, None, false),
    ];
    let (found, stats) = read(&store, &query()).await;
    assert_eq!(found, expected);
    assert_eq!(
        (stats.left.nulls, stats.right.nulls, stats.missing),
        (2, 2, 3)
    );
    drop(store);
    let store = Store::open(core, model(Kind::Text), &root).await.unwrap();
    assert_eq!(read(&store, &query()).await.0, expected);
}

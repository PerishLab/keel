use super::*;

#[tokio::test]
async fn early() {
    let home = tempfile::tempdir().unwrap();
    let core = estate(home.path()).await;
    let store = Store::open(core, model(Kind::Text), &home.path().join("column"))
        .await
        .unwrap();
    let mut cases = Vec::new();
    let mut q = query();
    q.left.key = "unknown".into();
    cases.push(q);
    let mut q = query();
    q.left.key = "at".into();
    cases.push(q);
    let mut q = query();
    q.left.columns.clear();
    cases.push(q);
    let mut q = query();
    q.right.columns = vec!["at".into(), "at".into()];
    cases.push(q);
    let mut q = query();
    q.batch = 0;
    cases.push(q);
    let mut q = query();
    q.batch = 65537;
    cases.push(q);
    let mut q = query();
    q.limit.batch = 64 * 1024 * 1024 + 1;
    cases.push(q);
    let mut q = query();
    q.left.when = vec![join::Condition::Equal("left".into(), Value::Null)];
    cases.push(q);
    let mut q = query();
    q.right.when = vec![join::Condition::Equal("right".into(), Value::Int(1))];
    cases.push(q);
    let mut q = query();
    q.right.when = vec![join::Condition::Missing("unknown".into())];
    cases.push(q);
    let mut q = query();
    q.left.filter.from = Some(10);
    q.left.filter.to = Some(1);
    cases.push(q);
    for q in cases {
        assert!(store.join(&q, &mut |_| unreachable!()).await.is_err());
    }
    let mut q = query();
    q.limit = join::Limit {
        scan: keel_column::Limit {
            candidates: 0,
            bytes: 0,
            rows: 0,
            memory: 0,
        },
        build: 0,
        memory: 0,
        work: 0,
        rows: 0,
        bytes: 0,
        batch: 0,
    };
    assert_eq!(
        store.join(&q, &mut |_| unreachable!()).await.unwrap(),
        join::Stats::default()
    );
}

#[tokio::test]
async fn limits() {
    let home = tempfile::tempdir().unwrap();
    let core = estate(home.path()).await;
    let root = home.path().join("column");
    let store = Store::open(core, model(Kind::Text), &root).await.unwrap();
    let rows = (0..10)
        .map(|id| row(id, "left", Value::Text("x".into())))
        .chain((20..30).map(|id| row(id, "right", Value::Text("x".into()))))
        .collect();
    store.write("all", rows).await.unwrap();
    let (_, baseline) = read(&store, &query()).await;
    let mut small = query();
    small.limit.batch = baseline.bytes / baseline.rows * 2 - 1;
    let (_, batched) = read(&store, &small).await;
    assert_eq!(batched.batches, batched.rows);
    let mut checks = query();
    checks.right.when = vec![join::Condition::Missing("value".into()); 3];
    checks.limit.work = baseline.work;
    assert!(store.join(&checks, &mut |_| Ok(())).await.is_err());
    let before = files(&root);
    for case in 0..9 {
        let mut q = query();
        q.batch = 2;
        match case {
            0 => q.limit.build = 9,
            1 => q.limit.memory = baseline.memory - 1,
            2 => q.limit.rows = 3,
            3 => q.limit.bytes = 1,
            4 => q.limit.batch = 1,
            5 => q.limit.work = 1,
            6 => q.limit.scan.candidates = 20,
            7 => q.limit.scan.rows = 19,
            _ => q.limit.scan.memory = 1,
        }
        assert!(
            store.join(&q, &mut |_| Ok(())).await.is_err(),
            "case {case}"
        );
        assert_eq!(files(&root), before);
    }
    let mut q = query();
    q.batch = 2;
    q.limit.rows = 7;
    let mut seen = 0;
    assert!(
        store
            .join(&q, &mut |batch| {
                seen += batch.left.num_rows();
                Ok(())
            })
            .await
            .is_err()
    );
    assert!(seen > 0 && seen <= 7);
    let mut calls = 0;
    let error = store
        .join(&q, &mut |_| {
            calls += 1;
            Err(std::io::Error::other("sink refusal").into())
        })
        .await
        .unwrap_err();
    assert_eq!(calls, 1);
    assert!(error.to_string().contains("sink refusal"));
    store.seal(u64::MAX).await.unwrap();
    let (_, sealed) = read(&store, &query()).await;
    let mut q = query();
    q.limit.scan.bytes = sealed.scan.bytes - 1;
    assert!(store.join(&q, &mut |_| Ok(())).await.is_err());
    let part = files(&root)
        .keys()
        .find(|path| path.extension().is_some_and(|ext| ext == "parquet"))
        .unwrap()
        .clone();
    std::fs::write(part, b"corrupt").unwrap();
    let mut q = query();
    q.left.key = "unknown".into();
    assert!(
        store
            .join(&q, &mut |_| unreachable!())
            .await
            .unwrap_err()
            .to_string()
            .contains("declared")
    );
    assert!(store.join(&query(), &mut |_| Ok(())).await.is_err());
}

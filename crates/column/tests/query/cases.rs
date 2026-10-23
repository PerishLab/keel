use super::*;

#[tokio::test]
async fn validates() {
    let home = tempfile::tempdir().unwrap();
    let (_, store) = store(home.path()).await;
    let mut queries = vec![
        Query::select(Filter::default(), &["absent"]),
        Query::select(Filter::default(), &[]),
        Query::select(Filter::default(), &["at", "at"]),
    ];
    let mut query = Query::count(Filter {
        from: Some(9),
        to: Some(2),
        keys: vec![],
    });
    queries.push(query.clone());
    query = Query::count(Filter {
        keys: vec![("absent".into(), "x".into())],
        ..Filter::default()
    });
    queries.push(query.clone());
    query = Query::count(Filter::default());
    query.batch = 0;
    queries.push(query.clone());
    query.batch = 65537;
    queries.push(query);
    for query in queries {
        let error = table()
            .query(
                &home.path().join("missing"),
                &query,
                &mut |_| unreachable!(),
            )
            .unwrap_err();
        assert!(!error.to_string().contains("No such file"));
        assert!(store.query(&query, &mut |_| unreachable!()).await.is_err());
    }
    assert_eq!(
        store
            .query(&Query::count(Filter::default()), &mut |_| unreachable!())
            .await
            .unwrap()
            .matched,
        0
    );
}

fn records() -> Vec<Record> {
    let mut seed = 17u64;
    (0..2048)
        .map(|time| {
            let raw = (0..2048)
                .map(|_| {
                    seed ^= seed << 13;
                    seed ^= seed >> 7;
                    seed ^= seed << 17;
                    seed as u8
                })
                .collect();
            Record {
                raw,
                time,
                keys: vec!["concord".into(), format!("trace-{}", time % 4)],
            }
        })
        .collect()
}

#[test]
fn costs() {
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join("part.parquet");
    let table = table();
    table.write(&path, records()).unwrap();
    let filter = Filter::default();
    let raw = table
        .query(&path, &Query::select(filter.clone(), &["raw"]), &mut |_| {
            Ok(())
        })
        .unwrap();
    let projected = table
        .query(
            &path,
            &Query::select(filter.clone(), &["trace", "at"]),
            &mut |_| Ok(()),
        )
        .unwrap();
    let count = table
        .query(&path, &Query::count(filter), &mut |_| unreachable!())
        .unwrap();
    assert_eq!(count.matched, 2048);
    assert_eq!(projected.matched, count.matched);
    assert!(projected.bytes * 10 < raw.bytes, "{projected:?} / {raw:?}");
    assert!(count.bytes <= projected.bytes);
    let absent = table
        .query(
            &path,
            &Query::count(Filter {
                from: Some(10000),
                ..Filter::default()
            }),
            &mut |_| unreachable!(),
        )
        .unwrap();
    assert_eq!(
        (absent.groups, absent.candidates, absent.matched),
        (0, 0, 0)
    );
    eprintln!("raw={raw:?} projected={projected:?} count={count:?} pruned={absent:?}");
    for field in 0..3 {
        let mut query = Query::select(Filter::default(), &["raw"]);
        match field {
            0 => query.limit.bytes = raw.bytes - 1,
            1 => query.limit.candidates = 2047,
            _ => query.limit.rows = 2047,
        }
        assert!(table.query(&path, &query, &mut |_| Ok(())).is_err());
    }
    let mut query = Query::count(Filter::default());
    query.limit.rows = 2047;
    assert!(table.query(&path, &query, &mut |_| unreachable!()).is_err());
}

#[tokio::test]
async fn refusals() {
    let home = tempfile::tempdir().unwrap();
    let (_, store) = store(home.path()).await;
    store.append("one", batch(1, 0)).await.unwrap();
    let mut query = Query::select(Filter::default(), &["raw"]);
    query.limit.memory = 1;
    assert!(store.query(&query, &mut |_| unreachable!()).await.is_err());
    query.limit.memory = 1024;
    query.limit.candidates = 2;
    assert!(store.query(&query, &mut |_| unreachable!()).await.is_err());
    query.limit.candidates = 3;
    query.limit.rows = 2;
    assert!(store.query(&query, &mut |_| unreachable!()).await.is_err());
    store.seal(u64::MAX).await.unwrap();
    query.limit.rows = 3;
    query.batch = 1;
    let mut calls = 0;
    let error = store
        .query(&query, &mut |_| {
            calls += 1;
            Err(std::io::Error::other("sink refusal").into())
        })
        .await
        .unwrap_err();
    assert!(error.to_string().contains("sink refusal"));
    assert_eq!(calls, 1);
    let part = std::fs::read_dir(home.path().join("column/parts"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let file = std::fs::read_dir(part)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    std::fs::write(file, b"corrupt").unwrap();
    assert!(store.query(&query, &mut |_| Ok(())).await.is_err());
}

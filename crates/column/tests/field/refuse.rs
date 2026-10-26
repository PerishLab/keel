use super::*;

#[test]
fn declarations() {
    for name in ["", "at", "raw", "producer", "ok"] {
        assert!(model().field(name, Kind::Text).is_err());
    }
    let original: serde_json::Value = serde_json::from_str(&model().describe().unwrap()).unwrap();
    for mutation in 0..8 {
        let mut value = original.clone();
        match mutation {
            0 => value["columns"][0]["nullable"] = serde_json::json!(false),
            1 => value["columns"][0]["kind"] = serde_json::json!("float"),
            2 => value["columns"][1]["name"] = serde_json::json!("ok"),
            3 => value["columns"][0]["name"] = serde_json::json!("at"),
            4 => value["format"] = serde_json::json!(1),
            5 => value["columns"] = serde_json::json!([]),
            6 => {
                value["columns"] = serde_json::Value::Null;
            }
            _ => {
                value.as_object_mut().unwrap().remove("columns");
            }
        }
        assert!(Table::parse(&value.to_string()).is_err(), "{value}");
    }
}

#[tokio::test]
async fn writes() {
    let home = tempfile::tempdir().unwrap();
    let core = estate(home.path()).await;
    let root = home.path().join("column");
    let store = Store::open(core.clone(), model(), &root).await.unwrap();
    let before = files(&root);
    for bad in [
        vec![Value::Int(1)],
        vec![Value::Null; 6],
        vec![Value::Bool(true), Value::Text("1".into())],
    ] {
        let error = store
            .write(
                "bad",
                vec![record(0).values(values(0)), record(1).values(bad)],
            )
            .await;
        assert!(error.is_err());
        assert_eq!(files(&root), before);
    }
    assert!(
        Store::evolve(
            core.clone(),
            model().field("more", Kind::Text).unwrap(),
            &root
        )
        .await
        .is_err()
    );
    let invalid = Query::select(Filter::default(), &["unknown"]);
    assert!(
        store
            .query(&invalid, &mut |_| unreachable!())
            .await
            .is_err()
    );
    let filter = Filter {
        keys: vec![("code".into(), "1".into())],
        ..Filter::default()
    };
    assert!(
        store
            .query(&Query::count(filter), &mut |_| unreachable!())
            .await
            .is_err()
    );
    assert_eq!(files(&root), before);
}

#[tokio::test]
async fn changes() {
    let home = tempfile::tempdir().unwrap();
    let core = estate(home.path()).await;
    let root = home.path().join("column");
    let original = table()
        .field("code", Kind::Int)
        .unwrap()
        .field("ok", Kind::Bool)
        .unwrap();
    let store = Store::open(core.clone(), original.clone(), &root)
        .await
        .unwrap();
    store
        .write(
            "one",
            vec![record(0).values(vec![Value::Int(-1), Value::Bool(true)])],
        )
        .await
        .unwrap();
    drop(store);
    let variants = vec![
        table(),
        table()
            .field("ok", Kind::Bool)
            .unwrap()
            .field("code", Kind::Int)
            .unwrap(),
        table()
            .field("code", Kind::Text)
            .unwrap()
            .field("ok", Kind::Bool)
            .unwrap(),
        table()
            .field("renamed", Kind::Int)
            .unwrap()
            .field("ok", Kind::Bool)
            .unwrap(),
        original.clone().sort(&[]).unwrap(),
        original.clone().grain(None, WIDTH).unwrap(),
    ];
    let before = files(&root);
    for candidate in variants {
        let error = Store::evolve(core.clone(), candidate, &root)
            .await
            .err()
            .unwrap();
        assert!(error.to_string().contains("permits only"));
        assert_eq!(files(&root), before);
    }
    let path = file(&root, "open");
    let mut bytes = std::fs::read(&path).unwrap();
    bytes.extend_from_slice(b"torn");
    std::fs::write(&path, bytes).unwrap();
    let before = files(&root);
    let next = original.clone().field("next", Kind::Bytes).unwrap();
    assert!(
        Store::evolve(core.clone(), next.clone(), &root)
            .await
            .is_err()
    );
    assert_eq!(files(&root), before);
    drop(Store::open(core.clone(), original, &root).await.unwrap());
    assert!(Store::evolve(core.clone(), next, &root).await.is_ok());
    let empty = home.path().join("empty");
    assert!(Store::evolve(core, model(), &empty).await.is_err());
    assert!(!empty.join("model.json").exists());
}

#[tokio::test]
async fn contradiction() {
    for sealed in [false, true] {
        let home = tempfile::tempdir().unwrap();
        let core = estate(home.path()).await;
        let root = home.path().join("column");
        let table = table().field("code", Kind::Int).unwrap();
        let store = Store::open(core.clone(), table.clone(), &root)
            .await
            .unwrap();
        store
            .write("one", vec![record(0).values(vec![Value::Int(42)])])
            .await
            .unwrap();
        if sealed {
            store.seal(u64::MAX).await.unwrap();
        }
        drop(store);
        std::fs::write(root.join("model.json"), super::table().describe().unwrap()).unwrap();
        let before = files(&root);
        assert!(Store::evolve(core, table, &root).await.is_err());
        assert_eq!(files(&root), before);
    }
}

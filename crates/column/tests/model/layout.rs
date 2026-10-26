use super::*;
use arrow_schema::DataType;
use keel_column::{Filter, Query};

#[test]
fn describes() {
    let table = table();
    let schema = table.schema();
    assert_eq!(schema.fields().len(), 4);
    assert_eq!(schema.field(0).data_type(), &DataType::UInt64);
    assert_eq!(schema.field(1).data_type(), &DataType::Utf8);
    assert_eq!(schema.field(2).data_type(), &DataType::Utf8);
    assert_eq!(schema.field(3).data_type(), &DataType::Binary);
    assert!(schema.fields().iter().all(|field| !field.is_nullable()));
    let text = table.describe().unwrap();
    assert_eq!(Table::parse(&text).unwrap(), table);
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    let pretty = serde_json::to_string_pretty(&value).unwrap();
    assert_eq!(Table::parse(&pretty).unwrap().describe().unwrap(), text);
    assert_eq!(
        Table::parse(&Table::new("time", &[]).unwrap().describe().unwrap()).unwrap(),
        Table::new("time", &[]).unwrap()
    );
}

#[test]
fn refuses() {
    let original: serde_json::Value = serde_json::from_str(&table().describe().unwrap()).unwrap();
    let cases = [
        ("format", serde_json::json!(2)),
        ("time", serde_json::json!("raw")),
        ("keys", serde_json::json!(["at"])),
        ("keys", serde_json::json!(["producer", "producer"])),
        ("sort", serde_json::json!(["absent"])),
        ("grain", serde_json::json!({"key":"producer", "width":0})),
        ("grain", serde_json::json!({"key":"absent", "width":10})),
        ("grain", serde_json::json!({"width":10})),
        ("extra", serde_json::json!(true)),
    ];
    for (key, value) in cases {
        let mut model = original.clone();
        model[key] = value;
        assert!(Table::parse(&model.to_string()).is_err(), "{model}");
    }
    for text in ["{}", "null", "{", "{\"format\":1,\"format\":1}"] {
        assert!(Table::parse(text).is_err());
    }
    assert!(
        Table::parse(&" ".repeat(1024 * 1024 + 1))
            .unwrap_err()
            .to_string()
            .contains("1 MiB")
    );
    let large = "x".repeat(1024 * 1024);
    assert!(Table::new(&large, &[]).unwrap().describe().is_err());
}

#[tokio::test]
async fn immutable() {
    let home = tempfile::tempdir().unwrap();
    let (core, store) = store(home.path()).await;
    store.append("sealed", batch(1, 1)).await.unwrap();
    store.seal(u64::MAX).await.unwrap();
    store.append("open", batch(2, 1)).await.unwrap();
    drop(store);
    let root = home.path().join("column");
    append(&file(&root, "open"), b"torn");
    let orphan = file(&root, "parts").with_file_name("00000000000000000999.parquet");
    std::fs::write(&orphan, b"unpublished").unwrap();
    let before = snapshot(&root);
    let variants = vec![
        Table::new("when", &["producer", "trace"]).unwrap(),
        Table::new("at", &["trace", "producer"]).unwrap(),
        Table::new("at", &["producer", "span"]).unwrap(),
        Table::new("at", &["producer", "trace"])
            .unwrap()
            .grain(Some("producer"), WIDTH)
            .unwrap(),
        table().sort(&["producer"]).unwrap(),
        table().grain(Some("trace"), WIDTH).unwrap(),
        table().grain(None, WIDTH).unwrap(),
        table().grain(Some("producer"), WIDTH + 1).unwrap(),
    ];
    for table in variants {
        assert!(
            Store::open(core.clone(), table.clone(), &root)
                .await
                .is_err()
        );
        assert!(Store::adopt(core.clone(), table, &root).await.is_err());
        assert_eq!(snapshot(&root), before);
    }
    let reopened = Store::open(core, table(), &root).await.unwrap();
    assert_eq!(
        reopened
            .query(&Query::count(Filter::default()), &mut |_| unreachable!())
            .await
            .unwrap()
            .matched,
        6
    );
    assert!(!orphan.exists());
}

#[tokio::test]
async fn damaged() {
    let home = tempfile::tempdir().unwrap();
    let (core, store) = store(home.path()).await;
    store.append("one", batch(1, 1)).await.unwrap();
    drop(store);
    let root = home.path().join("column");
    let path = root.join("model.json");
    let saved = std::fs::read(&path).unwrap();
    assert_eq!(saved, table().describe().unwrap().as_bytes());
    append(&file(&root, "open"), b"torn");
    for bytes in [
        b"{}".to_vec(),
        b"invalid".to_vec(),
        vec![b' '; 1024 * 1024 + 1],
    ] {
        std::fs::write(&path, bytes).unwrap();
        let before = snapshot(&root);
        assert!(Store::open(core.clone(), table(), &root).await.is_err());
        assert!(Store::adopt(core.clone(), table(), &root).await.is_err());
        assert_eq!(snapshot(&root), before);
    }
    std::fs::write(path, saved).unwrap();
    assert_eq!(
        everything(&Store::open(core, table(), &root).await.unwrap())
            .await
            .len(),
        3
    );
}

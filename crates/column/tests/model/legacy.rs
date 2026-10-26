use super::*;
use keel_column::{Filter, Query};

#[tokio::test]
async fn adopts() {
    let home = tempfile::tempdir().unwrap();
    let (core, store) = store(home.path()).await;
    store.append("sealed", batch(1, 1)).await.unwrap();
    store.seal(u64::MAX).await.unwrap();
    store.append("open", batch(2, 1)).await.unwrap();
    drop(store);
    let root = home.path().join("column");
    std::fs::remove_file(root.join("model.json")).unwrap();
    let before = snapshot(&root);
    let error = Store::open(core.clone(), table(), &root)
        .await
        .err()
        .unwrap();
    assert!(error.to_string().contains("explicitly adopt"));
    assert_eq!(snapshot(&root), before);
    let store = Store::adopt(core.clone(), table(), &root).await.unwrap();
    let mut after = snapshot(&root);
    after.remove(&root.join("model.json"));
    assert_eq!(before, after);
    store.append("next", batch(3, 1)).await.unwrap();
    store.seal(u64::MAX).await.unwrap();
    assert_eq!(
        store
            .query(&Query::count(Filter::default()), &mut |_| unreachable!())
            .await
            .unwrap()
            .matched,
        9
    );
    store.append("sealed", batch(1, 1)).await.unwrap();
    assert_eq!(everything(&store).await.len(), 9);
    drop(store);
    assert_eq!(
        everything(&Store::open(core, table(), &root).await.unwrap())
            .await
            .len(),
        9
    );
}

#[tokio::test]
async fn rejects() {
    let home = tempfile::tempdir().unwrap();
    let (core, store) = store(home.path()).await;
    store.append("one", batch(1, 1)).await.unwrap();
    store.seal(u64::MAX).await.unwrap();
    store.append("two", batch(2, 1)).await.unwrap();
    drop(store);
    let root = home.path().join("column");
    std::fs::remove_file(root.join("model.json")).unwrap();
    let before = snapshot(&root);
    let renamed = Table::new("when", &["producer", "trace"])
        .unwrap()
        .grain(Some("producer"), WIDTH)
        .unwrap();
    assert!(Store::adopt(core.clone(), renamed, &root).await.is_err());
    assert_eq!(snapshot(&root), before);
    assert!(
        Store::adopt(core.clone(), table().grain(None, WIDTH).unwrap(), &root)
            .await
            .is_err()
    );
    assert_eq!(snapshot(&root), before);
    let path = file(&root, "open");
    let saved = std::fs::read(&path).unwrap();
    for bytes in [vec![0; 8], saved[..saved.len() - 3].to_vec(), vec![255; 10]] {
        std::fs::write(&path, bytes).unwrap();
        let before = snapshot(&root);
        assert!(Store::adopt(core.clone(), table(), &root).await.is_err());
        assert_eq!(snapshot(&root), before);
    }
    std::fs::write(&path, saved).unwrap();
    let part = file(&root, "parts");
    let saved = std::fs::read(&part).unwrap();
    std::fs::write(&part, b"broken parquet").unwrap();
    let before = snapshot(&root);
    assert!(Store::adopt(core.clone(), table(), &root).await.is_err());
    assert_eq!(snapshot(&root), before);
    std::fs::write(part, saved).unwrap();
    assert!(Store::adopt(core, table(), &root).await.is_ok());
}

#[tokio::test]
async fn pending() {
    let home = tempfile::tempdir().unwrap();
    let core = estate(home.path()).await;
    let root = home.path().join("column");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(
        root.join(".model-interrupted"),
        b"partial model publication",
    )
    .unwrap();
    let store = Store::open(core.clone(), table(), &root).await.unwrap();
    assert_eq!(
        Table::parse(&std::fs::read_to_string(root.join("model.json")).unwrap()).unwrap(),
        table()
    );
    assert!(Store::adopt(core.clone(), table(), &root).await.is_err());
    store.append("one", batch(1, 1)).await.unwrap();
    drop(store);
    assert_eq!(
        everything(&Store::adopt(core, table(), &root).await.unwrap())
            .await
            .len(),
        3
    );
}

#[tokio::test]
async fn order() {
    let home = tempfile::tempdir().unwrap();
    let (core, store) = store(home.path()).await;
    let records = batch(1, 1).into_iter().chain(batch(4, 1)).collect();
    store.append("one", records).await.unwrap();
    store.seal(u64::MAX).await.unwrap();
    drop(store);
    let root = home.path().join("column");
    std::fs::remove_file(root.join("model.json")).unwrap();
    let before = snapshot(&root);
    let error = Store::adopt(core.clone(), table().sort(&[]).unwrap(), &root)
        .await
        .err()
        .unwrap();
    assert!(error.to_string().contains("order differs"));
    assert_eq!(snapshot(&root), before);
    assert!(Store::adopt(core, table(), &root).await.is_ok());
}

#[tokio::test]
async fn width() {
    let home = tempfile::tempdir().unwrap();
    let (core, store) = store(home.path()).await;
    store.append("one", batch(1, 1)).await.unwrap();
    drop(store);
    let root = home.path().join("column");
    std::fs::remove_file(root.join("model.json")).unwrap();
    let mut payload = vec![1];
    payload.extend_from_slice(&(WIDTH + 1).to_le_bytes());
    payload.extend_from_slice(&0u32.to_le_bytes());
    payload.extend_from_slice(&0u32.to_le_bytes());
    let mut bytes = frame(&payload);
    bytes.extend(frame(&[0, b't']));
    std::fs::write(file(&root, "open"), bytes).unwrap();
    let before = snapshot(&root);
    let error = Store::adopt(core, table(), &root).await.err().unwrap();
    assert!(error.to_string().contains("carries 0 keys"));
    assert_eq!(snapshot(&root), before);
}

fn frame(payload: &[u8]) -> Vec<u8> {
    let mut bytes = (payload.len() as u32).to_le_bytes().to_vec();
    bytes.extend_from_slice(&crc32fast::hash(payload).to_le_bytes());
    bytes.extend_from_slice(payload);
    bytes
}

#[tokio::test]
async fn ambiguous() {
    let home = tempfile::tempdir().unwrap();
    let (core, store) = store(home.path()).await;
    store.append("one", batch(1, 1)).await.unwrap();
    drop(store);
    let root = home.path().join("column");
    std::fs::remove_file(root.join("model.json")).unwrap();
    let path = file(&root, "open");
    let other = path.with_file_name("00000000000000000099.seg");
    std::fs::copy(&path, &other).unwrap();
    let before = snapshot(&root);
    assert!(Store::adopt(core.clone(), table(), &root).await.is_err());
    assert_eq!(snapshot(&root), before);
    std::fs::remove_file(&other).unwrap();
    let renamed = path.with_extension("unknown");
    std::fs::rename(&path, &renamed).unwrap();
    let before = snapshot(&root);
    assert!(Store::adopt(core.clone(), table(), &root).await.is_err());
    assert_eq!(snapshot(&root), before);
    std::fs::rename(renamed, path).unwrap();
    assert!(Store::adopt(core, table(), &root).await.is_ok());
}

#[tokio::test]
async fn settled() {
    let home = tempfile::tempdir().unwrap();
    let (core, store) = store(home.path()).await;
    store.append("one", batch(1, 1)).await.unwrap();
    let root = home.path().join("column");
    let path = file(&root, "open");
    let bytes = std::fs::read(&path).unwrap();
    store.seal(u64::MAX).await.unwrap();
    std::fs::write(&path, bytes).unwrap();
    store.append("two", batch(2, 1)).await.unwrap();
    let empty = path.with_file_name("00000000000000000099.seg");
    std::fs::write(&empty, b"").unwrap();
    drop(store);
    std::fs::remove_file(root.join("model.json")).unwrap();
    let adopted = Store::adopt(core, table(), &root).await.unwrap();
    assert_eq!(everything(&adopted).await.len(), 6);
    assert!(!path.exists());
    assert!(!empty.exists());
}

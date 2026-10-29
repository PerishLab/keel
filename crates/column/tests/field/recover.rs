use super::*;

#[tokio::test]
async fn torn() {
    let home = tempfile::tempdir().unwrap();
    let core = estate(home.path()).await;
    let root = home.path().join("column");
    let store = Store::open(core.clone(), model(), &root).await.unwrap();
    store
        .write("one", vec![record(0).values(values(0))])
        .await
        .unwrap();
    store
        .write("two", vec![record(1).values(values(1))])
        .await
        .unwrap();
    drop(store);
    let path = file(&root, "open");
    let length = std::fs::metadata(&path).unwrap().len();
    std::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .unwrap()
        .set_len(length - 3)
        .unwrap();
    let store = Store::open(core, model(), &root).await.unwrap();
    assert_eq!(
        read(&store).await,
        BTreeMap::from([(0, (record(0).raw, values(0)))])
    );
    store
        .write("three", vec![record(2).values(values(2))])
        .await
        .unwrap();
    store.seal(u64::MAX).await.unwrap();
    assert_eq!(read(&store).await.len(), 2);
}

use keel::config::{self, Kind};

#[tokio::test]
async fn defaults() {
    let cfg = config::load(std::env::temp_dir().join("keel-missing-root-xyz"));
    assert_eq!(cfg.listen.host, "127.0.0.1");
    assert_eq!(cfg.listen.port, 3000);
    assert!(cfg.listen.prefix.is_empty());
    assert_eq!(cfg.store.kind, Kind::Memory);
    assert!(cfg.store.path.is_empty());
    let store = cfg.open().await.expect("memory open");
    let _ = store;
}

#[tokio::test]
async fn file() {
    let root = std::env::temp_dir().join(format!("keel-cfg-{}", std::process::id()));
    std::fs::create_dir_all(&root).expect("root");
    let path = root.join("keel.toml");
    std::fs::write(
        &path,
        r#"
[listen]
host = "127.0.0.1"
port = 3001

[store]
kind = "file"
path = "data/keel.sqlite"
"#,
    )
    .expect("write");
    let cfg = config::load(&root);
    assert_eq!(cfg.listen.port, 3001);
    assert_eq!(cfg.store.kind, Kind::File);
    assert_eq!(cfg.store.path, "data/keel.sqlite");
    let store = cfg.open().await.expect("file open");
    let _ = store;
    let db = root.join("data/keel.sqlite");
    assert!(db.exists() || db.parent().map(|p| p.exists()).unwrap_or(false));
    let _ = std::fs::remove_dir_all(&root);
}

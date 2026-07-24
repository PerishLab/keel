use keel::config;
use plumb::config::{Cascade, Kind};

#[tokio::test]
async fn defaults() {
    let start = std::env::temp_dir().join("keel-missing-root-xyz");
    let (cfg, root) = config::load(&start).expect("absent file resolves");
    assert_eq!(cfg.listen.host, "127.0.0.1");
    assert_eq!(cfg.listen.port, 3000);
    assert!(cfg.listen.prefix.is_empty());
    assert_eq!(cfg.store.kind, Kind::Memory);
    assert!(cfg.store.path.is_empty());
    assert_eq!(cfg.cache.kind, config::Hold::Memory);
    assert_eq!(root, start);
    let store = cfg.open(&root).await.expect("memory open");
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
    let nested = root.join("nested");
    std::fs::create_dir_all(&nested).expect("nested");
    let (cfg, found) = config::load(&nested).expect("file resolves");
    assert_eq!(found, root);
    assert_eq!(cfg.listen.port, 3001);
    assert_eq!(cfg.store.kind, Kind::File);
    assert_eq!(cfg.store.path, "data/keel.sqlite");
    let store = cfg.open(&found).await.expect("file open");
    let _ = store;
    let db = root.join("data/keel.sqlite");
    assert!(db.exists() || db.parent().map(|p| p.exists()).unwrap_or(false));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn malformed() {
    let root = std::env::temp_dir().join(format!("keel-bad-{}", std::process::id()));
    std::fs::create_dir_all(&root).expect("root");
    std::fs::write(root.join("keel.toml"), "[listen]\nport = \"harbor\"\n").expect("write");
    let refused = config::load(&root);
    assert!(refused.is_err());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn prefixed() {
    assert_eq!(config::Config::prefix(), "KEEL");
}

#[test]
fn veiled() {
    let get = |key: &str| match key {
        "KEEL_LISTEN_PORT" => Some("7".to_string()),
        "KEEL_STORE_KIND" => Some("file".to_string()),
        "KEEL_CACHE_KIND" => Some("none".to_string()),
        "KEEL_IDENTITY_UNIT" => Some("veiled".to_string()),
        _ => None,
    };
    let over = config::Config::lookup("KEEL", &get).expect("env reads");
    let cfg = config::Config::default().merge(over);
    assert_eq!(cfg.listen.port, 7);
    assert_eq!(cfg.listen.host, "127.0.0.1");
    assert_eq!(cfg.store.kind, Kind::File);
    assert_eq!(cfg.cache.kind, config::Hold::None);
    assert_eq!(cfg.identity.unit, "veiled");
}

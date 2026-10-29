#[path = "support/mod.rs"]
mod support;

use std::fs;
use support::{store, temp};

#[tokio::test]
async fn exclusive() {
    let home = temp("exclusive");
    let (core, first) = store(&home).await;
    let path = home.join("column/lock");
    assert_eq!(probe(&path), 75);
    #[cfg(unix)]
    fs::write(&path, b"caller-owned content").expect("lock contents");
    let second =
        keel_column::Store::open(core.clone(), support::table(), &home.join("column")).await;
    assert!(second.is_err());
    #[cfg(unix)]
    assert_eq!(fs::metadata(&path).unwrap().len(), 0);
    assert_eq!(probe(&path), 75);
    drop(first);
    assert_eq!(probe(&path), 0);
    let reopened = keel_column::Store::open(core, support::table(), &home.join("column"))
        .await
        .unwrap();
    assert_eq!(probe(&path), 75);
    drop(reopened);
    assert_eq!(probe(&path), 0);
    assert!(path.is_file());
}

fn probe(path: &std::path::Path) -> i32 {
    std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "worker", "--nocapture"])
        .env("KEEL_COLUMN_LOCK_PROBE", path)
        .output()
        .unwrap()
        .status
        .code()
        .unwrap()
}

#[test]
fn worker() {
    let Some(path) = std::env::var_os("KEEL_COLUMN_LOCK_PROBE") else {
        return;
    };
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .unwrap();
    match file.try_lock() {
        Ok(()) => (),
        Err(fs::TryLockError::WouldBlock) => std::process::exit(75),
        Err(error) => panic!("unexpected independent std lock failure: {error}"),
    }
}

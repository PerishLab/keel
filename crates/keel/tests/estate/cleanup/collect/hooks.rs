use super::lab::{Lab, graph, policy};
use keel::adapt::db::Sqlite;
use keel::config::Estate;
use keel::estate::{Gone, Hook, Purge};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone)]
struct Capture {
    seen: Arc<Mutex<Vec<Vec<Gone>>>>,
    mode: u8,
}

impl Hook for Capture {
    async fn purge(&mut self, _: Purge) -> Result<(), String> {
        Ok(())
    }

    async fn collect(&mut self, gone: Vec<Gone>) -> Result<(), String> {
        self.seen.lock().expect("seen").push(gone);
        match self.mode {
            1 => Err("hook failed".into()),
            2 => std::future::pending().await,
            3 => panic!("hook panicked"),
            _ => Ok(()),
        }
    }
}

async fn check(mode: u8) {
    let mut lab = Lab::new(policy()).await;
    let hook = Capture {
        seen: Arc::default(),
        mode,
    };
    let seen = hook.seen.clone();
    let wire = Sqlite::file(lab.dir.path().join("estate.sqlite"))
        .await
        .expect("wire");
    let core = keel::bind(graph(), wire)
        .estate(&Estate {
            collection: policy(),
            ..Default::default()
        })
        .hook(hook)
        .await
        .expect("hook bind");
    let old = std::mem::replace(&mut lab.core, core);
    drop(old);
    let mut keys = Vec::new();
    for _ in 0..7 {
        let key = lab.org("old").await;
        lab.end("Org", key, 40 * 86400).await;
        keys.push(key);
    }
    lab.until("org", 0).await;
    let key = lab.org("later").await;
    lab.end("Org", key, 40 * 86400).await;
    lab.until("org", 0).await;
    if mode == 0 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let events = seen.lock().expect("seen");
        assert!(events.iter().all(|batch| batch.len() <= 2));
        let mut got = events.iter().flatten().map(Gone::key).collect::<Vec<_>>();
        keys.push(key);
        keys.sort();
        got.sort();
        assert_eq!(got, keys);
        assert!(events.iter().flatten().all(|row| row.path() == "org"));
    }
}

#[tokio::test]
async fn delivered() {
    check(0).await;
}

#[tokio::test]
async fn failed() {
    check(1).await;
}

#[tokio::test]
async fn stalled() {
    check(2).await;
}

#[tokio::test]
async fn panicked() {
    check(3).await;
}

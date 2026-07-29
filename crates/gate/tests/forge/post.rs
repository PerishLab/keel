use crate::right::{her, him, stage};
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Mutex;

const REPLY: &[u8] = b"HTTP/1.1 204 No Content\r\ncontent-length: 0\r\nconnection: close\r\n\r\n";

type Inbox = Arc<Mutex<Vec<Value>>>;

fn split(seen: &[u8]) -> Option<&[u8]> {
    seen.windows(4)
        .position(|part| part == b"\r\n\r\n")
        .map(|at| &seen[at + 4..])
}

async fn ear() -> (u16, Inbox) {
    let bound = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = bound.local_addr().expect("addr").port();
    let inbox: Inbox = Arc::new(Mutex::new(Vec::new()));
    let held = inbox.clone();
    tokio::spawn(async move {
        while let Ok((mut wire, _)) = bound.accept().await {
            let held = held.clone();
            tokio::spawn(async move {
                let mut seen = Vec::new();
                let mut part = [0u8; 1024];
                while let Ok(read) = wire.read(&mut part).await {
                    if read == 0 {
                        break;
                    }
                    seen.extend_from_slice(&part[..read]);
                    let done = split(&seen)
                        .and_then(|body| serde_json::from_slice::<Value>(body).ok())
                        .inspect(|_| ());
                    if let Some(value) = done {
                        held.lock().await.push(value);
                        break;
                    }
                }
                let _ = wire.write_all(REPLY).await;
            });
        }
    });
    (port, inbox)
}

#[tokio::test]
async fn bound() {
    let stage = stage().await;
    let rig = &stage.deck.rig;
    let (port, inbox) = ear().await;
    rig.grant(&stage.carol.to_string(), "see", ("@grant", "all"))
        .await;
    let url = format!("http://127.0.0.1:{port}/hooked");
    let sent = json!({ "url": url, "unit": "actor:repo", "verb": "", "actor": stage.carol });
    rig.made("/hook", sent, &her()).await;
    let sent = json!({ "url": url, "unit": "@grant", "verb": "put", "actor": stage.carol });
    rig.made("/hook", sent, &her()).await;
    tokio::time::sleep(Duration::from_millis(700)).await;
    inbox.lock().await.clear();

    let path = format!("/repo/{}", stage.den);
    rig.patch(&path, json!({ "about": "" }), &him()).await;
    let open = rig
        .patch(&path, json!({ "visibility": "public" }), &her())
        .await;
    assert!(open.status.is_success(), "{}", open.body);
    let vault = json!({ "name": "vault", "visibility": "private", "owner": stage.dave });
    rig.made("/repo", vault, &him()).await;
    tokio::time::sleep(Duration::from_millis(1500)).await;

    let seen = inbox.lock().await.clone();
    let repos: Vec<&Value> = seen.iter().filter(|e| e["unit"] == "actor:repo").collect();
    assert_eq!(repos.len(), 1, "{seen:?}");
    assert_eq!(repos[0]["verb"], "set");
    let key: i64 = repos[0]["key"].as_str().expect("key").parse().expect("int");
    assert_eq!(key, stage.den);

    let minted: Vec<&Value> = seen.iter().filter(|e| e["unit"] == "@grant").collect();
    assert!(!minted.is_empty(), "{seen:?}");
    assert!(minted.iter().all(|e| e["verb"] == "put"), "{minted:?}");
}

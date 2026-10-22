#![cfg(unix)]

use crate::rig::Site;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[tokio::test]
async fn term() {
    let site = Site::new();
    site.sealed().await;
    let store = format!("sqlite:{}", site.at("estate.db").display());
    let mut child = Command::new(env!("CARGO_BIN_EXE_keel-api"))
        .env("KEEL_STORE", store)
        .env("KEEL_LISTEN", "127.0.0.1:0")
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    let mut line = String::new();
    let mut err = BufReader::new(child.stderr.take().expect("stderr"));
    err.read_line(&mut line).expect("ready");
    assert!(line.contains("ready on"), "{line}");
    let pid = child.id().to_string();
    let sent = Command::new("kill").args(["-TERM", &pid]).status();
    assert!(sent.expect("kill").success());
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().expect("wait") {
            break status;
        }
        assert!(start.elapsed() < Duration::from_secs(5), "still running");
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(status.success(), "{status}");
}

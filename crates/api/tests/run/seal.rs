use crate::rig::{Site, graph};
use keel::Graph;
use keel::adapt::Error;
use keel::estate::Fault;

#[tokio::test]
async fn custody() {
    let site = Site::new();
    assert_eq!(
        api::status(site.wire().await).await.expect("status"),
        "vacant"
    );
    let token = site.sealed().await;
    assert!(token.len() == 64 && token.bytes().all(|b| b.is_ascii_hexdigit()));
    assert_eq!(
        api::status(site.wire().await).await.expect("status"),
        "sealed"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(site.at("sudo"))
            .expect("meta")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}

#[tokio::test]
async fn again() {
    let site = Site::new();
    site.sealed().await;
    let out = api::seal(site.wire().await, graph(), &site.at("second")).await;
    assert_eq!(out.err(), Some(Error::Estate(Fault::Occupied)));
    assert!(!site.at("second").exists());
}

#[tokio::test]
async fn held() {
    let site = Site::new();
    std::fs::write(site.at("sudo"), "kept").expect("write");
    let out = api::seal(site.wire().await, graph(), &site.at("sudo")).await;
    assert!(matches!(out, Err(Error::Adapt(_))), "{out:?}");
    assert_eq!(
        std::fs::read_to_string(site.at("sudo")).expect("read"),
        "kept"
    );
    assert_eq!(
        api::status(site.wire().await).await.expect("status"),
        "vacant"
    );
}

#[tokio::test]
async fn model() {
    let site = Site::new();
    site.sealed().await;
    let mut wire = site.wire().await;
    let held = keel::estate::read(&mut wire).await.expect("read");
    assert_eq!(held.write().expect("held"), graph().write().expect("graph"));
    let text = graph().write().expect("text");
    assert_eq!(
        Graph::read(&text).expect("read").write().expect("again"),
        text
    );
}

#[tokio::test]
async fn vacant() {
    let site = Site::new();
    let out = api::mount(site.wire().await, "").await;
    assert_eq!(
        out.err().map(|e| e.to_string()),
        Some(Error::Estate(Fault::Vacant).to_string())
    );
}

#[tokio::test]
async fn file() {
    let site = Site::new();
    let text = graph().write().expect("text");
    std::fs::write(site.at("model.json"), format!("{text}\n")).expect("write");
    let read = api::model(&site.at("model.json")).expect("model");
    assert_eq!(read.write().expect("again"), text);
    assert!(api::model(&site.at("absent.json")).is_err());
}

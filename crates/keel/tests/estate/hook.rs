use super::world::*;
use keel::adapt::db::Sqlite;
use keel::config::{Estate, Retain};
use keel::estate::{Hook, Purge};
use keel::wire::Wire;
use keel::{Ends, bind};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct Capture {
    seen: Arc<Mutex<Vec<Purge>>>,
    fail: bool,
}

impl Hook for Capture {
    async fn purge(&mut self, event: Purge) -> Result<(), String> {
        self.seen.lock().expect("capture").push(event);
        if self.fail {
            return Err("injected derivative failure".into());
        }
        Ok(())
    }
}

fn immediate() -> Estate {
    let mut estate = Estate::default();
    estate.generation.cleanup.retain = Retain::Span(0);
    estate
}

fn capture(fail: bool) -> (Capture, Arc<Mutex<Vec<Purge>>>) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    (
        Capture {
            seen: seen.clone(),
            fail,
        },
        seen,
    )
}

#[tokio::test]
async fn facts() {
    let path = spot("hook-facts");
    let core = bind(graph::<Alpha>(), Sqlite::file(&path).await.expect("first"))
        .await
        .expect("bind");
    let ended = core
        .put("Alpha", &[("alpha", "1"), ("zeta", "ended")])
        .await
        .expect("ended");
    core.end("Alpha", ended).await.expect("end");
    let held = core
        .put("Alpha", &[("alpha", "2"), ("zeta", "held")])
        .await
        .expect("held");
    drop(core);

    let (hook, seen) = capture(false);
    let core = bind(
        pair::<Alpha, Beta>(),
        Sqlite::file(&path).await.expect("evolve"),
    )
    .estate(&immediate())
    .hook(hook)
    .await
    .expect("evolve");
    assert_eq!(core.live("Alpha").await.expect("active")[0].key(), held);
    {
        let events = seen.lock().expect("events");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].generation(), 1);
        assert_eq!(events[0].gone().len(), 1);
        assert_eq!(events[0].gone()[0].path(), "alpha");
        assert_eq!(events[0].gone()[0].key(), ended);
    }
    drop(core);
    empty(&path).await;
    clean(&path);
}

#[tokio::test]
async fn fields() {
    let path = spot("hook-fields");
    let core = bind(graph::<Alpha>(), Sqlite::file(&path).await.expect("first"))
        .await
        .expect("bind");
    let key = core
        .put("Alpha", &[("alpha", "1"), ("zeta", "held")])
        .await
        .expect("put");
    drop(core);

    let (hook, seen) = capture(false);
    let core = bind(
        graph::<Shrink>(),
        Sqlite::file(&path).await.expect("evolve"),
    )
    .estate(&immediate())
    .hook(hook)
    .await
    .expect("evolve");
    {
        let events = seen.lock().expect("events");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].gone().len(), 1);
        assert_eq!(events[0].gone()[0].path(), "alpha.alpha");
        assert_eq!(events[0].gone()[0].key(), key);
    }
    drop(core);
    empty(&path).await;
    clean(&path);
}

#[tokio::test]
async fn bonds() {
    let path = spot("hook-bonds");
    let core = bind(
        pair::<Club, Member>(),
        Sqlite::file(&path).await.expect("first"),
    )
    .await
    .expect("bind");
    let club = core.put("Club", &[("name", "lab")]).await.expect("club");
    let member = core
        .put("Member", &[("name", "ada"), ("age", "42")])
        .await
        .expect("member");
    let tie = core
        .tie(
            "Member",
            "clubs",
            Ends {
                left: member,
                right: club,
            },
            &[("role", "owner")],
        )
        .await
        .expect("tie");
    core.cut("Member", "clubs", tie).await.expect("cut");
    drop(core);

    let (hook, seen) = capture(false);
    let core = bind(
        pair::<Club, Lean>(),
        Sqlite::file(&path).await.expect("evolve"),
    )
    .estate(&immediate())
    .hook(hook)
    .await
    .expect("evolve");
    {
        let events = seen.lock().expect("events");
        assert_eq!(events.len(), 1);
        let gone = events[0].gone();
        assert!(
            gone.iter()
                .any(|atom| { atom.path() == "member.age" && atom.key() == member })
        );
        assert!(
            gone.iter()
                .any(|atom| { atom.path() == "member.clubs" && atom.key() == tie })
        );
    }
    drop(core);
    empty(&path).await;
    clean(&path);
}

#[tokio::test]
async fn retry() {
    let path = spot("hook-retry");
    let core = bind(graph::<Alpha>(), Sqlite::file(&path).await.expect("first"))
        .await
        .expect("bind");
    let ended = core
        .put("Alpha", &[("alpha", "1"), ("zeta", "ended")])
        .await
        .expect("put");
    core.end("Alpha", ended).await.expect("end");
    drop(core);

    let (failed, first) = capture(true);
    let core = bind(graph::<Grow>(), Sqlite::file(&path).await.expect("evolve"))
        .estate(&immediate())
        .hook(failed)
        .await
        .expect("hook does not gate");
    assert!(!core.has("@g1:alpha").await.expect("estate collected"));
    drop(core);
    assert_eq!(first.lock().expect("first").len(), 1);

    let (hook, second) = capture(false);
    let core = bind(graph::<Grow>(), Sqlite::file(&path).await.expect("retry"))
        .hook(hook)
        .await
        .expect("retry");
    {
        let retried = second.lock().expect("second");
        assert_eq!(retried.len(), 1);
        assert_eq!(retried[0].generation(), 1);
        assert_eq!(retried[0].gone()[0].key(), ended);
    }
    drop(core);
    empty(&path).await;
    clean(&path);
}

async fn empty(path: &std::path::Path) {
    let mut wire = Sqlite::file(path).await.expect("inspect");
    let rows = wire
        .rows("SELECT generation FROM \"@derivative\"", &[])
        .await
        .expect("outbox");
    assert!(rows.is_empty());
}

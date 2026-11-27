use crate::world::Actor;
use keel::adapt::{Error, db::Sqlite};
use keel::{Core, Graph};

async fn core(cached: bool) -> (Core<Sqlite>, i64) {
    let mut graph = Graph::new();
    graph.plug::<Actor>();
    let core = crate::support::boot(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("boot")
        .identify("Actor")
        .expect("identity");
    let core = if cached { core } else { core.bare() };
    let actor = core
        .put("Actor", &[("login", "operator")])
        .await
        .expect("actor");
    core.grant(keel::Grant::new(&actor.to_string(), "put", "Actor", "all"))
        .await
        .expect("put grant");
    (core, actor)
}

async fn exercise(cached: bool, cancel: bool) {
    let (core, actor) = core(cached).await;
    let face = core.of(actor);
    assert!(face.live("Actor").await.expect("before").is_empty());
    let mut tentative = 0;
    let pending = face.batch(async |tx| {
        tentative = tx.put("Actor", &[("login", "tentative")]).await?;
        assert_eq!(tx.live("Actor").await?.len(), 1);
        if cancel {
            std::future::pending::<()>().await;
        }
        Err::<(), _>(Error::Adapt("intentional rollback".into()))
    });
    if cancel {
        let out = tokio::time::timeout(std::time::Duration::from_millis(50), pending).await;
        assert!(out.is_err());
    } else {
        assert!(pending.await.is_err());
    }
    assert_eq!(core.live("@grant").await.expect("durable grants").len(), 1);
    let replacement = core
        .put("Actor", &[("login", "replacement")])
        .await
        .expect("replacement");
    assert_eq!(replacement, tentative);
    assert!(face.live("Actor").await.expect("after").is_empty());
}

#[tokio::test]
async fn rollback() {
    exercise(true, false).await;
}

#[tokio::test]
async fn uncached() {
    exercise(false, false).await;
}

#[tokio::test]
async fn cancel() {
    exercise(true, true).await;
}

#[tokio::test]
async fn commit() {
    let (core, actor) = core(true).await;
    let face = core.of(actor);
    let owned = face
        .batch(async |tx| {
            let key = tx.put("Actor", &[("login", "owned")]).await?;
            assert_eq!(tx.live("Actor").await?.len(), 1);
            Ok(key)
        })
        .await
        .expect("commit");
    assert_eq!(core.live("@grant").await.expect("durable grants").len(), 2);
    core.put("Actor", &[("login", "foreign")])
        .await
        .expect("foreign");
    let visible = face.live("Actor").await.expect("committed ownership");
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].key(), owned);
}

use keel::Ends;
use keel::config::Retain;
use std::time::Duration;

mod hooks;
mod lab;
use lab::{Lab, policy};

const DAY: i64 = 24 * 60 * 60;

#[tokio::test]
async fn retention() {
    let mut lab = Lab::new(policy()).await;
    let old = lab.org("old").await;
    let recent = lab.org("recent").await;
    lab.org("live").await;
    lab.end("Org", old, 31 * DAY).await;
    lab.end("Org", recent, DAY).await;
    lab.until("org", 2).await;
    assert!(lab.core.trace("Org", old).await.expect("trace").is_none());
    assert!(
        lab.core
            .trace("Org", recent)
            .await
            .expect("trace")
            .is_some()
    );
    assert_eq!(lab.core.live("Org").await.expect("live").len(), 1);
    assert!(lab.org("next").await > recent);
    assert_eq!(lab.core.flow(0).await.expect("pulse").len(), 6);
}

#[tokio::test]
async fn lease() {
    let mut collection = policy();
    collection.retain = Retain::Span(2);
    let mut lab = Lab::new(collection).await;
    let key = lab.org("lease").await;
    let at = keel::life::tick() + 3;
    lab.core.lease("Org", key, at).await.expect("lease");
    tokio::time::sleep(Duration::from_millis(1200)).await;
    assert!(keel::life::tick() < at + 2);
    assert_eq!(lab.count("org").await, 1);
    lab.until("org", 0).await;
    assert!(keel::life::tick() >= at + 2);
}

#[tokio::test]
async fn forever() {
    let mut collection = policy();
    collection.retain = Retain::Forever;
    let mut lab = Lab::new(collection).await;
    let key = lab.org("kept").await;
    lab.end("Org", key, 100 * DAY).await;
    tokio::time::sleep(Duration::from_millis(1300)).await;
    assert_eq!(lab.count("org").await, 1);
}

#[tokio::test]
async fn referred() {
    let mut lab = Lab::new(policy()).await;
    let org = lab.org("held").await;
    let note = lab
        .core
        .put("Note", &[("name", "note"), ("org", &org.to_string())])
        .await
        .expect("note");
    lab.end("Note", note, DAY).await;
    lab.end("Org", org, 31 * DAY).await;
    tokio::time::sleep(Duration::from_millis(1300)).await;
    assert_eq!(lab.count("org").await, 1);
    lab.age("Note", note, 31 * DAY).await;
    lab.until("org", 0).await;
    lab.until("note", 0).await;
}

#[tokio::test]
async fn ties() {
    let mut lab = Lab::new(policy()).await;
    let org = lab.org("target").await;
    let team = lab
        .core
        .put("Team", &[("name", "owner")])
        .await
        .expect("team");
    lab.core
        .tie(
            "Team",
            "orgs",
            Ends {
                left: team,
                right: org,
            },
            &[],
        )
        .await
        .expect("tie");
    lab.end("Team", team, 31 * DAY).await;
    lab.end("Org", org, 31 * DAY).await;
    lab.until("team_orgs", 0).await;
    lab.until("team", 0).await;
    lab.until("org", 0).await;
}

#[tokio::test]
async fn writers() {
    let mut collection = policy();
    collection.retain = Retain::Span(0);
    let mut lab = Lab::new(collection).await;
    let key = lab.org("expired").await;
    let path = lab.dir.path().join("estate.sqlite");
    let writer = keel::bind(
        lab::graph(),
        keel::adapt::db::Sqlite::file(path).await.expect("writer"),
    )
    .estate(&keel::config::Estate {
        collection: keel::config::Collection {
            retain: Retain::Forever,
            ..Default::default()
        },
        ..Default::default()
    })
    .await
    .expect("bind writer");
    let team = writer.put("Team", &[("name", "live")]).await.expect("team");
    lab.core.end("Org", key).await.expect("end");
    let writes = tokio::spawn(async move {
        for _ in 0..100 {
            assert!(
                writer
                    .put("Note", &[("name", "race"), ("org", &key.to_string())])
                    .await
                    .is_err()
            );
            assert!(
                writer
                    .tie(
                        "Team",
                        "orgs",
                        Ends {
                            left: team,
                            right: key
                        },
                        &[]
                    )
                    .await
                    .is_err()
            );
            tokio::time::sleep(Duration::from_millis(15)).await;
        }
    });
    lab.until("org", 0).await;
    writes.await.expect("writer");
    assert_eq!(lab.count("note").await, 0);
    assert_eq!(lab.count("team_orgs").await, 0);
    assert_eq!(lab.count("team").await, 1);
}

#[tokio::test]
async fn obsolete() {
    let mut lab = Lab::new(policy()).await;
    let wire = keel::adapt::db::Sqlite::file(lab.dir.path().join("estate.sqlite"))
        .await
        .expect("wire");
    let core = keel::bind(lab::grown(), wire)
        .estate(&keel::config::Estate {
            collection: keel::config::Collection {
                retain: Retain::Forever,
                ..Default::default()
            },
            ..Default::default()
        })
        .await
        .expect("evolve");
    let key = core
        .put("Org", &[("name", "new"), ("extra", "field")])
        .await
        .expect("put");
    core.end("Org", key).await.expect("end");
    lab.age("Org", key, 40 * DAY).await;
    tokio::time::sleep(Duration::from_millis(1300)).await;
    assert_eq!(lab.count("org").await, 1);
}

#[tokio::test]
async fn cancelled() {
    let mut lab = Lab::new(policy()).await;
    let key = lab.org("old").await;
    lab.end("Org", key, 40 * DAY).await;
    let out = tokio::time::timeout(
        Duration::from_millis(100),
        lab.core.batch::<()>(async |tx| {
            tx.put("Org", &[("name", "uncommitted")]).await?;
            std::future::pending().await
        }),
    )
    .await;
    assert!(out.is_err());
    lab.until("org", 0).await;
    assert!(
        lab.core
            .live("Org")
            .await
            .expect("read after background rollback")
            .is_empty()
    );
    assert!(lab.org("after").await > key);
}

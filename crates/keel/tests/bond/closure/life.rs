use super::*;
use keel::adapt::Error;
use keel::life::Tie;

fn rights(ties: &[Tie]) -> Vec<i64> {
    let mut out = ties.iter().map(Tie::right).collect::<Vec<_>>();
    out.sort();
    out
}

#[tokio::test]
async fn reach() {
    let core = boot().await;
    let ids = seed(&core, &["ada", "bob", "cy"]).await;
    let (ada, bob, cy) = (ids[0], ids[1], ids[2]);
    let ab = core
        .tie("Actor", "members", ends(ada, bob), &[])
        .await
        .expect("direct ab");
    let bc = core
        .tie("Actor", "members", ends(bob, cy), &[])
        .await
        .expect("direct bc");

    assert_eq!(
        rights(&core.ties("Actor", "members", ada).await.expect("direct")),
        vec![bob]
    );
    assert_eq!(
        rights(
            &core
                .ties("Actor", "members_closure", ada)
                .await
                .expect("ada closure")
        ),
        vec![bob, cy]
    );
    assert_eq!(
        rights(
            &core
                .ties("Actor", "members_closure", bob)
                .await
                .expect("bob closure")
        ),
        vec![cy]
    );

    core.tie("Actor", "members", ends(ada, cy), &[])
        .await
        .expect("direct ac");
    assert_eq!(
        rights(
            &core
                .ties("Actor", "members_closure", ada)
                .await
                .expect("unique closure")
        ),
        vec![bob, cy]
    );
    let pack = core
        .query(&format!(
            r#"from Actor where members_closure has "{cy}" link members_closure"#
        ))
        .await
        .expect("closure query");
    assert_eq!(pack.rows().len(), 2);
    assert_eq!(pack.bond("actor.members_closure").expect("bag").len(), 3);

    core.cut("Actor", "members", bc).await.expect("cut bc");
    assert!(
        core.ties("Actor", "members_closure", bob)
            .await
            .expect("bob cleared")
            .is_empty()
    );
    let ac = core
        .ties("Actor", "members", ada)
        .await
        .expect("direct ties")
        .into_iter()
        .find(|tie| tie.right() == cy)
        .expect("ac")
        .key();
    core.cut("Actor", "members", ac).await.expect("cut ac");
    assert_eq!(
        rights(
            &core
                .ties("Actor", "members_closure", ada)
                .await
                .expect("ada reduced")
        ),
        vec![bob]
    );
    core.cut("Actor", "members", ab).await.expect("cut ab");
}

#[tokio::test]
async fn txn() {
    let core = boot().await;
    let ids = seed(&core, &["ada", "bob", "cy"]).await;
    let (ada, bob, cy) = (ids[0], ids[1], ids[2]);
    let rolled: Result<(), Error> = core
        .batch(async |tx| {
            tx.tie("Actor", "members", ends(ada, bob), &[]).await?;
            assert_eq!(
                rights(&tx.ties("Actor", "members_closure", ada).await?),
                vec![bob]
            );
            tx.tie("Actor", "members", ends(bob, cy), &[]).await?;
            let pack = tx
                .query(&format!(r#"from Actor where members_closure has "{cy}""#))
                .await?;
            assert_eq!(pack.rows().len(), 2);
            Err(Error::Adapt("rollback".into()))
        })
        .await;
    assert!(rolled.is_err());
    assert!(
        core.ties("Actor", "members", ada)
            .await
            .expect("direct")
            .is_empty()
    );
    assert!(
        core.ties("Actor", "members_closure", ada)
            .await
            .expect("closure")
            .is_empty()
    );

    let ab = core
        .tie("Actor", "members", ends(ada, bob), &[])
        .await
        .expect("ab");
    let bc = core
        .tie("Actor", "members", ends(bob, cy), &[])
        .await
        .expect("bc");
    assert_eq!((ab, bc), (1, 2));
    let before = core
        .ties("Actor", "members_closure", ada)
        .await
        .expect("before refusal");
    assert!(
        core.tie("Actor", "members", ends(ada, ada), &[])
            .await
            .is_err()
    );
    assert!(
        core.tie("Actor", "members", ends(cy, ada), &[])
            .await
            .is_err()
    );
    assert_eq!(
        core.ties("Actor", "members_closure", ada)
            .await
            .expect("after refusal"),
        before
    );
    assert!(
        core.ties("Actor", "members", cy)
            .await
            .expect("cy")
            .is_empty()
    );
    assert!(
        core.ties("Actor", "members_closure", cy)
            .await
            .expect("cy closure")
            .is_empty()
    );
}

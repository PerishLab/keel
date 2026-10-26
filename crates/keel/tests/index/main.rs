use keel::adapt::Error;
use keel::bind;
use keel::estate::Fault;
use keel::wire::Val;

#[path = "../support/mod.rs"]
mod support;

mod world;
use world::*;

async fn holds(spot: &Spot, names: &[&str]) {
    let found = spot.names().await;
    for name in names {
        assert!(found.contains(&name.to_string()), "{name} in {found:?}");
    }
}

#[tokio::test]
async fn fresh() {
    let spot = Spot::new("fresh");
    drop(
        support::boot(shop(), spot.wire().await)
            .await
            .expect("bind"),
    );
    holds(&spot, &NAMES).await;
    let kids = spot
        .plan("SELECT id FROM \"shop_item\" WHERE \"shop_id\" = ?1 AND (expires_at IS NULL OR expires_at > ?2)")
        .await;
    assert!(kids.contains("ref_shop_item_shop_id"), "{kids}");
    let feeds = spot
        .plan("SELECT 1 FROM \"shop_item_tags\" AS t WHERE t.\"tag_id\" = ?1 AND (t.expires_at IS NULL OR t.expires_at > ?2) AND EXISTS (SELECT 1 FROM \"shop_item\" AS h WHERE h.id = t.\"item_id\" AND (h.expires_at IS NULL OR h.expires_at > ?2)) LIMIT 1")
        .await;
    assert!(feeds.contains("ref_shop_item_tags_tag_id"), "{feeds}");
}

#[tokio::test]
async fn gained() {
    let spot = Spot::new("gained");
    spot.aged().await;
    let before = spot.held().await;

    let core = bind(shop(), spot.wire().await).await.expect("gain");
    assert_eq!(core.put("Shop", &[("name", "lab")]).await.expect("shop"), 1);
    drop(core);
    holds(&spot, &NAMES).await;
    let after = spot.held().await;
    assert_eq!(after[0][0], before[0][0]);
    assert_ne!(after[0][1], before[0][1]);
    assert_eq!(after[1..], before[1..]);

    drop(bind(shop(), spot.wire().await).await.expect("again"));
    assert_eq!(spot.held().await, after);
}

const EVOLVED: [&str; 4] = [
    "ref_@unit_@bond_unit_id",
    "ref_g2_shop_item_shop_id",
    "ref_g2_shop_item_tags_item_id",
    "ref_g2_shop_item_tags_tag_id",
];

#[tokio::test]
async fn evolved() {
    let spot = Spot::new("evolved");
    spot.aged().await;
    match bind(grown(), spot.old().await).await {
        Err(Error::Adapt(note)) => assert_eq!(note, "no in-place step"),
        Err(err) => panic!("unexpected {err}"),
        Ok(_) => panic!("expected the old Keel to stop after evolution"),
    }
    assert_eq!(spot.held().await[0][0], Val::Int(2));
    assert!(spot.names().await.is_empty());

    drop(bind(grown(), spot.wire().await).await.expect("gain"));
    holds(&spot, &EVOLVED).await;
    let after = spot.held().await;
    drop(bind(grown(), spot.wire().await).await.expect("again"));
    assert_eq!(spot.held().await, after);
}

#[tokio::test]
async fn changed() {
    let spot = Spot::new("changed");
    drop(
        support::boot(shop(), spot.wire().await)
            .await
            .expect("bind"),
    );
    drop(bind(grown(), spot.wire().await).await.expect("evolve"));
    holds(&spot, &EVOLVED).await;
    let after = spot.held().await;
    drop(bind(grown(), spot.wire().await).await.expect("again"));
    assert_eq!(spot.held().await, after);
}

#[tokio::test]
async fn stray() {
    let spot = Spot::new("stray");
    let mut wire = spot.old().await;
    wire.also = Some("CREATE INDEX \"stray\" ON \"shop\" (\"name\");");
    drop(support::boot(shop(), wire).await.expect("stray estate"));
    spot.drifts().await;
}

#[tokio::test]
async fn missing() {
    let spot = Spot::new("missing");
    let mut wire = spot.old().await;
    wire.skip = Some("CREATE TABLE IF NOT EXISTS \"shop_item_tags\"");
    drop(support::boot(shop(), wire).await.expect("missing estate"));
    spot.drifts().await;
}

#[tokio::test]
async fn refused() {
    let spot = Spot::new("refused");
    spot.aged().await;
    let before = spot.held().await;
    let wire = Refuse {
        wire: spot.wire().await,
    };
    match bind(shop(), wire).await {
        Err(Error::Estate(Fault::Denied { path, note })) => {
            assert_eq!(path, "ref_shop_item_shop_id");
            assert!(note.contains("injected index failure"), "{note}");
        }
        Err(err) => panic!("unexpected {err}"),
        Ok(_) => panic!("expected refusal"),
    }
    assert_eq!(spot.held().await, before);
    assert!(spot.names().await.is_empty());

    drop(bind(shop(), spot.wire().await).await.expect("retry"));
    holds(&spot, &NAMES).await;
}

use super::world::*;
use keel::adapt::Error;
use keel::adapt::db::Sqlite;
use keel::bind;
use keel::estate::{Check, Fault};

mod frozen;
mod rule;
mod value;

#[tokio::test]
async fn unique() {
    let path = spot("unique");
    let core = crate::support::boot(graph::<Loose>(), Sqlite::file(&path).await.expect("first"))
        .await
        .expect("bind");
    core.put("Label", &[("code", "same")]).await.expect("one");
    let two = core.put("Label", &[("code", "same")]).await.expect("two");
    drop(core);

    match bind(graph::<Sole>(), Sqlite::file(&path).await.expect("blocked")).await {
        Err(Error::Estate(Fault::Blocked { path, check })) => {
            assert_eq!(path, "label.code");
            assert_eq!(check, Check::Unique);
        }
        Err(err) => panic!("unexpected {err}"),
        Ok(_) => panic!("expected unique block"),
    }

    let core = bind(graph::<Loose>(), Sqlite::file(&path).await.expect("repair"))
        .await
        .expect("reopen");
    core.set("Label", two, &[("code", "other")])
        .await
        .expect("repair");
    drop(core);
    let core = bind(graph::<Sole>(), Sqlite::file(&path).await.expect("evolve"))
        .await
        .expect("evolve");
    assert!(core.put("Label", &[("code", "same")]).await.is_err());
    drop(core);
    clean(&path);
}

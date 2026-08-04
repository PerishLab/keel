use super::*;
use keel::adapt::Error;
use keel::ddl::Grain;
use keel::wire::{Val, Wire};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

struct Fail<W> {
    wire: W,
    armed: Arc<AtomicBool>,
}

impl<W: Wire> Wire for Fail<W> {
    fn grain(&self) -> Grain {
        self.wire.grain()
    }

    async fn run(&mut self, sql: &str, args: &[Val]) -> Result<u64, Error> {
        if self.armed.load(Ordering::SeqCst)
            && (sql.starts_with("INSERT INTO \"actor_members_closure\"")
                || sql.starts_with("UPDATE \"actor_members_closure\""))
        {
            return Err(Error::Adapt("injected closure failure".into()));
        }
        self.wire.run(sql, args).await
    }

    async fn plant(&mut self, sql: &str, args: &[Val]) -> Result<i64, Error> {
        self.wire.plant(sql, args).await
    }

    async fn rows(&mut self, sql: &str, args: &[Val]) -> Result<Vec<Vec<Val>>, Error> {
        self.wire.rows(sql, args).await
    }

    async fn script(&mut self, sql: &str) -> Result<(), Error> {
        self.wire.script(sql).await
    }
}

#[tokio::test]
async fn batch() {
    let armed = Arc::new(AtomicBool::new(false));
    let wire = Fail {
        wire: Sqlite::memory().await.expect("db"),
        armed: armed.clone(),
    };
    let core = crate::support::boot(graph(), wire).await.expect("bind");
    let ids = seed(&core, &["ada", "bob", "cy"]).await;
    let (ada, bob, cy) = (ids[0], ids[1], ids[2]);
    let failed: Result<(), Error> = core
        .batch(async |tx| {
            tx.tie("Actor", "members", ends(ada, bob), &[]).await?;
            armed.store(true, Ordering::SeqCst);
            tx.tie("Actor", "members", ends(bob, cy), &[]).await?;
            Ok(())
        })
        .await;
    assert!(failed.is_err());
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
}

#[tokio::test]
async fn cutback() {
    let armed = Arc::new(AtomicBool::new(false));
    let wire = Fail {
        wire: Sqlite::memory().await.expect("db"),
        armed: armed.clone(),
    };
    let core = crate::support::boot(graph(), wire).await.expect("bind");
    let ids = seed(&core, &["ada", "bob", "cy"]).await;
    let (ada, bob, cy) = (ids[0], ids[1], ids[2]);
    core.tie("Actor", "members", ends(ada, bob), &[])
        .await
        .expect("ab");
    let bc = core
        .tie("Actor", "members", ends(bob, cy), &[])
        .await
        .expect("bc");
    armed.store(true, Ordering::SeqCst);
    assert!(core.cut("Actor", "members", bc).await.is_err());
    assert_eq!(
        core.ties("Actor", "members", bob).await.expect("direct")[0].right(),
        cy
    );
    assert_eq!(
        core.ties("Actor", "members_closure", ada)
            .await
            .expect("closure")
            .len(),
        2
    );
}

fn denial<T>(result: Result<T, Error>) {
    match result {
        Err(Error::Adapt(note)) => assert!(note.contains("engine owned"), "{note}"),
        Err(err) => panic!("unexpected {err}"),
        Ok(_) => panic!("expected denial"),
    }
}

#[tokio::test]
async fn rollback() {
    let armed = Arc::new(AtomicBool::new(false));
    let wire = Fail {
        wire: Sqlite::memory().await.expect("db"),
        armed: armed.clone(),
    };
    let core = crate::support::boot(graph(), wire).await.expect("bind");
    let ada = core.put("Actor", &[("name", "ada")]).await.expect("ada");
    let bob = core.put("Actor", &[("name", "bob")]).await.expect("bob");
    armed.store(true, Ordering::SeqCst);
    assert!(
        core.tie("Actor", "members", ends(ada, bob), &[])
            .await
            .is_err()
    );
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
    armed.store(false, Ordering::SeqCst);
    assert_eq!(
        core.tie("Actor", "members", ends(ada, bob), &[])
            .await
            .expect("retry"),
        1
    );
    assert_eq!(
        core.ties("Actor", "members_closure", ada)
            .await
            .expect("refreshed")[0]
            .key(),
        1
    );
}

#[tokio::test]
async fn owned() {
    let core = boot().await;
    let ids = seed(&core, &["ada", "bob", "cy"]).await;
    let (ada, bob, cy) = (ids[0], ids[1], ids[2]);
    for verb in ["see", "tie", "cut"] {
        core.put(
            "@grant",
            &[
                ("who", &ada.to_string()),
                ("verb", verb),
                ("unit", "Actor"),
                ("scope", "all"),
            ],
        )
        .await
        .expect("grant");
    }
    let face = core.of(ada);
    face.tie("Actor", "members", ends(ada, bob), &[])
        .await
        .expect("operator direct");
    let key = face
        .ties("Actor", "members_closure", ada)
        .await
        .expect("operator read")[0]
        .key();
    let pack = face
        .query(&format!(
            r#"from Actor where members_closure has "{bob}" link members_closure"#
        ))
        .await
        .expect("operator query");
    assert_eq!(pack.rows().len(), 1);

    denial(
        core.tie("Actor", "members_closure", ends(ada, cy), &[])
            .await,
    );
    denial(core.tune("Actor", "members_closure", key, &[]).await);
    denial(core.cut("Actor", "members_closure", key).await);
    denial(
        face.tie("Actor", "members_closure", ends(ada, cy), &[])
            .await,
    );
    denial(face.tune("Actor", "members_closure", key, &[]).await);
    denial(face.cut("Actor", "members_closure", key).await);
    assert_eq!(
        core.ties("Actor", "members_closure", ada)
            .await
            .expect("still live")
            .len(),
        1
    );
}

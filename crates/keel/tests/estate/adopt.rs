use super::world::*;
use keel::adapt::Error;
use keel::adapt::db::Sqlite;
use keel::ddl::Grain;
use keel::estate::Fault;
use keel::wire::{Val, Wire};
use keel::{Ends, bind};

struct Fail<W> {
    wire: W,
}

impl<W: Wire> Wire for Fail<W> {
    fn grain(&self) -> Grain {
        self.wire.grain()
    }

    async fn run(&mut self, sql: &str, args: &[Val]) -> Result<u64, Error> {
        self.wire.run(sql, args).await
    }

    async fn plant(&mut self, sql: &str, args: &[Val]) -> Result<i64, Error> {
        self.wire.plant(sql, args).await
    }

    async fn rows(&mut self, sql: &str, args: &[Val]) -> Result<Vec<Vec<Val>>, Error> {
        self.wire.rows(sql, args).await
    }

    async fn script(&mut self, sql: &str) -> Result<(), Error> {
        if sql.contains("CREATE TABLE \"@generation\"") {
            return Err(Error::Adapt("injected adoption failure".into()));
        }
        self.wire.script(sql).await
    }
}

async fn unseal(path: &std::path::Path) {
    let mut wire = Sqlite::file(path).await.expect("unseal");
    for table in ["@estate", "@generation", "@clock", "@derivative"] {
        wire.script(&format!("DROP TABLE {}", keel::ddl::col(table)))
            .await
            .expect("drop catalog");
    }
}

#[tokio::test]
async fn exact() {
    let path = spot("adopt-exact");
    let core = crate::support::boot(
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
    let pulse = core
        .flow(0)
        .await
        .expect("flow")
        .last()
        .expect("pulse")
        .key();
    drop(core);
    unseal(&path).await;

    let core = bind(
        pair::<Club, Member>(),
        Sqlite::file(&path).await.expect("adopt"),
    )
    .adopt()
    .await
    .expect("adopt");
    assert!(core.has("@estate").await.expect("estate"));
    assert_eq!(core.live("Member").await.expect("member")[0].key(), member);
    let next = core
        .put("Club", &[("name", "other")])
        .await
        .expect("next club");
    assert_eq!(next, club + 1);
    let knot = core
        .tie(
            "Member",
            "clubs",
            Ends {
                left: member,
                right: next,
            },
            &[("role", "guest")],
        )
        .await
        .expect("next tie");
    assert_eq!(knot, tie + 1);
    let tail = core.flow(pulse).await.expect("tail");
    assert_eq!(tail.len(), 2);
    assert_eq!(tail[0].key(), pulse + 1);
    assert_eq!(tail[1].key(), pulse + 2);
    drop(core);
    clean(&path);
}

#[tokio::test]
async fn serial() {
    let path = spot("adopt-serial");
    let core = crate::support::boot(
        pair::<Ledger, Ticket>(),
        Sqlite::file(&path).await.expect("first"),
    )
    .await
    .expect("bind");
    let repo = core.put("Ledger", &[("name", "keel")]).await.expect("repo");
    for title in ["one", "two"] {
        core.put("Ticket", &[("title", title), ("repo", &repo.to_string())])
            .await
            .expect("issue");
    }
    drop(core);
    unseal(&path).await;

    let core = bind(
        pair::<Ledger, Ticket>(),
        Sqlite::file(&path).await.expect("adopt"),
    )
    .adopt()
    .await
    .expect("adopt");
    let issue = core
        .put("Ticket", &[("title", "three"), ("repo", &repo.to_string())])
        .await
        .expect("issue");
    assert_eq!(issue, 3);
    let rows = core.live("Ticket").await.expect("issues");
    assert_eq!(rows.last().and_then(|row| row.int("index")), Some(3));
    drop(core);
    clean(&path);
}

#[tokio::test]
async fn drift() {
    let path = spot("adopt-drift");
    let core = crate::support::boot(graph::<Alpha>(), Sqlite::file(&path).await.expect("first"))
        .await
        .expect("bind");
    drop(core);
    unseal(&path).await;
    let mut wire = Sqlite::file(&path).await.expect("mutate");
    wire.script("ALTER TABLE alpha ADD stray TEXT")
        .await
        .expect("drift");
    drop(wire);

    match bind(graph::<Alpha>(), Sqlite::file(&path).await.expect("adopt"))
        .adopt()
        .await
    {
        Err(Error::Estate(Fault::Drift { expected, found })) => {
            assert_ne!(expected, found);
        }
        Err(err) => panic!("unexpected {err}"),
        Ok(_) => panic!("expected drift"),
    }
    let mut wire = Sqlite::file(&path).await.expect("inspect");
    let rows = wire
        .rows("SELECT name FROM sqlite_master WHERE name = '@estate'", &[])
        .await
        .expect("catalog");
    assert!(rows.is_empty());
    drop(wire);
    clean(&path);
}

#[tokio::test]
async fn rollback() {
    let path = spot("adopt-rollback");
    let core = crate::support::boot(graph::<Alpha>(), Sqlite::file(&path).await.expect("first"))
        .await
        .expect("bind");
    core.put("Alpha", &[("alpha", "1"), ("zeta", "held")])
        .await
        .expect("put");
    drop(core);
    unseal(&path).await;

    let failed = Fail {
        wire: Sqlite::file(&path).await.expect("failed"),
    };
    assert!(bind(graph::<Alpha>(), failed).adopt().await.is_err());

    let core = bind(graph::<Alpha>(), Sqlite::file(&path).await.expect("retry"))
        .adopt()
        .await
        .expect("retry");
    assert_eq!(core.live("Alpha").await.expect("alpha").len(), 1);
    assert_eq!(
        core.put("Alpha", &[("alpha", "2"), ("zeta", "next")])
            .await
            .expect("next"),
        2
    );
    drop(core);
    clean(&path);
}

use keel::adapt::db::Sqlite;
use keel::atom::string;
use keel::resource;
use keel::{Ends, Graph, bind};

#[resource]
struct Actor {
    #[field(string, unique)]
    login: string,
}

#[resource]
struct Team {
    #[field(string, unique = org)]
    name: string,
    #[relation(Actor, many2one, root)]
    org: Actor,
    #[relation(Actor, many2many, crew)]
    members: Actor,
}

#[tokio::test]
async fn whole() {
    let mut graph = Graph::new();
    graph.plug::<Actor>().plug::<Team>();
    let core = bind(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind");
    let ada = core.put("Actor", &[("login", "ada")]).await.expect("ada");
    core.put(
        "@grant",
        &[
            ("who", &ada.to_string()),
            ("verb", "put"),
            ("unit", "Actor"),
            ("scope", "all"),
        ],
    )
    .await
    .expect("seed");
    core.put(
        "@grant",
        &[
            ("who", "all"),
            ("verb", "see"),
            ("unit", "Actor"),
            ("scope", "all"),
        ],
    )
    .await
    .expect("see seed");
    let boss = core.of(ada);

    let (org, team) = boss
        .batch(async |tx| {
            let org = tx.put("Actor", &[("login", "lab")]).await?;
            let team = tx
                .put("Team", &[("name", "owners"), ("org", &org.to_string())])
                .await?;
            tx.tie(
                "Team",
                "members",
                Ends {
                    left: team,
                    right: ada,
                },
                &[],
            )
            .await?;
            tx.put(
                "@grant",
                &[
                    ("who", &format!("team {team}")),
                    ("verb", "see"),
                    ("unit", "Actor"),
                    ("scope", &format!("row {org}")),
                ],
            )
            .await?;
            Ok((org, team))
        })
        .await
        .expect("batch");

    assert_eq!(core.live("Actor").await.expect("live").len(), 2);
    assert_eq!(core.live("Team").await.expect("teams").len(), 1);
    let seen = core.of(ada).live("Actor").await.expect("member sees");
    assert!(seen.iter().any(|row| row.key() == org));
    let _ = team;
}

#[tokio::test]
async fn undo() {
    let mut graph = Graph::new();
    graph.plug::<Actor>().plug::<Team>();
    let core = bind(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind");

    let out = core
        .batch(async |tx| {
            tx.put("Actor", &[("login", "lab")]).await?;
            tx.put("Actor", &[("login", "lab")]).await
        })
        .await;
    assert!(out.is_err());
    assert_eq!(core.live("Actor").await.expect("empty").len(), 0);
    let flow = core.flow(0).await.expect("flow");
    assert!(flow.iter().all(|row| {
        row.cells()
            .get("unit")
            .map(|c| c.show())
            .unwrap_or_default()
            != "actor"
    }));

    core.put("Actor", &[("login", "lab")])
        .await
        .expect("free after undo");
}

#[tokio::test]
async fn cancel() {
    let mut graph = Graph::new();
    graph.plug::<Actor>().plug::<Team>();
    let core = bind(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind");

    let stuck = core.batch(async |tx| {
        tx.put("Actor", &[("login", "lab")]).await?;
        std::future::pending::<()>().await;
        Ok(())
    });
    let killed = tokio::time::timeout(std::time::Duration::from_millis(50), stuck).await;
    assert!(killed.is_err());

    assert_eq!(core.live("Actor").await.expect("live").len(), 0);
    core.put("Actor", &[("login", "lab")])
        .await
        .expect("free after cancel");
}

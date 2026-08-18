use keel::adapt::db::Sqlite;
use keel::atom::string;
use keel::resource;
use keel::{Ends, Graph};

#[resource]
struct Actor {
    #[field(string, unique)]
    login: string,
}

#[resource]
struct Team {
    #[field(string)]
    name: string,
    #[relation(Actor, many2one, root)]
    org: Actor,
    #[relation(Actor, many2many, crew)]
    members: Actor,
}

#[resource]
struct Repo {
    #[field(string)]
    name: string,
    #[relation(Actor, many2one, root)]
    owner: Actor,
}

#[tokio::test]
async fn group() {
    let mut graph = Graph::new();
    graph.plug::<Actor>().plug::<Team>().plug::<Repo>();
    let core = crate::support::boot(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind");
    let sudo = core.sudo();
    let boss = sudo.put("Actor", &[("login", "boss")]).await.expect("boss");
    let ada = sudo.put("Actor", &[("login", "ada")]).await.expect("ada");
    let bob = sudo.put("Actor", &[("login", "bob")]).await.expect("bob");

    let crew = sudo
        .put("Team", &[("name", "core"), ("org", &boss.to_string())])
        .await
        .expect("team");
    let vault = sudo
        .put("Repo", &[("name", "vault"), ("owner", &boss.to_string())])
        .await
        .expect("repo");
    sudo.put(
        "@grant",
        &[
            ("who", &format!("team {crew}")),
            ("verb", "see"),
            ("unit", "Repo"),
            ("scope", &format!("row {vault}")),
        ],
    )
    .await
    .expect("group grant");

    assert_eq!(core.of(ada).live("Repo").await.expect("pre").len(), 0);
    sudo.tie(
        "Team",
        "members",
        Ends {
            left: crew,
            right: ada,
        },
        &[],
    )
    .await
    .expect("join");
    assert_eq!(core.of(ada).live("Repo").await.expect("member").len(), 1);
    assert_eq!(core.of(bob).live("Repo").await.expect("stranger").len(), 0);

    let ties = sudo.ties("Team", "members", crew).await.expect("ties");
    sudo.cut("Team", "members", ties[0].key())
        .await
        .expect("leave");
    assert_eq!(core.of(ada).live("Repo").await.expect("left").len(), 0);

    sudo.tie(
        "Team",
        "members",
        Ends {
            left: crew,
            right: bob,
        },
        &[],
    )
    .await
    .expect("rejoin");
    assert_eq!(core.of(bob).live("Repo").await.expect("member").len(), 1);
    sudo.end("Team", crew).await.expect("dissolve");
    assert_eq!(core.of(bob).live("Repo").await.expect("dissolved").len(), 0);
    let held = sudo.ties("Team", "members", crew).await.expect("ties");
    assert_eq!(held.len(), 1);

    assert!(
        sudo.put(
            "@grant",
            &[
                ("who", "ghost 9"),
                ("verb", "see"),
                ("unit", "Repo"),
                ("scope", "all"),
            ],
        )
        .await
        .is_err()
    );
}

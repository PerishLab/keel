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

#[test]
fn whole() {
    let mut graph = Graph::new();
    graph.plug::<Actor>().plug::<Team>();
    let core = bind(graph, Sqlite::memory()).expect("bind");
    let ada = core.put("Actor", &[("login", "ada")]).expect("ada");
    core.put(
        "@grant",
        &[
            ("who", &ada.to_string()),
            ("verb", "put"),
            ("unit", "Actor"),
            ("scope", "all"),
        ],
    )
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
    .expect("see seed");
    let boss = core.of(ada);

    let (org, team) = boss
        .batch(|tx| {
            let org = tx.put("Actor", &[("login", "lab")])?;
            let team = tx.put("Team", &[("name", "owners"), ("org", &org.to_string())])?;
            tx.tie(
                "Team",
                "members",
                Ends {
                    left: team,
                    right: ada,
                },
                &[],
            )?;
            tx.put(
                "@grant",
                &[
                    ("who", &format!("team {team}")),
                    ("verb", "see"),
                    ("unit", "Actor"),
                    ("scope", &format!("row {org}")),
                ],
            )?;
            Ok((org, team))
        })
        .expect("batch");

    assert_eq!(core.live("Actor").expect("live").len(), 2);
    assert_eq!(core.live("Team").expect("teams").len(), 1);
    let seen = core.of(ada).live("Actor").expect("member sees");
    assert!(seen.iter().any(|row| row.key() == org));
    let _ = team;
}

#[test]
fn undo() {
    let mut graph = Graph::new();
    graph.plug::<Actor>().plug::<Team>();
    let core = bind(graph, Sqlite::memory()).expect("bind");

    let out = core.batch(|tx| {
        tx.put("Actor", &[("login", "lab")])?;
        tx.put("Actor", &[("login", "lab")])
    });
    assert!(out.is_err());
    assert_eq!(core.live("Actor").expect("empty").len(), 0);
    let flow = core.flow(0).expect("flow");
    assert!(flow.iter().all(|row| {
        row.cells()
            .get("unit")
            .map(|c| c.show())
            .unwrap_or_default()
            != "actor"
    }));

    core.put("Actor", &[("login", "lab")])
        .expect("free after undo");
}

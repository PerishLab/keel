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

#[test]
fn group() {
    let mut graph = Graph::new();
    graph.plug::<Actor>().plug::<Team>().plug::<Repo>();
    let core = bind(graph, Sqlite::memory()).expect("bind");
    let sudo = core.sudo();
    let boss = sudo.put("Actor", &[("login", "boss")]).expect("boss");
    let ada = sudo.put("Actor", &[("login", "ada")]).expect("ada");
    let bob = sudo.put("Actor", &[("login", "bob")]).expect("bob");

    let crew = sudo
        .put("Team", &[("name", "core"), ("org", &boss.to_string())])
        .expect("team");
    let vault = sudo
        .put("Repo", &[("name", "vault"), ("owner", &boss.to_string())])
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
    .expect("group grant");

    assert_eq!(core.of(ada).live("Repo").expect("pre").len(), 0);
    sudo.tie(
        "Team",
        "members",
        Ends {
            left: crew,
            right: ada,
        },
        &[],
    )
    .expect("join");
    assert_eq!(core.of(ada).live("Repo").expect("member").len(), 1);
    assert_eq!(core.of(bob).live("Repo").expect("stranger").len(), 0);

    let ties = sudo.ties("Team", "members", crew).expect("ties");
    sudo.cut("Team", "members", ties[0].key()).expect("leave");
    assert_eq!(core.of(ada).live("Repo").expect("left").len(), 0);

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
        .is_err()
    );
}

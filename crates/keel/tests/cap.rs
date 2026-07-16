use keel::adapt::db::Sqlite;
use keel::atom::string;
use keel::resource;
use keel::{Graph, Who, bind};

#[resource]
struct Actor {
    #[field(string, unique)]
    login: string,
}

#[resource]
struct Repo {
    #[field(string, unique = owner)]
    name: string,
    #[field(string)]
    visibility: string,
    #[relation(Actor, many2one, root)]
    owner: Actor,
}

#[resource]
struct Issue {
    #[field(string)]
    title: string,
    #[relation(Repo, many2one, root)]
    repo: Repo,
    #[relation(Actor, many2one)]
    author: Actor,
}

#[test]
fn grant() {
    let mut graph = Graph::new();
    graph.plug::<Actor>();
    let core = bind(graph, Sqlite::memory()).expect("bind");
    let sudo = core.sudo();
    assert_eq!(sudo.who(), Who::Sudo);

    let row = sudo
        .put(
            "@grant",
            &[
                ("who", "anon"),
                ("verb", "see"),
                ("unit", "Actor"),
                ("scope", "all"),
            ],
        )
        .expect("seed");
    let rows = sudo.live("@grant").expect("live");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].key(), row);

    let pack = sudo.query("from @grant count").expect("audit");
    assert_eq!(pack.count(), Some(1));

    sudo.end("@grant", row).expect("revoke");
    assert_eq!(sudo.live("@grant").expect("live").len(), 0);

    assert!(sudo.set("@grant", row, &[("verb", "put")]).is_err());
}

#[test]
fn vet() {
    let mut graph = Graph::new();
    graph.plug::<Actor>();
    let core = bind(graph, Sqlite::memory()).expect("bind");
    let sudo = core.sudo();
    let seed = |who: &str, verb: &str, unit: &str, scope: &str| {
        sudo.put(
            "@grant",
            &[
                ("who", who),
                ("verb", verb),
                ("unit", unit),
                ("scope", scope),
            ],
        )
    };
    assert!(seed("anon", "grow", "Actor", "all").is_err());
    assert!(seed("someone", "see", "Actor", "all").is_err());
    assert!(seed("anon", "see", "Ghost", "all").is_err());
    assert!(seed("anon", "see", "Actor", "sometimes").is_err());
    assert!(seed("anon", "see", "*", r#"pred login = "a""#).is_err());
    assert!(seed("anon", "see", "Actor", "row x").is_err());
    seed("all", "*", "*", "all").expect("wildcard");
    seed("7", "put", "Actor", "row 3").expect("row scope");
    seed("7", "set", "Actor", r#"pred login = "@me""#).expect("pred scope");
}

#[test]
fn faces() {
    let mut graph = Graph::new();
    graph.plug::<Actor>();
    let core = bind(graph, Sqlite::memory()).expect("bind");
    let ada = core.put("Actor", &[("login", "ada")]).expect("ada");

    let op = core.of(ada);
    assert_eq!(op.who(), Who::Op(ada));
    assert!(op.put("Actor", &[("login", "eve")]).is_err());
    assert_eq!(op.live("Actor").expect("live").len(), 0);
    assert_eq!(core.anon().query("from Actor").expect("q").rows().len(), 0);
    assert_eq!(core.anon().who(), Who::Anon);
}

#[test]
fn birth() {
    let mut graph = Graph::new();
    graph.plug::<Actor>();
    let core = bind(graph, Sqlite::memory())
        .expect("bind")
        .identify("Actor")
        .expect("identify");
    let sudo = core.sudo();
    sudo.put(
        "@grant",
        &[
            ("who", "anon"),
            ("verb", "put"),
            ("unit", "Actor"),
            ("scope", "all"),
        ],
    )
    .expect("register seed");

    let eve = core
        .anon()
        .put("Actor", &[("login", "eve")])
        .expect("register");
    let own = core.of(eve);
    own.set("Actor", eve, &[("login", "eva")])
        .expect("newborn owns itself");
    assert_eq!(own.live("Actor").expect("live").len(), 1);
    assert!(
        core.of(eve + 1)
            .set("Actor", eve, &[("login", "x")])
            .is_err()
    );
}

#[test]
fn cover() {
    let mut graph = Graph::new();
    graph.plug::<Actor>().plug::<Repo>().plug::<Issue>();
    let core = bind(graph, Sqlite::memory()).expect("bind");
    let sudo = core.sudo();
    let ada = sudo.put("Actor", &[("login", "ada")]).expect("ada");
    let bob = sudo.put("Actor", &[("login", "bob")]).expect("bob");
    let seed = |who: &str, verb: &str, unit: &str, scope: &str| {
        sudo.put(
            "@grant",
            &[
                ("who", who),
                ("verb", verb),
                ("unit", unit),
                ("scope", scope),
            ],
        )
        .expect("seed");
    };
    seed(&ada.to_string(), "put", "Repo", "all");
    seed("anon", "see", "Repo", r#"pred visibility = "public""#);
    seed("all", "put", "Issue", r#"pred author = "@me""#);
    seed("all", "set", "Issue", r#"pred author = "@me""#);
    seed("all", "see", "Issue", r#"pred author = "@me""#);

    let her = core.of(ada);
    let him = core.of(bob);

    let repo = her
        .put(
            "Repo",
            &[
                ("name", "keel"),
                ("visibility", "private"),
                ("owner", &ada.to_string()),
            ],
        )
        .expect("mint on put");
    assert!(
        him.put(
            "Repo",
            &[
                ("name", "mast"),
                ("visibility", "public"),
                ("owner", &bob.to_string()),
            ],
        )
        .is_err()
    );

    assert_eq!(her.live("Repo").expect("live").len(), 1);
    assert_eq!(him.live("Repo").expect("live").len(), 0);
    her.set("Repo", repo, &[("visibility", "public")])
        .expect("owner sets");
    assert_eq!(him.live("Repo").expect("live").len(), 1);
    assert_eq!(
        him.query("from Repo count").expect("count").count(),
        Some(1)
    );
    assert!(him.set("Repo", repo, &[("name", "grab")]).is_err());

    let task = him
        .put(
            "Issue",
            &[
                ("title", "hello"),
                ("repo", &repo.to_string()),
                ("author", &bob.to_string()),
            ],
        )
        .expect("pred put");
    assert!(
        him.put(
            "Issue",
            &[
                ("title", "fake"),
                ("repo", &repo.to_string()),
                ("author", &ada.to_string()),
            ],
        )
        .is_err()
    );
    him.set("Issue", task, &[("title", "hey")]).expect("own");
    him.set("Issue", task, &[("author", &ada.to_string())])
        .expect("mint outranks pred scopes on own row");
    him.set("Issue", task, &[("author", &bob.to_string())])
        .expect("back");

    let mine = her
        .put(
            "Issue",
            &[
                ("title", "chain"),
                ("repo", &repo.to_string()),
                ("author", &ada.to_string()),
            ],
        )
        .expect("her issue");
    her.set("Issue", mine, &[("title", "subtree")])
        .expect("subtree set");
    her.set("Issue", task, &[("title", "mod")])
        .expect("subtree covers bob issue");

    her.put(
        "@grant",
        &[
            ("who", &bob.to_string()),
            ("verb", "see"),
            ("unit", "Repo"),
            ("scope", &format!("row {repo}")),
        ],
    )
    .expect("attenuated grant");
    assert!(
        him.put(
            "@grant",
            &[
                ("who", "all"),
                ("verb", "put"),
                ("unit", "Repo"),
                ("scope", "all"),
            ],
        )
        .is_err()
    );
    assert!(
        her.put(
            "@grant",
            &[
                ("who", "all"),
                ("verb", "see"),
                ("unit", "*"),
                ("scope", "all"),
            ],
        )
        .is_err()
    );

    assert_eq!(her.live("Issue").expect("live").len(), 2);
    assert_eq!(him.live("Issue").expect("live").len(), 2);
    assert_eq!(core.anon().live("Issue").expect("live").len(), 0);
}

use keel::adapt::db::Sqlite;
use keel::atom::string;
use keel::resource;
use keel::{Graph, Who, bind};

#[resource]
struct Actor {
    #[field(string, unique)]
    login: string,
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
    assert!(op.live("Actor").is_err());
    assert!(core.anon().query("from Actor").is_err());
    assert_eq!(core.anon().who(), Who::Anon);
}

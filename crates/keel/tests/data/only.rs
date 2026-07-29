use keel::adapt::db::Sqlite;
use keel::atom::{int, string};
use keel::resource;
use keel::{Cell, Graph};

#[resource]
struct Org {
    #[field(string, unique)]
    slug: string,
}

#[resource]
struct Repo {
    #[field(string, unique = org)]
    name: string,
    #[relation(Org, many2one)]
    org: Org,
}

#[tokio::test]
async fn sole() {
    let mut graph = Graph::new();
    graph.plug::<Org>();
    let core = crate::support::boot(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind");
    let lab = core.put("Org", &[("slug", "lab")]).await.expect("lab");
    assert!(core.put("Org", &[("slug", "lab")]).await.is_err());
    core.put("Org", &[("slug", "hub")]).await.expect("hub");
    core.end("Org", lab).await.expect("end lab");
    core.put("Org", &[("slug", "lab")]).await.expect("retake");
}

#[tokio::test]
async fn per() {
    let mut graph = Graph::new();
    graph.plug::<Org>().plug::<Repo>();
    let core = crate::support::boot(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind");
    let lab = core.put("Org", &[("slug", "lab")]).await.expect("lab");
    let hub = core.put("Org", &[("slug", "hub")]).await.expect("hub");

    let keel = core
        .put("Repo", &[("name", "keel"), ("org", &lab.to_string())])
        .await
        .expect("keel");
    assert!(
        core.put("Repo", &[("name", "keel"), ("org", &lab.to_string())])
            .await
            .is_err()
    );
    let twin = core
        .put("Repo", &[("name", "keel"), ("org", &hub.to_string())])
        .await
        .expect("same name other org");

    core.put("Repo", &[("name", "mast"), ("org", &lab.to_string())])
        .await
        .expect("mast");
    assert!(core.set("Repo", keel, &[("name", "mast")]).await.is_err());
    core.set("Repo", keel, &[("name", "keel")])
        .await
        .expect("self ok");

    assert!(
        core.set("Repo", twin, &[("org", &lab.to_string())])
            .await
            .is_err()
    );
    core.set("Repo", twin, &[("name", "sail"), ("org", &lab.to_string())])
        .await
        .expect("rename and move");
}

#[resource]
struct Issue {
    #[field(string)]
    title: string,
    #[field(serial, scope = repo)]
    index: int,
    #[relation(Repo, many2one)]
    repo: Repo,
}

#[tokio::test]
async fn tally() {
    let mut graph = Graph::new();
    graph.plug::<Org>().plug::<Repo>().plug::<Issue>();
    let core = crate::support::boot(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind");
    let lab = core.put("Org", &[("slug", "lab")]).await.expect("lab");
    let keel = core
        .put("Repo", &[("name", "keel"), ("org", &lab.to_string())])
        .await
        .expect("keel");
    let mast = core
        .put("Repo", &[("name", "mast"), ("org", &lab.to_string())])
        .await
        .expect("mast");

    let one = core
        .put("Issue", &[("title", "a"), ("repo", &keel.to_string())])
        .await
        .expect("one");
    let two = core
        .put("Issue", &[("title", "b"), ("repo", &keel.to_string())])
        .await
        .expect("two");
    let side = core
        .put("Issue", &[("title", "c"), ("repo", &mast.to_string())])
        .await
        .expect("side");

    let grab = async |key: i64| {
        let rows = core.live("Issue").await.expect("live");
        rows.iter()
            .find(|row| row.key() == key)
            .and_then(|row| row.cells().get("index").cloned())
    };
    assert_eq!(grab(one).await, Some(Cell::Int(1)));
    assert_eq!(grab(two).await, Some(Cell::Int(2)));
    assert_eq!(grab(side).await, Some(Cell::Int(1)));

    core.end("Issue", two).await.expect("end two");
    let three = core
        .put("Issue", &[("title", "d"), ("repo", &keel.to_string())])
        .await
        .expect("three");
    assert_eq!(grab(three).await, Some(Cell::Int(3)));

    assert!(
        core.put(
            "Issue",
            &[("title", "x"), ("index", "9"), ("repo", &keel.to_string())],
        )
        .await
        .is_err()
    );
    assert!(core.set("Issue", one, &[("index", "9")]).await.is_err());

    let pack = core
        .query(&format!(
            r#"from Issue where repo = "{keel}" order by index desc"#
        ))
        .await
        .expect("order");
    assert_eq!(pack.rows()[0].key(), three);
}

#[resource]
struct React {
    #[field(string)]
    channel: string,
    #[field(string, unique = (fan, issue, channel))]
    emoji: string,
    #[relation(Org, many2one, root)]
    fan: Org,
    #[relation(Repo, many2one)]
    issue: Repo,
}

#[tokio::test]
async fn composite() {
    let mut graph = Graph::new();
    graph.plug::<Org>().plug::<Repo>().plug::<React>();
    let core = crate::support::boot(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind");
    let lab = core.put("Org", &[("slug", "lab")]).await.expect("lab");
    let ada = core.put("Org", &[("slug", "ada")]).await.expect("ada");
    let one = core
        .put("Repo", &[("name", "one"), ("org", &lab.to_string())])
        .await
        .expect("one");
    let two = core
        .put("Repo", &[("name", "two"), ("org", &lab.to_string())])
        .await
        .expect("two");

    let seed = async |emoji: &str, channel: &str, actor: i64, issue: i64| {
        core.put(
            "React",
            &[
                ("channel", channel),
                ("emoji", emoji),
                ("fan", &actor.to_string()),
                ("issue", &issue.to_string()),
            ],
        )
        .await
    };
    seed("up", "web", lab, one).await.expect("first");
    seed("tada", "web", lab, one)
        .await
        .expect("same pair other emoji");
    seed("up", "web", ada, one)
        .await
        .expect("other actor same emoji");
    seed("up", "web", lab, two)
        .await
        .expect("other issue same emoji");
    seed("up", "mail", lab, one)
        .await
        .expect("other scalar scope");
    assert!(seed("up", "web", lab, one).await.is_err());

    let react = seed("heart", "web", lab, two).await.expect("live");
    core.end("React", react).await.expect("undo");
    seed("heart", "web", lab, two)
        .await
        .expect("re-react after undo");
}

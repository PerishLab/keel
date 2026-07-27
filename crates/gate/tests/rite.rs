use axum::http::HeaderMap;
use keel::adapt::db::Sqlite;
use keel::atom::string;
use keel::{Graph, bind, resource};
use keel_gate::{Gate, bake};

#[resource]
struct Actor {
    #[field(string, unique)]
    login: string,
    #[field(bool)]
    barred: bool,
}

keel_gate::gate!(Actor);

#[tokio::test]
async fn rites() {
    let mut graph = Graph::new();
    graph.plug::<Actor>();
    plug(&mut graph);
    let core = bind(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind")
        .identify("Actor")
        .expect("identify")
        .share();
    core.sudo()
        .put(
            "@grant",
            &[
                ("who", "anon"),
                ("verb", "put"),
                ("unit", "Actor"),
                ("scope", "all"),
            ],
        )
        .await
        .expect("seed");
    let ada = core
        .anon()
        .put("Actor", &[("login", "ada"), ("barred", "false")])
        .await
        .expect("ada");
    let svc = core
        .put("Actor", &[("login", "svc"), ("barred", "false")])
        .await
        .expect("svc");
    let gate = Gate::rise(core.clone(), svc)
        .await
        .expect("rise")
        .bar("barred");

    let (_, sid) = gate.session(ada).await.expect("session");
    let mut jar = HeaderMap::new();
    jar.insert("cookie", format!("session={sid}").parse().expect("jar"));
    assert_eq!(gate.whom(&jar).await, Some(ada));
    assert!(gate.logout(&sid).await.is_ok());
    assert!(gate.whom(&jar).await.is_none());
    assert!(gate.logout(&sid).await.is_err());

    let pat = gate.token(ada, "cli").await.expect("token");
    let mut head = HeaderMap::new();
    head.insert(
        "authorization",
        format!("token {pat}").parse().expect("head"),
    );
    assert_eq!(gate.whom(&head).await, Some(ada));
    assert!(gate.revoke(&pat).await.is_ok());
    assert!(gate.whom(&head).await.is_none());

    core.set("Actor", ada, &[("barred", "true")])
        .await
        .expect("bar");
    assert!(gate.barred(ada).await);
    let (_, held) = gate.session(ada).await.expect("held");
    let mut worn = HeaderMap::new();
    worn.insert("cookie", format!("session={held}").parse().expect("worn"));
    assert!(gate.whom(&worn).await.is_none());

    assert!(bake("s", true).contains("Secure"));
    assert!(!bake("s", false).contains("Secure"));
}

#[tokio::test]
async fn seeds() {
    let mut graph = Graph::new();
    graph.plug::<Actor>();
    plug(&mut graph);
    let core = bind(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind")
        .identify("Actor")
        .expect("identify")
        .share();
    let svc = core
        .put("Actor", &[("login", "svc"), ("barred", "false")])
        .await
        .expect("svc");
    let gate = Gate::rise(core.clone(), svc).await.expect("rise");

    let eve = gate
        .birth(&[("login", "eve"), ("barred", "false")])
        .await
        .expect("birth");
    core.of(eve)
        .set("Actor", eve, &[("login", "eva")])
        .await
        .expect("newborn owns itself");
    let (_, sid) = gate.session(eve).await.expect("session");
    assert!(gate.logout(&sid).await.is_ok());

    gate.sow(&[("all", "see", "Actor", "all")])
        .await
        .expect("sow");
    gate.sow(&[("all", "see", "Actor", "all")])
        .await
        .expect("resow");
    let held = core
        .query(r#"from @grant where who = "all" count"#)
        .await
        .expect("count");
    assert_eq!(held.count(), Some(1));
}

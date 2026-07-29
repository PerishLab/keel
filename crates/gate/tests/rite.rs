use axum::http::HeaderMap;
use keel::adapt::db::Sqlite;
use keel::atom::string;
use keel::{Core, Graph, Wire, bootstrap, resource};
use keel_gate::{Gate, bake};

#[resource]
struct Actor {
    #[field(string, unique)]
    login: string,
    #[field(bool)]
    barred: bool,
}

keel_gate::gate!(Actor);

async fn boot<W: Wire>(graph: Graph, wire: W) -> Core<W> {
    let mut boot = bootstrap(graph, wire).expect("bootstrap");
    let token = boot.mint().await.expect("mint");
    boot.seal(&token).await.expect("seal")
}

#[tokio::test]
async fn rites() {
    let mut graph = Graph::new();
    graph.plug::<Actor>();
    plug(&mut graph);
    let core = boot(graph, Sqlite::memory().await.expect("db"))
        .await
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
    let gate = Gate::rise(core.clone(), svc).expect("rise").bar("barred");
    assert!(!gate.ready().await.expect("unready"));
    gate.seed().await.expect("seed");
    assert!(gate.ready().await.expect("ready"));

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

    let flow = core.flow(0).await.expect("flow");
    let staff = svc.to_string();
    let guest = ada.to_string();
    let trail: Vec<_> = flow
        .iter()
        .map(|row| {
            (
                row.text("unit").unwrap_or_default(),
                row.text("verb").unwrap_or_default(),
                row.text("who").unwrap_or_default(),
            )
        })
        .collect();
    assert!(
        flow.iter().any(|row| {
            row.text("unit")
                .is_some_and(|unit| unit.ends_with(":session"))
                && row.text("verb") == Some("end")
                && row.text("who") == Some(staff.as_str())
        }),
        "{trail:?}"
    );
    assert!(flow.iter().any(|row| {
        row.text("unit")
            .is_some_and(|unit| unit.ends_with(":token"))
            && row.text("verb") == Some("put")
            && row.text("who") == Some(guest.as_str())
    }));
    assert!(flow.iter().all(|row| {
        !row.text("unit")
            .is_some_and(|unit| unit.ends_with(":session") || unit.ends_with(":token"))
            || row.text("who") != Some("sudo")
    }));

    assert!(bake("s", true).contains("Secure"));
    assert!(!bake("s", false).contains("Secure"));
}

#[tokio::test]
async fn seeds() {
    let mut graph = Graph::new();
    graph.plug::<Actor>();
    plug(&mut graph);
    let core = boot(graph, Sqlite::memory().await.expect("db"))
        .await
        .identify("Actor")
        .expect("identify")
        .share();
    let svc = core
        .put("Actor", &[("login", "svc"), ("barred", "false")])
        .await
        .expect("svc");
    let gate = Gate::rise(core.clone(), svc).expect("rise");
    assert!(!gate.ready().await.expect("unready"));
    gate.seed().await.expect("seed");
    gate.seed().await.expect("reseed");
    assert!(gate.ready().await.expect("ready"));

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

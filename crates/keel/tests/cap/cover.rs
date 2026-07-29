use crate::world::*;
use keel::Graph;
use keel::adapt::db::Sqlite;

#[tokio::test]
async fn cover() {
    let mut graph = Graph::new();
    graph.plug::<Actor>().plug::<Repo>().plug::<Issue>();
    let core = crate::support::boot(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind");
    let sudo = core.sudo();
    let ada = sudo.put("Actor", &[("login", "ada")]).await.expect("ada");
    let bob = sudo.put("Actor", &[("login", "bob")]).await.expect("bob");
    let seed = async |who: &str, verb: &str, unit: &str, scope: &str| {
        sudo.put(
            "@grant",
            &[
                ("who", who),
                ("verb", verb),
                ("unit", unit),
                ("scope", scope),
            ],
        )
        .await
        .expect("seed");
    };
    seed(&ada.to_string(), "put", "Repo", "all").await;
    seed("anon", "see", "Repo", r#"pred visibility = "public""#).await;
    seed("all", "put", "Issue", r#"pred author = "@me""#).await;
    seed("all", "set", "Issue", r#"pred author = "@me""#).await;
    seed("all", "see", "Issue", r#"pred author = "@me""#).await;

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
        .await
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
        .await
        .is_err()
    );

    assert_eq!(her.live("Repo").await.expect("live").len(), 1);
    assert_eq!(him.live("Repo").await.expect("live").len(), 0);
    her.set("Repo", repo, &[("visibility", "public")])
        .await
        .expect("owner sets");
    assert_eq!(him.live("Repo").await.expect("live").len(), 1);
    assert_eq!(
        him.query("from Repo count").await.expect("count").count(),
        Some(1)
    );
    assert!(him.set("Repo", repo, &[("name", "grab")]).await.is_err());

    let task = him
        .put(
            "Issue",
            &[
                ("title", "hello"),
                ("repo", &repo.to_string()),
                ("author", &bob.to_string()),
            ],
        )
        .await
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
        .await
        .is_err()
    );
    him.set("Issue", task, &[("title", "hey")])
        .await
        .expect("own");
    him.set("Issue", task, &[("author", &ada.to_string())])
        .await
        .expect("mint outranks pred scopes on own row");
    him.set("Issue", task, &[("author", &bob.to_string())])
        .await
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
        .await
        .expect("her issue");
    her.set("Issue", mine, &[("title", "subtree")])
        .await
        .expect("subtree set");
    her.set("Issue", task, &[("title", "mod")])
        .await
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
    .await
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
        .await
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
        .await
        .is_err()
    );

    assert_eq!(her.live("Issue").await.expect("live").len(), 2);
    assert_eq!(him.live("Issue").await.expect("live").len(), 2);
    assert_eq!(core.anon().live("Issue").await.expect("live").len(), 2);
}

#[tokio::test]
async fn descend() {
    let mut graph = Graph::new();
    graph.plug::<Actor>().plug::<Repo>().plug::<Issue>();
    let core = crate::support::boot(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind");
    let sudo = core.sudo();
    let ada = sudo.put("Actor", &[("login", "ada")]).await.expect("ada");
    let bob = sudo.put("Actor", &[("login", "bob")]).await.expect("bob");
    sudo.put(
        "@grant",
        &[
            ("who", "all"),
            ("verb", "see"),
            ("unit", "Repo"),
            ("scope", r#"pred visibility = "public""#),
        ],
    )
    .await
    .expect("seed");

    let shut = sudo
        .put(
            "Repo",
            &[
                ("name", "shut"),
                ("visibility", "private"),
                ("owner", &ada.to_string()),
            ],
        )
        .await
        .expect("shut");
    let open = sudo
        .put(
            "Repo",
            &[
                ("name", "open"),
                ("visibility", "public"),
                ("owner", &ada.to_string()),
            ],
        )
        .await
        .expect("open");
    sudo.put(
        "Issue",
        &[
            ("title", "hidden"),
            ("repo", &shut.to_string()),
            ("author", &ada.to_string()),
        ],
    )
    .await
    .expect("hidden");
    let shown = sudo
        .put(
            "Issue",
            &[
                ("title", "shown"),
                ("repo", &open.to_string()),
                ("author", &ada.to_string()),
            ],
        )
        .await
        .expect("shown");

    let seen = core.of(bob).live("Issue").await.expect("live");
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].key(), shown);

    sudo.set("Repo", open, &[("visibility", "private")])
        .await
        .expect("close");
    assert_eq!(core.of(bob).live("Issue").await.expect("live").len(), 0);
}

#[tokio::test]
async fn confine() {
    let mut graph = Graph::new();
    graph.plug::<Actor>().plug::<Repo>().plug::<Issue>();
    let core = crate::support::boot(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind");
    let sudo = core.sudo();
    let ada = sudo.put("Actor", &[("login", "ada")]).await.expect("ada");
    let bob = sudo.put("Actor", &[("login", "bob")]).await.expect("bob");
    sudo.put(
        "@grant",
        &[
            ("who", "all"),
            ("verb", "put"),
            ("unit", "Repo"),
            ("scope", r#"pred visibility = "public""#),
        ],
    )
    .await
    .expect("seed");
    let open = sudo
        .put(
            "Repo",
            &[
                ("name", "open"),
                ("visibility", "public"),
                ("owner", &ada.to_string()),
            ],
        )
        .await
        .expect("open");

    let out = core
        .of(bob)
        .put(
            "Issue",
            &[
                ("title", "x"),
                ("repo", &open.to_string()),
                ("author", &bob.to_string()),
            ],
        )
        .await;
    assert!(out.is_err());
}

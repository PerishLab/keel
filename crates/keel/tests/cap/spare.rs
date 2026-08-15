use crate::world::*;
use keel::Graph;
use keel::adapt::db::Sqlite;

async fn stage() -> (keel::Core<Sqlite>, i64, i64) {
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
    seed(&ada.to_string(), "*", "Actor", &format!("row {ada}")).await;
    seed(&bob.to_string(), "*", "Actor", &format!("row {bob}")).await;
    seed("all", "put", "Repo", r#"pred owner = "@me""#).await;
    seed("all", "put", "Issue", r#"pred author = "@me""#).await;
    (core, ada, bob)
}

async fn den<W: keel::Wire>(face: &keel::Face<'_, W>, who: i64) -> i64 {
    face.put(
        "Repo",
        &[
            ("name", "den"),
            ("visibility", "private"),
            ("owner", &who.to_string()),
        ],
    )
    .await
    .expect("repo")
}

#[tokio::test]
async fn spare() {
    let (core, ada, _) = stage().await;
    let before = core.sudo().live("@grant").await.expect("live").len();
    let her = core.of(ada);
    let repo = den(&her, ada).await;
    assert_eq!(
        core.sudo().live("@grant").await.expect("live").len(),
        before
    );
    assert_eq!(her.live("Repo").await.expect("see").len(), 1);
    her.set("Repo", repo, &[("name", "keep")])
        .await
        .expect("set what the subtree covers");
    her.end("Repo", repo).await.expect("end it too");
}

#[tokio::test]
async fn minted() {
    let (core, ada, bob) = stage().await;
    let her = core.of(ada);
    let him = core.of(bob);
    let repo = den(&her, ada).await;
    let before = core.sudo().live("@grant").await.expect("live").len();
    let issue = him
        .put(
            "Issue",
            &[
                ("title", "loose"),
                ("repo", &repo.to_string()),
                ("author", &bob.to_string()),
            ],
        )
        .await
        .expect("issue");
    assert_eq!(
        core.sudo().live("@grant").await.expect("live").len(),
        before + 1
    );
    him.set("Issue", issue, &[("title", "kept")])
        .await
        .expect("the minted row grant carries it");
}

#[tokio::test]
async fn handed() {
    let (core, ada, bob) = stage().await;
    let her = core.of(ada);
    let him = core.of(bob);
    let repo = den(&her, ada).await;
    assert_eq!(him.live("Repo").await.expect("stranger").len(), 0);
    assert!(
        him.put(
            "@grant",
            &[
                ("who", &ada.to_string()),
                ("verb", "end"),
                ("unit", "Repo"),
                ("scope", &format!("row {repo}")),
            ],
        )
        .await
        .is_err()
    );
    her.set("Repo", repo, &[("owner", &bob.to_string())])
        .await
        .expect("hand it over");
    assert_eq!(him.live("Repo").await.expect("taken").len(), 1);
    assert_eq!(her.live("Repo").await.expect("given up").len(), 0);
    him.set("Repo", repo, &[("name", "mine")])
        .await
        .expect("the new owner writes");
    assert!(her.set("Repo", repo, &[("name", "back")]).await.is_err());
}

#[tokio::test]
async fn seized() {
    let (core, ada, bob) = stage().await;
    let her = core.of(ada);
    let him = core.of(bob);
    let repo = den(&her, ada).await;
    assert!(
        him.set("Repo", repo, &[("owner", &bob.to_string())])
            .await
            .is_err()
    );
}

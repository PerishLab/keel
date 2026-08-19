use keel::adapt::db::Sqlite;
use keel::atom::string;
use keel::resource;
use keel::{Cell, Graph};

#[resource]
struct Author {
    #[field(string)]
    name: string,
    #[relation(Author, many2many)]
    follows: Author,
}

#[resource]
struct Post {
    #[field(string)]
    title: string,
    #[relation(Author, many2one, root)]
    author: Author,
    #[relation(Author, many2one, opt)]
    editor: Author,
}

#[resource]
struct Card {
    #[field(string)]
    label: string,
    #[relation(Author, one2one)]
    owner: Author,
}

fn ends(left: i64, right: i64) -> keel::Ends {
    keel::Ends { left, right }
}

#[tokio::test]
async fn point() {
    let mut graph = Graph::new();
    graph.plug::<Author>().plug::<Post>();
    let core = crate::support::boot(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind");
    let ada = core.put("Author", &[("name", "ada")]).await.expect("ada");
    let bob = core.put("Author", &[("name", "bob")]).await.expect("bob");

    let post = core
        .put("Post", &[("title", "hello"), ("author", &ada.to_string())])
        .await
        .expect("post");
    let rows = core.live("Post").await.expect("live");
    assert_eq!(rows[0].cells().get("author"), Some(&Cell::Int(ada)));
    assert_eq!(rows[0].cells().get("editor"), None);

    assert!(core.put("Post", &[("title", "x")]).await.is_err());
    assert!(
        core.put("Post", &[("title", "x"), ("author", "999")])
            .await
            .is_err()
    );
    assert!(
        core.put("Post", &[("title", "x"), ("author", "nope")])
            .await
            .is_err()
    );

    core.set("Post", post, &[("editor", &bob.to_string())])
        .await
        .expect("set editor");
    let rows = core.live("Post").await.expect("live");
    assert_eq!(rows[0].cells().get("editor"), Some(&Cell::Int(bob)));

    core.set("Post", post, &[("editor", "")])
        .await
        .expect("clear");
    let rows = core.live("Post").await.expect("live");
    assert_eq!(rows[0].cells().get("editor"), None);

    assert!(core.set("Post", post, &[("author", "")]).await.is_err());

    let pack = core
        .query(&format!(r#"from Post where author = "{ada}""#))
        .await
        .expect("pred");
    assert_eq!(pack.rows().len(), 1);
    let pack = core
        .query(&format!(r#"from Post where author = "{bob}""#))
        .await
        .expect("miss");
    assert_eq!(pack.rows().len(), 0);

    core.end("Author", ada)
        .await
        .expect("an author takes their posts with them");
    assert!(core.live("Post").await.expect("posts").is_empty());
    assert!(core.end("Post", post).await.is_err());

    assert!(core.query("from Post link author").await.is_err());
    assert!(
        core.tie("Post", "author", keel::Ends { left: 1, right: 1 }, &[])
            .await
            .is_err()
    );
}

#[tokio::test]
async fn lone() {
    let mut graph = Graph::new();
    graph.plug::<Author>().plug::<Card>();
    let core = crate::support::boot(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind");
    let ada = core.put("Author", &[("name", "ada")]).await.expect("ada");
    let bob = core.put("Author", &[("name", "bob")]).await.expect("bob");

    let one = core
        .put("Card", &[("label", "gold"), ("owner", &ada.to_string())])
        .await
        .expect("one");
    assert!(
        core.put("Card", &[("label", "dup"), ("owner", &ada.to_string())])
            .await
            .is_err()
    );
    let two = core
        .put("Card", &[("label", "iron"), ("owner", &bob.to_string())])
        .await
        .expect("two");

    assert!(
        core.set("Card", two, &[("owner", &ada.to_string())])
            .await
            .is_err()
    );
    core.set("Card", two, &[("owner", &bob.to_string())])
        .await
        .expect("same owner ok");

    core.end("Card", one).await.expect("end one");
    core.set("Card", two, &[("owner", &ada.to_string())])
        .await
        .expect("freed after end");
}

#[tokio::test]
async fn mirror() {
    let mut graph = Graph::new();
    graph.plug::<Author>().plug::<Post>().plug::<Card>();
    let core = crate::support::boot(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind");
    let ada = core.put("Author", &[("name", "ada")]).await.expect("ada");
    let bob = core.put("Author", &[("name", "bob")]).await.expect("bob");

    let tie = core
        .tie(
            "Author",
            "follows",
            keel::Ends {
                left: ada,
                right: bob,
            },
            &[],
        )
        .await
        .expect("self tie");
    let ties = core.ties("Author", "follows", ada).await.expect("ties");
    assert_eq!(ties.len(), 1);
    assert_eq!(ties[0].left(), ada);
    assert_eq!(ties[0].right(), bob);

    let pack = core
        .query(&format!(r#"from Author where follows has "{bob}""#))
        .await
        .expect("has");
    assert_eq!(pack.rows().len(), 1);
    assert_eq!(pack.rows()[0].key(), ada);

    assert!(core.end("Author", bob).await.is_err());
    core.end("Author", ada)
        .await
        .expect("a row its own tie hangs from may retire");
    core.cut("Author", "follows", tie).await.expect("cut");
    core.end("Author", bob).await.expect("end bob");
}

#[tokio::test]
async fn cycle() {
    let mut graph = Graph::new();
    graph.plug::<Author>();
    let core = crate::support::boot(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind");
    let ada = core.put("Author", &[("name", "ada")]).await.expect("ada");
    let bob = core.put("Author", &[("name", "bob")]).await.expect("bob");
    let cy = core.put("Author", &[("name", "cy")]).await.expect("cy");

    core.tie("Author", "follows", ends(ada, ada), &[])
        .await
        .expect("undeclared self tie");
    core.tie("Author", "follows", ends(ada, bob), &[])
        .await
        .expect("ada follows bob");
    core.tie("Author", "follows", ends(bob, cy), &[])
        .await
        .expect("bob follows cy");
    core.tie("Author", "follows", ends(cy, ada), &[])
        .await
        .expect("undeclared cycle");
}

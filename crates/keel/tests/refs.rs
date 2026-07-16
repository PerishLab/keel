use keel::adapt::db::Sqlite;
use keel::atom::string;
use keel::resource;
use keel::{Cell, Graph, bind};

#[resource]
struct Author {
    #[field(string)]
    name: string,
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

#[test]
fn point() {
    let mut graph = Graph::new();
    graph.plug::<Author>().plug::<Post>();
    let core = bind(graph, Sqlite::memory()).expect("bind");
    let ada = core.put("Author", &[("name", "ada")]).expect("ada");
    let bob = core.put("Author", &[("name", "bob")]).expect("bob");

    let post = core
        .put("Post", &[("title", "hello"), ("author", &ada.to_string())])
        .expect("post");
    let rows = core.live("Post").expect("live");
    assert_eq!(rows[0].cells().get("author"), Some(&Cell::Int(ada)));
    assert_eq!(rows[0].cells().get("editor"), None);

    assert!(core.put("Post", &[("title", "x")]).is_err());
    assert!(
        core.put("Post", &[("title", "x"), ("author", "999")])
            .is_err()
    );
    assert!(
        core.put("Post", &[("title", "x"), ("author", "nope")])
            .is_err()
    );

    core.set("Post", post, &[("editor", &bob.to_string())])
        .expect("set editor");
    let rows = core.live("Post").expect("live");
    assert_eq!(rows[0].cells().get("editor"), Some(&Cell::Int(bob)));

    core.set("Post", post, &[("editor", "")]).expect("clear");
    let rows = core.live("Post").expect("live");
    assert_eq!(rows[0].cells().get("editor"), None);

    assert!(core.set("Post", post, &[("author", "")]).is_err());

    let pack = core
        .query(&format!(r#"from Post where author = "{ada}""#))
        .expect("pred");
    assert_eq!(pack.rows().len(), 1);
    let pack = core
        .query(&format!(r#"from Post where author = "{bob}""#))
        .expect("miss");
    assert_eq!(pack.rows().len(), 0);

    assert!(core.end("Author", ada).is_err());
    core.end("Post", post).expect("end post");
    core.end("Author", ada).expect("end ada");

    assert!(core.query("from Post link author").is_err());
    assert!(
        core.tie("Post", "author", keel::Ends { left: 1, right: 1 }, &[])
            .is_err()
    );
}

#[test]
fn lone() {
    let mut graph = Graph::new();
    graph.plug::<Author>().plug::<Card>();
    let core = bind(graph, Sqlite::memory()).expect("bind");
    let ada = core.put("Author", &[("name", "ada")]).expect("ada");
    let bob = core.put("Author", &[("name", "bob")]).expect("bob");

    let one = core
        .put("Card", &[("label", "gold"), ("owner", &ada.to_string())])
        .expect("one");
    assert!(
        core.put("Card", &[("label", "dup"), ("owner", &ada.to_string())])
            .is_err()
    );
    let two = core
        .put("Card", &[("label", "iron"), ("owner", &bob.to_string())])
        .expect("two");

    assert!(
        core.set("Card", two, &[("owner", &ada.to_string())])
            .is_err()
    );
    core.set("Card", two, &[("owner", &bob.to_string())])
        .expect("same owner ok");

    core.end("Card", one).expect("end one");
    core.set("Card", two, &[("owner", &ada.to_string())])
        .expect("freed after end");
}

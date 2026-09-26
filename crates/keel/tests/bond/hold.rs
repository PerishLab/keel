use keel::adapt::db::Sqlite;
use keel::atom::string;
use keel::resource;
use keel::{Ends, Graph};

#[resource]
struct Shelf {
    #[field(string)]
    name: string,
}

#[resource]
struct Box {
    #[field(string)]
    name: string,
    #[relation(Shelf, many2one, root)]
    shelf: Shelf,
}

#[resource]
struct Note {
    #[field(string)]
    text: string,
    #[relation(Box, many2one, root)]
    held: Box,
    #[relation(Shelf, many2many)]
    seen: Shelf,
}

mod shelf {
    use super::Shelf;
    use keel::atom::string;
    use keel::resource;

    #[resource]
    pub(super) struct Addition {
        #[field(string)]
        text: string,
        #[relation(Shelf, many2one, root)]
        shelf: Shelf,
    }
}

mod parcel {
    use super::Box;
    use keel::atom::string;
    use keel::resource;

    #[resource]
    pub(super) struct Addition {
        #[field(string)]
        text: string,
        #[relation(Box, many2one, root)]
        parcel: Box,
    }
}

#[tokio::test]
async fn subtree() {
    let mut graph = Graph::new();
    graph.plug::<Shelf>().plug::<Box>().plug::<Note>();
    let core = crate::support::boot(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind");
    let shelf = core.put("Shelf", &[("name", "one")]).await.expect("shelf");
    let held = core
        .put("Box", &[("name", "inner"), ("shelf", &shelf.to_string())])
        .await
        .expect("box");
    let note = core
        .put("Note", &[("text", "hi"), ("held", &held.to_string())])
        .await
        .expect("note");

    core.end("Shelf", shelf)
        .await
        .expect("a shelf takes its contents with it");
    assert!(core.live("Shelf").await.expect("shelves").is_empty());
    assert!(core.live("Box").await.expect("boxes").is_empty());
    assert!(core.live("Note").await.expect("notes").is_empty());
    assert!(core.end("Note", note).await.is_err());
}

#[tokio::test]
async fn outside() {
    let mut graph = Graph::new();
    graph.plug::<Shelf>().plug::<Box>().plug::<Note>();
    let core = crate::support::boot(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind");
    let shelf = core.put("Shelf", &[("name", "one")]).await.expect("shelf");
    let spare = core.put("Shelf", &[("name", "two")]).await.expect("spare");
    let held = core
        .put("Box", &[("name", "inner"), ("shelf", &shelf.to_string())])
        .await
        .expect("box");
    let note = core
        .put("Note", &[("text", "hi"), ("held", &held.to_string())])
        .await
        .expect("note");
    core.tie(
        "Note",
        "seen",
        Ends {
            left: note,
            right: spare,
        },
        &[],
    )
    .await
    .expect("tie");

    assert!(core.end("Shelf", spare).await.is_err());
    core.end("Shelf", shelf)
        .await
        .expect("containment still cascades");
    assert!(core.live("Note").await.expect("notes").is_empty());
}

#[tokio::test]
async fn qualified() {
    let mut graph = Graph::new();
    graph
        .plug::<Shelf>()
        .plug::<Box>()
        .plug::<shelf::Addition>()
        .plug::<parcel::Addition>();
    let core = crate::support::boot(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind");
    let shelf = core.put("Shelf", &[("name", "one")]).await.expect("shelf");
    let parcel = core
        .put("Box", &[("name", "inner"), ("shelf", &shelf.to_string())])
        .await
        .expect("box");
    core.put(
        "shelf:addition",
        &[("text", "outer"), ("shelf", &shelf.to_string())],
    )
    .await
    .expect("shelf addition");
    core.put(
        "box:addition",
        &[("text", "inner"), ("parcel", &parcel.to_string())],
    )
    .await
    .expect("box addition");

    core.end("Shelf", shelf)
        .await
        .expect("qualified contents retire with their container");
    assert!(core.live("Shelf").await.expect("shelves").is_empty());
    assert!(
        core.live("shelf:addition")
            .await
            .expect("shelf additions")
            .is_empty()
    );
    assert!(
        core.live("box:addition")
            .await
            .expect("box additions")
            .is_empty()
    );
}

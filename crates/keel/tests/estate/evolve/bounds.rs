use super::rename::graph;
use keel::adapt::db::Sqlite;
use keel::atom::Kind;
use keel::bond;
use keel::spec::Spec;
use keel::{Graph, bind};

fn shape(name: &str, mode: &str) -> Graph {
    let mut graph = Graph::new();
    graph.add(Spec::build("Root").field("name", Kind::Text).seal());
    let mut leaf =
        Spec::build(name)
            .field("value", Kind::Text)
            .root("root", bond::Kind::Many2one, "Root");
    match mode {
        "serial" => leaf = leaf.serial("index", "root"),
        "many" => leaf = leaf.bond("peers", bond::Kind::Many2many, "Root", &[]),
        "incoming" => {
            graph.add(
                Spec::build("Holder")
                    .free("held", bond::Kind::Many2one, name)
                    .seal(),
            );
        }
        "self" => leaf = leaf.free("peer", bond::Kind::Many2one, name),
        _ => panic!("mode"),
    }
    graph.add(leaf.seal());
    graph
}

#[tokio::test]
async fn unsupported() {
    for mode in ["serial", "many", "incoming", "self"] {
        let directory = tempfile::tempdir().expect("directory");
        let path = directory.path().join("estate");
        drop(
            crate::support::boot(shape("Old", mode), Sqlite::file(&path).await.expect("wire"))
                .await
                .expect("boot"),
        );
        assert!(
            bind(shape("New", mode), Sqlite::file(&path).await.expect("wire"))
                .rename("root:old", "root:new")
                .await
                .is_err(),
            "{mode}"
        );
        bind(shape("Old", mode), Sqlite::file(&path).await.expect("wire"))
            .await
            .expect("old usable");
    }
}

#[tokio::test]
async fn occupied() {
    let directory = tempfile::tempdir().expect("directory");
    let path = directory.path().join("estate");
    let mut occupied = graph("Old");
    occupied.add(
        Spec::build("New")
            .field("value", Kind::Text)
            .root("root", bond::Kind::Many2one, "Root")
            .seal(),
    );
    drop(
        crate::support::boot(occupied.clone(), Sqlite::file(&path).await.expect("wire"))
            .await
            .expect("boot"),
    );
    assert!(
        bind(graph("New"), Sqlite::file(&path).await.expect("wire"))
            .rename("root:old", "root:new")
            .await
            .is_err()
    );
    bind(occupied, Sqlite::file(&path).await.expect("wire"))
        .await
        .expect("old usable");
}

#[tokio::test]
async fn authority() {
    let directory = tempfile::tempdir().expect("directory");
    let path = directory.path().join("estate");
    let core = crate::support::boot(graph("Old"), Sqlite::file(&path).await.expect("wire"))
        .await
        .expect("boot");
    core.put(
        "@grant",
        &[
            ("who", "all"),
            ("verb", "see"),
            ("unit", "root:old"),
            ("scope", "all"),
        ],
    )
    .await
    .expect("grant");
    drop(core);
    assert!(
        bind(graph("New"), Sqlite::file(&path).await.expect("wire"))
            .rename("root:old", "root:new")
            .await
            .is_err()
    );
    let core = bind(graph("Old"), Sqlite::file(&path).await.expect("wire"))
        .await
        .expect("old usable");
    assert_eq!(
        core.live("@grant").await.expect("grants")[0].text("unit"),
        Some("root:old")
    );
}

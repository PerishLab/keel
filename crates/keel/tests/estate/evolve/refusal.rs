use super::rename::{boot, graph, rows};
use keel::adapt::db::Sqlite;
use keel::atom::Kind;
use keel::bind;
use keel::spec::Spec;
use keel::wire::{Val, Wire};

#[tokio::test]
async fn declarations() {
    let directory = tempfile::tempdir().expect("directory");
    let path = directory.path().join("estate");
    boot(&path).await;
    let before = rows(&path, "root_old").await;
    for (from, to) in [
        ("missing", "root:new"),
        ("root:old", "missing"),
        ("root:old", "root:old"),
    ] {
        assert!(
            bind(graph("New"), Sqlite::file(&path).await.expect("wire"))
                .rename(from, to)
                .await
                .is_err()
        );
    }
    assert!(
        bind(graph("New"), Sqlite::file(&path).await.expect("wire"))
            .rename("root:old", "root:new")
            .rename("root:old", "root:new")
            .await
            .is_err()
    );
    assert!(
        bind(graph("New"), Sqlite::file(&path).await.expect("wire"))
            .await
            .is_err()
    );
    let mut changed = graph("New");
    changed.add(Spec::build("Extra").field("name", Kind::Text).seal());
    assert!(
        bind(changed, Sqlite::file(&path).await.expect("wire"))
            .rename("root:old", "root:new")
            .await
            .is_err()
    );
    bind(graph("Old"), Sqlite::file(&path).await.expect("wire"))
        .await
        .expect("old usable");
    assert_eq!(rows(&path, "root_old").await, before);
}

#[tokio::test]
async fn collision() {
    let directory = tempfile::tempdir().expect("directory");
    let path = directory.path().join("estate");
    boot(&path).await;
    let before = rows(&path, "root_old").await;
    let mut wire = Sqlite::file(&path).await.expect("wire");
    wire.run(
        "INSERT INTO \"@clock\" (name, value) VALUES (?1, ?2)",
        &[Val::Text("unit:root:new".into()), Val::Int(50)],
    )
    .await
    .expect("historical clock");
    drop(wire);
    assert!(
        bind(graph("New"), Sqlite::file(&path).await.expect("wire"))
            .rename("root:old", "root:new")
            .await
            .is_err()
    );
    bind(graph("Old"), Sqlite::file(&path).await.expect("wire"))
        .await
        .expect("old usable");
    assert_eq!(rows(&path, "root_old").await, before);
    let mut wire = Sqlite::file(&path).await.expect("wire");
    assert_eq!(
        wire.rows("SELECT COUNT(*) FROM \"@generation\"", &[])
            .await
            .expect("rollback"),
        vec![vec![Val::Int(1)]]
    );
}

use crate::world::*;
use crate::write::{each, open};
use keel::adapt::db::Sqlite;
use keel::{Core, Graph};
use std::time::Instant;

const ROWS: usize = 200;

async fn estate() -> Core<Sqlite> {
    let mut graph = Graph::new();
    graph.plug::<Flat>().plug::<Deep>().plug::<Deeper>();
    crate::support::boot(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind")
}

async fn sown(core: &Core<Sqlite>, unit: &str) {
    let sudo = core.sudo();
    let root = sudo
        .put("Flat", &[("note", "root")])
        .await
        .expect("root")
        .to_string();
    let under = sudo
        .put("Deep", &[("note", "under"), ("root", &root)])
        .await
        .expect("under")
        .to_string();
    for spot in 0..ROWS {
        let note = format!("row {spot}");
        let held: Vec<(&str, &str)> = match unit {
            "Flat" => vec![("note", note.as_str())],
            "Deep" => vec![("note", note.as_str()), ("root", root.as_str())],
            _ => vec![("note", note.as_str()), ("root", under.as_str())],
        };
        sudo.put(unit, &held).await.expect("row");
    }
}

async fn read(core: &Core<Sqlite>, unit: &str, bare: bool) -> f64 {
    let started = Instant::now();
    for _ in 0..10 {
        let seen = match bare {
            true => core.sudo().live(unit).await.expect("live"),
            false => core.of(1).live(unit).await.expect("live"),
        };
        assert!(seen.len() >= ROWS);
    }
    each(started, 10)
}

#[tokio::test]
#[ignore]
async fn reach() {
    for (unit, deep) in [("Flat", 0), ("Deep", 1), ("Deeper", 2)] {
        let core = estate().await;
        open(&core).await;
        sown(&core, unit).await;
        let bare = read(&core, unit, true).await;
        let held = read(&core, unit, false).await;
        println!(
            "{unit:<7} depth {deep}  sudo {bare:.2}ms  operator {held:.2}ms  walk {:.2}ms",
            held - bare
        );
    }
}

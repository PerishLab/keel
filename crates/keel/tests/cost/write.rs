use crate::world::*;
use keel::adapt::db::Sqlite;
use keel::{Core, Graph, Wire};
use std::time::Instant;

const ROUNDS: usize = 300;

async fn estate() -> Core<Sqlite> {
    let mut graph = Graph::new();
    graph.plug::<Flat>().plug::<Deep>().plug::<Deeper>();
    crate::support::boot(graph, Sqlite::memory().await.expect("db"))
        .await
        .expect("bind")
}

async fn open<W: Wire>(core: &Core<W>) {
    let sudo = core.sudo();
    for unit in ["Flat", "Deep", "Deeper"] {
        sudo.put(
            "@grant",
            &[
                ("who", "all"),
                ("verb", "*"),
                ("unit", unit),
                ("scope", "all"),
            ],
        )
        .await
        .expect("seed");
    }
}

fn each(started: Instant, count: usize) -> f64 {
    started.elapsed().as_secs_f64() * 1000.0 / count as f64
}

async fn grants<W: Wire>(core: &Core<W>) -> usize {
    core.sudo().live("@grant").await.expect("grants").len()
}

#[tokio::test]
#[ignore]
async fn floor() {
    let mut told = Vec::new();
    for unit in ["Flat", "Deep", "Deeper"] {
        let core = estate().await;
        open(&core).await;
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
        let held = match unit {
            "Flat" => Vec::new(),
            "Deep" => vec![("root", root.clone())],
            _ => vec![("root", under.clone())],
        };
        let fields = |note: &str, held: &[(&'static str, String)]| {
            let mut out = vec![("note".to_string(), note.to_string())];
            for (name, value) in held {
                out.push(((*name).to_string(), value.clone()));
            }
            out
        };
        let started = Instant::now();
        for spot in 0..ROUNDS {
            let row = fields(&format!("sudo {spot}"), &held);
            let seen: Vec<(&str, &str)> = row
                .iter()
                .map(|(name, value)| (name.as_str(), value.as_str()))
                .collect();
            sudo.put(unit, &seen).await.expect("put");
        }
        let bare = each(started, ROUNDS);

        let face = core.of(1);
        let started = Instant::now();
        for spot in 0..ROUNDS {
            let row = fields(&format!("operator {spot}"), &held);
            let seen: Vec<(&str, &str)> = row
                .iter()
                .map(|(name, value)| (name.as_str(), value.as_str()))
                .collect();
            face.put(unit, &seen).await.expect("put");
        }
        let held = each(started, ROUNDS);
        told.push((unit, bare, held));
    }
    for (unit, bare, held) in &told {
        println!(
            "{unit:<7} sudo {bare:.2}ms  operator {held:.2}ms  ceremony {:.2}ms",
            held - bare
        );
    }
}

#[tokio::test]
#[ignore]
async fn growth() {
    let core = estate().await;
    open(&core).await;
    let face = core.of(1);
    let mut held = Vec::new();
    for round in 1..=6 {
        let started = Instant::now();
        for spot in 0..ROUNDS {
            held.push(
                face.put("Flat", &[("note", &format!("round {round} {spot}"))])
                    .await
                    .expect("put"),
            );
        }
        let made = each(started, ROUNDS);
        let started = Instant::now();
        for key in held.iter().take(ROUNDS) {
            face.set("Flat", *key, &[("note", "moved")])
                .await
                .expect("set");
        }
        println!(
            "round {round}: put {made:.2}ms, set {:.2}ms, grants {}",
            each(started, ROUNDS),
            grants(&core).await
        );
    }
}

#[tokio::test]
#[ignore]
async fn parts() {
    let core = estate().await;
    open(&core).await;
    let sudo = core.sudo();
    let face = core.of(1);
    let mut held = Vec::new();
    for spot in 0..ROUNDS {
        held.push(
            sudo.put("Flat", &[("note", &format!("seed {spot}"))])
                .await
                .expect("seed"),
        );
    }
    let started = Instant::now();
    for key in &held {
        sudo.set("Flat", *key, &[("note", "sudo moved")])
            .await
            .expect("set");
    }
    let bare = each(started, ROUNDS);
    let started = Instant::now();
    for key in &held {
        face.set("Flat", *key, &[("note", "operator moved")])
            .await
            .expect("set");
    }
    let seen = each(started, ROUNDS);
    let started = Instant::now();
    for spot in 0..ROUNDS {
        face.put("Flat", &[("note", &format!("fresh {spot}"))])
            .await
            .expect("put");
    }
    let made = each(started, ROUNDS);
    let started = Instant::now();
    for spot in 0..ROUNDS {
        sudo.put("Flat", &[("note", &format!("bare {spot}"))])
            .await
            .expect("put");
    }
    let plain = each(started, ROUNDS);
    let check = (seen - bare) / 2.0;
    println!("sudo set (update plus pulse)     {bare:.2}ms");
    println!("operator set (two checks)        {seen:.2}ms");
    println!("sudo put (insert plus pulse)     {plain:.2}ms");
    println!("operator put (check plus mint)   {made:.2}ms");
    println!("one authority check              {check:.2}ms");
    println!(
        "mint alone                       {:.2}ms",
        made - plain - check
    );
    println!("grants now                       {}", grants(&core).await);
}

#[tokio::test]
#[ignore]
async fn covered() {
    let core = estate().await;
    open(&core).await;
    let sudo = core.sudo();
    let root = sudo
        .put("Flat", &[("note", "root")])
        .await
        .expect("root")
        .to_string();
    sudo.put(
        "@grant",
        &[
            ("who", "1"),
            ("verb", "*"),
            ("unit", "Flat"),
            ("scope", &format!("row {root}")),
        ],
    )
    .await
    .expect("row grant");
    let face = core.of(1);
    let started = Instant::now();
    for spot in 0..ROUNDS {
        face.put(
            "Deep",
            &[("note", &format!("under {spot}")), ("root", &root)],
        )
        .await
        .expect("put");
    }
    println!(
        "covered by a row grant, still mints  {:.2}ms each, grants {}",
        each(started, ROUNDS),
        grants(&core).await
    );
}

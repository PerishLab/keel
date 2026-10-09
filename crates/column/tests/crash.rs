#[path = "support/mod.rs"]
mod support;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;
use support::{batch, everything, store, temp};

const HOME: &str = "KEEL_COLUMN_WRITER";
const START: &str = "KEEL_COLUMN_START";
const FAR: u64 = u64::MAX / 2;
const ROUNDS: u64 = 12;

#[test]
fn writer() {
    let (Ok(home), Ok(start)) = (std::env::var(HOME), std::env::var(START)) else {
        return;
    };
    let start: u64 = start.parse().expect("start");
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    runtime.block_on(async {
        let (_core, store) = store(Path::new(&home)).await;
        let mut out = std::io::stdout();
        for index in start..start + 100_000 {
            let slot = slot(index);
            store
                .append(&format!("t{index}"), batch(index, slot))
                .await
                .expect("append");
            writeln!(out, "ack {index}")
                .and_then(|()| out.flush())
                .expect("ack");
            if index % 3 == 0 {
                store.seal(FAR).await.expect("seal");
            }
            if index % 10 == 9 && slot > 1 {
                writeln!(out, "floor {}", slot - 1)
                    .and_then(|()| out.flush())
                    .expect("floor");
                store.retain(slot - 1, FAR).await.expect("retain");
                store.sweep(FAR).await.expect("sweep");
            }
        }
    });
}

#[test]
fn killed() {
    let home = temp("killed");
    let mut ledger = Ledger::default();
    let mut start = 0;
    for round in 0..ROUNDS {
        let mut child = Command::new(std::env::current_exe().expect("test binary"))
            .args(["--exact", "writer", "--nocapture", "--test-threads", "1"])
            .env(HOME, &home)
            .env(START, start.to_string())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("writer");
        let stdout = child.stdout.take().expect("stdout");
        let reader = std::thread::spawn(move || {
            BufReader::new(stdout)
                .lines()
                .map_while(Result::ok)
                .collect::<Vec<_>>()
        });
        std::thread::sleep(Duration::from_millis(300 + round * 97 % 600));
        child.kill().expect("kill");
        child.wait().expect("reap");
        for line in reader.join().expect("lines") {
            ledger.absorb(&line);
        }
        start = ledger.acked.last().copied().unwrap_or(start);
        verify(&home, &ledger.acked, ledger.floor);
    }
    let total = ledger.acked.len();
    assert!(total > 30, "too few batches acknowledged: {total}");
}

#[derive(Default)]
struct Ledger {
    acked: BTreeSet<u64>,
    floor: u64,
}

impl Ledger {
    fn absorb(&mut self, line: &str) {
        match line.split_once(' ') {
            Some(("ack", index)) => {
                self.acked.insert(index.parse().expect("index"));
            }
            Some(("floor", value)) => self.floor = self.floor.max(value.parse().expect("floor")),
            _ => {}
        }
    }
}

fn verify(home: &Path, acked: &BTreeSet<u64>, floor: u64) {
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    runtime.block_on(async {
        let (core, store) = store(home).await;
        let mut counts: BTreeMap<u64, Vec<String>> = BTreeMap::new();
        for raw in everything(&store).await {
            let index = raw[1..raw.find('-').expect("dash")].parse().expect("index");
            counts.entry(index).or_default().push(raw);
        }
        for (index, found) in &counts {
            let distinct: BTreeSet<_> = found.iter().collect();
            assert_eq!(
                distinct.len(),
                found.len(),
                "batch {index} duplicated: {found:?}"
            );
            assert_eq!(found.len(), 3, "batch {index} partially visible: {found:?}");
        }
        for index in acked {
            if slot(*index) >= floor {
                assert!(
                    counts.contains_key(index),
                    "acknowledged batch {index} lost"
                );
            }
        }
        let live: BTreeSet<PathBuf> = core
            .live("Part")
            .await
            .expect("manifest")
            .iter()
            .map(|row| {
                let name = row.text("name").expect("name");
                let (scope, seq) = name.split_once('/').expect("name");
                home.join("column/parts")
                    .join(scope)
                    .join(format!("{:020}.parquet", seq.parse::<u64>().expect("seq")))
            })
            .collect();
        assert_eq!(files(&home.join("column/parts")), live);
    });
}

fn slot(index: u64) -> u64 {
    index / 10 + 1
}

fn files(area: &Path) -> BTreeSet<PathBuf> {
    fs::read_dir(area)
        .map(|scopes| {
            scopes
                .flat_map(|scope| fs::read_dir(scope.expect("scope").path()).expect("files"))
                .map(|file| file.expect("file").path())
                .collect()
        })
        .unwrap_or_default()
}

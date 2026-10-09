use keel_column::{Filter, Record, Table};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

const DAY: u64 = 86_400_000_000_000;

static SERIAL: AtomicUsize = AtomicUsize::new(0);

struct Fixture {
    table: Table,
    records: Vec<Record>,
    path: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let table = Table::new("at", &["producer", "trace"])
            .and_then(|table| table.sort(&["trace"]))
            .and_then(|table| table.grain(Some("producer"), DAY))
            .expect("table");
        let records = (0..600_u64)
            .map(|index| {
                let time = DAY * 3 + (index * 7_919) % 600 * 1_000;
                let trace = format!("trace-{}", index % 5);
                let mut raw =
                    format!(r#"{{"at":{time},"trace":"{trace}","n":{index}}}"#).into_bytes();
                raw.push(u8::try_from(index % 256).expect("byte"));
                Record {
                    raw,
                    time,
                    keys: vec!["concord".to_string(), trace],
                }
            })
            .collect();
        let directory = std::env::temp_dir().join(format!(
            "keel-column-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir_all(&directory).expect("directory");
        Self {
            table,
            records,
            path: directory.join("part.parquet"),
        }
    }

    fn written(self) -> Self {
        let summary = self
            .table
            .write(&self.path, self.records.clone())
            .expect("write");
        assert_eq!(summary.rows, 600);
        assert_eq!(summary.partition.key.as_deref(), Some("concord"));
        assert_eq!(summary.partition.slot, 3);
        self
    }

    fn scanned(&self, filter: &Filter) -> Vec<Vec<u8>> {
        let mut found = Vec::new();
        self.table
            .scan(&self.path, filter, &mut |raw| {
                found.push(raw.to_vec());
                Ok(())
            })
            .expect("scan");
        found
    }

    fn expected(&self, admits: impl Fn(&Record) -> bool) -> Vec<Vec<u8>> {
        let mut matching: Vec<&Record> = self
            .records
            .iter()
            .filter(|record| admits(record))
            .collect();
        matching
            .sort_by(|left, right| (&left.keys[1], left.time).cmp(&(&right.keys[1], right.time)));
        matching
            .into_iter()
            .map(|record| record.raw.clone())
            .collect()
    }
}

#[test]
fn verbatim() {
    let fixture = Fixture::new().written();
    let all = fixture.scanned(&Filter::default());
    assert_eq!(all, fixture.expected(|_| true));
}

#[test]
fn filtered() {
    let fixture = Fixture::new().written();
    let (from, to) = (DAY * 3 + 100_000, DAY * 3 + 250_000);
    let window = Filter {
        from: Some(from),
        to: Some(to),
        keys: Vec::new(),
    };
    let inside = |record: &Record| record.time >= from && record.time < to;
    assert_eq!(fixture.scanned(&window), fixture.expected(inside));
    let keyed = Filter {
        keys: vec![("trace".to_string(), "trace-3".to_string())],
        ..window.clone()
    };
    let wanted = fixture.expected(|record| inside(record) && record.keys[1] == "trace-3");
    assert!(!wanted.is_empty());
    assert_eq!(fixture.scanned(&keyed), wanted);
}

#[test]
fn absent() {
    let fixture = Fixture::new().written();
    let missing = Filter {
        keys: vec![("trace".to_string(), "trace-9".to_string())],
        ..Filter::default()
    };
    assert!(fixture.scanned(&missing).is_empty());
    let later = Filter {
        from: Some(DAY * 4),
        ..Filter::default()
    };
    assert!(fixture.scanned(&later).is_empty());
}

#[test]
fn immutable() {
    let fixture = Fixture::new().written();
    let before = fs::read(&fixture.path).expect("part");
    let again = fixture
        .table
        .write(&fixture.path, fixture.records[..1].to_vec());
    assert!(again.is_err());
    assert_eq!(fs::read(&fixture.path).expect("part"), before);
}

#[test]
fn refused() {
    let fixture = Fixture::new();
    let mut stray = fixture.records[..2].to_vec();
    stray[1].time += DAY;
    let mut short = fixture.records[..1].to_vec();
    short[0].keys.pop();
    for batch in [Vec::new(), stray, short] {
        assert!(fixture.table.write(&fixture.path, batch).is_err());
    }
    assert!(!fixture.path.exists());
    let undeclared = Filter {
        keys: vec![("span".to_string(), "s".to_string())],
        ..Filter::default()
    };
    let fixture = fixture.written();
    assert!(
        fixture
            .table
            .scan(&fixture.path, &undeclared, &mut |_| Ok(()))
            .is_err()
    );
}

#[test]
fn declared() {
    assert!(Table::new("at", &["trace", "trace"]).is_err());
    assert!(Table::new("at", &["raw"]).is_err());
    assert!(Table::new("", &["trace"]).is_err());
    let table = Table::new("at", &["trace"]).expect("table");
    assert!(table.clone().sort(&["span"]).is_err());
    assert!(table.clone().grain(Some("trace"), 0).is_err());
    assert!(table.grain(Some("span"), DAY).is_err());
}

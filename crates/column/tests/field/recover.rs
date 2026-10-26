use super::*;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

const HOME: &str = "KEEL_TYPED_HOME";

#[tokio::test]
async fn torn() {
    let home = tempfile::tempdir().unwrap();
    let core = estate(home.path()).await;
    let root = home.path().join("column");
    let store = Store::open(core.clone(), model(), &root).await.unwrap();
    store
        .write("one", vec![record(0).values(values(0))])
        .await
        .unwrap();
    store
        .write("two", vec![record(1).values(values(1))])
        .await
        .unwrap();
    drop(store);
    let path = file(&root, "open");
    let length = std::fs::metadata(&path).unwrap().len();
    std::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .unwrap()
        .set_len(length - 3)
        .unwrap();
    let store = Store::open(core, model(), &root).await.unwrap();
    assert_eq!(
        read(&store).await,
        BTreeMap::from([(0, (record(0).raw, values(0)))])
    );
    store
        .write("three", vec![record(2).values(values(2))])
        .await
        .unwrap();
    store.seal(u64::MAX).await.unwrap();
    assert_eq!(read(&store).await.len(), 2);
}

#[test]
fn writer() {
    let Ok(home) = std::env::var(HOME) else {
        return;
    };
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let home = Path::new(&home);
        let core = estate(home).await;
        let store = Store::open(core, model(), &home.join("column"))
            .await
            .unwrap();
        for time in 0..1000 {
            store
                .write(&format!("t{time}"), vec![record(time).values(values(time))])
                .await
                .unwrap();
            println!("\nack {time}");
            std::io::stdout().flush().unwrap();
            if time % 3 == 0 {
                store.seal(u64::MAX).await.unwrap();
            }
        }
        let mut release = String::new();
        std::io::stdin().read_line(&mut release).unwrap();
    });
}

struct Writer(Child);

impl Drop for Writer {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn killed() {
    let home = tempfile::tempdir().unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let core = estate(home.path()).await;
        drop(
            Store::open(core, model(), &home.path().join("column"))
                .await
                .unwrap(),
        );
    });
    let mut child = Writer(
        Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "recover::writer",
                "--nocapture",
                "--test-threads",
                "1",
            ])
            .env(HOME, home.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let stdout = child.0.stdout.take().unwrap();
    let (send, receive) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if send.send(line).is_err() {
                break;
            }
        }
    });
    let mut acked = Vec::new();
    while acked.len() < 8 {
        let line = receive
            .recv_timeout(Duration::from_secs(15))
            .expect("writer acknowledgment");
        if let Some(time) = line.strip_prefix("ack ") {
            acked.push(time.parse::<u64>().unwrap());
        }
    }
    child.0.kill().unwrap();
    let status = child.0.wait().unwrap();
    assert!(!status.success());
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(status.signal(), Some(9));
    }
    reader.join().unwrap();
    runtime.block_on(async {
        let core = estate(home.path()).await;
        let store = Store::open(core, model(), &home.path().join("column"))
            .await
            .unwrap();
        let found = read(&store).await;
        for time in acked {
            assert_eq!(found[&time], (record(time).raw, values(time)));
        }
        for (time, value) in found {
            assert_eq!(value, (record(time).raw, values(time)));
        }
    });
}

use keel::Wire;
use keel::{Cell, Core, Row};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

pub const TICK: u64 = 300;

#[macro_export]
macro_rules! relay {
    ($actor:ident) => {
        #[::keel::resource]
        pub struct Hook {
            #[field(url)]
            url: ::keel::atom::url,
            #[field(string)]
            unit: ::keel::atom::string,
            #[field(string)]
            verb: ::keel::atom::string,
            #[relation($actor, many2one, root)]
            actor: $actor,
        }

        pub fn wire(graph: &mut ::keel::Graph) {
            graph.plug::<Hook>();
        }
    };
}

pub struct Relay<W: Wire> {
    core: Arc<Core<W>>,
    svc: i64,
}

impl<W: Wire + 'static> Relay<W> {
    pub async fn rise(core: Arc<Core<W>>, svc: i64) -> Result<Self, keel::adapt::Error> {
        let sudo = core.sudo();
        let held = sudo
            .query(&format!(
                r#"from @grant where who = "{svc}" and unit = "Hook" count"#
            ))
            .await?;
        if held.count() == Some(0) {
            sudo.put(
                "@grant",
                &[
                    ("who", &svc.to_string()),
                    ("verb", "see"),
                    ("unit", "Hook"),
                    ("scope", "all"),
                ],
            )
            .await?;
        }
        Ok(Self { core, svc })
    }

    pub fn run(self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut cursor = self.floor().await;
            loop {
                tokio::time::sleep(Duration::from_millis(TICK)).await;
                cursor = self.tick(cursor).await;
            }
        })
    }

    async fn floor(&self) -> i64 {
        self.core
            .flow(0)
            .await
            .ok()
            .and_then(|rows| rows.last().map(Row::key))
            .unwrap_or(0)
    }

    async fn tick(&self, cursor: i64) -> i64 {
        let Ok(all) = self.core.flow(cursor).await else {
            return self.floor().await;
        };
        let Some(last) = all.last().map(Row::key) else {
            return cursor;
        };
        let Ok(hooks) = self.core.of(self.svc).live("Hook").await else {
            return cursor;
        };
        if self.send(&hooks, cursor).await {
            return last;
        }
        cursor
    }

    async fn send(&self, hooks: &[Row], cursor: i64) -> bool {
        let mut whole = true;
        for hook in hooks {
            whole &= self.serve(hook, cursor).await;
        }
        whole
    }

    async fn serve(&self, hook: &Row, cursor: i64) -> bool {
        let Some(Cell::Int(owner)) = hook.cells().get("actor") else {
            return true;
        };
        let Ok(events) = self.core.of(*owner).flow(cursor).await else {
            return true;
        };
        let mut whole = true;
        for event in events {
            if fits(hook, &event) {
                whole &= post(&text(hook, "url"), &letter(&event)).await;
            }
        }
        whole
    }
}

fn fits(hook: &Row, event: &Row) -> bool {
    let unit = text(hook, "unit");
    let verb = text(hook, "verb");
    let liked = unit.is_empty() || unit == text(event, "unit");
    liked && (verb.is_empty() || verb == text(event, "verb"))
}

fn letter(event: &Row) -> String {
    json!({
        "seq": event.key(),
        "verb": text(event, "verb"),
        "unit": text(event, "unit"),
        "key": event.cells().get("key").map(Cell::show).unwrap_or_default(),
        "who": text(event, "who"),
        "at": event.created(),
    })
    .to_string()
}

fn text(row: &Row, name: &str) -> String {
    row.cells().get(name).map(Cell::show).unwrap_or_default()
}

async fn post(url: &str, body: &str) -> bool {
    let Some(rest) = url.strip_prefix("http://") else {
        return false;
    };
    let (host, path) = match rest.split_once('/') {
        Some((host, path)) => (host.to_string(), format!("/{path}")),
        None => (rest.to_string(), "/".to_string()),
    };
    knock(&host, &path, body).await.unwrap_or_default()
}

async fn knock(host: &str, path: &str, body: &str) -> std::io::Result<bool> {
    let mut wire = TcpStream::connect(host).await?;
    let note = format!(
        "POST {path} HTTP/1.1\r\nhost: {host}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    wire.write_all(note.as_bytes()).await?;
    let mut head = vec![0u8; 15];
    wire.read_exact(&mut head).await?;
    let line = String::from_utf8_lossy(&head);
    Ok(line.contains(" 2"))
}

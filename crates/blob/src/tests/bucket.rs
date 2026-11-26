use super::Shed;
use axum::Router;
use axum::body::{Bytes, to_bytes};
use axum::extract::{Request, State};
use axum::http::{Method, StatusCode, Uri, request::Parts};
use axum::response::{IntoResponse, Response};
use base64::{Engine, engine::general_purpose::STANDARD};
use rusty_s3::actions::S3Action;
use rusty_s3::{Bucket as S3, Credentials, UrlStyle};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use time::{PrimitiveDateTime, macros::format_description};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use url::Url;

#[derive(Default)]
struct Memory {
    objects: Mutex<BTreeMap<String, Bytes>>,
    block: AtomicBool,
}

pub(super) struct Bucket {
    endpoint: String,
    memory: Arc<Memory>,
    task: JoinHandle<()>,
}

impl Drop for Bucket {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Bucket {
    pub(super) async fn open() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let memory = Arc::new(Memory::default());
        let router = Router::new().fallback(handle).with_state(memory.clone());
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        Self {
            endpoint,
            memory,
            task,
        }
    }

    pub(super) fn shed(&self) -> Shed<'_> {
        Shed {
            endpoint: &self.endpoint,
            name: "shed",
            region: "local",
            key: "test",
            secret: "fixture",
        }
    }

    pub(super) fn has(&self, path: &str) -> bool {
        let key = path.trim_start_matches("/shed/");
        self.memory.objects.lock().unwrap().contains_key(key)
    }

    pub(super) fn refuse(&self, block: bool) {
        self.memory.block.store(block, Ordering::Relaxed);
    }

    pub(super) async fn read(&self, url: Url) -> (StatusCode, Vec<u8>) {
        let response = reqwest::get(url).await.unwrap();
        (response.status(), response.bytes().await.unwrap().to_vec())
    }
}

async fn handle(State(memory): State<Arc<Memory>>, request: Request) -> Response {
    let (head, body) = request.into_parts();
    let Some(key) = head.uri.path().strip_prefix("/shed/") else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match head.method {
        Method::HEAD if key.is_empty() => StatusCode::OK.into_response(),
        Method::GET if key.is_empty() => listing(&memory, &head.uri).into_response(),
        Method::GET => match memory.objects.lock().unwrap().get(key) {
            Some(bytes) => bytes.clone().into_response(),
            None => StatusCode::NOT_FOUND.into_response(),
        },
        Method::PUT => {
            let bytes = to_bytes(body, 2048).await.unwrap();
            put(&memory, &head, key, bytes).into_response()
        }
        Method::DELETE if memory.block.load(Ordering::Relaxed) => {
            StatusCode::SERVICE_UNAVAILABLE.into_response()
        }
        Method::DELETE => {
            memory.objects.lock().unwrap().remove(key);
            StatusCode::NO_CONTENT.into_response()
        }
        _ => StatusCode::METHOD_NOT_ALLOWED.into_response(),
    }
}

fn put(memory: &Memory, head: &Parts, key: &str, bytes: Bytes) -> StatusCode {
    let length = head.headers["content-length"].to_str().unwrap();
    let checksum = head.headers["x-amz-checksum-sha256"].to_str().unwrap();
    if !signed(head, key, length, checksum) {
        return StatusCode::FORBIDDEN;
    }
    if length.parse::<usize>().unwrap() != bytes.len()
        || STANDARD.encode(Sha256::digest(&bytes)) != checksum
    {
        return StatusCode::BAD_REQUEST;
    }
    memory.objects.lock().unwrap().insert(key.into(), bytes);
    StatusCode::OK
}

fn signed(head: &Parts, key: &str, length: &str, checksum: &str) -> bool {
    let query = query(&head.uri);
    let time = PrimitiveDateTime::parse(
        &query["X-Amz-Date"],
        format_description!("[year][month][day]T[hour][minute][second]Z"),
    )
    .unwrap()
    .assume_utc();
    let host = head.headers["host"].to_str().unwrap();
    let bucket = S3::new(
        format!("http://{host}").parse().unwrap(),
        UrlStyle::Path,
        "shed",
        "local",
    )
    .unwrap();
    let creds = Credentials::new("test", "fixture");
    let mut put = bucket.put_object(Some(&creds), key);
    put.headers_mut().insert("content-length", length);
    put.headers_mut().insert("x-amz-checksum-sha256", checksum);
    let signed = put.sign_with_time(Duration::from_secs(crate::TTL), &time);
    signed.query() == head.uri.query()
}

fn query(uri: &Uri) -> HashMap<String, String> {
    url::form_urlencoded::parse(uri.query().unwrap_or_default().as_bytes())
        .into_owned()
        .collect()
}

fn listing(memory: &Memory, uri: &Uri) -> String {
    let query = query(uri);
    let prefix = &query["prefix"];
    assert_eq!(query["list-type"], "2");
    let mut xml = String::from("<ListBucketResult>");
    for (key, bytes) in memory.objects.lock().unwrap().iter() {
        if key.starts_with(prefix) {
            xml.push_str(&format!(
                "<Contents><Key>{key}</Key><ETag>fixture</ETag><LastModified>2026-01-01T00:00:00Z</LastModified><Size>{}</Size></Contents>",
                bytes.len()
            ));
        }
    }
    xml.push_str("</ListBucketResult>");
    xml
}

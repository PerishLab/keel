use super::{Shed, TTL};
use keel::adapt::Error;
use reqwest::{Client, Method, Response, redirect::Policy};
use rusty_s3::actions::{ListObjectsV2, S3Action};
use rusty_s3::{Bucket, Credentials, UrlStyle};
use std::time::Duration;
use url::Url;

const LIFE: Duration = Duration::from_secs(TTL);
const PAGE: usize = 64;
const LIMIT: usize = 256 * 1024;

pub(crate) struct Store {
    bucket: Bucket,
    creds: Credentials,
    client: Client,
}

pub(crate) struct Page {
    pub(crate) keys: Vec<String>,
    pub(crate) next: Option<String>,
}

impl Store {
    pub(crate) async fn open(shed: Shed<'_>) -> Result<Self, Error> {
        let base: Url = shed
            .endpoint
            .parse()
            .map_err(|_| fault("bad s3 endpoint"))?;
        if !base.username().is_empty() || base.password().is_some() {
            return Err(fault("bad s3 endpoint"));
        }
        if base.query().is_some() || base.fragment().is_some() {
            return Err(fault("bad s3 endpoint"));
        }
        let bucket = Bucket::new(
            base,
            UrlStyle::Path,
            shed.name.to_string(),
            shed.region.to_string(),
        )
        .map_err(|_| fault("bad s3 bucket"))?;
        let creds = Credentials::new(shed.key, shed.secret);
        let client = Client::builder()
            .redirect(Policy::none())
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|_| fault("cannot open s3 client"))?;
        let store = Self {
            bucket,
            creds,
            client,
        };
        let url = store.bucket.head_bucket(Some(&store.creds)).sign(LIFE);
        store.request(Method::HEAD, url).await?;
        Ok(store)
    }

    pub(crate) fn address(&self) -> &str {
        self.bucket.base_url().as_str()
    }

    pub(crate) fn put(&self, object: &str, headers: &[(&str, String)]) -> Url {
        let mut put = self.bucket.put_object(Some(&self.creds), object);
        for (name, value) in headers {
            put.headers_mut().insert(*name, value.as_str());
        }
        put.sign(LIFE)
    }

    pub(crate) fn get(&self, object: &str) -> Url {
        self.bucket.get_object(Some(&self.creds), object).sign(LIFE)
    }

    pub(crate) async fn delete(&self, object: &str) -> Result<(), Error> {
        let url = self
            .bucket
            .delete_object(Some(&self.creds), object)
            .sign(LIFE);
        self.request(Method::DELETE, url).await?;
        Ok(())
    }

    pub(crate) async fn list(&self, prefix: &str, cursor: Option<&str>) -> Result<Page, Error> {
        let mut list = self.bucket.list_objects_v2(Some(&self.creds));
        list.query_mut().remove("encoding-type");
        list.with_prefix(prefix);
        list.with_max_keys(PAGE);
        if let Some(cursor) = cursor {
            list.with_continuation_token(cursor);
        }
        let mut response = self.request(Method::GET, list.sign(LIFE)).await?;
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| fault("s3 listing interrupted"))?
        {
            if bytes.len() + chunk.len() > LIMIT {
                return Err(fault("s3 listing exceeds its limit"));
            }
            bytes.extend_from_slice(&chunk);
        }
        let page = ListObjectsV2::parse_response(bytes).map_err(|_| fault("invalid s3 listing"))?;
        if page.contents.len() > PAGE
            || page
                .next_continuation_token
                .as_deref()
                .is_some_and(|next| next.is_empty() || Some(next) == cursor)
        {
            return Err(fault("invalid s3 listing page"));
        }
        Ok(Page {
            keys: page.contents.into_iter().map(|item| item.key).collect(),
            next: page.next_continuation_token,
        })
    }

    async fn request(&self, method: Method, url: Url) -> Result<Response, Error> {
        let response = self
            .client
            .request(method, url)
            .send()
            .await
            .map_err(|_| fault("s3 request failed"))?;
        if !response.status().is_success() {
            return Err(fault(&format!(
                "s3 refused request ({})",
                response.status().as_u16()
            )));
        }
        Ok(response)
    }
}

fn fault(note: &str) -> Error {
    Error::Adapt(note.into())
}

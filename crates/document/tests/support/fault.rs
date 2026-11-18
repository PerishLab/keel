use keel::adapt::{Error, Store, db::Sqlite};
use keel::ddl::Grain;
use keel::{Val, Wire};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct Fault {
    pub inner: Sqlite,
    pub armed: Arc<AtomicBool>,
}

impl Wire for Fault {
    fn grain(&self) -> Grain {
        self.inner.grain()
    }

    async fn run(&mut self, sql: &str, args: &[Val]) -> Result<u64, Error> {
        self.inner.run(sql, args).await
    }

    async fn plant(&mut self, sql: &str, args: &[Val]) -> Result<i64, Error> {
        self.inner.plant(sql, args).await
    }

    async fn rows(&mut self, sql: &str, args: &[Val]) -> Result<Vec<Vec<Val>>, Error> {
        self.inner.rows(sql, args).await
    }

    async fn script(&mut self, sql: &str) -> Result<(), Error> {
        if sql == "COMMIT" && self.armed.swap(false, Ordering::SeqCst) {
            self.inner.script("ROLLBACK").await?;
            return Err(Error::Store(Store::Down("injected commit refusal".into())));
        }
        self.inner.script(sql).await
    }

    async fn revive(&mut self) -> Result<(), Error> {
        self.inner.revive().await
    }

    async fn epoch(&mut self) -> Result<Option<String>, Error> {
        self.inner.epoch().await
    }
}

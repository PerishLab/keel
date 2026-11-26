use super::{Binding, Vault, by, catalog};
use keel::Wire;
use keel::adapt::Error;
use std::sync::Arc;
use std::time::Duration;

impl<W: Wire + 'static> Vault<W> {
    pub(crate) fn watch(&self) {
        let core = Arc::downgrade(&self.core);
        let store = Arc::downgrade(&self.store);
        let prefix = self.prefix.clone();
        tokio::spawn(async move {
            let mut cursor = None;
            loop {
                let delay = if cursor.is_some() { 1 } else { 30 };
                tokio::time::sleep(Duration::from_secs(delay)).await;
                let (Some(core), Some(store)) = (core.upgrade(), store.upgrade()) else {
                    break;
                };
                let vault = Self {
                    core,
                    store,
                    prefix: prefix.clone(),
                };
                cursor = vault.next(cursor.as_deref()).await;
            }
        });
    }

    async fn next(&self, cursor: Option<&str>) -> Option<String> {
        match self.sweep(cursor).await {
            Ok(next) => next,
            Err(err) => {
                eprintln!("keel-blob: {err}");
                None
            }
        }
    }

    pub(crate) async fn sweep(&self, cursor: Option<&str>) -> Result<Option<String>, Error> {
        let page = self.store.list(&self.prefix, cursor).await?;
        for key in page.keys {
            if !key.starts_with(self.prefix.as_ref()) {
                return Err(Error::Adapt(
                    "bucket returned an object outside its prefix".into(),
                ));
            }
            if let Err(err) = self.reap(&key).await {
                eprintln!("keel-blob: {err}");
            }
        }
        Ok(page.next)
    }

    async fn reap(&self, key: &str) -> Result<(), Error> {
        let sudo = self.core.sudo();
        let Some(bound) = sudo.one(&by(catalog::PARCEL, "object", key)).await? else {
            return Ok(());
        };
        let binding = Binding::read(&bound)?;
        if sudo
            .one(&by("Asset", "id", &binding.asset))
            .await?
            .is_none()
        {
            self.store.delete(key).await?;
        }
        Ok(())
    }
}

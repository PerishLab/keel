use crate::adapt::Error;
use crate::ddl::Grain;
use crate::plan::Plan;
use crate::wire::Wire;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(1);

struct Projection<'a> {
    plan: &'a Plan,
}

pub(super) async fn shape<W: Wire>(plan: &Plan, wire: &mut W) -> Result<String, Error> {
    Projection { plan }.shape(wire).await
}

impl Projection<'_> {
    async fn shape<W: Wire>(&self, wire: &mut W) -> Result<String, Error> {
        match wire.grain() {
            Grain::Lite => self.lite().await,
            Grain::Pg => self.pg(wire).await,
        }
    }

    async fn lite(&self) -> Result<String, Error> {
        let mut wire = crate::adapt::db::Sqlite::memory().await?;
        self.build(&mut wire).await?;
        super::catalog::Catalog(&mut wire).shape().await
    }

    async fn pg<W: Wire>(&self, wire: &mut W) -> Result<String, Error> {
        wire.script("BEGIN").await?;
        let out = self.shadow(wire).await;
        let undone = wire.script("ROLLBACK").await;
        match (out, undone) {
            (Ok(shape), Ok(())) => Ok(shape),
            (Err(err), _) => Err(err),
            (Ok(_), Err(err)) => Err(err),
        }
    }

    async fn shadow<W: Wire>(&self, wire: &mut W) -> Result<String, Error> {
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let name = format!(
            "keel_adopt_{}_{}_{}",
            std::process::id(),
            crate::life::tick(),
            id
        );
        wire.script(&format!("CREATE SCHEMA {}", crate::ddl::col(&name)))
            .await?;
        wire.script(&format!(
            "SET LOCAL search_path TO {}",
            crate::ddl::col(&name)
        ))
        .await?;
        self.build(wire).await?;
        super::catalog::Catalog(wire).shape().await
    }

    async fn build<W: Wire>(&self, wire: &mut W) -> Result<(), Error> {
        for stmt in crate::ddl::script(self.plan, wire.grain()) {
            wire.script(&stmt).await?;
        }
        Ok(())
    }
}

use super::*;
use crate::adapt::Error;
use crate::cap;
use crate::ddl::Grain;
use crate::life::{Ends, Row, Tie};
use crate::plan::Plan;
use crate::query::{self, Pack, Tree};
use crate::wire::{Val, Wire};
use std::sync::Arc;

impl<W: Wire> Core<W> {
    pub(crate) fn new(plan: Plan, wire: W) -> Self {
        Self {
            plan,
            seat: tokio::sync::Mutex::new(Seat { wire, dirty: false }),
            identity: None,
            stash: Stash {
                on: true,
                ..Default::default()
            },
            chart: Chart::default(),
            deeds: Deeds {
                on: true,
                ..Default::default()
            },
        }
    }

    pub fn bare(mut self) -> Self {
        self.stash.on = false;
        self.deeds.on = false;
        self
    }

    pub fn identify(mut self, unit: &str) -> Result<Self, Error> {
        let name = query::resolve(&self.plan, unit)?;
        self.identity = Some(name);
        Ok(self)
    }

    pub fn identity(&self) -> Option<&str> {
        self.identity.as_deref()
    }

    pub fn plan(&self) -> &Plan {
        &self.plan
    }

    pub(super) async fn seize(&self) -> Result<tokio::sync::MutexGuard<'_, Seat<W>>, Error> {
        let mut seat = self.seat.lock().await;
        seat.wire.revive().await?;
        if seat.dirty {
            seat.wire.script("ROLLBACK").await?;
            seat.dirty = false;
        }
        Ok(seat)
    }

    pub async fn put(&self, name: &str, fields: &[(&str, &str)]) -> Result<i64, Error> {
        self.sudo().put(name, fields).await
    }

    pub async fn set(&self, name: &str, key: i64, fields: &[(&str, &str)]) -> Result<(), Error> {
        self.sudo().set(name, key, fields).await
    }

    pub async fn unset(&self, name: &str, key: i64, fields: &[&str]) -> Result<(), Error> {
        self.sudo().unset(name, key, fields).await
    }

    pub async fn live(&self, name: &str) -> Result<Vec<Row>, Error> {
        self.sudo().live(name).await
    }

    pub async fn query(&self, text: &str) -> Result<Pack, Error> {
        self.sudo().query(text).await
    }

    pub async fn ask(&self, tree: &Tree) -> Result<Pack, Error> {
        self.sudo().ask(tree).await
    }

    pub async fn one(&self, tree: &Tree) -> Result<Option<Row>, Error> {
        self.sudo().one(tree).await
    }

    pub async fn end(&self, name: &str, key: i64) -> Result<(), Error> {
        self.sudo().end(name, key).await
    }

    pub async fn lease(&self, name: &str, key: i64, at: i64) -> Result<(), Error> {
        self.sudo().lease(name, key, at).await
    }

    pub async fn tie(
        &self,
        owner: &str,
        bond: &str,
        ends: Ends,
        fields: &[(&str, &str)],
    ) -> Result<i64, Error> {
        self.sudo().tie(owner, bond, ends, fields).await
    }

    pub async fn tune(
        &self,
        owner: &str,
        bond: &str,
        key: i64,
        fields: &[(&str, &str)],
    ) -> Result<(), Error> {
        self.sudo().tune(owner, bond, key, fields).await
    }

    pub async fn ties(&self, owner: &str, bond: &str, left: i64) -> Result<Vec<Tie>, Error> {
        self.sudo().ties(owner, bond, left).await
    }

    pub async fn cut(&self, owner: &str, bond: &str, key: i64) -> Result<(), Error> {
        self.sudo().cut(owner, bond, key).await
    }

    pub async fn flow(&self, cursor: i64) -> Result<Vec<Row>, Error> {
        self.sudo().flow(cursor).await
    }

    pub async fn has(&self, name: &str) -> Result<bool, Error> {
        let mut seat = self.seize().await?;
        let table = name.to_string();
        let rows = match seat.wire.grain() {
            Grain::Lite => {
                seat.wire
                    .rows(
                        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
                        &[Val::Text(table)],
                    )
                    .await?
            }
            Grain::Pg => {
                seat.wire
                    .rows(
                        "SELECT 1 FROM information_schema.tables WHERE table_name = ?1",
                        &[Val::Text(table)],
                    )
                    .await?
            }
        };
        Ok(!rows.is_empty())
    }

    pub async fn cols(&self, name: &str) -> Result<Vec<String>, Error> {
        let mut seat = self.seize().await?;
        let table = name.to_string();
        match seat.wire.grain() {
            Grain::Lite => {
                let rows = seat
                    .wire
                    .rows(&format!("PRAGMA table_info({table})"), &[])
                    .await?;
                Ok(rows.into_iter().map(|line| line[1].text()).collect())
            }
            Grain::Pg => {
                let rows = seat
                    .wire
                    .rows(
                        "SELECT column_name FROM information_schema.columns WHERE table_name = ?1 ORDER BY ordinal_position",
                        &[Val::Text(table)],
                    )
                    .await?;
                Ok(rows.into_iter().map(|line| line[0].text()).collect())
            }
        }
    }

    pub async fn seal(&self, token: &str) -> Result<bool, Error> {
        let mut seat = self.seize().await?;
        cap::sealed(&self.plan, &mut seat.wire, token).await
    }

    pub async fn batch<T>(
        &self,
        run: impl AsyncFnOnce(&mut Tx<'_, W>) -> Result<T, Error>,
    ) -> Result<T, Error> {
        self.sudo().batch(run).await
    }

    pub fn share(self) -> Arc<Self> {
        Arc::new(self)
    }

    pub fn sudo(&self) -> Face<'_, W> {
        Face {
            core: self,
            who: Who::Sudo,
        }
    }

    pub fn of(&self, operator: i64) -> Face<'_, W> {
        Face {
            core: self,
            who: Who::Op(operator),
        }
    }

    pub fn anon(&self) -> Face<'_, W> {
        Face {
            core: self,
            who: Who::Anon,
        }
    }
}

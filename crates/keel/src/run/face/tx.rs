use super::*;
use super::{label, lane};
use crate::adapt::Error;
use crate::cap;
use crate::ddl;
use crate::life::{Ends, Row, Work};
use crate::plan::Plan;
use crate::query::{self, Pack, Tree};
use crate::wire::Wire;

impl<W: Wire> Tx<'_, W> {
    pub fn who(&self) -> Who {
        self.who
    }

    pub(super) fn free(&self) -> bool {
        matches!(self.who, Who::Sudo)
    }

    pub(super) fn plan(&self) -> &Plan {
        self.core.plan()
    }

    pub(super) async fn craft(
        &mut self,
        name: &str,
        fields: &[(&str, &str)],
    ) -> Result<i64, Error> {
        let plan = self.core.plan();
        let unit = query::resolve(plan, name)?;
        let key = Work::new(&mut self.seat.wire, plan)
            .put(&unit, fields)
            .await?;
        self.beat("put", &ddl::table(&unit), key).await;
        Ok(key)
    }

    pub(super) async fn beat(&mut self, verb: &str, unit: &str, key: i64) {
        self.core.stash.bump(unit);
        if unit == ddl::table(cap::PULSE) || unit == ddl::table(cap::SEAL) {
            return;
        }
        let told = label(self.who);
        let plan = self.core.plan();
        if let Err(err) = Work::new(&mut self.seat.wire, plan)
            .pulse(verb, unit, key, &told)
            .await
        {
            eprintln!("keel: pulse: {err}");
        }
    }

    pub(super) async fn shift(
        &mut self,
        name: &str,
        key: i64,
        fields: &[(&str, &str)],
    ) -> Result<(), Error> {
        let plan = self.core.plan();
        let unit = query::resolve(plan, name)?;
        Work::new(&mut self.seat.wire, plan)
            .set(&unit, key, fields)
            .await?;
        self.beat("set", &ddl::table(&unit), key).await;
        Ok(())
    }

    pub(super) async fn fell(
        &mut self,
        name: &str,
        key: i64,
        at: Option<i64>,
    ) -> Result<(), Error> {
        let plan = self.core.plan();
        let unit = query::resolve(plan, name)?;
        match at {
            Some(at) => {
                Work::new(&mut self.seat.wire, plan)
                    .lease(&unit, key, at)
                    .await?
            }
            None => Work::new(&mut self.seat.wire, plan).end(&unit, key).await?,
        }
        self.beat("end", &ddl::table(&unit), key).await;
        Ok(())
    }

    pub(super) async fn knot(
        &mut self,
        owner: &str,
        bond: &str,
        ends: Ends,
        fields: &[(&str, &str)],
    ) -> Result<i64, Error> {
        let plan = self.core.plan();
        let unit = query::resolve(plan, owner)?;
        let key = Work::new(&mut self.seat.wire, plan)
            .tie(&unit, bond, ends, fields)
            .await?;
        self.beat("tie", &lane(&unit, bond), key).await;
        Ok(key)
    }

    pub(super) async fn bend(
        &mut self,
        owner: &str,
        bond: &str,
        key: i64,
        fields: &[(&str, &str)],
    ) -> Result<(), Error> {
        let plan = self.core.plan();
        let unit = query::resolve(plan, owner)?;
        Work::new(&mut self.seat.wire, plan)
            .tune(&unit, bond, key, fields)
            .await?;
        self.beat("tie", &lane(&unit, bond), key).await;
        Ok(())
    }

    pub(super) async fn snip(&mut self, owner: &str, bond: &str, key: i64) -> Result<(), Error> {
        let plan = self.core.plan();
        let unit = query::resolve(plan, owner)?;
        Work::new(&mut self.seat.wire, plan)
            .cut(&unit, bond, key)
            .await?;
        self.beat("cut", &lane(&unit, bond), key).await;
        Ok(())
    }

    pub(super) async fn sight(&mut self, tree: &Tree) -> Result<Pack, Error> {
        let key = query::digest(tree);
        let units = tree.involved(self.core.plan())?;
        if let Some(pack) = self.core.stash.read(&key, &units) {
            return Ok(pack);
        }
        let scope = self.core.chart.scope(self.core.plan(), tree)?;
        let pack = query::run(self.core.plan(), &mut self.seat.wire, tree, &scope).await?;
        self.core.stash.keep(key, &units, &pack);
        Ok(pack)
    }

    pub(super) async fn seen(&mut self, unit: &str, key: i64) -> Result<Row, Error> {
        let plan = self.core.plan();
        let node = plan.find(unit)?;
        let row = Work::new(&mut self.seat.wire, plan)
            .one(node, key)
            .await?
            .ok_or_else(|| Error::Adapt(format!("missing row {key}")))?;
        let mark = cap::Mark {
            key: Some(key),
            cells: row.cells(),
        };
        if !self.held("see", unit, &mark).await? {
            return Err(Error::Adapt(format!("missing row {key}")));
        }
        Ok(row)
    }

    pub(super) async fn deeds(&mut self) -> Result<Arc<Vec<Row>>, Error> {
        let step = self.core.stash.step(&ddl::table(cap::GRANT));
        if let Some(rows) = self.core.deeds.read(step) {
            return Ok(rows);
        }
        let plan = self.core.plan();
        let node = plan.find(cap::GRANT)?;
        let rows = Work::new(&mut self.seat.wire, plan).scan(node).await?;
        Ok(self.core.deeds.keep(step, rows))
    }

    pub(super) async fn held(
        &mut self,
        verb: &str,
        unit: &str,
        mark: &cap::Mark<'_>,
    ) -> Result<bool, Error> {
        let deeds = self.deeds().await?;
        let plea = cap::Plea {
            who: self.who,
            verb,
            unit: &ddl::table(unit),
            mark,
        };
        cap::check(self.core.plan(), &mut self.seat.wire, &plea, &deeds).await
    }

    pub(super) async fn may(
        &mut self,
        verb: &str,
        unit: &str,
        mark: &cap::Mark<'_>,
    ) -> Result<(), Error> {
        if self.held(verb, unit, mark).await? {
            return Ok(());
        }
        Err(Error::Adapt(format!("refused {verb}")))
    }
}

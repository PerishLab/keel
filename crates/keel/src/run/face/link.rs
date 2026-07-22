use super::*;
use crate::adapt::Error;
use crate::cap;
use crate::life::{Ends, Row, Tie, Work};
use crate::query::{self};
use crate::wire::Wire;

impl<W: Wire> Tx<'_, W> {
    pub async fn tie(
        &mut self,
        owner: &str,
        bond: &str,
        ends: Ends,
        fields: &[(&str, &str)],
    ) -> Result<i64, Error> {
        if self.free() {
            return self.knot(owner, bond, ends, fields).await;
        }
        let unit = query::resolve(self.plan(), owner)?;
        let target = self.target(&unit, bond)?;
        let left = self.seen(&unit, ends.left).await?;
        let mark = cap::Mark {
            key: Some(ends.left),
            cells: left.cells(),
        };
        self.may("tie", &unit, &mark).await?;
        if !self.spot(&target, ends.right).await? {
            return Err(Error::Adapt(format!("missing row {}", ends.right)));
        }
        self.knot(&unit, bond, ends, fields).await
    }

    pub async fn tune(
        &mut self,
        owner: &str,
        bond: &str,
        key: i64,
        fields: &[(&str, &str)],
    ) -> Result<(), Error> {
        if self.free() {
            return self.bend(owner, bond, key, fields).await;
        }
        let unit = query::resolve(self.plan(), owner)?;
        let tie = self.grip(&unit, bond, key).await?;
        let left = self.seen(&unit, tie.left()).await?;
        let mark = cap::Mark {
            key: Some(tie.left()),
            cells: left.cells(),
        };
        self.may("tie", &unit, &mark).await?;
        self.bend(&unit, bond, key, fields).await
    }

    pub async fn ties(&mut self, owner: &str, bond: &str, left: i64) -> Result<Vec<Tie>, Error> {
        let plan = self.core.plan();
        if self.free() {
            let (node, edge) = plan.edge(owner, bond)?;
            return Work::new(&mut self.seat.wire, plan)
                .ties(node, edge, left)
                .await;
        }
        let unit = query::resolve(plan, owner)?;
        let _ = self.seen(&unit, left).await?;
        let target = self.target(&unit, bond)?;
        let (node, edge) = plan.edge(&unit, bond)?;
        let ties = Work::new(&mut self.seat.wire, plan)
            .ties(node, edge, left)
            .await?;
        let mut out = Vec::new();
        for tie in ties {
            if self.spot(&target, tie.right()).await? {
                out.push(tie);
            }
        }
        Ok(out)
    }

    pub async fn cut(&mut self, owner: &str, bond: &str, key: i64) -> Result<(), Error> {
        if self.free() {
            return self.snip(owner, bond, key).await;
        }
        let unit = query::resolve(self.plan(), owner)?;
        let tie = self.grip(&unit, bond, key).await?;
        let left = self.seen(&unit, tie.left()).await?;
        let mark = cap::Mark {
            key: Some(tie.left()),
            cells: left.cells(),
        };
        self.may("cut", &unit, &mark).await?;
        self.snip(&unit, bond, key).await
    }

    pub async fn flow(&mut self, cursor: i64) -> Result<Vec<Row>, Error> {
        let plan = self.core.plan();
        let node = plan.find(cap::PULSE)?;
        let rows = Work::new(&mut self.seat.wire, plan).scan(node).await?;
        if let Some(first) = rows.first()
            && cursor + 1 < first.key()
        {
            return Err(Error::Adapt("cursor past window".into()));
        }
        let rows: Vec<Row> = rows.into_iter().filter(|row| row.key() > cursor).collect();
        if self.free() {
            return Ok(rows);
        }
        let mut out = Vec::new();
        for row in rows {
            if self.heard(&row).await? {
                out.push(row);
            }
        }
        Ok(out)
    }

    pub(super) async fn heard(&mut self, event: &Row) -> Result<bool, Error> {
        let place = event
            .cells()
            .get("unit")
            .map(crate::life::Cell::show)
            .unwrap_or_default();
        let key = match event.cells().get("key") {
            Some(crate::life::Cell::Int(key)) => *key,
            _ => return Ok(false),
        };
        if let Some((owner, bond)) = place.split_once('.') {
            return self.caught(owner, bond, key).await;
        }
        let Ok(unit) = query::resolve(self.plan(), &place) else {
            return Ok(false);
        };
        let plan = self.core.plan();
        let node = plan.find(&unit)?;
        match Work::new(&mut self.seat.wire, plan).one(node, key).await? {
            Some(row) => {
                let mark = cap::Mark {
                    key: Some(key),
                    cells: row.cells(),
                };
                self.held("see", &unit, &mark).await
            }
            None => self.wide("see", &unit).await,
        }
    }

    pub(super) async fn caught(
        &mut self,
        owner: &str,
        bond: &str,
        key: i64,
    ) -> Result<bool, Error> {
        let Ok(unit) = query::resolve(self.plan(), owner) else {
            return Ok(false);
        };
        if let Ok(tie) = self.grip(&unit, bond, key).await {
            return Ok(self.seen(&unit, tie.left()).await.is_ok());
        }
        self.wide("see", &unit).await
    }

    pub(super) async fn wide(&mut self, verb: &str, unit: &str) -> Result<bool, Error> {
        let unit = self.core.plan().find(unit)?.key();
        let deeds = self.deeds().await?;
        let plea = cap::Plea {
            who: self.who,
            verb,
            unit: &unit,
            mark: &cap::Mark::none(),
        };
        cap::broad(self.core.plan(), &mut self.seat.wire, &plea, &deeds).await
    }

    pub(super) async fn grip(&mut self, unit: &str, bond: &str, key: i64) -> Result<Tie, Error> {
        let node = self.plan().find(unit)?;
        let name = node
            .bonds()
            .iter()
            .find(|e| e.name().eq_ignore_ascii_case(bond))
            .map(|e| e.name().to_string())
            .ok_or_else(|| Error::Adapt(format!("missing bond {bond}")))?;
        let pack = self.sight(&query::form(unit)).await?;
        let lefts = pack.rows().to_vec();
        for row in lefts {
            let plan = self.core.plan();
            let (node, edge) = plan.edge(unit, &name)?;
            let ties = Work::new(&mut self.seat.wire, plan)
                .ties(node, edge, row.key())
                .await?;
            if let Some(tie) = ties.into_iter().find(|t| t.key() == key) {
                return Ok(tie);
            }
        }
        Err(Error::Adapt(format!("missing tie {key}")))
    }

    pub(super) fn target(&self, unit: &str, bond: &str) -> Result<String, Error> {
        let node = self.plan().find(unit)?;
        node.bonds()
            .iter()
            .find(|e| e.name().eq_ignore_ascii_case(bond))
            .map(|e| e.target().to_string())
            .ok_or_else(|| Error::Adapt(format!("missing bond {bond}")))
    }
}

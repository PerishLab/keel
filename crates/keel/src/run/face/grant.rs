use super::*;
use crate::adapt::Error;
use crate::cap::{self, Grant};
use crate::life::{Cell, Work};
use crate::query::{self};
use crate::wire::Wire;
use std::collections::BTreeMap;

impl<W: Wire> Tx<'_, W> {
    pub async fn birth(&mut self, fields: &[(&str, &str)]) -> Result<i64, Error> {
        if self.who != Who::Sudo {
            return Err(Error::Adapt("identity birth needs sudo".into()));
        }
        let unit = self
            .core
            .identity()
            .ok_or_else(|| Error::Adapt("no identity unit".into()))?
            .to_string();
        let key = self.put(&unit, fields).await?;
        self.grant(Grant::new(
            &key.to_string(),
            "*",
            &unit,
            &format!("row {key}"),
        ))
        .await?;
        Ok(key)
    }

    pub async fn put(&mut self, name: &str, fields: &[(&str, &str)]) -> Result<i64, Error> {
        let unit = self.resource(name)?;
        if self.free() {
            return self.craft(&unit, fields).await;
        }
        let cells = cap::mold(self.plan(), &unit, fields);
        let mark = cap::Mark {
            key: None,
            cells: &cells,
        };
        self.may("put", &unit, &mark).await?;
        let key = self.craft(&unit, fields).await?;
        self.mint(&unit, key, &cells).await?;
        Ok(key)
    }

    pub async fn grant(&mut self, grant: Grant<'_>) -> Result<i64, Error> {
        let fields = grant.fields();
        if !self.free() {
            self.narrow(fields).await?;
        }
        self.craft(cap::GRANT, fields).await
    }

    pub(super) async fn mint(
        &mut self,
        unit: &str,
        key: i64,
        cells: &BTreeMap<String, Cell>,
    ) -> Result<(), Error> {
        let who = match self.who {
            Who::Op(op) => op,
            Who::Anon if self.core.identity() == Some(unit) => key,
            _ => return Ok(()),
        };
        if self.spans(unit, key, cells).await? {
            return Ok(());
        }
        self.craft(
            cap::GRANT,
            Grant::new(&who.to_string(), "*", unit, &format!("row {key}")).fields(),
        )
        .await?;
        Ok(())
    }

    async fn spans(
        &mut self,
        unit: &str,
        key: i64,
        cells: &BTreeMap<String, Cell>,
    ) -> Result<bool, Error> {
        let deeds = self.deeds().await?;
        let mark = cap::Mark {
            key: Some(key),
            cells,
        };
        let plea = cap::Plea {
            who: self.who,
            verb: "*",
            unit,
            mark: &mark,
        };
        cap::Court::new(self.core.plan(), &mut self.seat.wire, &deeds)
            .spans(&plea)
            .await
    }

    pub async fn revoke(&mut self, key: i64) -> Result<(), Error> {
        if self.free() {
            return self.fell(cap::GRANT, key, None).await;
        }
        let plan = self.core.plan();
        let node = plan.find(cap::GRANT)?;
        let row = Work::new(&mut self.seat.wire, plan)
            .one(node, key)
            .await?
            .ok_or_else(|| Error::Adapt(format!("missing row {key}")))?;
        let verb = row
            .cells()
            .get("verb")
            .map(|c| c.show())
            .unwrap_or_default();
        let unit = row
            .cells()
            .get("unit")
            .map(|c| c.show())
            .unwrap_or_default();
        let span = row
            .cells()
            .get("scope")
            .map(|c| c.show())
            .unwrap_or_default();
        self.narrow(&[("verb", &verb), ("unit", &unit), ("scope", &span)])
            .await?;
        self.fell(cap::GRANT, key, None).await
    }

    pub(super) async fn narrow(&mut self, fields: &[(&str, &str)]) -> Result<(), Error> {
        let verb = cap::field(fields, "verb");
        let unit = cap::field(fields, "unit");
        let span = cap::field(fields, "scope");
        if unit == "*" {
            return Err(Error::Adapt("refused put".into()));
        }
        let unit = query::resolve(self.plan(), unit)?;
        let verbs: Vec<&str> = if verb == "*" {
            cap::VERBS.to_vec()
        } else {
            vec![verb]
        };
        for verb in verbs {
            self.beneath(verb, &unit, span).await?;
        }
        Ok(())
    }

    pub(super) async fn beneath(
        &mut self,
        verb: &str,
        unit: &str,
        span: &str,
    ) -> Result<(), Error> {
        if let Some(id) = span.strip_prefix("row ") {
            let key = id
                .parse::<i64>()
                .map_err(|_| Error::Adapt("row scope needs id".into()))?;
            let plan = self.core.plan();
            let node = plan.find(unit)?;
            let row = Work::new(&mut self.seat.wire, plan)
                .one(node, key)
                .await?
                .ok_or_else(|| Error::Adapt("refused put".into()))?;
            let mark = cap::Mark {
                key: Some(key),
                cells: row.cells(),
            };
            return self.may(verb, unit, &mark).await;
        }
        let deeds = self.deeds().await?;
        let plea = cap::Plea {
            who: self.who,
            verb,
            unit,
            mark: &cap::Mark::none(),
        };
        if cap::Court::new(self.core.plan(), &mut self.seat.wire, &deeds)
            .broad(&plea)
            .await?
        {
            return Ok(());
        }
        Err(Error::Adapt("refused put".into()))
    }
}

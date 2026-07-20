use super::*;
use crate::adapt::Error;
use crate::cap;
use crate::ddl;
use crate::life::Work;
use crate::query::{self};
use crate::wire::Wire;

impl<W: Wire> Tx<'_, W> {
    pub async fn put(&mut self, name: &str, fields: &[(&str, &str)]) -> Result<i64, Error> {
        if self.free() {
            return self.craft(name, fields).await;
        }
        let unit = query::resolve(self.plan(), name)?;
        if unit == cap::GRANT {
            self.narrow(fields).await?;
            return self.craft(&unit, fields).await;
        }
        let cells = cap::mold(self.plan(), &unit, fields);
        let mark = cap::Mark {
            key: None,
            cells: &cells,
        };
        self.may("put", &unit, &mark).await?;
        let key = self.craft(&unit, fields).await?;
        self.mint(&unit, key).await?;
        Ok(key)
    }

    pub(super) async fn mint(&mut self, unit: &str, key: i64) -> Result<(), Error> {
        if unit == cap::GRANT {
            return Ok(());
        }
        let who = match self.who {
            Who::Op(op) => op,
            Who::Anon if self.core.identity() == Some(unit) => key,
            _ => return Ok(()),
        };
        self.craft(
            cap::GRANT,
            &[
                ("who", &who.to_string()),
                ("verb", "*"),
                ("unit", unit),
                ("scope", &format!("row {key}")),
            ],
        )
        .await?;
        Ok(())
    }

    pub(super) async fn revoke(&mut self, key: i64) -> Result<(), Error> {
        let plan = self.core.plan();
        let row = Work::new(&mut self.seat.wire, plan)
            .one(cap::GRANT, key)
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
            let row = Work::new(&mut self.seat.wire, plan)
                .one(unit, key)
                .await?
                .ok_or_else(|| Error::Adapt("refused put".into()))?;
            let mark = cap::Mark {
                key: Some(key),
                cells: row.cells(),
            };
            return self.may(verb, unit, &mark).await;
        }
        if cap::broad(
            self.core.plan(),
            &mut self.seat.wire,
            self.who,
            verb,
            &ddl::table(unit),
        )
        .await?
        {
            return Ok(());
        }
        Err(Error::Adapt("refused put".into()))
    }
}

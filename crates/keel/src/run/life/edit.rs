use super::*;
use crate::adapt::Error;
use crate::bond;
use crate::ddl;
use crate::plan::{Edge, Unit};
use crate::wire::{Val, Wire};

impl<'a, W: Wire> Work<'a, W> {
    pub async fn end(&mut self, name: &str, key: i64) -> Result<(), Error> {
        self.lease(name, key, now()).await
    }

    pub async fn lease(&mut self, name: &str, key: i64, at: i64) -> Result<(), Error> {
        let unit = self.plan.find(name)?;
        if unit.name() == crate::cap::PULSE {
            return Err(Error::Adapt("pulse is engine owned".into()));
        }
        let tick = now();
        if at < tick {
            return Err(Error::Adapt("lease is not the past".into()));
        }
        if self.live_in(unit.name(), key).await? || self.live_out(unit, key).await? {
            return Err(Error::Adapt("live ties remain".into()));
        }
        let text = format!(
            "UPDATE {} SET {} = ?1, {} = ?2 WHERE {} = ?3 AND ({} IS NULL OR {} > ?2)",
            ddl::seat(unit.name()),
            ddl::EXPIRES,
            ddl::UPDATED,
            ddl::KEY,
            ddl::EXPIRES,
            ddl::EXPIRES
        );
        let n = self
            .wire
            .run(&text, &[Val::Int(at), Val::Int(tick), Val::Int(key)])
            .await?;
        if n == 0 {
            return Err(Error::Adapt(format!("missing row {key}")));
        }
        Ok(())
    }

    pub async fn pulse(
        &mut self,
        verb: &str,
        unit: &str,
        key: i64,
        who: &str,
    ) -> Result<(), Error> {
        let seat = self.plan.find(crate::cap::PULSE)?;
        let tick = now();
        let text = format!(
            "INSERT INTO {} (verb, unit, who, {}, {}, {}, {}) VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?5)",
            ddl::seat(seat.name()),
            ddl::col("key"),
            ddl::EXPIRES,
            ddl::CREATED,
            ddl::UPDATED
        );
        let args = [
            Val::Text(verb.into()),
            Val::Text(unit.into()),
            Val::Text(who.into()),
            Val::Int(key),
            Val::Int(tick),
        ];
        self.wire.run(&text, &args).await?;
        self.trim(seat).await
    }

    pub(super) async fn trim(&mut self, seat: &Unit) -> Result<(), Error> {
        let text = format!(
            "DELETE FROM {} WHERE {} <= (SELECT MAX({}) FROM {}) - ?1",
            ddl::seat(seat.name()),
            ddl::KEY,
            ddl::KEY,
            ddl::seat(seat.name())
        );
        self.wire
            .run(&text, &[Val::Int(crate::cap::WINDOW as i64)])
            .await?;
        Ok(())
    }

    pub(super) async fn fresh(&mut self, name: &str, key: i64) -> Result<bool, Error> {
        let unit = self.plan.find(name)?;
        let text = format!(
            "SELECT 1 FROM {} WHERE {} = ?1 AND {} IS NULL LIMIT 1",
            ddl::seat(unit.name()),
            ddl::KEY,
            ddl::EXPIRES
        );
        Ok(!self.wire.rows(&text, &[Val::Int(key)]).await?.is_empty())
    }

    pub(super) async fn live_in(&mut self, target: &str, key: i64) -> Result<bool, Error> {
        for unit in self.plan.units().values() {
            for edge in unit.bonds() {
                if edge.target() != target {
                    continue;
                }
                if self.live_from(unit, edge, key).await? {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    pub(super) async fn live_from(
        &mut self,
        unit: &Unit,
        edge: &Edge,
        key: i64,
    ) -> Result<bool, Error> {
        let tick = now();
        let (place, col) = if edge.kind().point() {
            (ddl::seat(unit.name()), ddl::col(&ddl::side(edge.name())))
        } else {
            (
                ddl::joint(unit.name(), edge.name()),
                ddl::col(&ddl::mate(unit.name(), edge.name(), edge.target())),
            )
        };
        let text = format!(
            "SELECT 1 FROM {} WHERE {} = ?1 AND ({} IS NULL OR {} > ?2) LIMIT 1",
            place,
            col,
            ddl::EXPIRES,
            ddl::EXPIRES
        );
        Ok(!self
            .wire
            .rows(&text, &[Val::Int(key), Val::Int(tick)])
            .await?
            .is_empty())
    }

    pub(super) async fn live_out(&mut self, unit: &Unit, key: i64) -> Result<bool, Error> {
        let tick = now();
        for edge in unit.bonds() {
            if edge.kind() != bond::Kind::Many2many {
                continue;
            }
            let left = ddl::col(&ddl::side(unit.name()));
            let text = format!(
                "SELECT 1 FROM {} WHERE {} = ?1 AND ({} IS NULL OR {} > ?2) LIMIT 1",
                ddl::joint(unit.name(), edge.name()),
                left,
                ddl::EXPIRES,
                ddl::EXPIRES
            );
            if !self
                .wire
                .rows(&text, &[Val::Int(key), Val::Int(tick)])
                .await?
                .is_empty()
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub async fn set(
        &mut self,
        name: &str,
        key: i64,
        fields: &[(&str, &str)],
    ) -> Result<(), Error> {
        let unit = self.plan.find(name)?;
        if unit.name() == crate::cap::PULSE {
            return Err(Error::Adapt("pulse is engine owned".into()));
        }
        if unit.name() == crate::cap::GRANT {
            return Err(Error::Adapt("grant rows are put or end".into()));
        }
        unit.part(fields)?;
        let base = self.peek(unit, key).await?;
        self.solid(unit, fields, Some((key, &base))).await?;
        let tick = now();
        let mut text = format!("UPDATE {} SET ", ddl::seat(unit.name()));
        let mut vals: Vec<Val> = Vec::new();
        for (i, (col, val)) in fields.iter().enumerate() {
            if i > 0 {
                text.push_str(", ");
            }
            let (name, cell) = self.entry(unit, col, val, key).await?;
            text.push_str(&ddl::col(&name));
            text.push_str(" = ?");
            text.push_str(&(i + 1).to_string());
            vals.push(cell);
        }
        let n = fields.len();
        text.push_str(&format!(
            ", {} = ?{} WHERE {} = ?{} AND ({} IS NULL OR {} > ?{})",
            ddl::UPDATED,
            n + 1,
            ddl::KEY,
            n + 2,
            ddl::EXPIRES,
            ddl::EXPIRES,
            n + 3
        ));
        vals.push(Val::Int(tick));
        vals.push(Val::Int(key));
        vals.push(Val::Int(tick));
        let changed = self.wire.run(&text, &vals).await?;
        if changed == 0 {
            return Err(Error::Adapt(format!("missing row {key}")));
        }
        Ok(())
    }
}

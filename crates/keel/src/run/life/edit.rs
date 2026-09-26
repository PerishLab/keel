use super::*;
use crate::adapt::Error;
use crate::ddl;
use crate::plan::{Edge, Unit};
use crate::wire::{Val, Wire};

impl<'a, W: Wire> Work<'a, W> {
    pub(crate) async fn end(&mut self, name: &str, key: i64) -> Result<(), Error> {
        self.lease(name, key, now()).await
    }

    pub(crate) async fn lease(&mut self, name: &str, key: i64, at: i64) -> Result<(), Error> {
        let held = self.brood(name, key, at).await?;
        for (unit, key) in held.iter().rev() {
            self.alone(unit, *key, at).await?;
        }
        self.alone(name, key, at).await
    }

    async fn alone(&mut self, name: &str, key: i64, at: i64) -> Result<(), Error> {
        let unit = self.plan.find(name)?;
        if unit.name() == crate::cap::PULSE {
            return Err(Error::Adapt("pulse is engine owned".into()));
        }
        let tick = now();
        if at < tick {
            return Err(Error::Adapt("lease is not the past".into()));
        }
        if self.inbound(unit.name(), key, at).await? {
            return Err(Error::Adapt("live ties remain".into()));
        }
        let text = format!(
            "UPDATE {} SET {} = ?1, {} = ?2 WHERE {} = ?3 AND ({} IS NULL OR {} > ?2)",
            ddl::seat(unit),
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

    async fn brood(&mut self, name: &str, key: i64, at: i64) -> Result<Vec<(String, i64)>, Error> {
        let mut out = Vec::new();
        let mut open = vec![(name.to_string(), key)];
        for _ in 0..crate::cap::DEPTH {
            let mut next = Vec::new();
            for (name, key) in &open {
                next.extend(self.kids(name, *key, at).await?);
            }
            if next.is_empty() {
                break;
            }
            out.extend(next.clone());
            open = next;
        }
        Ok(out)
    }

    async fn kids(&mut self, name: &str, key: i64, at: i64) -> Result<Vec<(String, i64)>, Error> {
        let mut out = Vec::new();
        let held = self.plan.find(name)?.name().to_string();
        let held: Vec<(String, String)> = self
            .plan
            .units()
            .values()
            .filter_map(|unit| unit.root().map(|edge| (unit, edge)))
            .filter(|(_, edge)| {
                self.plan
                    .find(edge.target())
                    .is_ok_and(|mate| mate.name() == held)
            })
            .map(|(unit, edge)| (unit.key(), edge.name().to_string()))
            .collect();
        for (unit, edge) in held {
            let seat = self.plan.find(&unit)?;
            let text = format!(
                "SELECT {} FROM {} WHERE {} = ?1 AND ({} IS NULL OR {} > ?2)",
                ddl::KEY,
                ddl::seat(seat),
                ddl::col(&ddl::side(&edge)),
                ddl::EXPIRES,
                ddl::EXPIRES
            );
            for row in self
                .wire
                .rows(&text, &[Val::Int(key), Val::Int(at)])
                .await?
            {
                out.push((unit.clone(), row[0].int()));
            }
        }
        Ok(out)
    }

    pub(crate) async fn pulse(
        &mut self,
        verb: &str,
        unit: &str,
        key: i64,
        who: &str,
    ) -> Result<(), Error> {
        let seat = self.plan.find(crate::cap::PULSE)?;
        let tick = now();
        let pulse = crate::estate::next(self.wire, "pulse").await?;
        let text = format!(
            "INSERT INTO {} ({}, verb, unit, who, {}, {}, {}, {}) VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, ?6)",
            ddl::seat(seat),
            ddl::KEY,
            ddl::col("key"),
            ddl::EXPIRES,
            ddl::CREATED,
            ddl::UPDATED
        );
        let args = [
            Val::Int(pulse),
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
            ddl::seat(seat),
            ddl::KEY,
            ddl::KEY,
            ddl::seat(seat)
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
            ddl::seat(unit),
            ddl::KEY,
            ddl::EXPIRES
        );
        Ok(!self.wire.rows(&text, &[Val::Int(key)]).await?.is_empty())
    }

    pub(super) async fn inbound(&mut self, target: &str, key: i64, at: i64) -> Result<bool, Error> {
        for unit in self.plan.units().values() {
            for edge in unit.bonds() {
                if edge.target() != target {
                    continue;
                }
                if self.feeds(unit, edge, key, at).await? {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    pub(super) async fn feeds(
        &mut self,
        unit: &Unit,
        edge: &Edge,
        key: i64,
        at: i64,
    ) -> Result<bool, Error> {
        let (place, col) = if edge.kind().point() {
            (ddl::seat(unit), ddl::col(&ddl::side(edge.name())))
        } else {
            (
                ddl::joint(unit, edge.name()),
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
            .rows(&text, &[Val::Int(key), Val::Int(at)])
            .await?
            .is_empty())
    }

    pub(crate) async fn set(
        &mut self,
        name: &str,
        key: i64,
        fields: &[(&str, &str)],
    ) -> Result<(), Error> {
        let unit = self.plan.find(name)?;
        if unit.frozen() {
            return Err(Error::Adapt(format!("frozen unit {}", unit.name())));
        }
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
        let mut text = format!("UPDATE {} SET ", ddl::seat(unit));
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

    pub(crate) async fn unset(
        &mut self,
        name: &str,
        key: i64,
        fields: &[&str],
    ) -> Result<(), Error> {
        let unit = self.plan.find(name)?;
        if unit.frozen() {
            return Err(Error::Adapt(format!("frozen unit {}", unit.name())));
        }
        unit.loose(fields)?;
        self.peek(unit, key).await?;
        let tick = now();
        let assignments = fields
            .iter()
            .map(|name| {
                let column = unit
                    .refs()
                    .find(|edge| edge.name() == *name)
                    .map(|_| ddl::side(name))
                    .unwrap_or_else(|| (*name).to_string());
                format!("{} = NULL", ddl::col(&column))
            })
            .collect::<Vec<_>>()
            .join(", ");
        let text = format!(
            "UPDATE {} SET {assignments}, {} = ?1 WHERE {} = ?2 AND ({} IS NULL OR {} > ?1)",
            ddl::seat(unit),
            ddl::UPDATED,
            ddl::KEY,
            ddl::EXPIRES,
            ddl::EXPIRES
        );
        let changed = self
            .wire
            .run(&text, &[Val::Int(tick), Val::Int(key)])
            .await?;
        if changed == 0 {
            return Err(Error::Adapt(format!("missing row {key}")));
        }
        Ok(())
    }
}

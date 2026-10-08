use super::*;
use crate::adapt::Error;
use crate::ddl;
use crate::wire::{Val, Wire};

impl<'a, W: Wire> Work<'a, W> {
    pub(crate) async fn end(&mut self, name: &str, key: i64) -> Result<(), Error> {
        self.retire(name, key, now()).await
    }

    pub(crate) async fn lease(&mut self, name: &str, key: i64, at: i64) -> Result<(), Error> {
        if at < now() {
            return Err(Error::Adapt("lease is not the past".into()));
        }
        self.retire(name, key, at).await
    }

    async fn retire(&mut self, name: &str, key: i64, at: i64) -> Result<(), Error> {
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
}

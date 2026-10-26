use super::*;
use crate::adapt::Error;
use crate::ddl::{self, Grain, Lock};
use crate::plan::{Edge, Plan, Unit};
use crate::wire::{Val, Wire};

const FALL: &str = "\"@fall\"";

impl<'a, W: Wire> Work<'a, W> {
    pub(crate) async fn trace(&mut self, unit: &Unit, key: i64) -> Result<Option<Row>, Error> {
        let text = format!(
            "SELECT {} FROM {} WHERE {} = ?1",
            unit.sheet(),
            ddl::seat(unit),
            ddl::KEY
        );
        let rows = self.wire.rows(&text, &[Val::Int(key)]).await?;
        rows.first().map(|line| Row::read(unit, line)).transpose()
    }

    pub(crate) async fn end(&mut self, name: &str, key: i64) -> Result<Vec<String>, Error> {
        self.retire(name, key, now()).await
    }

    pub(crate) async fn lease(
        &mut self,
        name: &str,
        key: i64,
        at: i64,
    ) -> Result<Vec<String>, Error> {
        if at < now() {
            return Err(Error::Adapt("lease is not the past".into()));
        }
        self.retire(name, key, at).await
    }

    async fn retire(&mut self, name: &str, key: i64, at: i64) -> Result<Vec<String>, Error> {
        let root = self.plan.find(name)?;
        if root.name() == crate::cap::PULSE {
            return Err(Error::Adapt("pulse is engine owned".into()));
        }
        if !self.claim(root, key).await? {
            return Err(Error::Adapt(format!("missing row {key}")));
        }
        self.sow(root, key).await?;
        let fallen = self.fell(root, at).await?;
        let tick = now();
        for (unit, count) in &fallen {
            self.mark(unit, *count, (at, tick)).await?;
        }
        for (unit, _) in &fallen {
            if self.bound(unit, at).await? {
                return Err(Error::Adapt("live ties remain".into()));
            }
        }
        Ok(fallen.into_iter().map(|(unit, _)| unit).collect())
    }

    async fn sow(&mut self, root: &Unit, key: i64) -> Result<(), Error> {
        let int = match self.wire.grain() {
            Grain::Lite => "INTEGER",
            Grain::Pg => "BIGINT",
        };
        self.wire
            .script(&format!(
                "CREATE TEMP TABLE IF NOT EXISTS {FALL} (unit TEXT NOT NULL, layer {int} NOT NULL, key {int} NOT NULL)"
            ))
            .await?;
        self.wire.run(&format!("DELETE FROM {FALL}"), &[]).await?;
        self.wire
            .run(
                &format!("INSERT INTO {FALL} (unit, layer, key) VALUES (?1, 0, ?2)"),
                &[Val::Text(root.key()), Val::Int(key)],
            )
            .await?;
        Ok(())
    }

    async fn fell(&mut self, root: &Unit, at: i64) -> Result<Vec<(String, i64)>, Error> {
        let mut open = vec![root.key()];
        for layer in 0..crate::cap::DEPTH as i64 {
            open = self.grow(&open, layer, at).await?;
            if open.is_empty() {
                break;
            }
            for unit in &open {
                self.pin(unit, layer + 1).await?;
            }
        }
        let text = format!("SELECT unit, COUNT(DISTINCT key) FROM {FALL} GROUP BY unit");
        let rows = self.wire.rows(&text, &[]).await?;
        Ok(rows
            .into_iter()
            .map(|row| (row[0].text(), row[1].int()))
            .collect())
    }

    async fn grow(&mut self, open: &[String], layer: i64, at: i64) -> Result<Vec<String>, Error> {
        let plan = self.plan;
        let mut next: Vec<String> = Vec::new();
        for parent in open {
            for (unit, edge) in kids(plan, parent)? {
                let text = format!(
                    "INSERT INTO {FALL} (unit, layer, key) SELECT ?1, ?2, c.{} FROM {} AS c WHERE c.{} IN (SELECT key FROM {FALL} WHERE unit = ?3 AND layer = ?4) AND (c.{} IS NULL OR c.{} > ?5)",
                    ddl::KEY,
                    ddl::seat(unit),
                    ddl::col(&ddl::side(edge.name())),
                    ddl::EXPIRES,
                    ddl::EXPIRES
                );
                let args = [
                    Val::Text(unit.key()),
                    Val::Int(layer + 1),
                    Val::Text(parent.clone()),
                    Val::Int(layer),
                    Val::Int(at),
                ];
                if self.wire.run(&text, &args).await? > 0 && !next.contains(&unit.key()) {
                    next.push(unit.key());
                }
            }
        }
        Ok(next)
    }

    async fn pin(&mut self, unit: &str, layer: i64) -> Result<(), Error> {
        let lock = ddl::grip(self.wire.grain(), Some(Lock::Update));
        if lock.is_empty() {
            return Ok(());
        }
        let seat = self.plan.find(unit)?;
        let text = format!(
            "SELECT {} FROM {} WHERE {} IN (SELECT key FROM {FALL} WHERE unit = ?1 AND layer = ?2){lock}",
            ddl::KEY,
            ddl::seat(seat),
            ddl::KEY
        );
        self.wire
            .rows(&text, &[Val::Text(unit.into()), Val::Int(layer)])
            .await?;
        Ok(())
    }

    async fn mark(&mut self, unit: &str, count: i64, time: (i64, i64)) -> Result<(), Error> {
        let seat = self.plan.find(unit)?;
        if seat.name() == crate::cap::PULSE {
            return Err(Error::Adapt("pulse is engine owned".into()));
        }
        let text = format!(
            "UPDATE {} SET {} = ?1, {} = ?2 WHERE {} IN (SELECT key FROM {FALL} WHERE unit = ?3) AND ({} IS NULL OR {} > ?2)",
            ddl::seat(seat),
            ddl::EXPIRES,
            ddl::UPDATED,
            ddl::KEY,
            ddl::EXPIRES,
            ddl::EXPIRES
        );
        let args = [Val::Int(time.0), Val::Int(time.1), Val::Text(unit.into())];
        if self.wire.run(&text, &args).await? as i64 != count {
            return Err(Error::Adapt(format!("missing row in {}", seat.name())));
        }
        Ok(())
    }

    async fn bound(&mut self, unit: &str, at: i64) -> Result<bool, Error> {
        let plan = self.plan;
        let target = plan.find(unit)?.name().to_string();
        for holder in plan.units().values() {
            for edge in holder.bonds().iter().filter(|edge| edge.target() == target) {
                let text = held(holder, edge);
                let args = [Val::Text(unit.into()), Val::Int(at)];
                if !self.wire.rows(&text, &args).await?.is_empty() {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
}

fn kids<'p>(plan: &'p Plan, parent: &str) -> Result<Vec<(&'p Unit, &'p Edge)>, Error> {
    let held = plan.find(parent)?.name().to_string();
    Ok(plan
        .units()
        .values()
        .filter_map(|unit| unit.root().map(|edge| (unit, edge)))
        .filter(|(_, edge)| {
            plan.find(edge.target())
                .is_ok_and(|mate| mate.name() == held)
        })
        .collect())
}

fn held(holder: &Unit, edge: &Edge) -> String {
    let (place, col, owner) = if edge.kind().point() {
        (
            ddl::seat(holder),
            ddl::col(&ddl::side(edge.name())),
            String::new(),
        )
    } else {
        (
            ddl::joint(holder, edge.name()),
            ddl::col(&ddl::mate(holder.name(), edge.name(), edge.target())),
            format!(
                " AND EXISTS (SELECT 1 FROM {} AS h WHERE h.{} = t.{} AND (h.{} IS NULL OR h.{} > ?2))",
                ddl::seat(holder),
                ddl::KEY,
                ddl::col(&ddl::side(holder.name())),
                ddl::EXPIRES,
                ddl::EXPIRES
            ),
        )
    };
    format!(
        "SELECT 1 FROM {place} AS t WHERE t.{col} IN (SELECT key FROM {FALL} WHERE unit = ?1) AND (t.{} IS NULL OR t.{} > ?2){owner} LIMIT 1",
        ddl::EXPIRES,
        ddl::EXPIRES
    )
}

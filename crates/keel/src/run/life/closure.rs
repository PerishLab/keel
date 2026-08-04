use super::{Ends, Work, now};
use crate::adapt::Error;
use crate::ddl;
use crate::plan::{Edge, Unit};
use crate::wire::{Val, Wire};
use std::collections::{BTreeMap, BTreeSet};

type Pair = (i64, i64);
type Held = BTreeMap<Pair, Vec<i64>>;

pub(super) fn writable(edge: &Edge) -> Result<(), Error> {
    if edge.derived() {
        return Err(Error::Adapt(format!(
            "bond {} is engine owned",
            edge.name()
        )));
    }
    Ok(())
}

impl<W: Wire> Work<'_, W> {
    pub(crate) async fn refreshes(&mut self) -> Result<(), Error> {
        let mut paths = Vec::new();
        for unit in self.plan.units().values() {
            for edge in unit.bonds().iter().filter(|edge| edge.closure()) {
                paths.push((unit.key(), edge.name().to_string()));
            }
        }
        for (owner, bond) in paths {
            let (unit, edge) = self.plan.edge(&owner, &bond)?;
            self.refresh(unit, edge).await?;
        }
        Ok(())
    }

    pub(super) async fn cycle(
        &mut self,
        unit: &Unit,
        edge: &Edge,
        ends: Ends,
    ) -> Result<bool, Error> {
        if ends.left == ends.right {
            return Ok(true);
        }
        let mut seen = BTreeSet::new();
        let mut open = vec![ends.right];
        while let Some(left) = open.pop() {
            if !seen.insert(left) {
                continue;
            }
            for tie in self.ties(unit, edge, left).await? {
                if tie.right() == ends.left {
                    return Ok(true);
                }
                open.push(tie.right());
            }
        }
        Ok(false)
    }

    pub(super) async fn refresh(&mut self, unit: &Unit, edge: &Edge) -> Result<(), Error> {
        if !edge.closure() {
            return Ok(());
        }
        let tick = now();
        let wanted = reach(&self.pairs(unit, edge, tick).await?);
        let derived = derive(unit, edge)?;
        let held = self.held(unit, derived, tick).await?;
        for (pair, keys) in &held {
            let keep = wanted.contains(pair);
            for key in keys.iter().skip(usize::from(keep)) {
                self.expire(unit, derived, *key, tick).await?;
            }
        }
        for pair in wanted {
            if held.get(&pair).is_none_or(Vec::is_empty) {
                self.insert(unit, derived, pair, tick).await?;
            }
        }
        Ok(())
    }

    async fn pairs(&mut self, unit: &Unit, edge: &Edge, tick: i64) -> Result<Vec<Pair>, Error> {
        let left = ddl::col(&ddl::side(unit.name()));
        let right = ddl::col(&ddl::mate(unit.name(), edge.name(), edge.target()));
        let text = format!(
            "SELECT {left}, {right} FROM {} WHERE {} IS NULL OR {} > ?1",
            ddl::joint(unit, edge.name()),
            ddl::EXPIRES,
            ddl::EXPIRES
        );
        let rows = self.wire.rows(&text, &[Val::Int(tick)]).await?;
        Ok(rows
            .into_iter()
            .map(|row| (row[0].int(), row[1].int()))
            .collect())
    }

    async fn held(&mut self, unit: &Unit, edge: &Edge, tick: i64) -> Result<Held, Error> {
        let left = ddl::col(&ddl::side(unit.name()));
        let right = ddl::col(&ddl::mate(unit.name(), edge.name(), edge.target()));
        let text = format!(
            "SELECT {}, {left}, {right} FROM {} WHERE {} IS NULL OR {} > ?1 ORDER BY {}",
            ddl::KEY,
            ddl::joint(unit, edge.name()),
            ddl::EXPIRES,
            ddl::EXPIRES,
            ddl::KEY
        );
        let mut held = Held::new();
        for row in self.wire.rows(&text, &[Val::Int(tick)]).await? {
            held.entry((row[1].int(), row[2].int()))
                .or_default()
                .push(row[0].int());
        }
        Ok(held)
    }

    async fn expire(&mut self, unit: &Unit, edge: &Edge, key: i64, tick: i64) -> Result<(), Error> {
        let text = format!(
            "UPDATE {} SET {} = ?1, {} = ?1 WHERE {} = ?2",
            ddl::joint(unit, edge.name()),
            ddl::EXPIRES,
            ddl::UPDATED,
            ddl::KEY
        );
        self.wire
            .run(&text, &[Val::Int(tick), Val::Int(key)])
            .await?;
        Ok(())
    }

    async fn insert(
        &mut self,
        unit: &Unit,
        edge: &Edge,
        pair: Pair,
        tick: i64,
    ) -> Result<(), Error> {
        let left = ddl::col(&ddl::side(unit.name()));
        let right = ddl::col(&ddl::mate(unit.name(), edge.name(), edge.target()));
        let text = format!(
            "INSERT INTO {} ({}, {left}, {right}, {}, {}, {}) VALUES (?1, ?2, ?3, NULL, ?4, ?4)",
            ddl::joint(unit, edge.name()),
            ddl::KEY,
            ddl::EXPIRES,
            ddl::CREATED,
            ddl::UPDATED
        );
        let clock = crate::estate::clock::bond(&unit.key(), edge.name());
        let key = crate::estate::next(self.wire, &clock).await?;
        let args = [
            Val::Int(key),
            Val::Int(pair.0),
            Val::Int(pair.1),
            Val::Int(tick),
        ];
        self.wire.run(&text, &args).await?;
        Ok(())
    }
}

fn derive<'a>(unit: &'a Unit, edge: &Edge) -> Result<&'a Edge, Error> {
    let name = format!("{}_closure", edge.name());
    unit.bonds()
        .iter()
        .find(|held| held.derived() && held.name().eq_ignore_ascii_case(&name))
        .ok_or_else(|| Error::Adapt(format!("missing bond {name}")))
}

fn reach(pairs: &[Pair]) -> BTreeSet<Pair> {
    let mut graph: BTreeMap<i64, Vec<i64>> = BTreeMap::new();
    for &(left, right) in pairs {
        graph.entry(left).or_default().push(right);
    }
    let mut out = BTreeSet::new();
    for &root in graph.keys() {
        walk(root, &graph, &mut out);
    }
    out
}

fn walk(root: i64, graph: &BTreeMap<i64, Vec<i64>>, out: &mut BTreeSet<Pair>) {
    let mut seen = BTreeSet::new();
    let mut open = graph.get(&root).cloned().unwrap_or_default();
    while let Some(right) = open.pop() {
        if !seen.insert(right) {
            continue;
        }
        if right != root {
            out.insert((root, right));
        }
        if let Some(next) = graph.get(&right) {
            open.extend(next);
        }
    }
}

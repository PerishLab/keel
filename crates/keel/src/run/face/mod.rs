use crate::adapt::Error;
use crate::ddl;
use crate::life::{Row, Tie};
use crate::plan::Plan;
use crate::query::Pack;
use crate::wire::Wire;
use std::collections::HashMap;
use std::sync::Mutex;

pub const HOLD: usize = 1024;

struct Deal {
    gens: Vec<(String, i64)>,
    until: Option<i64>,
    pack: Pack,
}

#[derive(Default)]
struct Stash {
    on: bool,
    gens: Mutex<HashMap<String, i64>>,
    deals: Mutex<HashMap<String, Deal>>,
}

impl Stash {
    fn bump(&self, unit: &str) {
        if !self.on {
            return;
        }
        let owner = unit.split('.').next().unwrap_or(unit).to_string();
        if let Ok(mut gens) = self.gens.lock() {
            *gens.entry(owner).or_insert(0) += 1;
        }
    }

    fn stamp(&self, units: &[String]) -> Vec<(String, i64)> {
        let gens = self.gens.lock().ok();
        units
            .iter()
            .map(|unit| {
                let step = gens
                    .as_ref()
                    .and_then(|g| g.get(unit).copied())
                    .unwrap_or(0);
                (unit.clone(), step)
            })
            .collect()
    }

    fn read(&self, key: &str, units: &[String]) -> Option<Pack> {
        if !self.on {
            return None;
        }
        let deals = self.deals.lock().ok()?;
        let deal = deals.get(key)?;
        if deal.gens != self.stamp(units) {
            return None;
        }
        if let Some(until) = deal.until
            && crate::life::tick() >= until
        {
            return None;
        }
        Some(deal.pack.clone())
    }

    fn keep(&self, key: String, units: &[String], pack: &Pack) {
        if !self.on {
            return;
        }
        let Ok(mut deals) = self.deals.lock() else {
            return;
        };
        if deals.len() >= HOLD {
            deals.clear();
        }
        deals.insert(
            key,
            Deal {
                gens: self.stamp(units),
                until: horizon(pack),
                pack: pack.clone(),
            },
        );
    }

    fn spoil(&self) {
        if let Ok(mut deals) = self.deals.lock() {
            deals.clear();
        }
    }
}

fn horizon(pack: &Pack) -> Option<i64> {
    let mut edge: Option<i64> = None;
    for bag in pack.bags().values() {
        let ats: Vec<i64> = match bag {
            crate::query::Bag::Unit(rows) => rows.iter().filter_map(Row::expires).collect(),
            crate::query::Bag::Bond(ties) => ties.iter().filter_map(Tie::expires).collect(),
        };
        for at in ats {
            edge = Some(edge.map_or(at, |held| held.min(at)));
        }
    }
    edge
}

pub(crate) struct Seat<W: Wire> {
    wire: W,
    dirty: bool,
}

impl<W: Wire> Seat<W> {
    async fn open(&mut self) -> Result<(), Error> {
        self.wire.script("BEGIN").await?;
        self.dirty = true;
        Ok(())
    }

    async fn close(&mut self, keep: bool) -> Result<(), Error> {
        let word = if keep { "COMMIT" } else { "ROLLBACK" };
        self.wire.script(word).await?;
        self.dirty = false;
        Ok(())
    }
}

pub struct Core<W: Wire> {
    plan: Plan,
    seat: tokio::sync::Mutex<Seat<W>>,
    identity: Option<String>,
    stash: Stash,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Who {
    Sudo,
    Op(i64),
    Anon,
}

pub struct Face<'a, W: Wire> {
    core: &'a Core<W>,
    who: Who,
}

pub struct Tx<'a, W: Wire> {
    core: &'a Core<W>,
    seat: &'a mut Seat<W>,
    who: Who,
}

fn label(who: Who) -> String {
    match who {
        Who::Sudo => "sudo".into(),
        Who::Anon => "anon".into(),
        Who::Op(id) => id.to_string(),
    }
}

fn lane(unit: &str, bond: &str) -> String {
    format!("{}.{}", ddl::table(unit), bond)
}

mod core;
mod deed;
mod grant;
mod link;
mod tx;
mod verb;

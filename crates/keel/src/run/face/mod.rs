use crate::adapt::Error;
use crate::ddl;
use crate::life::{Row, Tie};
use crate::plan::Plan;
use crate::query::{Pack, Scope, Tree};
use crate::wire::Wire;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

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
        let owner = unit.split('.').next().unwrap_or(unit).to_string();
        if let Ok(mut gens) = self.gens.lock() {
            *gens.entry(owner).or_insert(0) += 1;
        }
    }

    fn step(&self, unit: &str) -> i64 {
        self.gens
            .lock()
            .ok()
            .and_then(|gens| gens.get(unit).copied())
            .unwrap_or(0)
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

#[derive(Default)]
struct Deeds {
    on: bool,
    held: Mutex<Option<(i64, Arc<Vec<Row>>)>>,
}

impl Deeds {
    fn read(&self, step: i64) -> Option<Arc<Vec<Row>>> {
        if !self.on {
            return None;
        }
        let held = self.held.lock().ok()?;
        let (at, rows) = held.as_ref()?;
        (*at == step).then(|| rows.clone())
    }

    fn keep(&self, step: i64, rows: Vec<Row>) -> Arc<Vec<Row>> {
        let rows = Arc::new(rows);
        if !self.on {
            return rows;
        }
        if let Ok(mut held) = self.held.lock() {
            *held = Some((step, rows.clone()));
        }
        rows
    }
}

#[derive(Default)]
struct Chart {
    scopes: Mutex<HashMap<String, Arc<Scope>>>,
}

impl Chart {
    fn scope(&self, plan: &Plan, tree: &Tree) -> Result<Arc<Scope>, Error> {
        let key = crate::query::shape(tree);
        if let Ok(scopes) = self.scopes.lock()
            && let Some(scope) = scopes.get(&key)
        {
            return Ok(scope.clone());
        }
        let scope = Arc::new(crate::query::analyze(plan, tree)?);
        let Ok(mut scopes) = self.scopes.lock() else {
            return Ok(scope);
        };
        if scopes.len() >= HOLD {
            scopes.clear();
        }
        scopes.insert(key, scope.clone());
        Ok(scope)
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
    chart: Chart,
    deeds: Deeds,
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

use super::Gone;
use crate::adapt::Error;
use crate::model::manifest::{Bond, Edge, Manifest, Unit};
use crate::wire::Wire;
use std::collections::BTreeSet;

struct World<'a, W> {
    active: &'a Manifest,
    wire: &'a mut W,
}

struct Arc<'a> {
    generation: i64,
    unit: &'a Unit,
    edge: &'a Edge,
    active: Option<&'a Unit>,
}

pub(super) async fn gone<W: Wire>(
    generation: i64,
    prior: &Manifest,
    active: &Manifest,
    wire: &mut W,
) -> Result<Vec<Gone>, Error> {
    let mut world = World { active, wire };
    let mut out = BTreeSet::new();
    for unit in prior.units() {
        world.unit(generation, unit, &mut out).await?;
    }
    Ok(out.into_iter().collect())
}

impl<W: Wire> World<'_, W> {
    async fn unit(
        &mut self,
        generation: i64,
        prior: &Unit,
        out: &mut BTreeSet<Gone>,
    ) -> Result<(), Error> {
        let current = self
            .active
            .units()
            .iter()
            .find(|unit| unit.key == prior.key)
            .cloned();
        let table = crate::ddl::stage(generation, &prior.table());
        let before = self.keys(&table).await?;
        let after = match &current {
            Some(unit) => self.keys(&unit.table()).await?,
            None => BTreeSet::new(),
        };
        for key in before.difference(&after) {
            out.insert(Gone::new(prior.key.clone(), *key));
        }
        let held: BTreeSet<i64> = before.intersection(&after).copied().collect();
        if let Some(unit) = &current {
            self.fields(prior, unit, &held, out);
            self.points(prior, unit, &held, out);
        }
        self.bonds(generation, prior, current.as_ref(), out).await
    }

    fn fields(&self, prior: &Unit, active: &Unit, held: &BTreeSet<i64>, out: &mut BTreeSet<Gone>) {
        for field in &prior.fields {
            if active.fields.iter().any(|slot| slot.name == field.name) {
                continue;
            }
            let path = format!("{}.{}", prior.key, field.name);
            mark(&path, held, out);
        }
    }

    fn points(&self, prior: &Unit, active: &Unit, held: &BTreeSet<i64>, out: &mut BTreeSet<Gone>) {
        for edge in prior
            .bonds
            .iter()
            .filter(|edge| edge.kind != Bond::Many2many)
        {
            if active.bonds.iter().any(|bond| bond.name == edge.name) {
                continue;
            }
            let path = format!("{}.{}", prior.key, edge.name);
            mark(&path, held, out);
        }
    }

    async fn bonds(
        &mut self,
        generation: i64,
        prior: &Unit,
        active: Option<&Unit>,
        out: &mut BTreeSet<Gone>,
    ) -> Result<(), Error> {
        for edge in prior
            .bonds
            .iter()
            .filter(|edge| edge.kind == Bond::Many2many)
        {
            self.bond(
                Arc {
                    generation,
                    unit: prior,
                    edge,
                    active,
                },
                out,
            )
            .await?;
        }
        Ok(())
    }

    async fn bond(&mut self, arc: Arc<'_>, out: &mut BTreeSet<Gone>) -> Result<(), Error> {
        let table = crate::ddl::joiner(&arc.unit.table(), &arc.edge.name);
        let before = self
            .keys(&crate::ddl::stage(arc.generation, &table))
            .await?;
        let current = arc.active.and_then(|unit| {
            unit.bonds
                .iter()
                .find(|bond| bond.name == arc.edge.name && bond.kind == Bond::Many2many)
        });
        let after = match (arc.active, current) {
            (Some(unit), Some(bond)) => {
                let table = crate::ddl::joiner(&unit.table(), &bond.name);
                self.keys(&table).await?
            }
            _ => BTreeSet::new(),
        };
        let path = format!("{}.{}", arc.unit.key, arc.edge.name);
        for key in before.difference(&after) {
            out.insert(Gone::new(path.clone(), *key));
        }
        let Some(current) = current else {
            return Ok(());
        };
        let held: BTreeSet<i64> = before.intersection(&after).copied().collect();
        for field in &arc.edge.fields {
            if current.fields.iter().any(|slot| slot.name == field.name) {
                continue;
            }
            mark(&format!("{path}.{}", field.name), &held, out);
        }
        Ok(())
    }

    async fn keys(&mut self, table: &str) -> Result<BTreeSet<i64>, Error> {
        let rows = self
            .wire
            .rows(
                &format!("SELECT {} FROM {}", crate::ddl::KEY, crate::ddl::col(table)),
                &[],
            )
            .await?;
        let mut out = BTreeSet::new();
        for row in rows {
            if row.len() != 1 {
                return Err(Error::Estate(super::super::Fault::Unknown(
                    "derivative key shape".into(),
                )));
            }
            out.insert(row[0].int());
        }
        Ok(out)
    }
}

fn mark(path: &str, keys: &BTreeSet<i64>, out: &mut BTreeSet<Gone>) {
    for key in keys {
        out.insert(Gone::new(path.into(), *key));
    }
}

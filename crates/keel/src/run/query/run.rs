use super::*;
use crate::adapt::Error;
use crate::ddl;
use crate::life::{Cell, Row, Tie, Work};
use crate::plan::{Edge, Plan, Unit};
use crate::wire::Wire;
use std::collections::BTreeMap;

pub async fn run<W: Wire>(
    plan: &Plan,
    wire: &mut W,
    tree: &Tree,
    scope: &Scope,
) -> Result<Pack, Error> {
    scope.verify(tree)?;
    let mut work = Work::new(wire, plan);
    let mut rows = match tree.slice() {
        Slice::Live => work.scan(scope.unit()).await?,
    };
    if !tree.preds().is_empty() {
        rows.retain(|row| pass(row, tree.preds()));
    }
    let mut sweep = Sweep {
        work: &mut work,
        scope,
    };
    sweep.hold(&mut rows, tree.preds()).await?;
    if tree.tally() {
        return Ok(Pack {
            root: scope.name().to_string(),
            bags: BTreeMap::new(),
            count: Some(rows.len()),
        });
    }
    order(&mut rows, tree.sort());
    page(&mut rows, tree.after(), tree.limit());
    let keys: Vec<i64> = rows.iter().map(|row| row.key()).collect();
    let root = scope.name().to_string();
    let mut bags = BTreeMap::new();
    bags.insert(root.clone(), Bag::Unit(rows));
    for name in tree.links() {
        let bond = edge(scope.unit(), name)?;
        let ties = sweep.pull(bond, &keys).await?;
        bags.insert(format!("{root}.{}", bond.name()), Bag::Bond(ties));
    }
    Ok(Pack {
        root,
        bags,
        count: None,
    })
}

pub(crate) struct Sweep<'a, 'w, W: Wire> {
    work: &'a mut Work<'w, W>,
    scope: &'a Scope,
}

impl<W: Wire> Sweep<'_, '_, W> {
    pub(crate) async fn pull(&mut self, bond: &Edge, keys: &[i64]) -> Result<Vec<Tie>, Error> {
        let unit = self.scope.unit();
        let target = self.scope.at(bond.target())?;
        let mut ties = Vec::new();
        for &key in keys {
            for tie in self.work.ties(unit, bond, key).await? {
                if !self.work.alive(target, tie.right()).await? {
                    continue;
                }
                ties.push(tie);
                if ties.len() > CAP {
                    return Err(Error::Adapt("tie cap exceeded".into()));
                }
            }
        }
        ties.sort_by_key(|tie| tie.key());
        Ok(ties)
    }

    pub(crate) async fn hold(&mut self, rows: &mut Vec<Row>, preds: &[Pred]) -> Result<(), Error> {
        for pred in preds {
            match pred.op() {
                Op::Has => self.has(rows, pred).await?,
                Op::Some => self.some(rows, pred).await?,
                _ => {}
            }
        }
        Ok(())
    }

    async fn has(&mut self, rows: &mut Vec<Row>, pred: &Pred) -> Result<(), Error> {
        let unit = self.scope.unit();
        let bond = edge(unit, pred.field())?;
        let right = key(pred.value())?;
        let mut keep = Vec::new();
        for row in rows.iter() {
            let ties = self.work.ties(unit, bond, row.key()).await;
            if ties.is_ok_and(|ties| ties.iter().any(|tie| tie.right() == right)) {
                keep.push(row.key());
            }
        }
        rows.retain(|row| keep.contains(&row.key()));
        Ok(())
    }

    async fn some(&mut self, rows: &mut Vec<Row>, pred: &Pred) -> Result<(), Error> {
        let bond = edge(self.scope.unit(), pred.field())?;
        let nest = pred
            .nest()
            .ok_or_else(|| Error::Adapt("some needs inner".into()))?;
        let mut keep = Vec::new();
        for row in rows.iter() {
            if self.probe(bond, row.key(), nest).await.unwrap_or(false) {
                keep.push(row.key());
            }
        }
        rows.retain(|row| keep.contains(&row.key()));
        Ok(())
    }

    async fn probe(&mut self, bond: &Edge, left: i64, nest: &Pred) -> Result<bool, Error> {
        let unit = self.scope.unit();
        let target = self.scope.at(bond.target())?;
        let ties = self.work.ties(unit, bond, left).await?;
        let owned = bond.fields().iter().any(|s| s.name() == nest.field());
        for tie in ties {
            if nest.field() == ddl::KEY {
                if nest.hits(&Cell::Int(tie.right())) {
                    return Ok(true);
                }
                continue;
            }
            if owned {
                if nest.finds(tie.cells()) {
                    return Ok(true);
                }
                continue;
            }
            if self.mate(target, tie.right(), nest).await? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    async fn mate(&mut self, target: &Unit, key: i64, nest: &Pred) -> Result<bool, Error> {
        if !self.work.alive(target, key).await? {
            return Ok(false);
        }
        let rows = self.work.scan(target).await?;
        Ok(rows
            .iter()
            .find(|row| row.key() == key)
            .is_some_and(|row| hit(row, nest)))
    }
}

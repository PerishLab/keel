use super::*;
use crate::adapt::Error;
use crate::ddl;
use crate::life::{Ends, Row, Tie, Work};
use crate::plan::{Plan, Unit};
use crate::wire::Wire;
use std::collections::BTreeMap;

pub async fn run<W: Wire>(plan: &Plan, wire: &mut W, tree: &Tree) -> Result<Pack, Error> {
    let scope = analyze(plan, tree)?;
    let mut work = Work::new(wire, plan);
    let mut rows = match tree.slice() {
        Slice::Live => work.scan(scope.unit()).await?,
    };
    if !tree.preds().is_empty() {
        rows.retain(|row| pass(row, tree.preds()));
    }
    hold(
        &mut work,
        scope.unit(),
        scope.name(),
        &mut rows,
        tree.preds(),
    )
    .await?;
    if tree.tally() {
        return Ok(Pack {
            root: ddl::table(scope.name()),
            bags: BTreeMap::new(),
            count: Some(rows.len()),
        });
    }
    order(&mut rows, tree.sort());
    page(&mut rows, tree.after(), tree.limit());
    let keys: Vec<i64> = rows.iter().map(|row| row.key()).collect();
    let root = ddl::table(scope.name());
    let mut bags = BTreeMap::new();
    bags.insert(root.clone(), Bag::Unit(rows));
    let unit = scope.unit();
    for bond in tree.links() {
        let bond = edge(unit, bond)?;
        let ties = pull(&mut work, scope.name(), &bond, &keys).await?;
        let key = format!("{root}.{bond}");
        bags.insert(key, Bag::Bond(ties));
    }
    Ok(Pack {
        root,
        bags,
        count: None,
    })
}

pub(crate) async fn pull<W: Wire>(
    work: &mut Work<'_, W>,
    owner: &str,
    bond: &str,
    keys: &[i64],
) -> Result<Vec<Tie>, Error> {
    let target = {
        let (_, edge) = work.plan().edge(owner, bond)?;
        edge.target().to_string()
    };
    let mut ties = Vec::new();
    for &key in keys {
        let part = work.ties(owner, bond, key).await?;
        for tie in part {
            if !work.live_has(&target, tie.right()).await? {
                continue;
            }
            ties.push(tie);
            if ties.len() > TIE_CAP {
                return Err(Error::Adapt("tie cap exceeded".into()));
            }
        }
    }
    ties.sort_by_key(|tie| tie.key());
    Ok(ties)
}

pub(crate) async fn hold<W: Wire>(
    work: &mut Work<'_, W>,
    unit: &Unit,
    owner: &str,
    rows: &mut Vec<Row>,
    preds: &[Pred],
) -> Result<(), Error> {
    for pred in preds {
        hold_one(work, unit, owner, rows, pred).await?;
    }
    Ok(())
}

pub(crate) async fn hold_one<W: Wire>(
    work: &mut Work<'_, W>,
    unit: &crate::plan::Unit,
    owner: &str,
    rows: &mut Vec<Row>,
    pred: &Pred,
) -> Result<(), Error> {
    match pred.op() {
        Op::Has => hold_has(work, unit, owner, rows, pred).await,
        Op::Some => hold_some(work, unit, owner, rows, pred).await,
        _ => Ok(()),
    }
}

pub(crate) async fn hold_has<W: Wire>(
    work: &mut Work<'_, W>,
    unit: &crate::plan::Unit,
    owner: &str,
    rows: &mut Vec<Row>,
    pred: &Pred,
) -> Result<(), Error> {
    let bond = edge(unit, pred.field())?;
    let right = key_text(pred.value())?;
    let mut keep = Vec::new();
    for row in rows.iter() {
        if has_right(
            work,
            owner,
            &bond,
            Ends {
                left: row.key(),
                right,
            },
        )
        .await
        {
            keep.push(row.key());
        }
    }
    rows.retain(|row| keep.contains(&row.key()));
    Ok(())
}

pub(crate) async fn has_right<W: Wire>(
    work: &mut Work<'_, W>,
    owner: &str,
    bond: &str,
    ends: Ends,
) -> bool {
    match work.ties(owner, bond, ends.left).await {
        Ok(ties) => ties.iter().any(|tie| tie.right() == ends.right),
        Err(_) => false,
    }
}

pub(crate) async fn hold_some<W: Wire>(
    work: &mut Work<'_, W>,
    unit: &crate::plan::Unit,
    owner: &str,
    rows: &mut Vec<Row>,
    pred: &Pred,
) -> Result<(), Error> {
    let bond = edge(unit, pred.field())?;
    let nest = pred
        .nest()
        .ok_or_else(|| Error::Adapt("some needs inner".into()))?;
    let target = unit
        .bonds()
        .iter()
        .find(|e| e.name() == bond)
        .map(|e| e.target().to_string())
        .ok_or_else(|| Error::Adapt(format!("unknown bond {bond}")))?;
    let mut keep = Vec::new();
    for row in rows.iter() {
        let hit = some_hit(work, owner, &bond, &target, row.key(), nest)
            .await
            .unwrap_or(false);
        if hit {
            keep.push(row.key());
        }
    }
    rows.retain(|row| keep.contains(&row.key()));
    Ok(())
}

pub(crate) async fn some_hit<W: Wire>(
    work: &mut Work<'_, W>,
    owner: &str,
    bond: &str,
    target: &str,
    left: i64,
    nest: &Pred,
) -> Result<bool, Error> {
    let ties = work.ties(owner, bond, left).await?;
    let on_bond = bond_slot(work.plan(), owner, bond, nest.field());
    for tie in ties {
        if nest.field() == ddl::KEY {
            if hit_key(tie.right(), nest) {
                return Ok(true);
            }
            continue;
        }
        if on_bond {
            if hit_map(tie.cells(), nest) {
                return Ok(true);
            }
            continue;
        }
        if target_hit(work, target, tie.right(), nest).await? {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(crate) fn bond_slot(plan: &Plan, owner: &str, bond: &str, field: &str) -> bool {
    plan.units()
        .get(owner)
        .and_then(|u| u.bonds().iter().find(|e| e.name() == bond))
        .is_some_and(|e| e.fields().iter().any(|s| s.name() == field))
}

pub(crate) async fn target_hit<W: Wire>(
    work: &mut Work<'_, W>,
    target: &str,
    key: i64,
    nest: &Pred,
) -> Result<bool, Error> {
    if !work.live_has(target, key).await? {
        return Ok(false);
    }
    match find_live(work, target, key).await? {
        Some(row) => Ok(hit(&row, nest)),
        None => Ok(false),
    }
}

pub(crate) async fn find_live<W: Wire>(
    work: &mut Work<'_, W>,
    name: &str,
    key: i64,
) -> Result<Option<Row>, Error> {
    let rows = work.live(name).await?;
    Ok(rows.into_iter().find(|row| row.key() == key))
}

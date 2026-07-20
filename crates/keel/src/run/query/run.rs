use super::*;
use crate::adapt::Error;
use crate::ddl;
use crate::life::{Row, Tie, Work};
use crate::plan::Plan;
use crate::wire::Wire;
use std::collections::BTreeMap;

pub async fn run<W: Wire>(plan: &Plan, wire: &mut W, tree: &Tree) -> Result<Pack, Error> {
    let mut work = Work::new(wire);
    let name = resolve(plan, tree.from())?;
    check(plan, &name, tree.preds(), tree.sort())?;
    check_links(plan, &name, tree.links())?;
    let mut rows = match tree.slice() {
        Slice::Live => work.live(plan, &name).await?,
    };
    if !tree.preds().is_empty() {
        rows.retain(|row| pass(row, tree.preds()));
    }
    hold(plan, &mut work, &name, &mut rows, tree.preds()).await?;
    if tree.tally() {
        return Ok(Pack {
            root: ddl::table(&name),
            bags: BTreeMap::new(),
            count: Some(rows.len()),
        });
    }
    order(&mut rows, tree.sort());
    page(&mut rows, tree.after(), tree.limit());
    let keys: Vec<i64> = rows.iter().map(|row| row.key()).collect();
    let root = ddl::table(&name);
    let mut bags = BTreeMap::new();
    bags.insert(root.clone(), Bag::Unit(rows));
    let unit = plan
        .units()
        .get(&name)
        .ok_or_else(|| Error::Missing(name.clone()))?;
    for bond in tree.links() {
        let bond = edge(unit, bond)?;
        let target = unit
            .bonds()
            .iter()
            .find(|e| e.name() == bond)
            .map(|e| e.target().to_string())
            .ok_or_else(|| Error::Adapt(format!("unknown bond {bond}")))?;
        let ties = pull(plan, &mut work, &name, &bond, &target, &keys).await?;
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
    plan: &Plan,
    work: &mut Work<'_, W>,
    owner: &str,
    bond: &str,
    target: &str,
    keys: &[i64],
) -> Result<Vec<Tie>, Error> {
    let mut ties = Vec::new();
    for &key in keys {
        let part = work.ties(plan, owner, bond, key).await?;
        for tie in part {
            if !work.live_has(plan, target, tie.right()).await? {
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
    plan: &Plan,
    work: &mut Work<'_, W>,
    owner: &str,
    rows: &mut Vec<Row>,
    preds: &[Pred],
) -> Result<(), Error> {
    let unit = plan
        .units()
        .get(owner)
        .ok_or_else(|| Error::Missing(owner.into()))?;
    for pred in preds {
        hold_one(plan, work, unit, owner, rows, pred).await?;
    }
    Ok(())
}

pub(crate) async fn hold_one<W: Wire>(
    plan: &Plan,
    work: &mut Work<'_, W>,
    unit: &crate::plan::Unit,
    owner: &str,
    rows: &mut Vec<Row>,
    pred: &Pred,
) -> Result<(), Error> {
    match pred.op() {
        Op::Has => hold_has(plan, work, unit, owner, rows, pred).await,
        Op::Some => hold_some(plan, work, unit, owner, rows, pred).await,
        _ => Ok(()),
    }
}

pub(crate) async fn hold_has<W: Wire>(
    plan: &Plan,
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
        if has_right(work, plan, owner, &bond, row.key(), right).await {
            keep.push(row.key());
        }
    }
    rows.retain(|row| keep.contains(&row.key()));
    Ok(())
}

pub(crate) async fn has_right<W: Wire>(
    work: &mut Work<'_, W>,
    plan: &Plan,
    owner: &str,
    bond: &str,
    left: i64,
    right: i64,
) -> bool {
    match work.ties(plan, owner, bond, left).await {
        Ok(ties) => ties.iter().any(|tie| tie.right() == right),
        Err(_) => false,
    }
}

pub(crate) async fn hold_some<W: Wire>(
    plan: &Plan,
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
        let hit = some_hit(plan, work, owner, &bond, &target, row.key(), nest)
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
    plan: &Plan,
    work: &mut Work<'_, W>,
    owner: &str,
    bond: &str,
    target: &str,
    left: i64,
    nest: &Pred,
) -> Result<bool, Error> {
    let ties = work.ties(plan, owner, bond, left).await?;
    let on_bond = bond_slot(plan, owner, bond, nest.field());
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
        if target_hit(work, plan, target, tie.right(), nest).await? {
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
    plan: &Plan,
    target: &str,
    key: i64,
    nest: &Pred,
) -> Result<bool, Error> {
    if !work.live_has(plan, target, key).await? {
        return Ok(false);
    }
    match find_live(work, plan, target, key).await? {
        Some(row) => Ok(hit(&row, nest)),
        None => Ok(false),
    }
}

pub(crate) async fn find_live<W: Wire>(
    work: &mut Work<'_, W>,
    plan: &Plan,
    name: &str,
    key: i64,
) -> Result<Option<Row>, Error> {
    let rows = work.live(plan, name).await?;
    Ok(rows.into_iter().find(|row| row.key() == key))
}

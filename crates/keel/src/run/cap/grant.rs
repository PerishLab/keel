use super::*;
use crate::adapt::Error;
use crate::ddl;
use crate::plan::Plan;
use crate::query;

pub fn vet(plan: &Plan, fields: &[(&str, &str)]) -> Result<(), Error> {
    let verb = get(fields, "verb");
    if verb != "*" && !VERBS.contains(&verb) {
        return Err(Error::Adapt(format!("unknown verb {verb}")));
    }
    let who = get(fields, "who");
    if !whole(plan, who) {
        return Err(Error::Adapt("who is an id, group, anon, or all".into()));
    }
    let unit = get(fields, "unit");
    let place = if unit == "*" {
        None
    } else {
        Some(query::resolve(plan, unit)?)
    };
    scope(place.as_deref(), get(fields, "scope"))
}

pub(crate) fn whole(plan: &Plan, who: &str) -> bool {
    if who == "anon" || who == "all" || who.parse::<i64>().is_ok() {
        return true;
    }
    let Some((place, id)) = who.split_once(' ') else {
        return false;
    };
    if id.parse::<i64>().is_err() {
        return false;
    }
    plan.find(&ddl::table(place))
        .ok()
        .is_some_and(|node| node.crew().is_some())
}

pub(crate) fn scope(unit: Option<&str>, value: &str) -> Result<(), Error> {
    if value == "all" {
        return Ok(());
    }
    if let Some(id) = value.strip_prefix("row ") {
        if unit.is_none() {
            return Err(Error::Adapt("row scope needs a unit".into()));
        }
        id.parse::<i64>()
            .map_err(|_| Error::Adapt("row scope needs id".into()))?;
        return Ok(());
    }
    if let Some(pred) = value.strip_prefix("pred ") {
        let Some(unit) = unit else {
            return Err(Error::Adapt("pred scope needs a unit".into()));
        };
        let tree = query::parse(&format!("from {unit} where {pred}"))?;
        let plain = tree
            .preds()
            .iter()
            .all(|p| !matches!(p.op(), query::Op::Has | query::Op::Some));
        if !plain {
            return Err(Error::Adapt("scope pred is cells only".into()));
        }
        return Ok(());
    }
    Err(Error::Adapt(
        "scope is all | row <id> | pred <where>".into(),
    ))
}

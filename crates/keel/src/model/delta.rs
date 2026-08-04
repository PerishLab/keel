use super::manifest::{Atom, Bond, Edge, Field, Limit, Manifest, Unit};
use crate::estate::{Act, Change, Check, Fault, Step};
use std::collections::BTreeMap;

pub(crate) fn plan(active: &Manifest, requested: &Manifest) -> Result<Change, Fault> {
    let old = units(active);
    let new = units(requested);
    let mut steps = Vec::new();
    for (key, unit) in &new {
        match old.get(key) {
            None => steps.push(Step::new((*key).into(), Act::Add, None)),
            Some(prior) => revise(prior, unit, &mut steps)?,
        }
    }
    for key in old.keys().filter(|key| !new.contains_key(*key)) {
        steps.push(Step::new((*key).into(), Act::Drop, Some(Check::Empty)));
    }
    Ok(Change::new(steps))
}

fn revise(active: &Unit, requested: &Unit, steps: &mut Vec<Step>) -> Result<(), Fault> {
    if active.frozen != requested.frozen {
        return Err(Fault::Denied {
            path: active.key.clone(),
            note: "resource mode change needs a new resource".into(),
        });
    }
    if active.veil != requested.veil {
        steps.push(Step::new(active.key.clone(), Act::Alter, None));
    }
    fields(&active.key, &active.fields, &requested.fields, steps)?;
    bonds(active, requested, steps)
}

fn fields(
    owner: &str,
    active: &[Field],
    requested: &[Field],
    steps: &mut Vec<Step>,
) -> Result<(), Fault> {
    let old: BTreeMap<&str, &Field> = active
        .iter()
        .map(|field| (field.name.as_str(), field))
        .collect();
    let new: BTreeMap<&str, &Field> = requested
        .iter()
        .map(|field| (field.name.as_str(), field))
        .collect();
    for (name, field) in &new {
        let path = format!("{owner}.{name}");
        match old.get(name) {
            None => steps.push(Step::new(
                path,
                Act::Add,
                (field.need && field.rule.default.is_none()).then_some(Check::Empty),
            )),
            Some(prior) => scalar(&path, prior, field, steps)?,
        }
    }
    for name in old.keys().filter(|name| !new.contains_key(*name)) {
        steps.push(Step::new(format!("{owner}.{name}"), Act::Drop, None));
    }
    Ok(())
}

fn scalar(
    path: &str,
    active: &Field,
    requested: &Field,
    steps: &mut Vec<Step>,
) -> Result<(), Fault> {
    if active.kind != requested.kind {
        let check = cast(active.kind, requested.kind).map_err(|note| Fault::Denied {
            path: path.into(),
            note: note.into(),
        })?;
        steps.push(Step::new(path.into(), Act::Cast, check));
    }
    if active.only != requested.only {
        let check = strengthen(&active.only, &requested.only);
        steps.push(Step::new(path.into(), Act::Alter, check));
    }
    if !active.need && requested.need {
        steps.push(Step::new(
            path.into(),
            Act::Alter,
            requested.rule.default.is_none().then_some(Check::Presence),
        ));
    } else if active.need && !requested.need {
        steps.push(Step::new(path.into(), Act::Alter, None));
    }
    if domain(&active.rule) != domain(&requested.rule)
        && !contains(requested.kind, &active.rule, &requested.rule)
    {
        steps.push(Step::new(path.into(), Act::Alter, Some(Check::Values)));
    }
    match (&active.serial, &requested.serial) {
        (None, Some(_)) => {
            return Err(Fault::Denied {
                path: path.into(),
                note: "serial needs staged ceremony".into(),
            });
        }
        (Some(_), None) => steps.push(Step::new(path.into(), Act::Alter, None)),
        (Some(a), Some(b)) if a != b => {
            return Err(Fault::Denied {
                path: path.into(),
                note: "serial scope change needs staged ceremony".into(),
            });
        }
        _ => {}
    }
    Ok(())
}

fn bonds(active: &Unit, requested: &Unit, steps: &mut Vec<Step>) -> Result<(), Fault> {
    let old: BTreeMap<&str, &Edge> = active
        .bonds
        .iter()
        .map(|edge| (edge.name.as_str(), edge))
        .collect();
    let new: BTreeMap<&str, &Edge> = requested
        .bonds
        .iter()
        .map(|edge| (edge.name.as_str(), edge))
        .collect();
    for (name, edge) in &new {
        let path = format!("{}.{}", active.key, name);
        match old.get(name) {
            None => {
                let check = (edge.kind != Bond::Many2many && edge.need).then_some(Check::Empty);
                steps.push(Step::new(path, Act::Add, check));
            }
            Some(prior) => bond(&path, prior, edge, steps)?,
        }
    }
    for name in old.keys().filter(|name| !new.contains_key(*name)) {
        steps.push(Step::new(
            format!("{}.{}", active.key, name),
            Act::Drop,
            Some(Check::Clear),
        ));
    }
    Ok(())
}

fn bond(path: &str, active: &Edge, requested: &Edge, steps: &mut Vec<Step>) -> Result<(), Fault> {
    if active.target != requested.target {
        return Err(Fault::Denied {
            path: path.into(),
            note: "target change needs a new bond".into(),
        });
    }
    match (active.kind, requested.kind) {
        (Bond::Many2one, Bond::One2one) => {
            steps.push(Step::new(path.into(), Act::Alter, Some(Check::Unique)))
        }
        (Bond::One2one, Bond::Many2one) => steps.push(Step::new(path.into(), Act::Alter, None)),
        (a, b) if a != b => {
            return Err(Fault::Denied {
                path: path.into(),
                note: "bond family change needs a new bond".into(),
            });
        }
        _ => {}
    }
    if !active.need && requested.need {
        steps.push(Step::new(path.into(), Act::Alter, Some(Check::Presence)));
    } else if active.need && !requested.need {
        steps.push(Step::new(path.into(), Act::Alter, None));
    }
    if active.crew != requested.crew {
        steps.push(Step::new(path.into(), Act::Alter, Some(Check::Authority)));
    }
    if active.closure != requested.closure {
        steps.push(Step::new(path.into(), Act::Alter, None));
    }
    fields(path, &active.fields, &requested.fields, steps)
}

fn strengthen(active: &Limit, requested: &Limit) -> Option<Check> {
    match (active, requested) {
        (Limit::All | Limit::Per(_), Limit::Free) => None,
        _ => Some(Check::Unique),
    }
}

fn cast(active: Atom, requested: Atom) -> Result<Option<Check>, &'static str> {
    use Atom::{Bool, Int, Link, Text};
    match (active, requested) {
        (Text, Int) | (Text, Bool) | (Int, Bool) => Ok(Some(Check::Values)),
        (Link, Text) | (Int, Text) | (Bool, Text) | (Bool, Int) => Ok(None),
        (Text, Link) => Err("link canon is not legislated"),
        _ => Err("scalar cast is not legislated"),
    }
}

fn units(manifest: &Manifest) -> BTreeMap<&str, &Unit> {
    manifest
        .units()
        .iter()
        .map(|unit| (unit.key.as_str(), unit))
        .collect()
}

fn domain(rule: &super::manifest::Rule) -> (&[String], Option<i64>, Option<i64>) {
    (&rule.values, rule.min, rule.max)
}

fn contains(kind: Atom, active: &super::manifest::Rule, requested: &super::manifest::Rule) -> bool {
    if !active.values.is_empty() {
        return active
            .values
            .iter()
            .all(|value| accepts(kind, requested, value));
    }
    match kind {
        Atom::Text | Atom::Link => requested.values.is_empty(),
        Atom::Bool => ["false", "true"]
            .iter()
            .all(|value| accepts(kind, requested, value)),
        Atom::Int => {
            requested.values.is_empty()
                && requested
                    .min
                    .is_none_or(|min| active.min.is_some_and(|old| min <= old))
                && requested
                    .max
                    .is_none_or(|max| active.max.is_some_and(|old| max >= old))
        }
    }
}

fn accepts(kind: Atom, rule: &super::manifest::Rule, value: &str) -> bool {
    if !rule.values.is_empty() && !rule.values.iter().any(|item| item == value) {
        return false;
    }
    if kind == Atom::Int {
        let Ok(value) = value.parse::<i64>() else {
            return false;
        };
        if rule.min.is_some_and(|min| value < min) || rule.max.is_some_and(|max| value > max) {
            return false;
        }
    }
    true
}

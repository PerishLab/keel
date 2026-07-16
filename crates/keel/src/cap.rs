use crate::adapt::Error;
use crate::plan::Plan;
use crate::query;

pub const GRANT: &str = "@grant";
pub const VERBS: [&str; 6] = ["see", "put", "set", "end", "tie", "cut"];

pub fn vet(plan: &Plan, fields: &[(&str, &str)]) -> Result<(), Error> {
    let verb = get(fields, "verb");
    if verb != "*" && !VERBS.contains(&verb) {
        return Err(Error::Adapt(format!("unknown verb {verb}")));
    }
    let who = get(fields, "who");
    let named = who == "anon" || who == "all" || who.parse::<i64>().is_ok();
    if !named {
        return Err(Error::Adapt("who is an id, anon, or all".into()));
    }
    let unit = get(fields, "unit");
    let place = if unit == "*" {
        None
    } else {
        Some(query::resolve(plan, unit)?)
    };
    scope(place.as_deref(), get(fields, "scope"))
}

fn scope(unit: Option<&str>, value: &str) -> Result<(), Error> {
    if value == "all" {
        return Ok(());
    }
    if let Some(id) = value.strip_prefix("row ") {
        id.parse::<i64>()
            .map_err(|_| Error::Adapt("row scope needs id".into()))?;
        return Ok(());
    }
    if let Some(pred) = value.strip_prefix("pred ") {
        let Some(unit) = unit else {
            return Err(Error::Adapt("pred scope needs a unit".into()));
        };
        query::parse(&format!("from {unit} where {pred}"))?;
        return Ok(());
    }
    Err(Error::Adapt(
        "scope is all | row <id> | pred <where>".into(),
    ))
}

fn get<'a>(fields: &[(&'a str, &'a str)], name: &str) -> &'a str {
    fields
        .iter()
        .find(|(k, _)| *k == name)
        .map(|(_, v)| *v)
        .unwrap_or("")
}

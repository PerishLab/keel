use crate::adapt::Error;
use crate::ddl;
use crate::plan::Plan;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Ask {
    unit: String,
}

impl Ask {
    pub fn unit(&self) -> &str {
        &self.unit
    }
}

pub fn parse(text: &str) -> Result<Ask, Error> {
    let text = text.trim();
    if text.is_empty() {
        return Err(Error::Adapt("empty query".into()));
    }
    let mut parts = text.split_whitespace();
    let head = parts
        .next()
        .ok_or_else(|| Error::Adapt("empty query".into()))?;
    if !head.eq_ignore_ascii_case("from") {
        return Err(Error::Adapt(format!("expected from, got {head}")));
    }
    let unit = parts
        .next()
        .ok_or_else(|| Error::Adapt("from needs a resource".into()))?;
    if parts.next().is_some() {
        return Err(Error::Adapt("query has trailing tokens".into()));
    }
    Ok(Ask {
        unit: unit.to_string(),
    })
}

pub fn resolve(plan: &Plan, unit: &str) -> Result<String, Error> {
    let want = ddl::table(unit);
    plan.units()
        .values()
        .find(|node| ddl::table(node.name()) == want)
        .map(|node| node.name().to_string())
        .ok_or_else(|| Error::Missing(unit.into()))
}

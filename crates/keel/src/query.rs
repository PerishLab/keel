use crate::adapt::Error;
use crate::ddl;
use crate::life::Row;
use crate::plan::Plan;
use crate::store::Store;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Slice {
    Live,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Tree {
    from: String,
    slice: Slice,
}

impl Tree {
    pub fn from(&self) -> &str {
        &self.from
    }

    pub fn slice(&self) -> Slice {
        self.slice
    }
}

pub type Ask = Tree;

pub fn form(unit: &str) -> Tree {
    Tree {
        from: unit.to_string(),
        slice: Slice::Live,
    }
}

pub fn parse(text: &str) -> Result<Tree, Error> {
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
    Ok(form(unit))
}

pub fn resolve(plan: &Plan, unit: &str) -> Result<String, Error> {
    let want = ddl::table(unit);
    plan.units()
        .values()
        .find(|node| ddl::table(node.name()) == want)
        .map(|node| node.name().to_string())
        .ok_or_else(|| Error::Missing(unit.into()))
}

pub fn run(plan: &Plan, store: &impl Store, tree: &Tree) -> Result<Vec<Row>, Error> {
    let name = resolve(plan, tree.from())?;
    match tree.slice() {
        Slice::Live => store.live(plan, &name),
    }
}

pub fn digest(tree: &Tree) -> String {
    let unit = ddl::table(tree.from());
    match tree.slice() {
        Slice::Live => format!("from {unit} slice live"),
    }
}

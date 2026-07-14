use crate::adapt::Error;
use crate::ddl;
use crate::life::Row;
use crate::plan::Plan;
use crate::store::Store;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Slice {
    Live,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Op {
    Eq,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Pred {
    field: String,
    op: Op,
    value: String,
}

impl Pred {
    pub fn field(&self) -> &str {
        &self.field
    }

    pub fn op(&self) -> Op {
        self.op
    }

    pub fn value(&self) -> &str {
        &self.value
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Tree {
    from: String,
    slice: Slice,
    preds: Vec<Pred>,
}

impl Tree {
    pub fn from(&self) -> &str {
        &self.from
    }

    pub fn slice(&self) -> Slice {
        self.slice
    }

    pub fn preds(&self) -> &[Pred] {
        &self.preds
    }
}

pub type Ask = Tree;

pub fn form(unit: &str) -> Tree {
    Tree {
        from: unit.to_string(),
        slice: Slice::Live,
        preds: Vec::new(),
    }
}

pub fn parse(text: &str) -> Result<Tree, Error> {
    let mut scan = Scan::new(text);
    scan.ws();
    scan.kw("from")?;
    let unit = scan.ident()?;
    let mut preds = Vec::new();
    scan.ws();
    if scan.opt("where") {
        loop {
            let field = scan.ident()?;
            scan.ws();
            scan.ch('=')?;
            scan.ws();
            let value = scan.quoted()?;
            preds.push(Pred {
                field,
                op: Op::Eq,
                value,
            });
            scan.ws();
            if !scan.opt("and") {
                break;
            }
        }
    }
    scan.ws();
    if !scan.done() {
        return Err(Error::Adapt("query has trailing tokens".into()));
    }
    Ok(Tree {
        from: unit,
        slice: Slice::Live,
        preds,
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

pub fn run(plan: &Plan, store: &impl Store, tree: &Tree) -> Result<Vec<Row>, Error> {
    let name = resolve(plan, tree.from())?;
    check(plan, &name, tree.preds())?;
    let mut rows = match tree.slice() {
        Slice::Live => store.live(plan, &name)?,
    };
    if !tree.preds().is_empty() {
        rows.retain(|row| pass(row, tree.preds()));
    }
    Ok(rows)
}

pub fn digest(tree: &Tree) -> String {
    let unit = ddl::table(tree.from());
    let mut out = match tree.slice() {
        Slice::Live => format!("from {unit} slice live"),
    };
    for (i, pred) in tree.preds().iter().enumerate() {
        let _ = pred.op();
        if i == 0 {
            out.push_str(" where ");
        } else {
            out.push_str(" and ");
        }
        out.push_str(pred.field());
        out.push_str(" = \"");
        out.push_str(&escape(pred.value()));
        out.push('"');
    }
    out
}

fn check(plan: &Plan, name: &str, preds: &[Pred]) -> Result<(), Error> {
    let unit = plan
        .units()
        .get(name)
        .ok_or_else(|| Error::Missing(name.into()))?;
    for pred in preds {
        if !unit.fields().iter().any(|slot| slot.name() == pred.field()) {
            return Err(Error::Adapt(format!("unknown field {}", pred.field())));
        }
    }
    Ok(())
}

fn pass(row: &Row, preds: &[Pred]) -> bool {
    preds.iter().all(|pred| match pred.op() {
        Op::Eq => row.cells().get(pred.field()).map(String::as_str) == Some(pred.value()),
    })
}

fn escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

struct Scan<'a> {
    text: &'a str,
    at: usize,
}

impl<'a> Scan<'a> {
    fn new(text: &'a str) -> Self {
        Self { text, at: 0 }
    }

    fn done(&self) -> bool {
        self.at >= self.text.len()
    }

    fn rest(&self) -> &'a str {
        &self.text[self.at..]
    }

    fn ws(&mut self) {
        while let Some(c) = self.rest().chars().next() {
            if !c.is_whitespace() {
                break;
            }
            self.at += c.len_utf8();
        }
    }

    fn kw(&mut self, want: &str) -> Result<(), Error> {
        self.ws();
        if self.opt(want) {
            Ok(())
        } else {
            Err(Error::Adapt(format!("expected {want}")))
        }
    }

    fn opt(&mut self, want: &str) -> bool {
        self.ws();
        let rest = self.rest();
        if rest.len() < want.len() {
            return false;
        }
        if !rest[..want.len()].eq_ignore_ascii_case(want) {
            return false;
        }
        let after = rest.get(want.len()..).unwrap_or("");
        let cont = after
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
        if cont {
            return false;
        }
        self.at += want.len();
        true
    }

    fn ident(&mut self) -> Result<String, Error> {
        self.ws();
        let rest = self.rest();
        let mut end = 0;
        for c in rest.chars() {
            if c.is_ascii_alphanumeric() || c == '_' {
                end += c.len_utf8();
            } else {
                break;
            }
        }
        if end == 0 {
            return Err(Error::Adapt("expected name".into()));
        }
        let name = rest[..end].to_string();
        self.at += end;
        Ok(name)
    }

    fn ch(&mut self, want: char) -> Result<(), Error> {
        self.ws();
        let mut chars = self.rest().chars();
        match chars.next() {
            Some(c) if c == want => {
                self.at += c.len_utf8();
                Ok(())
            }
            _ => Err(Error::Adapt(format!("expected {want}"))),
        }
    }

    fn quoted(&mut self) -> Result<String, Error> {
        self.ws();
        let rest = self.rest();
        if !rest.starts_with('"') {
            return Err(Error::Adapt("expected string".into()));
        }
        match take(rest) {
            Some((out, used)) => {
                self.at += used;
                Ok(out)
            }
            None => Err(Error::Adapt("unterminated string".into())),
        }
    }
}

fn take(rest: &str) -> Option<(String, usize)> {
    let bytes = rest.as_bytes();
    let mut i = 1;
    let mut out = String::new();
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'"' {
            return Some((out, i + 1));
        }
        if b == b'\\' {
            i += 1;
            if i >= bytes.len() {
                return None;
            }
            out.push(bytes[i] as char);
            i += 1;
            continue;
        }
        out.push(b as char);
        i += 1;
    }
    None
}

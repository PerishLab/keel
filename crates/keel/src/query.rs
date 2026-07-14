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
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    In,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Rank {
    Asc,
    Desc,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Pred {
    field: String,
    op: Op,
    values: Vec<String>,
}

impl Pred {
    pub fn field(&self) -> &str {
        &self.field
    }

    pub fn op(&self) -> Op {
        self.op
    }

    pub fn value(&self) -> &str {
        self.values.first().map(String::as_str).unwrap_or("")
    }

    pub fn values(&self) -> &[String] {
        &self.values
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Sort {
    field: String,
    rank: Rank,
}

impl Sort {
    pub fn field(&self) -> &str {
        &self.field
    }

    pub fn rank(&self) -> Rank {
        self.rank
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Tree {
    from: String,
    slice: Slice,
    preds: Vec<Pred>,
    sort: Option<Sort>,
    limit: Option<usize>,
    after: Option<i64>,
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

    pub fn sort(&self) -> Option<&Sort> {
        self.sort.as_ref()
    }

    pub fn limit(&self) -> Option<usize> {
        self.limit
    }

    pub fn after(&self) -> Option<i64> {
        self.after
    }
}

pub type Ask = Tree;

pub fn form(unit: &str) -> Tree {
    Tree {
        from: unit.to_string(),
        slice: Slice::Live,
        preds: Vec::new(),
        sort: None,
        limit: None,
        after: None,
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
            preds.push(take_pred(&mut scan)?);
            scan.ws();
            if !scan.opt("and") {
                break;
            }
        }
    }
    let sort = take_sort(&mut scan)?;
    let limit = take_limit(&mut scan)?;
    let after = take_after(&mut scan)?;
    scan.ws();
    if !scan.done() {
        return Err(Error::Adapt("query has trailing tokens".into()));
    }
    Ok(Tree {
        from: unit,
        slice: Slice::Live,
        preds,
        sort,
        limit,
        after,
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
    check(plan, &name, tree.preds(), tree.sort())?;
    let mut rows = match tree.slice() {
        Slice::Live => store.live(plan, &name)?,
    };
    if !tree.preds().is_empty() {
        rows.retain(|row| pass(row, tree.preds()));
    }
    order(&mut rows, tree.sort());
    page(&mut rows, tree.after(), tree.limit());
    Ok(rows)
}

pub fn digest(tree: &Tree) -> String {
    let unit = ddl::table(tree.from());
    let mut out = match tree.slice() {
        Slice::Live => format!("from {unit} slice live"),
    };
    for (i, pred) in tree.preds().iter().enumerate() {
        if i == 0 {
            out.push_str(" where ");
        } else {
            out.push_str(" and ");
        }
        write_pred(&mut out, pred);
    }
    if let Some(sort) = tree.sort() {
        out.push_str(" order by ");
        out.push_str(sort.field());
        out.push(' ');
        out.push_str(match sort.rank() {
            Rank::Asc => "asc",
            Rank::Desc => "desc",
        });
    }
    if let Some(n) = tree.limit() {
        out.push_str(" limit ");
        out.push_str(&n.to_string());
    }
    if let Some(id) = tree.after() {
        out.push_str(" after \"");
        out.push_str(&id.to_string());
        out.push('"');
    }
    out
}

fn take_pred(scan: &mut Scan<'_>) -> Result<Pred, Error> {
    let field = scan.ident()?;
    scan.ws();
    if scan.opt("in") {
        let values = take_list(scan)?;
        return Ok(Pred {
            field,
            op: Op::In,
            values,
        });
    }
    let op = scan.op()?;
    scan.ws();
    let value = scan.quoted()?;
    Ok(Pred {
        field,
        op,
        values: vec![value],
    })
}

fn take_list(scan: &mut Scan<'_>) -> Result<Vec<String>, Error> {
    scan.ch('(')?;
    let mut values = Vec::new();
    loop {
        values.push(scan.quoted()?);
        scan.ws();
        if !scan.comma() {
            break;
        }
    }
    scan.ch(')')?;
    if values.is_empty() {
        return Err(Error::Adapt("empty in list".into()));
    }
    Ok(values)
}

fn write_pred(out: &mut String, pred: &Pred) {
    out.push_str(pred.field());
    out.push(' ');
    if pred.op() == Op::In {
        out.push_str("in (");
        for (i, value) in pred.values().iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            out.push('"');
            out.push_str(&escape(value));
            out.push('"');
        }
        out.push(')');
        return;
    }
    out.push_str(mark(pred.op()));
    out.push_str(" \"");
    out.push_str(&escape(pred.value()));
    out.push('"');
}

fn mark(op: Op) -> &'static str {
    match op {
        Op::Eq => "=",
        Op::Ne => "!=",
        Op::Lt => "<",
        Op::Le => "<=",
        Op::Gt => ">",
        Op::Ge => ">=",
        Op::In => "in",
    }
}

fn take_sort(scan: &mut Scan<'_>) -> Result<Option<Sort>, Error> {
    scan.ws();
    if !scan.opt("order") {
        return Ok(None);
    }
    scan.kw("by")?;
    let field = scan.ident()?;
    let mut rank = Rank::Asc;
    scan.ws();
    if scan.opt("desc") {
        rank = Rank::Desc;
    } else {
        let _ = scan.opt("asc");
    }
    Ok(Some(Sort { field, rank }))
}

fn take_limit(scan: &mut Scan<'_>) -> Result<Option<usize>, Error> {
    scan.ws();
    if !scan.opt("limit") {
        return Ok(None);
    }
    let n = scan.num()?;
    if n == 0 {
        return Err(Error::Adapt("limit must be positive".into()));
    }
    Ok(Some(n))
}

fn take_after(scan: &mut Scan<'_>) -> Result<Option<i64>, Error> {
    scan.ws();
    if !scan.opt("after") {
        return Ok(None);
    }
    let text = scan.quoted()?;
    let id = text
        .parse::<i64>()
        .map_err(|_| Error::Adapt("after needs integer id".into()))?;
    Ok(Some(id))
}

fn check(plan: &Plan, name: &str, preds: &[Pred], sort: Option<&Sort>) -> Result<(), Error> {
    let unit = plan
        .units()
        .get(name)
        .ok_or_else(|| Error::Missing(name.into()))?;
    for pred in preds {
        if !unit.fields().iter().any(|slot| slot.name() == pred.field()) {
            return Err(Error::Adapt(format!("unknown field {}", pred.field())));
        }
    }
    if let Some(sort) = sort
        && !unit.fields().iter().any(|slot| slot.name() == sort.field())
    {
        return Err(Error::Adapt(format!("unknown field {}", sort.field())));
    }
    Ok(())
}

fn pass(row: &Row, preds: &[Pred]) -> bool {
    preds.iter().all(|pred| hit(row, pred))
}

fn hit(row: &Row, pred: &Pred) -> bool {
    let Some(got) = row.cells().get(pred.field()) else {
        return false;
    };
    match pred.op() {
        Op::Eq => got == pred.value(),
        Op::Ne => got != pred.value(),
        Op::Lt => got.as_str() < pred.value(),
        Op::Le => got.as_str() <= pred.value(),
        Op::Gt => got.as_str() > pred.value(),
        Op::Ge => got.as_str() >= pred.value(),
        Op::In => pred.values().iter().any(|want| want == got),
    }
}

fn order(rows: &mut [Row], sort: Option<&Sort>) {
    match sort {
        None => rows.sort_by_key(|row| row.key()),
        Some(sort) => {
            let field = sort.field().to_string();
            let desc = sort.rank() == Rank::Desc;
            rows.sort_by(|a, b| by(a, b, &field, desc));
        }
    }
}

fn by(a: &Row, b: &Row, field: &str, desc: bool) -> std::cmp::Ordering {
    let left = cell(a, field);
    let right = cell(b, field);
    let primary = grade(&left, &right, desc);
    primary.then_with(|| a.key().cmp(&b.key()))
}

fn grade(left: &str, right: &str, desc: bool) -> std::cmp::Ordering {
    if desc {
        right.cmp(left)
    } else {
        left.cmp(right)
    }
}

fn page(rows: &mut Vec<Row>, after: Option<i64>, limit: Option<usize>) {
    if let Some(id) = after {
        past(rows, id);
    }
    if let Some(n) = limit {
        rows.truncate(n);
    }
}

fn past(rows: &mut Vec<Row>, id: i64) {
    match rows.iter().position(|row| row.key() == id) {
        Some(i) => {
            rows.drain(0..=i);
        }
        None => rows.clear(),
    }
}

fn cell(row: &Row, field: &str) -> String {
    row.cells().get(field).cloned().unwrap_or_default()
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

    fn num(&mut self) -> Result<usize, Error> {
        self.ws();
        let rest = self.rest();
        let mut end = 0;
        for c in rest.chars() {
            if c.is_ascii_digit() {
                end += c.len_utf8();
            } else {
                break;
            }
        }
        if end == 0 {
            return Err(Error::Adapt("expected number".into()));
        }
        let n = rest[..end]
            .parse::<usize>()
            .map_err(|_| Error::Adapt("bad number".into()))?;
        self.at += end;
        Ok(n)
    }

    fn op(&mut self) -> Result<Op, Error> {
        self.ws();
        let rest = self.rest();
        let (op, n) = if rest.starts_with("!=") {
            (Op::Ne, 2)
        } else if rest.starts_with("<=") {
            (Op::Le, 2)
        } else if rest.starts_with(">=") {
            (Op::Ge, 2)
        } else if rest.starts_with('<') {
            (Op::Lt, 1)
        } else if rest.starts_with('>') {
            (Op::Gt, 1)
        } else if rest.starts_with('=') {
            (Op::Eq, 1)
        } else {
            return Err(Error::Adapt("expected op".into()));
        };
        self.at += n;
        Ok(op)
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

    fn comma(&mut self) -> bool {
        self.ws();
        if self.rest().starts_with(',') {
            self.at += 1;
            true
        } else {
            false
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

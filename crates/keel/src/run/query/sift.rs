use super::*;
use crate::adapt::Error;
use crate::ddl;
use crate::life::{Cell, Row};
use std::collections::BTreeMap;

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
    for bond in tree.links() {
        out.push_str(" link ");
        out.push_str(bond);
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
    if tree.tally() {
        out.push_str(" count");
    }
    out
}

pub fn shape(tree: &Tree) -> String {
    let unit = ddl::table(tree.from());
    let mut out = match tree.slice() {
        Slice::Live => format!("from {unit} slice live"),
    };
    for (i, pred) in tree.preds().iter().enumerate() {
        out.push_str(if i == 0 { " where " } else { " and " });
        write_shape(&mut out, pred);
    }
    for bond in tree.links() {
        out.push_str(" link ");
        out.push_str(bond);
    }
    if let Some(sort) = tree.sort() {
        out.push_str(" order by ");
        out.push_str(sort.field());
    }
    out
}

pub(crate) fn write_shape(out: &mut String, pred: &Pred) {
    out.push_str(pred.field());
    out.push(' ');
    out.push_str(mark(pred.op()));
    if pred.op() != Op::Some {
        return;
    }
    out.push_str(" (");
    if let Some(nest) = pred.nest() {
        write_shape(out, nest);
    }
    out.push(')');
}

pub(crate) fn write_pred(out: &mut String, pred: &Pred) {
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
    if pred.op() == Op::Has {
        out.push_str("has \"");
        out.push_str(&escape(pred.value()));
        out.push('"');
        return;
    }
    if pred.op() == Op::Some {
        out.push_str("some (");
        if let Some(nest) = pred.nest() {
            write_pred(out, nest);
        }
        out.push(')');
        return;
    }
    out.push_str(mark(pred.op()));
    out.push_str(" \"");
    out.push_str(&escape(pred.value()));
    out.push('"');
}

pub(crate) fn mark(op: Op) -> &'static str {
    match op {
        Op::Eq => "=",
        Op::Ne => "!=",
        Op::Lt => "<",
        Op::Le => "<=",
        Op::Gt => ">",
        Op::Ge => ">=",
        Op::In => "in",
        Op::Like => "like",
        Op::Has => "has",
        Op::Some => "some",
    }
}

pub(crate) fn hit_map(cells: &BTreeMap<String, Cell>, pred: &Pred) -> bool {
    let Some(got) = cells.get(pred.field()) else {
        return false;
    };
    match got {
        Cell::Text(value) => hit_text(value, pred),
        Cell::Int(value) => hit_key(*value, pred),
        Cell::Bool(value) => hit_flag(*value, pred),
    }
}

pub(crate) fn hit_text(got: &str, pred: &Pred) -> bool {
    match pred.op() {
        Op::Eq => got == pred.value(),
        Op::Ne => got != pred.value(),
        Op::Lt => got < pred.value(),
        Op::Le => got <= pred.value(),
        Op::Gt => got > pred.value(),
        Op::Ge => got >= pred.value(),
        Op::In => pred.values().iter().any(|want| want == got),
        Op::Like => got.to_lowercase().contains(&pred.value().to_lowercase()),
        Op::Has | Op::Some => false,
    }
}

pub(crate) fn hit_flag(got: bool, pred: &Pred) -> bool {
    let want = got.to_string();
    match pred.op() {
        Op::Eq => pred.value() == want,
        Op::Ne => pred.value() != want,
        Op::In => pred.values().contains(&want),
        _ => false,
    }
}

pub(crate) fn key_text(text: &str) -> Result<i64, Error> {
    text.parse::<i64>()
        .map_err(|_| Error::Adapt("id needs integer".into()))
}

pub fn cover(key: Option<i64>, cells: &BTreeMap<String, Cell>, preds: &[Pred]) -> bool {
    preds.iter().all(|pred| hit_at(key, cells, pred))
}

pub(crate) fn hit_at(key: Option<i64>, cells: &BTreeMap<String, Cell>, pred: &Pred) -> bool {
    if matches!(pred.op(), Op::Has | Op::Some) {
        return false;
    }
    if pred.field() == ddl::KEY {
        return key.is_some_and(|key| hit_key(key, pred));
    }
    hit_map(cells, pred)
}

pub fn bare(tree: &Tree) -> Tree {
    let mut out = tree.clone();
    out.tally = false;
    out
}

pub(crate) fn pass(row: &Row, preds: &[Pred]) -> bool {
    preds.iter().all(|pred| hit(row, pred))
}

pub(crate) fn hit(row: &Row, pred: &Pred) -> bool {
    if matches!(pred.op(), Op::Has | Op::Some) {
        return true;
    }
    if pred.field() == ddl::KEY {
        return hit_key(row.key(), pred);
    }
    hit_map(row.cells(), pred)
}

pub(crate) fn hit_key(key: i64, pred: &Pred) -> bool {
    match pred.op() {
        Op::Eq => key_text(pred.value()).is_ok_and(|want| want == key),
        Op::Ne => key_text(pred.value()).is_ok_and(|want| want != key),
        Op::Lt => key_text(pred.value()).is_ok_and(|want| key < want),
        Op::Le => key_text(pred.value()).is_ok_and(|want| key <= want),
        Op::Gt => key_text(pred.value()).is_ok_and(|want| key > want),
        Op::Ge => key_text(pred.value()).is_ok_and(|want| key >= want),
        Op::In => pred
            .values()
            .iter()
            .any(|want| key_text(want).is_ok_and(|n| n == key)),
        Op::Like | Op::Has | Op::Some => false,
    }
}

pub(crate) fn order(rows: &mut [Row], sort: Option<&Sort>) {
    match sort {
        None => rows.sort_by_key(|row| row.key()),
        Some(sort) if sort.field() == ddl::KEY => {
            let desc = sort.rank() == Rank::Desc;
            rows.sort_by(|a, b| rank_key(a, b, desc));
        }
        Some(sort) => {
            let field = sort.field().to_string();
            let desc = sort.rank() == Rank::Desc;
            rows.sort_by(|a, b| by(a, b, &field, desc));
        }
    }
}

pub(crate) fn rank_key(a: &Row, b: &Row, desc: bool) -> std::cmp::Ordering {
    let primary = a.key().cmp(&b.key());
    if desc { primary.reverse() } else { primary }
}

pub(crate) fn by(a: &Row, b: &Row, field: &str, desc: bool) -> std::cmp::Ordering {
    let left = cell(a, field);
    let right = cell(b, field);
    let primary = grade(&left, &right, desc);
    primary.then_with(|| a.key().cmp(&b.key()))
}

pub(crate) fn grade(left: &Cell, right: &Cell, desc: bool) -> std::cmp::Ordering {
    if desc {
        right.cmp(left)
    } else {
        left.cmp(right)
    }
}

pub(crate) fn page(rows: &mut Vec<Row>, after: Option<i64>, limit: Option<usize>) {
    if let Some(id) = after {
        past(rows, id);
    }
    if let Some(n) = limit {
        rows.truncate(n);
    }
}

pub(crate) fn past(rows: &mut Vec<Row>, id: i64) {
    match rows.iter().position(|row| row.key() == id) {
        Some(i) => {
            rows.drain(0..=i);
        }
        None => rows.clear(),
    }
}

pub(crate) fn cell(row: &Row, field: &str) -> Cell {
    row.cells()
        .get(field)
        .cloned()
        .unwrap_or(Cell::Text(String::new()))
}

pub(crate) fn escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

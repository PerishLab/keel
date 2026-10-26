use super::{EXPIRES, col, join, mate, place, side};
use crate::bond;
use crate::plan::Unit;

pub(super) fn all(units: &[&Unit], generation: Option<i64>) -> Vec<String> {
    let mark = generation.map(|id| format!("g{id}_")).unwrap_or_default();
    let mut out = Vec::new();
    for node in units {
        let expires = if node.reign().expires() {
            format!(", {EXPIRES}")
        } else {
            String::new()
        };
        for (table, column) in sides(node) {
            out.push(format!(
                "CREATE INDEX {} ON {} ({}{expires});",
                col(&format!("ref_{mark}{table}_{column}")),
                col(&place(&table, generation)),
                col(&column)
            ));
        }
    }
    out
}

fn sides(node: &Unit) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for edge in node.bonds() {
        if edge.kind() == bond::Kind::Many2many {
            let table = join(node, edge.name());
            out.push((table.clone(), side(node.name())));
            out.push((table, mate(node.name(), edge.name(), edge.target())));
        } else {
            out.push((node.table(), side(edge.name())));
        }
    }
    out
}

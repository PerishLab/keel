use crate::atom;
use crate::bond;
use crate::plan::{Plan, Reign, Unit};

pub const KEY: &str = "id";
pub const EXPIRES: &str = "expires_at";
pub const CREATED: &str = "created_at";
pub const UPDATED: &str = "updated_at";

pub fn table(name: &str) -> String {
    name.to_ascii_lowercase()
}

pub fn join(owner: &str, bond: &str) -> String {
    format!("{}_{}", table(owner), table(bond))
}

pub fn script(plan: &Plan) -> Vec<String> {
    let mut out = Vec::new();
    out.push("PRAGMA foreign_keys = ON;".into());
    for node in plan.units().values() {
        out.push(form(node));
    }
    for node in plan.units().values() {
        for bond in node.bonds() {
            out.push(arc(node, bond.name(), bond.target(), bond.kind()));
        }
    }
    out
}

fn form(node: &Unit) -> String {
    let mut cols = vec![format!("{KEY} INTEGER PRIMARY KEY NOT NULL")];
    for slot in node.fields() {
        cols.push(format!("{} {} NOT NULL", slot.name(), cast(slot.kind())));
    }
    stamp(node.reign(), &mut cols);
    format!(
        "CREATE TABLE IF NOT EXISTS {} ({});",
        table(node.name()),
        cols.join(", ")
    )
}

fn arc(node: &Unit, bond: &str, target: &str, kind: bond::Kind) -> String {
    match kind {
        bond::Kind::N2m => {
            let left = format!("{}_id", table(node.name()));
            let right = format!("{}_id", table(target));
            let mut cols = vec![
                format!("{KEY} INTEGER PRIMARY KEY NOT NULL"),
                format!("{left} INTEGER NOT NULL"),
                format!("{right} INTEGER NOT NULL"),
            ];
            stamp(node.reign(), &mut cols);
            cols.push(format!("UNIQUE({left}, {right})"));
            format!(
                "CREATE TABLE IF NOT EXISTS {} ({});",
                join(node.name(), bond),
                cols.join(", ")
            )
        }
    }
}

fn stamp(reign: &Reign, cols: &mut Vec<String>) {
    if reign.expires() {
        cols.push(format!("{EXPIRES} INTEGER"));
    }
    if reign.created() {
        cols.push(format!("{CREATED} INTEGER NOT NULL"));
    }
    if reign.updated() {
        cols.push(format!("{UPDATED} INTEGER NOT NULL"));
    }
}

fn cast(kind: atom::Kind) -> &'static str {
    match kind {
        atom::Kind::Text | atom::Kind::Link => "TEXT",
    }
}

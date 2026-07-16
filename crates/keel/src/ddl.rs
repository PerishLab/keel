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

pub fn side(name: &str) -> String {
    format!("{}_id", table(name))
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
            let edge = node
                .bonds()
                .iter()
                .find(|edge| edge.name() == bond)
                .expect("bond");
            let left = side(node.name());
            let right = side(target);
            let mut cols = vec![
                format!("{KEY} INTEGER PRIMARY KEY NOT NULL"),
                format!("{left} INTEGER NOT NULL"),
                format!("{right} INTEGER NOT NULL"),
            ];
            for slot in edge.fields() {
                cols.push(format!("{} {} NOT NULL", slot.name(), cast(slot.kind())));
            }
            stamp(node.reign(), &mut cols);
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
        atom::Kind::Int | atom::Kind::Bool => "INTEGER",
    }
}

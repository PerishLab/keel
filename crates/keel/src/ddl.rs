use crate::atom;
use crate::bond;
use crate::plan::{Plan, Reign, Slot, Unit};
use crate::spec::Only;

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

pub fn col(name: &str) -> String {
    format!("\"{name}\"")
}

pub fn seat(name: &str) -> String {
    col(&table(name))
}

pub fn joint(owner: &str, bond: &str) -> String {
    col(&join(owner, bond))
}

pub fn mate(owner: &str, bond: &str, target: &str) -> String {
    if table(owner) == table(target) {
        return side(bond);
    }
    side(target)
}

pub fn script(plan: &Plan) -> Vec<String> {
    let mut out = Vec::new();
    out.push("PRAGMA foreign_keys = ON;".into());
    for node in plan.units().values() {
        out.push(form(node));
    }
    for node in plan.units().values() {
        for bond in node.bonds() {
            if bond.kind() == bond::Kind::Many2many {
                out.push(arc(node, bond.name(), bond.target()));
            }
        }
    }
    for node in plan.units().values() {
        for slot in node.fields() {
            if *slot.only() != Only::Free || slot.serial().is_some() {
                out.push(lock(node, slot));
            }
        }
    }
    out
}

fn lock(node: &Unit, slot: &Slot) -> String {
    let place = table(node.name());
    let scope = match slot.only() {
        Only::Per(rel) => Some(rel.as_str()),
        _ => slot.serial(),
    };
    let cols = match scope {
        Some(rel) => format!("{}, {}", col(&side(rel)), col(slot.name())),
        None => col(slot.name()),
    };
    format!(
        "CREATE UNIQUE INDEX IF NOT EXISTS only_{place}_{} ON {} ({cols}) WHERE {EXPIRES} IS NULL;",
        slot.name(),
        col(&place)
    )
}

fn form(node: &Unit) -> String {
    let mut cols = vec![format!("{KEY} INTEGER PRIMARY KEY NOT NULL")];
    for slot in node.fields() {
        cols.push(format!(
            "{} {} NOT NULL",
            col(slot.name()),
            cast(slot.kind())
        ));
    }
    for edge in node.bonds() {
        if edge.kind().point() {
            let null = if edge.need() { " NOT NULL" } else { "" };
            cols.push(format!("{} INTEGER{}", col(&side(edge.name())), null));
        }
    }
    stamp(node.reign(), &mut cols);
    format!(
        "CREATE TABLE IF NOT EXISTS {} ({});",
        seat(node.name()),
        cols.join(", ")
    )
}

fn arc(node: &Unit, bond: &str, target: &str) -> String {
    let edge = node
        .bonds()
        .iter()
        .find(|edge| edge.name() == bond)
        .expect("bond");
    let left = col(&side(node.name()));
    let right = col(&mate(node.name(), bond, target));
    let mut cols = vec![
        format!("{KEY} INTEGER PRIMARY KEY NOT NULL"),
        format!("{left} INTEGER NOT NULL"),
        format!("{right} INTEGER NOT NULL"),
    ];
    for slot in edge.fields() {
        cols.push(format!(
            "{} {} NOT NULL",
            col(slot.name()),
            cast(slot.kind())
        ));
    }
    stamp(node.reign(), &mut cols);
    format!(
        "CREATE TABLE IF NOT EXISTS {} ({});",
        joint(node.name(), bond),
        cols.join(", ")
    )
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

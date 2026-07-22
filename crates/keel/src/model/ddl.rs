use crate::atom;
use crate::bond;
use crate::plan::{Plan, Reign, Slot, Unit};
use crate::spec::Only;

pub const KEY: &str = "id";
pub const EXPIRES: &str = "expires_at";
pub const CREATED: &str = "created_at";
pub const UPDATED: &str = "updated_at";

pub fn table(name: &str, root: Option<&str>) -> String {
    match root {
        Some(root) => format!(
            "{}_{}",
            root.to_ascii_lowercase(),
            name.to_ascii_lowercase()
        ),
        None => name.to_ascii_lowercase(),
    }
}

pub fn join(unit: &Unit, bond: &str) -> String {
    format!("{}_{}", unit.table(), bond.to_ascii_lowercase())
}

pub fn side(name: &str) -> String {
    format!("{}_id", name.to_ascii_lowercase())
}

pub fn col(name: &str) -> String {
    format!("\"{name}\"")
}

pub fn seat(unit: &Unit) -> String {
    col(&unit.table())
}

pub fn joint(unit: &Unit, bond: &str) -> String {
    col(&join(unit, bond))
}

pub fn mate(owner: &str, bond: &str, target: &str) -> String {
    if owner.eq_ignore_ascii_case(target) {
        return side(bond);
    }
    side(target)
}

#[derive(Clone, Copy, PartialEq)]
pub enum Grain {
    Lite,
    Pg,
}

fn stub(grain: Grain) -> &'static str {
    match grain {
        Grain::Lite => "INTEGER PRIMARY KEY NOT NULL",
        Grain::Pg => "BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY",
    }
}

fn whole(grain: Grain) -> &'static str {
    match grain {
        Grain::Lite => "INTEGER",
        Grain::Pg => "BIGINT",
    }
}

pub fn script(plan: &Plan, grain: Grain) -> Vec<String> {
    let mut out = Vec::new();
    if grain == Grain::Lite {
        out.push("PRAGMA foreign_keys = ON;".into());
    }
    for node in plan.units().values() {
        out.push(form(node, grain));
    }
    for node in plan.units().values() {
        for bond in node.bonds() {
            if bond.kind() == bond::Kind::Many2many {
                out.push(arc(node, bond.name(), bond.target(), grain));
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
    let place = node.table();
    let scopes: Vec<String> = match slot.only() {
        Only::Per(rels) => rels.iter().map(|rel| col(&side(rel))).collect(),
        _ => slot
            .serial()
            .map(|rel| col(&side(rel)))
            .into_iter()
            .collect(),
    };
    let mut parts = scopes;
    parts.push(col(slot.name()));
    let cols = parts.join(", ");
    format!(
        "CREATE UNIQUE INDEX IF NOT EXISTS only_{place}_{} ON {} ({cols}) WHERE {EXPIRES} IS NULL;",
        slot.name(),
        col(&place)
    )
}

fn form(node: &Unit, grain: Grain) -> String {
    let mut cols = vec![format!("{KEY} {}", stub(grain))];
    for slot in node.fields() {
        cols.push(format!(
            "{} {} NOT NULL",
            col(slot.name()),
            cast(slot.kind(), grain)
        ));
    }
    for edge in node.bonds() {
        if edge.kind().point() {
            let null = if edge.need() { " NOT NULL" } else { "" };
            cols.push(format!(
                "{} {}{}",
                col(&side(edge.name())),
                whole(grain),
                null
            ));
        }
    }
    stamp(node.reign(), &mut cols, grain);
    format!(
        "CREATE TABLE IF NOT EXISTS {} ({});",
        seat(node),
        cols.join(", ")
    )
}

fn arc(node: &Unit, bond: &str, target: &str, grain: Grain) -> String {
    let edge = node
        .bonds()
        .iter()
        .find(|edge| edge.name() == bond)
        .expect("bond");
    let left = col(&side(node.name()));
    let right = col(&mate(node.name(), bond, target));
    let mut cols = vec![
        format!("{KEY} {}", stub(grain)),
        format!("{left} {} NOT NULL", whole(grain)),
        format!("{right} {} NOT NULL", whole(grain)),
    ];
    for slot in edge.fields() {
        cols.push(format!(
            "{} {} NOT NULL",
            col(slot.name()),
            cast(slot.kind(), grain)
        ));
    }
    stamp(node.reign(), &mut cols, grain);
    format!(
        "CREATE TABLE IF NOT EXISTS {} ({});",
        joint(node, bond),
        cols.join(", ")
    )
}

fn stamp(reign: &Reign, cols: &mut Vec<String>, grain: Grain) {
    let kind = whole(grain);
    if reign.expires() {
        cols.push(format!("{EXPIRES} {kind}"));
    }
    if reign.created() {
        cols.push(format!("{CREATED} {kind} NOT NULL"));
    }
    if reign.updated() {
        cols.push(format!("{UPDATED} {kind} NOT NULL"));
    }
}

fn cast(kind: atom::Kind, grain: Grain) -> &'static str {
    match kind {
        atom::Kind::Text | atom::Kind::Link => "TEXT",
        atom::Kind::Int | atom::Kind::Bool => whole(grain),
    }
}

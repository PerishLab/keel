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
    joiner(&unit.table(), bond)
}

pub(crate) fn joiner(table: &str, bond: &str) -> String {
    format!("{}_{}", table, bond.to_ascii_lowercase())
}

pub fn side(name: &str) -> String {
    format!("{}_id", name.to_ascii_lowercase().replace(':', "_"))
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
        Grain::Pg => "BIGINT PRIMARY KEY NOT NULL",
    }
}

fn whole(grain: Grain) -> &'static str {
    match grain {
        Grain::Lite => "INTEGER",
        Grain::Pg => "BIGINT",
    }
}

pub fn script(plan: &Plan, grain: Grain) -> Vec<String> {
    project(plan, grain, None)
}

pub(crate) fn candidate(plan: &Plan, grain: Grain, generation: i64) -> Vec<String> {
    project(plan, grain, Some(generation))
}

fn project(plan: &Plan, grain: Grain, generation: Option<i64>) -> Vec<String> {
    let mut out = Vec::new();
    if grain == Grain::Lite {
        out.push("PRAGMA foreign_keys = ON;".into());
    }
    let units: Vec<&Unit> = plan
        .units()
        .values()
        .filter(|unit| generation.is_none() || !unit.name().starts_with('@'))
        .collect();
    for node in &units {
        let place = place(&node.table(), generation);
        out.push(form(node, &place, grain));
    }
    for node in &units {
        for bond in node.bonds() {
            if bond.kind() == bond::Kind::Many2many {
                let joint = place(&join(node, bond.name()), generation);
                out.push(arc(node, bond.name(), &joint, grain));
            }
        }
    }
    for node in units {
        for slot in node.fields() {
            if *slot.only() != Only::Free || slot.serial().is_some() {
                let place = place(&node.table(), generation);
                out.push(lock(node, slot, &place, generation));
            }
        }
    }
    out
}

fn lock(node: &Unit, slot: &Slot, place: &str, generation: Option<i64>) -> String {
    let scopes: Vec<String> = match slot.only() {
        Only::Per(fields) => fields
            .iter()
            .map(|field| node.column(field).expect("validated unique scope"))
            .collect(),
        _ => slot
            .serial()
            .map(|rel| col(&side(rel)))
            .into_iter()
            .collect(),
    };
    let mut parts = scopes;
    parts.push(col(slot.name()));
    let cols = parts.join(", ");
    let mark = generation.map(|id| format!("g{id}_")).unwrap_or_default();
    let index = col(&format!("only_{mark}{}_{}", node.table(), slot.name()));
    format!(
        "CREATE UNIQUE INDEX IF NOT EXISTS {index} ON {} ({cols}) WHERE {EXPIRES} IS NULL;",
        col(place)
    )
}

fn form(node: &Unit, place: &str, grain: Grain) -> String {
    let mut cols = vec![format!("{KEY} {}", stub(grain))];
    for slot in node.fields() {
        let null = if slot.need() { " NOT NULL" } else { "" };
        let default = slot
            .rule()
            .fallback()
            .map(|value| format!(" DEFAULT {}", literal(slot.kind(), value)))
            .unwrap_or_default();
        cols.push(format!(
            "{} {}{}{}{}",
            col(slot.name()),
            cast(slot.kind(), grain),
            null,
            default,
            checks(slot)
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
        col(place),
        cols.join(", ")
    )
}

fn arc(node: &Unit, bond: &str, joint: &str, grain: Grain) -> String {
    let edge = node
        .bonds()
        .iter()
        .find(|edge| edge.name() == bond)
        .expect("bond");
    let left = col(&side(node.name()));
    let right = col(&mate(node.name(), bond, edge.target()));
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
        col(joint),
        cols.join(", ")
    )
}

pub(crate) fn stage(generation: i64, table: &str) -> String {
    format!("@g{generation}:{table}")
}

fn place(table: &str, generation: Option<i64>) -> String {
    generation.map_or_else(|| table.into(), |id| stage(id, table))
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

fn checks(slot: &Slot) -> String {
    let rule = slot.rule();
    let name = col(slot.name());
    let mut checks = Vec::new();
    if !rule.admitted().is_empty() {
        let values = rule
            .admitted()
            .iter()
            .map(|value| literal(slot.kind(), value))
            .collect::<Vec<_>>()
            .join(", ");
        checks.push(format!("{name} IN ({values})"));
    }
    if let Some(min) = rule.minimum() {
        checks.push(format!("{name} >= {min}"));
    }
    if let Some(max) = rule.maximum() {
        checks.push(format!("{name} <= {max}"));
    }
    checks
        .into_iter()
        .map(|check| format!(" CHECK ({check})"))
        .collect()
}

fn literal(kind: atom::Kind, value: &str) -> String {
    match kind {
        atom::Kind::Text | atom::Kind::Link => format!("'{}'", value.replace('\'', "''")),
        atom::Kind::Int => value.to_string(),
        atom::Kind::Bool => match value {
            "true" => "1".into(),
            "false" => "0".into(),
            _ => unreachable!("normalized bool"),
        },
    }
}

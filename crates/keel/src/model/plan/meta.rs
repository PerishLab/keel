use super::{Edge, Reign, Slot, Unit};
use crate::atom;
use crate::bond;
use crate::model::manifest::rows::{BOND, FIELD, SCOPE, UNIT, VALUE};
use crate::spec::{Only, Rule};

pub(crate) fn all() -> [Unit; 5] {
    [unit(), bond(), field(), scope(), value()]
}

pub(crate) fn owned(unit: &str) -> bool {
    [UNIT, BOND, FIELD, SCOPE, VALUE].contains(&unit)
}

fn unit() -> Unit {
    born(
        UNIT,
        vec![
            slot("key", atom::Kind::Text, true),
            slot("name", atom::Kind::Text, true),
            slot("veil", atom::Kind::Bool, true),
            slot("frozen", atom::Kind::Bool, true),
            slot("generation", atom::Kind::Int, true),
        ],
        Vec::new(),
    )
}

fn bond() -> Unit {
    born(
        BOND,
        vec![
            slot("name", atom::Kind::Text, true),
            slot("kind", atom::Kind::Text, true),
            slot("target", atom::Kind::Text, true),
            slot("need", atom::Kind::Bool, true),
            slot("root", atom::Kind::Bool, true),
            slot("crew", atom::Kind::Bool, true),
        ],
        vec![edge("unit", UNIT, true)],
    )
}

fn field() -> Unit {
    born(
        FIELD,
        vec![
            slot("name", atom::Kind::Text, true),
            slot("kind", atom::Kind::Text, true),
            slot("only", atom::Kind::Text, true),
            slot("need", atom::Kind::Bool, true),
            slot("serial", atom::Kind::Text, false),
            slot("fallback", atom::Kind::Text, false),
            slot("min", atom::Kind::Int, false),
            slot("max", atom::Kind::Int, false),
        ],
        vec![edge("unit", UNIT, true), edge("bond", BOND, false)],
    )
}

fn scope() -> Unit {
    born(
        SCOPE,
        vec![slot("scope", atom::Kind::Text, true)],
        vec![edge("field", FIELD, true)],
    )
}

fn value() -> Unit {
    born(
        VALUE,
        vec![slot("value", atom::Kind::Text, true)],
        vec![edge("field", FIELD, true)],
    )
}

fn born(name: &str, fields: Vec<Slot>, bonds: Vec<Edge>) -> Unit {
    Unit {
        name: name.into(),
        fields,
        bonds,
        reign: Reign::engine(),
        veil: false,
        frozen: false,
    }
}

fn slot(name: &str, kind: atom::Kind, need: bool) -> Slot {
    Slot {
        name: name.into(),
        kind,
        only: Only::Free,
        serial: None,
        need,
        rule: Rule::new(),
    }
}

fn edge(name: &str, target: &str, root: bool) -> Edge {
    Edge {
        name: name.into(),
        kind: bond::Kind::Many2one,
        target: target.into(),
        fields: Vec::new(),
        need: root,
        root,
        crew: false,
    }
}

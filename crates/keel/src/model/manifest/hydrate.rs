use super::{Atom, Bond, Edge, Field, Limit, Manifest, Rule, Unit};
use crate::graph::Graph;
use crate::spec::{Builder, Only, Spec};
use crate::{atom, bond};

pub(crate) fn read(text: &str) -> Result<Graph, crate::adapt::Error> {
    let held = Manifest::read(text).map_err(crate::adapt::Error::Adapt)?;
    Ok(lift(&held))
}

pub(crate) fn lift(manifest: &Manifest) -> Graph {
    let mut graph = Graph::new();
    for unit in manifest.units() {
        graph.add(grow(unit));
    }
    graph
}

fn grow(unit: &Unit) -> Spec {
    let mut held = Spec::build(&unit.name);
    for field in &unit.fields {
        held = sow(held, field);
    }
    for edge in &unit.bonds {
        held = tie(held, edge);
    }
    if unit.veil {
        held = held.veil();
    }
    if unit.frozen {
        held = held.freeze();
    }
    held.seal()
}

fn sow(held: Builder, field: &Field) -> Builder {
    let kind = sort(field.kind);
    let held = match (&field.serial, &field.only, field.need) {
        (Some(scope), _, _) => return held.serial(&field.name, scope),
        (None, Limit::Free, true) => held.field(&field.name, kind),
        (None, Limit::All, true) => held.sole(&field.name, kind),
        (None, Limit::Per(scopes), true) => {
            let scopes: Vec<&str> = scopes.iter().map(String::as_str).collect();
            held.per(&field.name, kind, &scopes)
        }
        (None, only, false) => held.optional(&field.name, kind, bound(only)),
    };
    match law(&field.rule) {
        Some(rule) => held.rule(&field.name, rule),
        None => held,
    }
}

fn tie(held: Builder, edge: &Edge) -> Builder {
    let kind = link(edge.kind);
    if edge.root {
        return held.root(&edge.name, kind, &edge.target);
    }
    if edge.crew {
        return held.crew(&edge.name, kind, &edge.target);
    }
    if !edge.need {
        return held.free(&edge.name, kind, &edge.target);
    }
    let fields: Vec<(&str, atom::Kind)> = edge
        .fields
        .iter()
        .map(|field| (field.name.as_str(), sort(field.kind)))
        .collect();
    held.bond(&edge.name, kind, &edge.target, &fields)
}

fn law(rule: &Rule) -> Option<crate::spec::Rule> {
    if rule.default.is_none() && rule.values.is_empty() && rule.min.is_none() && rule.max.is_none()
    {
        return None;
    }
    let mut held = crate::spec::Rule::new();
    if let Some(value) = &rule.default {
        held = held.default(value);
    }
    if !rule.values.is_empty() {
        let values: Vec<&str> = rule.values.iter().map(String::as_str).collect();
        held = held.values(&values);
    }
    if let Some(value) = rule.min {
        held = held.min(value);
    }
    if let Some(value) = rule.max {
        held = held.max(value);
    }
    Some(held)
}

fn bound(only: &Limit) -> Only {
    match only {
        Limit::Free => Only::Free,
        Limit::All => Only::All,
        Limit::Per(scopes) => Only::Per(scopes.clone()),
    }
}

fn sort(kind: Atom) -> atom::Kind {
    match kind {
        Atom::Text => atom::Kind::Text,
        Atom::Link => atom::Kind::Link,
        Atom::Int => atom::Kind::Int,
        Atom::Bool => atom::Kind::Bool,
    }
}

fn link(kind: Bond) -> bond::Kind {
    match kind {
        Bond::Many2many => bond::Kind::Many2many,
        Bond::Many2one => bond::Kind::Many2one,
        Bond::One2one => bond::Kind::One2one,
    }
}

use crate::atom;
use crate::bond;
use crate::graph::Graph;
use crate::spec::{Only, Spec};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct Plan {
    units: BTreeMap<String, Unit>,
}

#[derive(Clone, Debug)]
pub struct Unit {
    name: String,
    fields: Vec<Slot>,
    bonds: Vec<Edge>,
    reign: Reign,
}

#[derive(Clone, Debug)]
pub struct Slot {
    name: String,
    kind: atom::Kind,
    only: Only,
    serial: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Edge {
    name: String,
    kind: bond::Kind,
    target: String,
    fields: Vec<Slot>,
    need: bool,
    root: bool,
}

#[derive(Clone, Debug)]
pub struct Reign {
    expires: bool,
    created: bool,
    updated: bool,
}

impl Plan {
    pub(crate) fn lift(graph: &Graph) -> Result<Self, crate::adapt::Error> {
        let mut units = BTreeMap::new();
        for (name, spec) in graph.nodes() {
            units.insert(name.clone(), Unit::lift(spec)?);
        }
        for unit in units.values() {
            for edge in &unit.bonds {
                if !units.contains_key(&edge.target) {
                    return Err(crate::adapt::Error::Missing(edge.target.clone()));
                }
            }
        }
        units.insert(crate::cap::GRANT.into(), Unit::grant());
        Ok(Self { units })
    }

    pub fn units(&self) -> &BTreeMap<String, Unit> {
        &self.units
    }
}

impl Unit {
    fn grant() -> Self {
        let fields = ["who", "verb", "unit", "scope"]
            .iter()
            .map(|name| Slot {
                name: (*name).to_string(),
                kind: atom::Kind::Text,
                only: Only::Free,
                serial: None,
            })
            .collect();
        Self {
            name: crate::cap::GRANT.into(),
            fields,
            bonds: Vec::new(),
            reign: Reign::engine(),
        }
    }

    fn lift(spec: &Spec) -> Result<Self, crate::adapt::Error> {
        let fields: Vec<Slot> = spec
            .fields()
            .iter()
            .map(|field| Slot {
                name: field.name().to_string(),
                kind: field.kind(),
                only: field.only().clone(),
                serial: field.serial().map(str::to_string),
            })
            .collect();
        let bonds: Vec<Edge> = spec
            .bonds()
            .iter()
            .map(|bond| Edge {
                name: bond.name().to_string(),
                kind: bond.kind(),
                target: bond.target().to_string(),
                fields: bond
                    .fields()
                    .iter()
                    .map(|field| Slot {
                        name: field.name().to_string(),
                        kind: field.kind(),
                        only: Only::Free,
                        serial: None,
                    })
                    .collect(),
                need: bond.need(),
                root: bond.root(),
            })
            .collect();
        let roots = bonds.iter().filter(|e| e.root).count();
        if roots > 1 {
            return Err(crate::adapt::Error::Adapt("unit has two roots".into()));
        }
        if bonds.iter().any(|e| e.root && !(e.kind.point() && e.need)) {
            return Err(crate::adapt::Error::Adapt(
                "root must be a required ref".into(),
            ));
        }
        for slot in &fields {
            let scope = match slot.only() {
                Only::Per(scope) => Some(scope.as_str()),
                _ => slot.serial.as_deref(),
            };
            let Some(scope) = scope else {
                continue;
            };
            let held = bonds.iter().any(|e| e.kind().point() && e.name() == scope);
            if !held {
                return Err(crate::adapt::Error::Adapt(format!(
                    "unique scope {scope} is not a ref"
                )));
            }
        }
        Ok(Self {
            name: spec.name().to_string(),
            fields,
            bonds,
            reign: Reign::engine(),
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn fields(&self) -> &[Slot] {
        &self.fields
    }

    pub fn bonds(&self) -> &[Edge] {
        &self.bonds
    }

    pub fn root(&self) -> Option<&Edge> {
        self.bonds.iter().find(|e| e.root)
    }

    pub fn reign(&self) -> &Reign {
        &self.reign
    }
}

impl Slot {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn kind(&self) -> atom::Kind {
        self.kind
    }

    pub fn only(&self) -> &Only {
        &self.only
    }

    pub fn serial(&self) -> Option<&str> {
        self.serial.as_deref()
    }
}

impl Edge {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn kind(&self) -> bond::Kind {
        self.kind
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn fields(&self) -> &[Slot] {
        &self.fields
    }

    pub fn need(&self) -> bool {
        self.need
    }

    pub fn root(&self) -> bool {
        self.root
    }
}

impl Reign {
    fn engine() -> Self {
        Self {
            expires: true,
            created: true,
            updated: true,
        }
    }

    pub fn expires(&self) -> bool {
        self.expires
    }

    pub fn created(&self) -> bool {
        self.created
    }

    pub fn updated(&self) -> bool {
        self.updated
    }
}

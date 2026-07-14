use crate::atom;
use crate::bond;
use crate::graph::Graph;
use crate::spec::Spec;
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
}

#[derive(Clone, Debug)]
pub struct Edge {
    name: String,
    kind: bond::Kind,
    target: String,
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
        Ok(Self { units })
    }

    pub fn units(&self) -> &BTreeMap<String, Unit> {
        &self.units
    }
}

impl Unit {
    fn lift(spec: &Spec) -> Result<Self, crate::adapt::Error> {
        let fields = spec
            .fields()
            .iter()
            .map(|field| Slot {
                name: field.name().to_string(),
                kind: field.kind(),
            })
            .collect();
        let bonds = spec
            .bonds()
            .iter()
            .map(|bond| Edge {
                name: bond.name().to_string(),
                kind: bond.kind(),
                target: bond.target().to_string(),
            })
            .collect();
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

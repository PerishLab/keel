use crate::atom;
use crate::bond;
use crate::graph::Graph;
use crate::spec::Only;
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
    veil: bool,
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
    crew: bool,
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
        units.insert(crate::cap::SEAL.into(), Unit::seal());
        units.insert(crate::cap::PULSE.into(), Unit::pulse());
        Ok(Self { units })
    }

    pub fn units(&self) -> &BTreeMap<String, Unit> {
        &self.units
    }

    pub fn veiled(&self, name: &str) -> bool {
        self.units.get(name).is_some_and(Unit::veil)
    }

    pub fn shrouds(&self, tables: &[String]) -> bool {
        self.units
            .values()
            .any(|unit| unit.veil() && tables.contains(&crate::ddl::table(unit.name())))
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

    pub fn crew(&self) -> bool {
        self.crew
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

mod unit;

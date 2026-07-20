use super::*;
use crate::atom;
use crate::spec::{Only, Spec};

impl Unit {
    pub(crate) fn grant() -> Self {
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
            veil: false,
        }
    }

    pub(crate) fn pulse() -> Self {
        let mut fields: Vec<Slot> = ["verb", "unit", "who"]
            .iter()
            .map(|name| Slot {
                name: (*name).to_string(),
                kind: atom::Kind::Text,
                only: Only::Free,
                serial: None,
            })
            .collect();
        fields.push(Slot {
            name: "key".into(),
            kind: atom::Kind::Int,
            only: Only::Free,
            serial: None,
        });
        Self {
            name: crate::cap::PULSE.into(),
            fields,
            bonds: Vec::new(),
            reign: Reign::engine(),
            veil: false,
        }
    }

    pub(crate) fn seal() -> Self {
        Self {
            name: crate::cap::SEAL.into(),
            fields: vec![Slot {
                name: "hash".into(),
                kind: atom::Kind::Text,
                only: Only::Free,
                serial: None,
            }],
            bonds: Vec::new(),
            reign: Reign::engine(),
            veil: false,
        }
    }

    pub(crate) fn lift(spec: &Spec) -> Result<Self, crate::adapt::Error> {
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
                crew: bond.crew(),
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
            let scopes: Vec<&str> = match slot.only() {
                Only::Per(scopes) => scopes.iter().map(String::as_str).collect(),
                _ => slot.serial.as_deref().into_iter().collect(),
            };
            for scope in scopes {
                let held = bonds.iter().any(|e| e.kind().point() && e.name() == scope);
                if !held {
                    return Err(crate::adapt::Error::Adapt(format!(
                        "unique scope {scope} is not a ref"
                    )));
                }
            }
        }
        Ok(Self {
            name: spec.name().to_string(),
            fields,
            bonds,
            reign: Reign::engine(),
            veil: spec.veiled(),
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

    pub fn crew(&self) -> Option<&Edge> {
        self.bonds.iter().find(|e| e.crew)
    }

    pub fn reign(&self) -> &Reign {
        &self.reign
    }

    pub fn veil(&self) -> bool {
        self.veil
    }
}

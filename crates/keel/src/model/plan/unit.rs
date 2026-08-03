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
                need: true,
                rule: crate::spec::Rule::new(),
            })
            .collect();
        Self {
            name: crate::cap::GRANT.into(),
            fields,
            bonds: Vec::new(),
            reign: Reign::engine(),
            veil: false,
            frozen: false,
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
                need: true,
                rule: crate::spec::Rule::new(),
            })
            .collect();
        fields.push(Slot {
            name: "key".into(),
            kind: atom::Kind::Int,
            only: Only::Free,
            serial: None,
            need: true,
            rule: crate::spec::Rule::new(),
        });
        Self {
            name: crate::cap::PULSE.into(),
            fields,
            bonds: Vec::new(),
            reign: Reign::engine(),
            veil: false,
            frozen: false,
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
                need: true,
                rule: crate::spec::Rule::new(),
            }],
            bonds: Vec::new(),
            reign: Reign::engine(),
            veil: false,
            frozen: false,
        }
    }

    pub(crate) fn lift(spec: &Spec) -> Result<Self, crate::adapt::Error> {
        if let Some(fault) = spec.faults().first() {
            return Err(crate::adapt::Error::Adapt(fault.clone()));
        }
        let mut names = std::collections::BTreeSet::new();
        for field in spec.fields() {
            let name = field.name().to_ascii_lowercase();
            if !names.insert(name) {
                return Err(crate::adapt::Error::Adapt(format!(
                    "duplicate field {}",
                    field.name()
                )));
            }
            if matches!(field.only(), Only::Per(scopes) if scopes.is_empty()) {
                return Err(crate::adapt::Error::Adapt(format!(
                    "empty unique scope {}",
                    field.name()
                )));
            }
        }
        for bond in spec.bonds() {
            let name = bond.name().to_ascii_lowercase();
            if !names.insert(name) {
                return Err(crate::adapt::Error::Adapt(format!(
                    "duplicate field {}",
                    bond.name()
                )));
            }
            let mut fields = std::collections::BTreeSet::new();
            for field in bond.fields() {
                if !fields.insert(field.name().to_ascii_lowercase()) {
                    return Err(crate::adapt::Error::Adapt(format!(
                        "duplicate bond field {}",
                        field.name()
                    )));
                }
            }
        }
        let fields: Vec<Slot> = spec
            .fields()
            .iter()
            .map(|field| {
                Ok(Slot {
                    name: field.name().to_string(),
                    kind: field.kind(),
                    only: field.only().clone(),
                    serial: field.serial().map(str::to_string),
                    need: field.need(),
                    rule: field.rule().normalize(field.kind(), field.need())?,
                })
            })
            .collect::<Result<_, crate::adapt::Error>>()?;
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
                        need: true,
                        rule: crate::spec::Rule::new(),
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
        if bonds.iter().filter(|e| e.crew).count() > 1 {
            return Err(crate::adapt::Error::Adapt("unit has two crews".into()));
        }
        super::mode::check(spec.frozen(), &bonds)?;
        super::scope::check(&fields, &bonds)?;
        Ok(Self {
            name: spec.name().to_string(),
            fields,
            bonds,
            reign: Reign::engine(),
            veil: spec.veiled(),
            frozen: spec.frozen(),
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

    pub fn stem(&self) -> Option<&str> {
        self.root().map(Edge::target)
    }

    pub fn key(&self) -> String {
        crate::name::key(&self.name, self.stem())
    }

    pub fn table(&self) -> String {
        crate::ddl::table(&self.name, self.stem())
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

    pub fn frozen(&self) -> bool {
        self.frozen
    }

    pub(crate) fn refs(&self) -> impl Iterator<Item = &Edge> {
        self.bonds.iter().filter(|edge| edge.kind().point())
    }

    pub(crate) fn knows(&self, name: &str) -> bool {
        self.fields.iter().any(|s| s.name() == name) || self.refs().any(|e| e.name() == name)
    }

    pub(crate) fn column(&self, name: &str) -> Result<String, crate::adapt::Error> {
        if self.fields.iter().any(|slot| slot.name() == name) {
            return Ok(crate::ddl::col(name));
        }
        if self.refs().any(|edge| edge.name() == name) {
            return Ok(crate::ddl::col(&crate::ddl::side(name)));
        }
        Err(crate::adapt::Error::Adapt(format!(
            "unknown scope field {name}"
        )))
    }

    pub(crate) fn kind(&self, field: &str) -> Result<atom::Kind, crate::adapt::Error> {
        if field == crate::ddl::KEY {
            return Ok(atom::Kind::Int);
        }
        if let Some(slot) = self.fields.iter().find(|slot| slot.name() == field) {
            return Ok(slot.kind());
        }
        if self
            .bonds
            .iter()
            .any(|e| e.kind().point() && e.name() == field)
        {
            return Ok(atom::Kind::Int);
        }
        Err(crate::adapt::Error::Adapt(format!("unknown field {field}")))
    }

    pub(crate) fn sheet(&self) -> String {
        let mut cols = vec![crate::ddl::KEY.to_string()];
        for slot in &self.fields {
            cols.push(crate::ddl::col(slot.name()));
        }
        for edge in self.refs() {
            cols.push(crate::ddl::col(&crate::ddl::side(edge.name())));
        }
        cols.push(crate::ddl::EXPIRES.to_string());
        cols.push(crate::ddl::CREATED.to_string());
        cols.push(crate::ddl::UPDATED.to_string());
        cols.join(", ")
    }

    pub(crate) fn part(&self, fields: &[(&str, &str)]) -> Result<(), crate::adapt::Error> {
        if fields.is_empty() {
            return Err(crate::adapt::Error::Adapt("empty set".into()));
        }
        const CONTROL: [&str; 4] = [
            crate::ddl::KEY,
            crate::ddl::EXPIRES,
            crate::ddl::CREATED,
            crate::ddl::UPDATED,
        ];
        for (k, _) in fields {
            if CONTROL.contains(k) {
                return Err(crate::adapt::Error::Adapt(format!("control field {k}")));
            }
            let held = self
                .fields
                .iter()
                .any(|s| s.name() == *k && s.serial().is_some());
            if held {
                return Err(crate::adapt::Error::Adapt(format!("serial field {k}")));
            }
            if !self.knows(k) {
                return Err(crate::adapt::Error::Adapt(format!("unknown field {k}")));
            }
        }
        Ok(())
    }
}

use super::super::spec::*;
use crate::{atom, bond};

impl Builder {
    pub fn field(self, name: impl Into<String>, kind: atom::Kind) -> Self {
        self.scalar(name, kind, Only::Free, true)
    }

    pub fn optional(self, name: impl Into<String>, kind: atom::Kind, only: Only) -> Self {
        self.scalar(name, kind, only, false)
    }

    pub fn sole(self, name: impl Into<String>, kind: atom::Kind) -> Self {
        self.scalar(name, kind, Only::All, true)
    }

    pub fn per(self, name: impl Into<String>, kind: atom::Kind, scope: &[&str]) -> Self {
        self.scalar(
            name,
            kind,
            Only::Per(scope.iter().map(|s| s.to_string()).collect()),
            true,
        )
    }

    fn scalar(mut self, name: impl Into<String>, kind: atom::Kind, only: Only, need: bool) -> Self {
        self.fields.push(Field {
            name: name.into(),
            kind,
            only,
            serial: None,
            need,
            rule: Rule::new(),
        });
        self
    }

    pub fn rule(mut self, name: &str, rule: Rule) -> Self {
        match self.fields.iter_mut().find(|field| field.name == name) {
            Some(field) if field.serial.is_none() => field.rule = rule,
            Some(_) => self.faults.push(format!("serial field {name} has rule")),
            None => self.faults.push(format!("unknown rule field {name}")),
        }
        self
    }

    pub fn serial(mut self, name: impl Into<String>, scope: impl Into<String>) -> Self {
        self.fields.push(Field {
            name: name.into(),
            kind: atom::Kind::Int,
            only: Only::Free,
            serial: Some(scope.into()),
            need: true,
            rule: Rule::new(),
        });
        self
    }

    pub fn bond(
        self,
        name: impl Into<String>,
        kind: bond::Kind,
        target: impl Into<String>,
        fields: &[(&str, atom::Kind)],
    ) -> Self {
        self.join(
            Wale {
                name: name.into(),
                kind,
                target: target.into(),
                cast: Cast::Bond,
            },
            fields,
        )
    }

    pub fn free(
        self,
        name: impl Into<String>,
        kind: bond::Kind,
        target: impl Into<String>,
    ) -> Self {
        self.join(
            Wale {
                name: name.into(),
                kind,
                target: target.into(),
                cast: Cast::Free,
            },
            &[],
        )
    }

    pub fn root(
        self,
        name: impl Into<String>,
        kind: bond::Kind,
        target: impl Into<String>,
    ) -> Self {
        self.join(
            Wale {
                name: name.into(),
                kind,
                target: target.into(),
                cast: Cast::Root,
            },
            &[],
        )
    }

    pub fn crew(
        self,
        name: impl Into<String>,
        kind: bond::Kind,
        target: impl Into<String>,
    ) -> Self {
        self.join(
            Wale {
                name: name.into(),
                kind,
                target: target.into(),
                cast: Cast::Crew,
            },
            &[],
        )
    }

    fn join(mut self, wale: Wale, fields: &[(&str, atom::Kind)]) -> Self {
        let Wale {
            name,
            kind,
            target,
            cast,
        } = wale;
        let fields = fields
            .iter()
            .map(|(n, k)| Field {
                name: (*n).to_string(),
                kind: *k,
                only: Only::Free,
                serial: None,
                need: true,
                rule: Rule::new(),
            })
            .collect();
        self.bonds.push(Bond {
            name,
            kind,
            target,
            fields,
            need: cast != Cast::Free,
            root: cast == Cast::Root,
            crew: cast == Cast::Crew,
        });
        self
    }

    pub fn veil(mut self) -> Self {
        self.veil = true;
        self
    }

    pub fn freeze(mut self) -> Self {
        self.frozen = true;
        self
    }

    pub fn seal(self) -> Spec {
        Spec {
            name: self.name,
            fields: self.fields,
            bonds: self.bonds,
            veil: self.veil,
            frozen: self.frozen,
            faults: self.faults,
        }
    }
}

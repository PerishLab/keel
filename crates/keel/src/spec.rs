use crate::atom;
use crate::bond;

pub trait Resource {
    fn name() -> &'static str;
    fn spec() -> Spec;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Spec {
    name: String,
    fields: Vec<Field>,
    bonds: Vec<Bond>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Field {
    name: String,
    kind: atom::Kind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Bond {
    name: String,
    kind: bond::Kind,
    target: String,
    fields: Vec<Field>,
}

pub struct Builder {
    name: String,
    fields: Vec<Field>,
    bonds: Vec<Bond>,
}

impl Spec {
    pub fn build(name: impl Into<String>) -> Builder {
        Builder {
            name: name.into(),
            fields: Vec::new(),
            bonds: Vec::new(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn fields(&self) -> &[Field] {
        &self.fields
    }

    pub fn bonds(&self) -> &[Bond] {
        &self.bonds
    }
}

impl Field {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn kind(&self) -> atom::Kind {
        self.kind
    }
}

impl Bond {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn kind(&self) -> bond::Kind {
        self.kind
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn fields(&self) -> &[Field] {
        &self.fields
    }
}

impl Builder {
    pub fn field(mut self, name: impl Into<String>, kind: atom::Kind) -> Self {
        self.fields.push(Field {
            name: name.into(),
            kind,
        });
        self
    }

    pub fn bond(
        mut self,
        name: impl Into<String>,
        kind: bond::Kind,
        target: impl Into<String>,
        fields: &[(&str, atom::Kind)],
    ) -> Self {
        let fields = fields
            .iter()
            .map(|(n, k)| Field {
                name: (*n).to_string(),
                kind: *k,
            })
            .collect();
        self.bonds.push(Bond {
            name: name.into(),
            kind,
            target: target.into(),
            fields,
        });
        self
    }

    pub fn seal(self) -> Spec {
        Spec {
            name: self.name,
            fields: self.fields,
            bonds: self.bonds,
        }
    }
}

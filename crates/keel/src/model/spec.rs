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
    veil: bool,
}

struct Wale {
    name: String,
    kind: bond::Kind,
    target: String,
    cast: Cast,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Cast {
    Bond,
    Free,
    Root,
    Crew,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Only {
    Free,
    All,
    Per(Vec<String>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Field {
    name: String,
    kind: atom::Kind,
    only: Only,
    serial: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Bond {
    name: String,
    kind: bond::Kind,
    target: String,
    fields: Vec<Field>,
    need: bool,
    root: bool,
    crew: bool,
}

pub struct Builder {
    name: String,
    fields: Vec<Field>,
    bonds: Vec<Bond>,
    veil: bool,
}

impl Spec {
    pub fn build(name: impl Into<String>) -> Builder {
        Builder {
            name: name.into(),
            fields: Vec::new(),
            bonds: Vec::new(),
            veil: false,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn stem(&self) -> Option<&str> {
        self.bonds.iter().find(|bond| bond.root()).map(Bond::target)
    }

    pub fn key(&self) -> String {
        crate::name::key(&self.name, self.stem())
    }

    pub fn veiled(&self) -> bool {
        self.veil
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

    pub fn only(&self) -> &Only {
        &self.only
    }

    pub fn serial(&self) -> Option<&str> {
        self.serial.as_deref()
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

impl Builder {
    pub fn field(mut self, name: impl Into<String>, kind: atom::Kind) -> Self {
        self.fields.push(Field {
            name: name.into(),
            kind,
            only: Only::Free,
            serial: None,
        });
        self
    }

    pub fn sole(mut self, name: impl Into<String>, kind: atom::Kind) -> Self {
        self.fields.push(Field {
            name: name.into(),
            kind,
            only: Only::All,
            serial: None,
        });
        self
    }

    pub fn per(mut self, name: impl Into<String>, kind: atom::Kind, scope: &[&str]) -> Self {
        self.fields.push(Field {
            name: name.into(),
            kind,
            only: Only::Per(scope.iter().map(|s| s.to_string()).collect()),
            serial: None,
        });
        self
    }

    pub fn serial(mut self, name: impl Into<String>, scope: impl Into<String>) -> Self {
        self.fields.push(Field {
            name: name.into(),
            kind: atom::Kind::Int,
            only: Only::Free,
            serial: Some(scope.into()),
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

    pub fn seal(self) -> Spec {
        Spec {
            name: self.name,
            fields: self.fields,
            bonds: self.bonds,
            veil: self.veil,
        }
    }
}

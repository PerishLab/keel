pub(crate) mod hydrate;
pub(crate) mod rows;

use crate::atom;
use crate::bond;
use crate::plan::Plan;
use crate::spec::Only;

const VERSION: u32 = 4;

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Manifest {
    version: u32,
    units: Vec<Unit>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Unit {
    pub(crate) key: String,
    pub(crate) name: String,
    pub(crate) veil: bool,
    pub(crate) frozen: bool,
    pub(crate) fields: Vec<Field>,
    pub(crate) bonds: Vec<Edge>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Field {
    pub(crate) name: String,
    pub(crate) kind: Atom,
    pub(crate) only: Limit,
    pub(crate) serial: Option<String>,
    pub(crate) need: bool,
    pub(crate) rule: Rule,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Rule {
    pub(crate) default: Option<String>,
    pub(crate) values: Vec<String>,
    pub(crate) min: Option<i64>,
    pub(crate) max: Option<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Edge {
    pub(crate) name: String,
    pub(crate) kind: Bond,
    pub(crate) target: String,
    pub(crate) fields: Vec<Field>,
    pub(crate) need: bool,
    pub(crate) root: bool,
    pub(crate) crew: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Atom {
    Text,
    Link,
    Int,
    Bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Bond {
    Many2many,
    Many2one,
    One2one,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(tag = "kind", content = "scopes", rename_all = "lowercase")]
pub(crate) enum Limit {
    Free,
    All,
    Per(Vec<String>),
}

impl Manifest {
    pub(crate) fn lift(plan: &Plan) -> Self {
        let held = Self::raw(plan);
        debug_assert!(held.mirrors(), "manifest is not round trip stable");
        held
    }

    #[cfg(debug_assertions)]
    fn mirrors(&self) -> bool {
        let text = self.write();
        let Ok(graph) = crate::graph::Graph::read(&text) else {
            return false;
        };
        let Ok(plan) = Plan::lift(&graph) else {
            return false;
        };
        if Self::raw(&plan).write() != text {
            return false;
        }
        match rows::Sheet(&rows::spill(self)).gather() {
            Ok(back) => back.write() == text,
            Err(_) => false,
        }
    }

    fn raw(plan: &Plan) -> Self {
        let mut units: Vec<Unit> = plan
            .units()
            .values()
            .filter(|unit| !unit.name().starts_with('@'))
            .map(|unit| {
                let mut fields: Vec<Field> = unit
                    .fields()
                    .iter()
                    .map(|field| Field {
                        name: field.name().to_string(),
                        kind: field.kind().into(),
                        only: field.only().into(),
                        serial: field.serial().map(str::to_string),
                        need: field.need(),
                        rule: field.rule().into(),
                    })
                    .collect();
                fields.sort_by(|a, b| a.name.cmp(&b.name));
                let mut bonds: Vec<Edge> = unit
                    .bonds()
                    .iter()
                    .map(|edge| {
                        let mut fields: Vec<Field> = edge
                            .fields()
                            .iter()
                            .map(|field| Field {
                                name: field.name().to_string(),
                                kind: field.kind().into(),
                                only: field.only().into(),
                                serial: field.serial().map(str::to_string),
                                need: field.need(),
                                rule: field.rule().into(),
                            })
                            .collect();
                        fields.sort_by(|a, b| a.name.cmp(&b.name));
                        Edge {
                            name: edge.name().to_string(),
                            kind: edge.kind().into(),
                            target: edge.target().to_string(),
                            fields,
                            need: edge.need(),
                            root: edge.root(),
                            crew: edge.crew(),
                        }
                    })
                    .collect();
                bonds.sort_by(|a, b| a.name.cmp(&b.name));
                Unit {
                    key: unit.key(),
                    name: unit.name().to_string(),
                    veil: unit.veil(),
                    frozen: unit.frozen(),
                    fields,
                    bonds,
                }
            })
            .collect();
        units.sort_by(|a, b| a.key.cmp(&b.key));
        Self {
            version: VERSION,
            units,
        }
    }

    pub(crate) fn read(text: &str) -> Result<Self, String> {
        let found: Self = serde_json::from_str(text).map_err(|err| err.to_string())?;
        if found.version != VERSION {
            return Err(format!("manifest version {}", found.version));
        }
        if found.write() != text {
            return Err("manifest is not canonical".into());
        }
        Ok(found)
    }

    pub(crate) fn write(&self) -> String {
        serde_json::to_string(self).expect("manifest")
    }

    pub(crate) fn digest(&self) -> String {
        digest(&self.write())
    }

    pub(crate) fn units(&self) -> &[Unit] {
        &self.units
    }
}

impl Unit {
    pub(crate) fn table(&self) -> String {
        let root = self.bonds.iter().find(|edge| edge.root);
        crate::ddl::table(&self.name, root.map(|edge| edge.target.as_str()))
    }

    pub(crate) fn bond(&self, name: &str) -> Option<&Edge> {
        self.bonds.iter().find(|edge| edge.name == name)
    }
}

impl From<atom::Kind> for Atom {
    fn from(value: atom::Kind) -> Self {
        match value {
            atom::Kind::Text => Self::Text,
            atom::Kind::Link => Self::Link,
            atom::Kind::Int => Self::Int,
            atom::Kind::Bool => Self::Bool,
        }
    }
}

impl From<bond::Kind> for Bond {
    fn from(value: bond::Kind) -> Self {
        match value {
            bond::Kind::Many2many => Self::Many2many,
            bond::Kind::Many2one => Self::Many2one,
            bond::Kind::One2one => Self::One2one,
        }
    }
}

impl From<&Only> for Limit {
    fn from(value: &Only) -> Self {
        match value {
            Only::Free => Self::Free,
            Only::All => Self::All,
            Only::Per(scopes) => {
                let mut scopes = scopes.clone();
                scopes.sort();
                Self::Per(scopes)
            }
        }
    }
}

impl From<&crate::spec::Rule> for Rule {
    fn from(value: &crate::spec::Rule) -> Self {
        Self {
            default: value.fallback().map(str::to_string),
            values: value.admitted().to_vec(),
            min: value.minimum(),
            max: value.maximum(),
        }
    }
}

pub(crate) fn digest(text: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())
}

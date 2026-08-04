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
    frozen: bool,
    faults: Vec<String>,
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
    Closure,
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
    need: bool,
    rule: Rule,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Rule {
    default: Option<String>,
    values: Vec<String>,
    min: Option<i64>,
    max: Option<i64>,
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
    closure: bool,
}

pub struct Builder(Spec);

impl Spec {
    pub fn build(name: impl Into<String>) -> Builder {
        Builder(Spec {
            name: name.into(),
            fields: Vec::new(),
            bonds: Vec::new(),
            veil: false,
            frozen: false,
            faults: Vec::new(),
        })
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

    pub fn frozen(&self) -> bool {
        self.frozen
    }

    pub fn fields(&self) -> &[Field] {
        &self.fields
    }

    pub fn bonds(&self) -> &[Bond] {
        &self.bonds
    }

    pub(crate) fn faults(&self) -> &[String] {
        &self.faults
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

    pub fn need(&self) -> bool {
        self.need
    }

    pub fn rule(&self) -> &Rule {
        &self.rule
    }
}

impl Rule {
    pub fn new() -> Self {
        <Self as Default>::default()
    }

    pub fn default(mut self, value: impl Into<String>) -> Self {
        self.default = Some(value.into());
        self
    }

    pub fn values(mut self, values: &[&str]) -> Self {
        self.values = values.iter().map(|value| (*value).to_string()).collect();
        self
    }

    pub fn min(mut self, value: i64) -> Self {
        self.min = Some(value);
        self
    }

    pub fn max(mut self, value: i64) -> Self {
        self.max = Some(value);
        self
    }

    pub fn fallback(&self) -> Option<&str> {
        self.default.as_deref()
    }

    pub fn admitted(&self) -> &[String] {
        &self.values
    }

    pub fn minimum(&self) -> Option<i64> {
        self.min
    }

    pub fn maximum(&self) -> Option<i64> {
        self.max
    }

    pub(crate) fn normalize(
        &self,
        kind: atom::Kind,
        need: bool,
    ) -> Result<Self, crate::adapt::Error> {
        if !need && self.default.is_some() {
            return Err(crate::adapt::Error::Adapt(
                "optional field cannot have default".into(),
            ));
        }
        if (self.min.is_some() || self.max.is_some()) && kind != atom::Kind::Int {
            return Err(crate::adapt::Error::Adapt("range needs int field".into()));
        }
        if self.min.zip(self.max).is_some_and(|(min, max)| min > max) {
            return Err(crate::adapt::Error::Adapt("empty integer range".into()));
        }
        let default = self
            .default
            .as_deref()
            .map(|value| canon(kind, value))
            .transpose()?;
        let mut values = self
            .values
            .iter()
            .map(|value| canon(kind, value))
            .collect::<Result<Vec<_>, _>>()?;
        values.sort();
        if values.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(crate::adapt::Error::Adapt(
                "duplicate admitted value".into(),
            ));
        }
        let rule = Self {
            default,
            values,
            min: self.min,
            max: self.max,
        };
        for value in &rule.values {
            rule.check(kind, value).map_err(|_| {
                crate::adapt::Error::Adapt("admitted value violates field range".into())
            })?;
        }
        if let Some(value) = rule.default.as_deref() {
            rule.check(kind, value)
                .map_err(|_| crate::adapt::Error::Adapt("default violates field rule".into()))?;
        }
        Ok(rule)
    }

    pub(crate) fn check(&self, kind: atom::Kind, value: &str) -> Result<(), crate::adapt::Error> {
        let value = canon(kind, value)?;
        if !self.values.is_empty() && !self.values.contains(&value) {
            return Err(crate::adapt::Error::Adapt("value is not admitted".into()));
        }
        if kind == atom::Kind::Int {
            let value = value
                .parse::<i64>()
                .map_err(|_| crate::adapt::Error::Adapt("value needs integer".into()))?;
            if self.min.is_some_and(|min| value < min) || self.max.is_some_and(|max| value > max) {
                return Err(crate::adapt::Error::Adapt("value is outside range".into()));
            }
        }
        Ok(())
    }
}

fn canon(kind: atom::Kind, value: &str) -> Result<String, crate::adapt::Error> {
    kind.fit(value)?;
    match kind {
        atom::Kind::Int => value
            .parse::<i64>()
            .map(|value| value.to_string())
            .map_err(|_| crate::adapt::Error::Adapt("value needs integer".into())),
        _ => Ok(value.to_string()),
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

    pub fn closure(&self) -> bool {
        self.closure
    }
}

#[path = "plan/builder.rs"]
mod builder;

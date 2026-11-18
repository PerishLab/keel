use crate::{Error, Failure};
use radicula_document::{Document, Value};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    deny_unknown_fields,
    tag = "kind",
    content = "value",
    rename_all = "snake_case"
)]
pub enum Shape {
    Any,
    Null,
    Bool,
    Number,
    Text,
    Array(Box<Shape>),
    Map(Box<Shape>),
    Object(Vec<Field>),
    Nullable(Box<Shape>),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Field {
    name: String,
    shape: Shape,
    required: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Collection {
    name: String,
    shape: Shape,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Model {
    format: u32,
    collections: Vec<Collection>,
}

impl Field {
    pub fn required(name: impl Into<String>, shape: Shape) -> Self {
        Self {
            name: name.into(),
            shape,
            required: true,
        }
    }

    pub fn optional(name: impl Into<String>, shape: Shape) -> Self {
        Self {
            name: name.into(),
            shape,
            required: false,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn shape(&self) -> &Shape {
        &self.shape
    }
    pub fn needed(&self) -> bool {
        self.required
    }
}

impl Collection {
    pub fn new(name: impl Into<String>, shape: Shape) -> Result<Self, Error> {
        let held = Self {
            name: name.into(),
            shape,
        };
        held.check()?;
        Ok(held)
    }

    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn shape(&self) -> &Shape {
        &self.shape
    }

    pub fn validate(&self, value: &Document) -> Result<(), Error> {
        self.check()?;
        self.shape.accept(value.root(), &mut Vec::new())
    }

    fn check(&self) -> Result<(), Error> {
        if self.name.is_empty() {
            return Err(Error::Model("collection needs a name".into()));
        }
        self.shape.check()
    }
}

impl Model {
    pub(crate) fn extends(&self, saved: &Self) -> Result<(), Error> {
        self.check()?;
        let compatible = self.collections.len() >= saved.collections.len()
            && self
                .collections
                .iter()
                .zip(&saved.collections)
                .all(|(new, old)| new.name == old.name && new.shape.extends(&old.shape));
        if !compatible {
            return Err(Error::Model(
                "evolution requires a compatible declaration suffix".into(),
            ));
        }
        Ok(())
    }

    pub fn new(collections: Vec<Collection>) -> Result<Self, Error> {
        let held = Self {
            format: 1,
            collections,
        };
        held.check()?;
        Ok(held)
    }

    pub fn collections(&self) -> &[Collection] {
        &self.collections
    }

    pub fn collection(&self, name: &str) -> Result<&Collection, Error> {
        self.collections
            .iter()
            .find(|held| held.name == name)
            .ok_or_else(|| Error::Collection(name.to_string()))
    }

    pub fn describe(&self) -> Result<String, Error> {
        self.check()?;
        serde_json::to_string(self).map_err(|_| Error::Syntax)
    }

    pub fn parse(text: &str) -> Result<Self, Error> {
        let held: Self = serde_json::from_str(text).map_err(|_| Error::Syntax)?;
        held.check()?;
        Ok(held)
    }

    fn check(&self) -> Result<(), Error> {
        if self.format != 1 {
            return Err(Error::Model("unsupported document model format".into()));
        }
        if self.collections.is_empty() {
            return Err(Error::Model("model needs a collection".into()));
        }
        let mut names = BTreeSet::new();
        for held in &self.collections {
            if !names.insert(&held.name) {
                return Err(Error::Model("duplicate collection name".into()));
            }
            held.check()?;
        }
        Ok(())
    }
}

impl Shape {
    fn extends(&self, saved: &Self) -> bool {
        match (self, saved) {
            (Self::Array(new), Self::Array(old))
            | (Self::Map(new), Self::Map(old))
            | (Self::Nullable(new), Self::Nullable(old)) => new.extends(old),
            (Self::Object(new), Self::Object(old)) => extension(new, old),
            _ => self == saved,
        }
    }

    fn check(&self) -> Result<(), Error> {
        match self {
            Self::Array(inner) | Self::Map(inner) | Self::Nullable(inner) => inner.check(),
            Self::Object(fields) => members(fields),
            _ => Ok(()),
        }
    }

    fn accept(&self, value: &Value, path: &mut Vec<String>) -> Result<(), Error> {
        match self {
            Self::Any => Ok(()),
            Self::Null if value.is_null() => Ok(()),
            Self::Bool if value.is_boolean() => Ok(()),
            Self::Number if value.is_number() => Ok(()),
            Self::Text if value.is_string() => Ok(()),
            Self::Nullable(_) if value.is_null() => Ok(()),
            Self::Nullable(inner) => inner.accept(value, path),
            Self::Array(inner) => {
                let items = value.as_array().ok_or_else(|| fault(path, Failure::Kind))?;
                for (at, item) in items.iter().enumerate() {
                    path.push(at.to_string());
                    inner.accept(item, path)?;
                    path.pop();
                }
                Ok(())
            }
            Self::Map(inner) => {
                let items = value
                    .as_object()
                    .ok_or_else(|| fault(path, Failure::Kind))?;
                for (key, item) in items {
                    path.push(key.clone());
                    inner.accept(item, path)?;
                    path.pop();
                }
                Ok(())
            }
            Self::Object(fields) => object(fields, value, path),
            _ => Err(fault(path, Failure::Kind)),
        }
    }
}

fn extension(new: &[Field], old: &[Field]) -> bool {
    new.len() >= old.len()
        && new.iter().zip(old).all(|(new, old)| {
            new.name == old.name && new.required == old.required && new.shape.extends(&old.shape)
        })
        && new.iter().skip(old.len()).all(|field| !field.required)
}

fn members(fields: &[Field]) -> Result<(), Error> {
    let mut names = BTreeSet::new();
    for field in fields {
        if !names.insert(&field.name) {
            return Err(Error::Model("duplicate object field".into()));
        }
        field.shape.check()?;
    }
    Ok(())
}

fn object(fields: &[Field], value: &Value, path: &mut Vec<String>) -> Result<(), Error> {
    let items = value
        .as_object()
        .ok_or_else(|| fault(path, Failure::Kind))?;
    for field in fields {
        path.push(field.name.clone());
        match items.get(&field.name) {
            Some(item) => field.shape.accept(item, path)?,
            None if field.required => return Err(fault(path, Failure::Missing)),
            None => {}
        }
        path.pop();
    }
    if let Some(key) = items
        .keys()
        .find(|key| !fields.iter().any(|field| field.name == **key))
    {
        path.push(key.clone());
        return Err(fault(path, Failure::Unknown));
    }
    Ok(())
}

fn fault(path: &[String], failure: Failure) -> Error {
    Error::Value {
        parts: path.to_vec(),
        failure,
    }
}

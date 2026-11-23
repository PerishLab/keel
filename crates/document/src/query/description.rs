use super::{Condition, Limit, Projection, Query, path, text};
use crate::Error;
use radicula_document::{Document, Value};
use serde::de::Deserializer;
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};

const TEXT: usize = 1024 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Description {
    format: u32,
    collection: String,
    conditions: Vec<Row>,
    select: Vec<Field>,
    #[serde(default)]
    limit: Limit,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    kind: String,
    path: String,
    #[serde(
        default,
        skip_serializing_if = "Present::absent",
        deserialize_with = "Present::held"
    )]
    value: Present,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Field {
    name: String,
    path: String,
}

#[derive(Clone, Debug, Default)]
enum Present {
    #[default]
    Absent,
    Held(Value),
}

impl Present {
    fn absent(&self) -> bool {
        matches!(self, Self::Absent)
    }

    fn held<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Value::deserialize(deserializer).map(Self::Held)
    }
}

impl Serialize for Present {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Held(value) => value.serialize(serializer),
            Self::Absent => serializer.serialize_none(),
        }
    }
}

impl Query {
    pub fn describe(&self) -> Result<String, Error> {
        self.validate()?;
        let conditions = self.conditions.iter().map(Row::spell).collect();
        let select = self
            .select
            .iter()
            .map(|projection| Field {
                name: projection.name.clone(),
                path: text(&projection.path),
            })
            .collect();
        let description = Description {
            format: 1,
            collection: self.collection.clone(),
            conditions,
            select,
            limit: self.limit.clone(),
        };
        let held = serde_json::to_string(&description).map_err(|_| Error::Syntax)?;
        exceeded(held.len())?;
        Ok(held)
    }

    pub fn parse(held: &str) -> Result<Self, Error> {
        exceeded(held.len())?;
        let description: Description = serde_json::from_str(held).map_err(|_| Error::Syntax)?;
        if description.format != 1 {
            return Err(Error::Query("unsupported document query format".into()));
        }
        let conditions = description
            .conditions
            .into_iter()
            .map(Row::condition)
            .collect::<Result<Vec<_>, Error>>()?;
        let select = description
            .select
            .into_iter()
            .map(|field| {
                Ok(Projection {
                    name: field.name,
                    path: path(&field.path)?,
                })
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let query = Self {
            collection: description.collection,
            conditions,
            select,
            limit: description.limit,
        };
        query.validate()?;
        Ok(query)
    }
}

impl Row {
    fn spell(condition: &Condition) -> Self {
        match condition {
            Condition::Equal { path, value } => Self {
                kind: "equal".into(),
                path: text(path),
                value: Present::Held(value.clone()),
            },
            Condition::Missing(path) => Self {
                kind: "missing".into(),
                path: text(path),
                value: Present::Absent,
            },
        }
    }

    fn condition(self) -> Result<Condition, Error> {
        let path = path(&self.path)?;
        match self.kind.as_str() {
            "equal" => match self.value {
                Present::Held(value) => {
                    Document::new(value.clone()).map_err(Error::Document)?;
                    Ok(Condition::Equal { path, value })
                }
                Present::Absent => Err(Error::Query("equal needs a value".into())),
            },
            "missing" => match self.value {
                Present::Absent => Ok(Condition::Missing(path)),
                Present::Held(_) => Err(Error::Query("missing takes no value".into())),
            },
            _ => Err(Error::Query("unknown condition kind".into())),
        }
    }
}

fn exceeded(length: usize) -> Result<(), Error> {
    if length > TEXT {
        return Err(Error::Query("query exceeds 1 MiB".into()));
    }
    Ok(())
}

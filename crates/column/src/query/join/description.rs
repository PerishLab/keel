use super::{Condition, Query, Side};
use crate::Error;
use crate::Value;
use crate::query::description::exceeded;
use crate::query::wire;

impl Query {
    pub fn describe(&self) -> Result<String, Error> {
        self.check()?;
        let description = wire::Join {
            format: 1,
            kind: "join".into(),
            left: wire::Side::from(&self.left),
            right: wire::Side::from(&self.right),
            mode: self.mode,
            batch: self.batch,
            limit: self.limit.clone(),
        };
        let text =
            serde_json::to_string(&description).map_err(|_| Error::new("invalid column join"))?;
        exceeded(text.len())?;
        Ok(text)
    }

    pub fn parse(text: &str) -> Result<Self, Error> {
        exceeded(text.len())?;
        let description: wire::Join = serde_json::from_str(text)
            .map_err(|error| Error::new(format!("invalid column join: {error}")))?;
        if description.format != 1 {
            return Err(Error::new("unsupported column query format"));
        }
        if description.kind != "join" {
            return Err(Error::new("unsupported column query kind"));
        }
        let query = Self {
            left: Side::try_from(description.left)?,
            right: Side::try_from(description.right)?,
            mode: description.mode,
            batch: description.batch,
            limit: description.limit,
        };
        query.check()?;
        Ok(query)
    }

    fn check(&self) -> Result<(), Error> {
        for side in [&self.left, &self.right] {
            if side
                .filter
                .from
                .zip(side.filter.to)
                .is_some_and(|(from, to)| from >= to)
            {
                return Err(Error::new("query window requires from < to"));
            }
        }
        if self.batch == 0 || self.batch > 65536 {
            return Err(Error::new("query batch must be between 1 and 65536 rows"));
        }
        if self.limit.batch > 64 * 1024 * 1024 {
            return Err(Error::new("join batch charge must not exceed 64 MiB"));
        }
        self.left.check()?;
        self.right.check()?;
        Ok(())
    }
}

impl Side {
    fn check(&self) -> Result<(), Error> {
        let mut names = std::collections::BTreeSet::new();
        for (name, _) in &self.filter.keys {
            if !names.insert(name) {
                return Err(Error::new("duplicate query key"));
            }
        }
        if self.columns.is_empty() || self.key.is_empty() {
            return Err(Error::new("join side needs a key and columns"));
        }
        if self
            .columns
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != self.columns.len()
        {
            return Err(Error::new("join side columns must be distinct"));
        }
        for (index, condition) in self.when.iter().enumerate() {
            if self.when[..index].contains(condition) {
                return Err(Error::new("duplicate join condition"));
            }
        }
        Ok(())
    }
}

impl From<&Side> for wire::Side {
    fn from(side: &Side) -> Self {
        Self {
            filter: wire::Filter::from(&side.filter),
            key: side.key.clone(),
            columns: side.columns.clone(),
            when: side.when.iter().map(wire::Condition::from).collect(),
        }
    }
}

impl TryFrom<wire::Side> for Side {
    type Error = Error;

    fn try_from(held: wire::Side) -> Result<Self, Error> {
        Ok(Self {
            filter: crate::Filter::from(held.filter),
            key: held.key,
            columns: held.columns,
            when: held
                .when
                .into_iter()
                .map(Condition::try_from)
                .collect::<Result<Vec<_>, Error>>()?,
        })
    }
}

impl From<&Condition> for wire::Condition {
    fn from(condition: &Condition) -> Self {
        match condition {
            Condition::Equal(column, value) => Self {
                kind: "equal".into(),
                column: column.clone(),
                value: Some(wire::Value::from(value)),
            },
            Condition::Missing(column) => Self {
                kind: "missing".into(),
                column: column.clone(),
                value: None,
            },
        }
    }
}

impl TryFrom<wire::Condition> for Condition {
    type Error = Error;

    fn try_from(held: wire::Condition) -> Result<Self, Error> {
        match (held.kind.as_str(), held.value) {
            ("equal", Some(value)) => Ok(Self::Equal(held.column, Value::try_from(value)?)),
            ("equal", None) => Err(Error::new("equal needs a value")),
            ("missing", None) => Ok(Self::Missing(held.column)),
            ("missing", Some(_)) => Err(Error::new("missing takes no value")),
            _ => Err(Error::new("unknown join condition kind")),
        }
    }
}

impl From<&Value> for wire::Value {
    fn from(value: &Value) -> Self {
        let mut held = Self::default();
        match value {
            Value::Null => held.null = Some(true),
            Value::Bool(value) => held.bool = Some(*value),
            Value::Int(value) => held.int = Some(*value),
            Value::Uint(value) => held.uint = Some(*value),
            Value::Text(value) => held.text = Some(value.clone()),
            Value::Bytes(value) => held.bytes = Some(value.clone()),
        }
        held
    }
}

impl TryFrom<wire::Value> for Value {
    type Error = Error;

    fn try_from(held: wire::Value) -> Result<Self, Error> {
        let tags = [
            held.null.is_some(),
            held.bool.is_some(),
            held.int.is_some(),
            held.uint.is_some(),
            held.text.is_some(),
            held.bytes.is_some(),
        ];
        if tags.iter().filter(|held| **held).count() != 1 {
            return Err(Error::new("a value needs exactly one tag"));
        }
        if held.null.is_some() {
            return Err(Error::new(
                "equal null is unsupported; use Missing explicitly",
            ));
        }
        if let Some(value) = held.bool {
            return Ok(Self::Bool(value));
        }
        if let Some(value) = held.int {
            return Ok(Self::Int(value));
        }
        if let Some(value) = held.uint {
            return Ok(Self::Uint(value));
        }
        if let Some(value) = held.text {
            return Ok(Self::Text(value));
        }
        Ok(Self::Bytes(held.bytes.unwrap_or_default()))
    }
}

use super::wire;
use super::{Filter, Projection, Query};
use crate::Error;
use std::collections::BTreeSet;

const TEXT: usize = 1024 * 1024;

impl Query {
    pub fn describe(&self) -> Result<String, Error> {
        self.check()?;
        let description = wire::Scan {
            format: 1,
            kind: "scan".into(),
            filter: wire::Filter::from(&self.filter),
            projection: wire::Projection::from(&self.projection),
            batch: self.batch,
            limit: self.limit.clone(),
        };
        let text =
            serde_json::to_string(&description).map_err(|_| Error::new("invalid column query"))?;
        exceeded(text.len())?;
        Ok(text)
    }

    pub fn parse(text: &str) -> Result<Self, Error> {
        exceeded(text.len())?;
        let description: wire::Scan = serde_json::from_str(text)
            .map_err(|error| Error::new(format!("invalid column query: {error}")))?;
        if description.format != 1 {
            return Err(Error::new("unsupported column query format"));
        }
        if description.kind != "scan" {
            return Err(Error::new("unsupported column query kind"));
        }
        let query = Self {
            filter: Filter::from(description.filter),
            projection: Projection::try_from(description.projection)?,
            batch: description.batch,
            limit: description.limit,
        };
        query.check()?;
        Ok(query)
    }

    fn check(&self) -> Result<(), Error> {
        if self
            .filter
            .from
            .zip(self.filter.to)
            .is_some_and(|(from, to)| from >= to)
        {
            return Err(Error::new("query window requires from < to"));
        }
        if self.batch == 0 || self.batch > 65536 {
            return Err(Error::new("query batch must be between 1 and 65536 rows"));
        }
        let mut names = BTreeSet::new();
        for (name, _) in &self.filter.keys {
            if !names.insert(name) {
                return Err(Error::new("duplicate query key"));
            }
        }
        if let Projection::Columns(columns) = &self.projection
            && (columns.is_empty()
                || columns.iter().collect::<BTreeSet<_>>().len() != columns.len())
        {
            return Err(Error::new(
                "query projection needs distinct non-empty columns",
            ));
        }
        Ok(())
    }
}

impl From<&Filter> for wire::Filter {
    fn from(filter: &Filter) -> Self {
        Self {
            from: filter.from,
            to: filter.to,
            keys: filter
                .keys
                .iter()
                .map(|(name, value)| wire::Key {
                    name: name.clone(),
                    value: value.clone(),
                })
                .collect(),
        }
    }
}

impl From<wire::Filter> for Filter {
    fn from(held: wire::Filter) -> Self {
        Self {
            from: held.from,
            to: held.to,
            keys: held
                .keys
                .into_iter()
                .map(|key| (key.name, key.value))
                .collect(),
        }
    }
}

impl From<&Projection> for wire::Projection {
    fn from(projection: &Projection) -> Self {
        match projection {
            Projection::Columns(columns) => Self {
                columns: Some(columns.clone()),
                count: None,
            },
            Projection::Count => Self {
                columns: None,
                count: Some(true),
            },
        }
    }
}

impl TryFrom<wire::Projection> for Projection {
    type Error = Error;

    fn try_from(held: wire::Projection) -> Result<Self, Error> {
        match (held.columns, held.count) {
            (Some(columns), None) => Ok(Self::Columns(columns)),
            (None, Some(true)) => Ok(Self::Count),
            (None, Some(false)) => Err(Error::new("count must be true")),
            _ => Err(Error::new(
                "projection needs exactly one of columns or count",
            )),
        }
    }
}

pub(super) fn exceeded(length: usize) -> Result<(), Error> {
    if length > TEXT {
        return Err(Error::new("query exceeds 1 MiB"));
    }
    Ok(())
}

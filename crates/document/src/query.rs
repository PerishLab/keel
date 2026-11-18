use crate::{Error, Head, View};
use keel::Wire;
use radicula_document::{Document, Path, Presence, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq)]
pub enum Condition {
    Equal { path: Path, value: Value },
    Missing(Path),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Projection {
    pub name: String,
    pub path: Path,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Query {
    pub conditions: Vec<Condition>,
    pub select: Vec<Projection>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Selection {
    pub head: Head,
    pub values: BTreeMap<String, Value>,
}

impl Query {
    fn validate(&self) -> Result<(), Error> {
        let mut names = BTreeSet::new();
        for projection in &self.select {
            if !names.insert(&projection.name) {
                return Err(Error::Model("duplicate projection name".into()));
            }
        }
        for condition in &self.conditions {
            if let Condition::Equal { value, .. } = condition {
                Document::new(value.clone()).map_err(Error::Document)?;
            }
        }
        Ok(())
    }

    fn accepts(&self, document: &Document) -> Result<bool, Error> {
        for condition in &self.conditions {
            let accepted = match condition {
                Condition::Equal { path, value } => {
                    document.read(path).map_err(Error::Document)? == Presence::Present(value)
                }
                Condition::Missing(path) => {
                    document.read(path).map_err(Error::Document)? == Presence::Missing
                }
            };
            if !accepted {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn project(&self, document: &Document) -> Result<BTreeMap<String, Value>, Error> {
        let mut values = BTreeMap::new();
        for projection in &self.select {
            if let Presence::Present(value) =
                document.read(&projection.path).map_err(Error::Document)?
            {
                values.insert(projection.name.clone(), value.clone());
            }
        }
        Ok(values)
    }
}

impl<W: Wire> View<'_, W> {
    pub async fn query(&self, collection: &str, query: &Query) -> Result<Vec<Selection>, Error> {
        self.store.model().collection(collection)?;
        query.validate()?;
        let carrier = self.store.carrier.clone();
        let candidates = tokio::task::spawn_blocking(move || carrier.inventory())
            .await
            .map_err(|error| Error::carrier(error.to_string()))??;
        let ids: BTreeSet<_> = candidates.into_iter().map(|(id, _)| id).collect();
        let mut result = Vec::new();
        for id in ids {
            let record = match self.read(collection, id).await {
                Ok(record) => record,
                Err(Error::Missing) => continue,
                Err(error) => return Err(error),
            };
            if query.accepts(&record.value)? {
                result.push(Selection {
                    head: record.head,
                    values: query.project(&record.value)?,
                });
            }
        }
        Ok(result)
    }
}

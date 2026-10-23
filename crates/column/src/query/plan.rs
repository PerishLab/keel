use super::{Projection, Query};
use crate::{Error, Table};
use std::collections::BTreeSet;

pub(super) struct Plan {
    pub columns: Vec<usize>,
    pub order: Vec<usize>,
    pub keys: Vec<(usize, String)>,
    pub raw: bool,
}

impl Table {
    pub(super) fn plan(&self, query: &Query) -> Result<Plan, Error> {
        let filter = &query.filter;
        if filter
            .from
            .zip(filter.to)
            .is_some_and(|(from, to)| from >= to)
        {
            return Err(Error::new("query window requires from < to"));
        }
        if query.batch == 0 || query.batch > 65536 {
            return Err(Error::new("query batch must be between 1 and 65536 rows"));
        }
        let keys = filter
            .keys
            .iter()
            .map(|(name, value)| Ok((self.index(name)? + 1, value.clone())))
            .collect::<Result<_, Error>>()?;
        let requested = match &query.projection {
            Projection::Count => Vec::new(),
            Projection::Columns(names) => self.columns(names)?,
        };
        let columns: Vec<_> = requested
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let order = requested
            .iter()
            .map(|index| columns.binary_search(index).unwrap())
            .collect();
        Ok(Plan {
            raw: columns.contains(&(self.keys().len() + 1)),
            columns,
            order,
            keys,
        })
    }

    fn columns(&self, names: &[String]) -> Result<Vec<usize>, Error> {
        if names.is_empty() || names.iter().collect::<BTreeSet<_>>().len() != names.len() {
            return Err(Error::new(
                "query projection needs distinct non-empty columns",
            ));
        }
        names.iter().map(|name| self.column(name)).collect()
    }

    fn column(&self, name: &str) -> Result<usize, Error> {
        if name == self.time() {
            Ok(0)
        } else if name == "raw" {
            Ok(self.keys().len() + 1)
        } else {
            self.index(name).map(|index| index + 1)
        }
    }
}

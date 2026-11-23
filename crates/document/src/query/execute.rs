use super::{Query, Selection, Stats};
use crate::{Error, View};
use keel::Wire;
use std::collections::BTreeSet;

impl<W: Wire> View<'_, W> {
    pub async fn query(&self, query: &Query) -> Result<(Vec<Selection>, Stats), Error> {
        let collection = query.collection.clone();
        self.store.model().collection(&collection)?;
        query.validate()?;
        let carrier = self.store.carrier.clone();
        let candidates = tokio::task::spawn_blocking(move || carrier.inventory())
            .await
            .map_err(|error| Error::carrier(error.to_string()))??;
        let ids: BTreeSet<_> = candidates.into_iter().map(|(id, _)| id).collect();
        let mut stats = Stats::default();
        let mut result = Vec::new();
        for id in ids {
            stats.candidates += 1;
            over(stats.candidates, query.limit.candidates, "candidate")?;
            let record = match self.read(&collection, id).await {
                Ok(record) => record,
                Err(Error::Missing) => continue,
                Err(error) => return Err(error),
            };
            stats.admitted += 1;
            stats.bytes += record.value.text().len() as u64;
            over(stats.bytes, query.limit.bytes, "byte")?;
            if query.accepts(&record.value)? {
                stats.matched += 1;
                over(stats.matched, query.limit.rows, "row")?;
                result.push(Selection {
                    head: record.head,
                    values: query.project(&record.value)?,
                });
            }
        }
        Ok((result, stats))
    }
}

fn over(held: u64, limit: u64, name: &str) -> Result<(), Error> {
    if held > limit {
        return Err(Error::Query(format!("query {name} limit exceeded")));
    }
    Ok(())
}

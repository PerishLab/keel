use crate::query::join;
use crate::query::scan::Sink;
use crate::{Error, Filter, Record, Store, Value};
use keel::atom::string;
use keel::{Op, Wire, form};
use sha2::{Digest, Sha256};

#[keel::resource(frozen)]
pub struct Channel {
    #[field(string)]
    mode: string,
    #[field(string, unique = mode)]
    source: string,
}

pub struct View<'a, W: Wire> {
    store: &'a Store<W>,
    operator: i64,
    source: &'a str,
}

impl<W: Wire> Store<W> {
    pub fn of<'a>(&'a self, operator: i64, source: &'a str) -> View<'a, W> {
        View {
            store: self,
            operator,
            source,
        }
    }
}

impl<W: Wire> View<'_, W> {
    async fn admit(&self, mode: &str) -> Result<usize, Error> {
        let axis = self.store.table.axis()?;
        let tree = form("Channel")
            .when("source", Op::Eq, self.source)
            .when("mode", Op::Eq, mode);
        let channel = self.store.core.of(self.operator).one(&tree).await?;
        channel.ok_or_else(|| Error::deny("column source refused"))?;
        Ok(axis)
    }

    pub async fn append(&self, token: &str, records: Vec<Record>) -> Result<(), Error> {
        let axis = self.admit("append").await?;
        if records.is_empty() || token.is_empty() || token.contains('\n') {
            return Err(Error::new(
                "expected a nonempty batch and a single-line token",
            ));
        }
        let mut digest = Sha256::new();
        digest.update(self.operator.to_be_bytes());
        feed(&mut digest, token.as_bytes());
        for record in &records {
            self.store.table.check(record)?;
            if record.keys[axis] != self.source {
                return Err(Error::deny("batch contains a different source"));
            }
            digest.update(record.time.to_be_bytes());
            for value in &record.keys {
                feed(&mut digest, value.as_bytes());
            }
            feed(&mut digest, &record.raw);
        }
        let token = format!("column:{:x}", digest.finalize());
        self.store.append(&token, records).await
    }

    pub async fn scan(&self, filter: &Filter, sink: &mut Sink<'_>) -> Result<(), Error> {
        let axis = self.admit("scan").await?;
        let key = &self.store.table.keys()[axis];
        let mut filter = filter.clone();
        filter.keys.push((key.clone(), self.source.to_owned()));
        self.store.scan(&filter, sink).await
    }

    pub async fn query(
        &self,
        query: &crate::Query,
        output: &mut crate::query::Output<'_>,
    ) -> Result<crate::query::Stats, Error> {
        let axis = self.admit("scan").await?;
        let mut query = query.clone();
        self.scope(&mut query.filter, axis)?;
        self.store.query(&query, output).await
    }

    pub async fn join(
        &self,
        query: &join::Query,
        output: &mut join::Output<'_>,
    ) -> Result<join::Stats, Error> {
        let axis = self.admit("scan").await?;
        let mut query = query.clone();
        self.scope(&mut query.left.filter, axis)?;
        self.scope(&mut query.right.filter, axis)?;
        let key = &self.store.table.keys()[axis];
        for side in [&query.left, &query.right] {
            for condition in &side.when {
                if let join::Condition::Equal(column, Value::Text(text)) = condition
                    && column == key
                    && text.as_str() != self.source
                {
                    return Err(Error::deny("query names a different source"));
                }
            }
        }
        self.store.join(&query, output).await
    }

    fn scope(&self, filter: &mut Filter, axis: usize) -> Result<(), Error> {
        let key = self.store.table.keys()[axis].clone();
        if let Some((_, value)) = filter.keys.iter().find(|(name, _)| name == &key)
            && value.as_str() != self.source
        {
            return Err(Error::deny("query names a different source"));
        }
        filter.keys.push((key, self.source.to_owned()));
        Ok(())
    }
}

fn feed(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_be_bytes());
    digest.update(bytes);
}

use crate::carrier::nonce;
use crate::manifest::{DOCUMENT, REVISION};
use crate::{Error, Head, Record, Store};
use keel::{Face, Op, Row, Tx, Wire, form};
use radicula_document::{Document, Operation};

pub struct View<'a, W: Wire> {
    pub(crate) store: &'a Store<W>,
    pub(crate) face: Face<'a, W>,
}

impl<W: Wire> View<'_, W> {
    pub async fn create(
        &self,
        collection: &str,
        name: &str,
        value: Document,
    ) -> Result<Record, Error> {
        self.store
            .model()
            .collection(collection)?
            .validate(&value)?;
        let token = nonce()?;
        self.transact(async |tx| {
            let root = self.store.binding.root.to_string();
            let id = tx
                .put(
                    DOCUMENT,
                    &[
                        ("store", &root),
                        ("collection", collection),
                        ("name", name),
                        ("head", &token),
                    ],
                )
                .await?;
            fence(tx, id, "0", &token).await?;
            let record = Record {
                head: Head {
                    collection: collection.into(),
                    id,
                    token,
                },
                value,
            };
            self.store.save(&record).await?;
            Ok(record)
        })
        .await
    }

    pub async fn read(&self, collection: &str, id: i64) -> Result<Record, Error> {
        self.store.model().collection(collection)?;
        let row = self
            .face
            .one(&self.anchor(collection, id))
            .await?
            .ok_or(Error::Missing)?;
        self.record(&row, collection).await
    }

    pub async fn edit(&self, head: &Head, operations: &[Operation]) -> Result<Record, Error> {
        self.store.model().collection(&head.collection)?;
        self.transact(async |tx| {
            let row = self.row(tx, &head.collection, head.id).await?;
            expected(&row, &head.token)?;
            let mut record = self.record(&row, &head.collection).await?;
            record.value.apply(operations)?;
            self.store
                .model()
                .collection(&head.collection)?
                .validate(&record.value)?;
            record.head.token = nonce()?;
            fence(tx, head.id, &head.token, &record.head.token).await?;
            tx.set(DOCUMENT, head.id, &[("head", &record.head.token)])
                .await?;
            self.store.save(&record).await?;
            Ok(record)
        })
        .await
    }

    pub async fn end(&self, head: &Head) -> Result<(), Error> {
        self.store.model().collection(&head.collection)?;
        self.transact(async |tx| {
            let row = self.row(tx, &head.collection, head.id).await?;
            expected(&row, &head.token)?;
            tx.end(DOCUMENT, head.id).await?;
            Ok(())
        })
        .await
    }

    fn anchor(&self, collection: &str, id: i64) -> keel::query::Tree {
        form(DOCUMENT)
            .when("id", Op::Eq, &id.to_string())
            .when("store", Op::Eq, &self.store.binding.root.to_string())
            .when("collection", Op::Eq, collection)
    }

    async fn row(&self, tx: &mut Tx<'_, W>, collection: &str, id: i64) -> Result<Row, Error> {
        tx.one(&self.anchor(collection, id))
            .await?
            .ok_or(Error::Missing)
    }

    async fn record(&self, row: &Row, collection: &str) -> Result<Record, Error> {
        let head = Head {
            collection: collection.into(),
            id: row.key(),
            token: token(row)?.into(),
        };
        let value = self.store.load(&head).await?;
        self.store
            .model()
            .collection(collection)?
            .validate(&value)?;
        Ok(Record { head, value })
    }

    async fn transact<T>(
        &self,
        run: impl AsyncFnOnce(&mut Tx<'_, W>) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let mut fault = None;
        let result = self
            .face
            .batch(async |tx| match run(tx).await {
                Ok(value) => Ok(value),
                Err(error) => {
                    fault = Some(error);
                    Err(keel::adapt::Error::Adapt(
                        "document operation refused".into(),
                    ))
                }
            })
            .await;
        match result {
            Ok(value) => Ok(value),
            Err(error) => Err(fault.unwrap_or_else(|| Error::keel(error))),
        }
    }
}

async fn fence<W: Wire>(tx: &mut Tx<'_, W>, id: i64, base: &str, token: &str) -> Result<(), Error> {
    tx.put(
        REVISION,
        &[
            ("document", &id.to_string()),
            ("base", &format!("{id}/{base}")),
            ("token", token),
        ],
    )
    .await?;
    Ok(())
}

fn token(row: &Row) -> Result<&str, Error> {
    row.text("head")
        .ok_or_else(|| Error::Binding("native head is absent".into()))
}

fn expected(row: &Row, expected: &str) -> Result<(), Error> {
    let actual = token(row)?;
    if actual != expected {
        return Err(Error::Conflict {
            expected: expected.into(),
            actual: actual.into(),
        });
    }
    Ok(())
}

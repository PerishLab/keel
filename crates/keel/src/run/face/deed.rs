use super::*;
use crate::adapt::Error;
use crate::life::{Ends, Row, Tie};
use crate::query::{self, Pack, Tree};
use crate::wire::Wire;

impl<W: Wire> Face<'_, W> {
    pub fn who(&self) -> Who {
        self.who
    }

    async fn read<T>(
        &self,
        run: impl AsyncFnOnce(&mut Tx<'_, W>) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let mut seat = self.core.seize().await?;
        let mut tx = Tx {
            core: self.core,
            seat: &mut *seat,
            who: self.who,
        };
        run(&mut tx).await
    }

    async fn write<T>(
        &self,
        run: impl AsyncFnOnce(&mut Tx<'_, W>) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let mut seat = self.core.seize().await?;
        seat.open().await?;
        let mut tx = Tx {
            core: self.core,
            seat: &mut *seat,
            who: self.who,
        };
        let out = run(&mut tx).await;
        match out {
            Ok(value) => {
                seat.close(true).await?;
                Ok(value)
            }
            Err(err) => {
                let _ = seat.close(false).await;
                self.core.stash.spoil();
                Err(err)
            }
        }
    }

    pub async fn put(&self, name: &str, fields: &[(&str, &str)]) -> Result<i64, Error> {
        self.write(async |tx| tx.put(name, fields).await).await
    }

    pub async fn set(&self, name: &str, key: i64, fields: &[(&str, &str)]) -> Result<(), Error> {
        self.write(async |tx| tx.set(name, key, fields).await).await
    }

    pub async fn end(&self, name: &str, key: i64) -> Result<(), Error> {
        self.write(async |tx| tx.end(name, key).await).await
    }

    pub async fn lease(&self, name: &str, key: i64, at: i64) -> Result<(), Error> {
        self.write(async |tx| tx.lease(name, key, at).await).await
    }

    pub async fn tie(
        &self,
        owner: &str,
        bond: &str,
        ends: Ends,
        fields: &[(&str, &str)],
    ) -> Result<i64, Error> {
        self.write(async |tx| tx.tie(owner, bond, ends, fields).await)
            .await
    }

    pub async fn set_tie(
        &self,
        owner: &str,
        bond: &str,
        key: i64,
        fields: &[(&str, &str)],
    ) -> Result<(), Error> {
        self.write(async |tx| tx.set_tie(owner, bond, key, fields).await)
            .await
    }

    pub async fn cut(&self, owner: &str, bond: &str, key: i64) -> Result<(), Error> {
        self.write(async |tx| tx.cut(owner, bond, key).await).await
    }

    pub async fn live(&self, name: &str) -> Result<Vec<Row>, Error> {
        self.read(async |tx| tx.live(name).await).await
    }

    pub async fn query(&self, text: &str) -> Result<Pack, Error> {
        let tree = query::parse(text)?;
        self.ask(&tree).await
    }

    pub async fn ask(&self, tree: &Tree) -> Result<Pack, Error> {
        self.read(async |tx| tx.ask(tree).await).await
    }

    pub async fn ties(&self, owner: &str, bond: &str, left: i64) -> Result<Vec<Tie>, Error> {
        self.read(async |tx| tx.ties(owner, bond, left).await).await
    }

    pub async fn flow(&self, cursor: i64) -> Result<Vec<Row>, Error> {
        self.read(async |tx| tx.flow(cursor).await).await
    }

    pub async fn batch<T>(
        &self,
        run: impl AsyncFnOnce(&mut Tx<'_, W>) -> Result<T, Error>,
    ) -> Result<T, Error> {
        self.write(run).await
    }
}

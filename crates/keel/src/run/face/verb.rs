use super::*;
use crate::adapt::Error;
use crate::cap;
use crate::life::{Row, Work};
use crate::query::{self, Pack, Tree};
use crate::wire::Wire;

impl<W: Wire> Tx<'_, W> {
    pub async fn set(
        &mut self,
        name: &str,
        key: i64,
        fields: &[(&str, &str)],
    ) -> Result<(), Error> {
        if self.free() {
            return self.shift(name, key, fields).await;
        }
        let unit = query::resolve(self.plan(), name)?;
        let pre = self.seen(&unit, key).await?;
        let mark = cap::Mark {
            key: Some(key),
            cells: pre.cells(),
        };
        self.may("set", &unit, &mark).await?;
        let mut post = pre.cells().clone();
        cap::blend(self.plan(), &unit, &mut post, fields);
        let after = cap::Mark {
            key: Some(key),
            cells: &post,
        };
        self.may("set", &unit, &after).await?;
        self.shift(&unit, key, fields).await
    }

    pub async fn end(&mut self, name: &str, key: i64) -> Result<(), Error> {
        if self.free() {
            return self.fell(name, key, None).await;
        }
        let unit = query::resolve(self.plan(), name)?;
        if unit == cap::GRANT {
            return self.revoke(key).await;
        }
        let row = self.seen(&unit, key).await?;
        let mark = cap::Mark {
            key: Some(key),
            cells: row.cells(),
        };
        self.may("end", &unit, &mark).await?;
        self.fell(&unit, key, None).await
    }

    pub async fn lease(&mut self, name: &str, key: i64, at: i64) -> Result<(), Error> {
        if self.free() {
            return self.fell(name, key, Some(at)).await;
        }
        let unit = query::resolve(self.plan(), name)?;
        let row = self.seen(&unit, key).await?;
        let mark = cap::Mark {
            key: Some(key),
            cells: row.cells(),
        };
        self.may("end", &unit, &mark).await?;
        self.fell(&unit, key, Some(at)).await
    }

    pub async fn live(&mut self, name: &str) -> Result<Vec<Row>, Error> {
        let unit = query::resolve(self.plan(), name)?;
        let pack = self.sight(&query::form(&unit)).await?;
        let mut rows = pack.rows().to_vec();
        if self.free() {
            return Ok(rows);
        }
        self.sift(&unit, &mut rows).await?;
        Ok(rows)
    }

    pub(super) async fn sift(&mut self, unit: &str, rows: &mut Vec<Row>) -> Result<(), Error> {
        let mut keep = Vec::new();
        for row in rows.iter() {
            let mark = cap::Mark {
                key: Some(row.key()),
                cells: row.cells(),
            };
            if self.held("see", unit, &mark).await? {
                keep.push(row.key());
            }
        }
        rows.retain(|row| keep.contains(&row.key()));
        Ok(())
    }

    pub async fn query(&mut self, text: &str) -> Result<Pack, Error> {
        let tree = query::parse(text)?;
        self.ask(&tree).await
    }

    pub async fn one(&mut self, tree: &Tree) -> Result<Option<Row>, Error> {
        let pack = self.ask(tree).await?;
        Ok(pack.rows().first().cloned())
    }

    pub async fn ask(&mut self, tree: &Tree) -> Result<Pack, Error> {
        if self.free() {
            return self.sight(tree).await;
        }
        let unit = query::resolve(self.plan(), tree.from())?;
        if tree.tally() {
            let flat = query::bare(tree);
            let pack = self.sight(&flat).await?;
            let mut rows = pack.rows().to_vec();
            self.sift(&unit, &mut rows).await?;
            return Ok(Pack::tallied(unit, rows.len()));
        }
        let mut pack = self.sight(tree).await?;
        self.strain(&unit, &mut pack).await?;
        Ok(pack)
    }

    pub(super) async fn strain(&mut self, unit: &str, pack: &mut Pack) -> Result<(), Error> {
        let root = unit.to_string();
        let mut kept: Vec<i64> = Vec::new();
        if let Some(crate::query::Bag::Unit(rows)) = pack.amend().get_mut(&root) {
            self.sift(unit, rows).await?;
            kept = rows.iter().map(Row::key).collect();
        }
        let node = self.plan().find(unit)?;
        let bonds: Vec<(String, String)> = node
            .bonds()
            .iter()
            .map(|e| (format!("{root}.{}", e.name()), e.target().to_string()))
            .collect();
        for (key, target) in bonds {
            let Some(crate::query::Bag::Bond(ties)) = pack.amend().get_mut(&key) else {
                continue;
            };
            let mut hold = Vec::new();
            for tie in ties.iter() {
                if !kept.contains(&tie.left()) {
                    continue;
                }
                if self.spot(&target, tie.right()).await? {
                    hold.push(tie.key());
                }
            }
            ties.retain(|tie| hold.contains(&tie.key()));
        }
        Ok(())
    }

    pub(super) async fn spot(&mut self, unit: &str, key: i64) -> Result<bool, Error> {
        let plan = self.core.plan();
        let node = plan.find(unit)?;
        let Some(row) = Work::new(&mut self.seat.wire, plan).one(node, key).await? else {
            return Ok(false);
        };
        let mark = cap::Mark {
            key: Some(key),
            cells: row.cells(),
        };
        self.held("see", unit, &mark).await
    }
}

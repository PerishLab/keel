use super::Work;
use crate::adapt::Error;
use crate::life::{Cell, Row};
use crate::model::manifest::rows::{BOND, FIELD, Line, SCOPE, UNIT, VALUE};
use crate::wire::Wire;
use std::collections::BTreeMap;

struct Spot {
    unit: String,
    bond: String,
    name: String,
}

#[derive(Default)]
struct Back {
    units: BTreeMap<i64, String>,
    bonds: BTreeMap<i64, String>,
    fields: BTreeMap<i64, Spot>,
}

fn cell(row: &Row, head: &str) -> Option<(String, String)> {
    row.cell(head).map(|held| (head.to_string(), held.show()))
}

fn draw(row: &Row, heads: &[&str]) -> Vec<(String, String)> {
    heads.iter().filter_map(|head| cell(row, head)).collect()
}

fn kept(held: &[i64], row: &Row, head: &str) -> bool {
    let at = row.cell(head).map(Cell::show).unwrap_or_default();
    at.parse::<i64>().is_ok_and(|at| held.contains(&at))
}

impl Back {
    fn owns(&self, row: &Row) -> bool {
        let held = row.cell("unit").map(Cell::show).unwrap_or_default();
        held.parse::<i64>()
            .is_ok_and(|at| self.units.contains_key(&at))
    }

    fn tied(&self, row: &Row) -> bool {
        let held = row.cell("field").map(Cell::show).unwrap_or_default();
        held.parse::<i64>()
            .is_ok_and(|at| self.fields.contains_key(&at))
    }

    fn moor(&self, row: &Row) -> Result<Vec<(String, String)>, Error> {
        let held = row.cell("unit").map(Cell::show).unwrap_or_default();
        let key = held.parse::<i64>().ok().and_then(|at| self.units.get(&at));
        let mut out = vec![(
            "unit".to_string(),
            key.cloned()
                .ok_or_else(|| Error::Adapt(format!("schema unit {held}")))?,
        )];
        if let Some(held) = row.cell("bond").map(Cell::show) {
            let tied = held.parse::<i64>().ok().and_then(|at| self.bonds.get(&at));
            let tied = tied.ok_or_else(|| Error::Adapt(format!("schema bond {held}")))?;
            out.push(("bond".to_string(), tied.clone()));
        }
        Ok(out)
    }

    fn leaf(&self, row: &Row, name: &'static str) -> Result<Vec<(String, String)>, Error> {
        let held = row.cell("field").map(Cell::show).unwrap_or_default();
        let at = held.parse::<i64>().ok().and_then(|at| self.fields.get(&at));
        let at = at.ok_or_else(|| Error::Adapt(format!("schema field {held}")))?;
        let mut cells = vec![
            ("unit".to_string(), at.unit.clone()),
            ("name".to_string(), at.name.clone()),
        ];
        if !at.bond.is_empty() {
            cells.push(("bond".to_string(), at.bond.clone()));
        }
        let head = if name == SCOPE { "scope" } else { "value" };
        cells.extend(cell(row, head));
        Ok(cells)
    }
}

impl<W: Wire> Work<'_, W> {
    pub(crate) async fn glean(&mut self) -> Result<Vec<Line>, Error> {
        let roots = self.sift(UNIT).await?;
        self.weave(roots).await
    }

    pub(crate) async fn recall(&mut self, at: i64) -> Result<Vec<Line>, Error> {
        let held = at.to_string();
        let roots: Vec<Row> = self
            .every(UNIT)
            .await?
            .into_iter()
            .filter(|row| row.cell("generation").map(Cell::show) == Some(held.clone()))
            .collect();
        self.weave(roots).await
    }

    async fn weave(&mut self, roots: Vec<Row>) -> Result<Vec<Line>, Error> {
        let mut back = Back::default();
        let mut out = Vec::new();
        for row in roots {
            back.units.insert(
                row.key(),
                row.cell("key").map(Cell::show).unwrap_or_default(),
            );
            out.push(Line {
                unit: UNIT,
                cells: draw(&row, &["key", "name", "veil", "frozen"]),
            });
        }
        for row in self.every(BOND).await? {
            if !back.owns(&row) {
                continue;
            }
            let mut cells = back.moor(&row)?;
            let name = row.cell("name").map(Cell::show).unwrap_or_default();
            back.bonds.insert(row.key(), name);
            cells.extend(draw(
                &row,
                &["name", "kind", "target", "need", "root", "crew"],
            ));
            out.push(Line { unit: BOND, cells });
        }
        for row in self.every(FIELD).await? {
            if !back.owns(&row) {
                continue;
            }
            let mut cells = back.moor(&row)?;
            let name = row.cell("name").map(Cell::show).unwrap_or_default();
            let bond = cells
                .iter()
                .find(|(head, _)| head == "bond")
                .map(|(_, at)| at.clone());
            back.fields.insert(
                row.key(),
                Spot {
                    unit: cells[0].1.clone(),
                    bond: bond.unwrap_or_default(),
                    name,
                },
            );
            cells.extend(draw(
                &row,
                &[
                    "name", "kind", "only", "need", "serial", "fallback", "min", "max",
                ],
            ));
            out.push(Line { unit: FIELD, cells });
        }
        for name in [SCOPE, VALUE] {
            for row in self.every(name).await? {
                if !back.tied(&row) {
                    continue;
                }
                out.push(Line {
                    unit: name,
                    cells: back.leaf(&row, name)?,
                });
            }
        }
        Ok(out)
    }

    pub(crate) async fn purge(&mut self, at: i64) -> Result<(), Error> {
        let held = at.to_string();
        let units: Vec<i64> = self
            .every(UNIT)
            .await?
            .iter()
            .filter(|row| row.cell("generation").map(Cell::show) == Some(held.clone()))
            .map(Row::key)
            .collect();
        let mut fields = Vec::new();
        let mut bonds = Vec::new();
        for row in self.every(FIELD).await? {
            if kept(&units, &row, "unit") {
                fields.push(row.key());
            }
        }
        for row in self.every(BOND).await? {
            if kept(&units, &row, "unit") {
                bonds.push(row.key());
            }
        }
        for name in [SCOPE, VALUE] {
            let mut leaves = Vec::new();
            for row in self.every(name).await? {
                if kept(&fields, &row, "field") {
                    leaves.push(row.key());
                }
            }
            self.raze(name, &leaves).await?;
        }
        self.raze(FIELD, &fields).await?;
        self.raze(BOND, &bonds).await?;
        self.raze(UNIT, &units).await?;
        Ok(())
    }

    async fn raze(&mut self, name: &'static str, keys: &[i64]) -> Result<(), Error> {
        let unit = self.plan.find(name)?;
        let text = format!(
            "DELETE FROM {} WHERE {} = ?1",
            crate::ddl::seat(unit),
            crate::ddl::KEY
        );
        for key in keys {
            self.wire.run(&text, &[crate::wire::Val::Int(*key)]).await?;
        }
        Ok(())
    }

    async fn sift(&mut self, name: &'static str) -> Result<Vec<Row>, Error> {
        let unit = self.plan.find(name)?;
        self.scan(unit).await
    }

    async fn every(&mut self, name: &'static str) -> Result<Vec<Row>, Error> {
        let unit = self.plan.find(name)?;
        let text = format!(
            "SELECT {} FROM {} ORDER BY {}",
            unit.sheet(),
            crate::ddl::seat(unit),
            crate::ddl::KEY
        );
        let mut out = Vec::new();
        for line in self.wire.rows(&text, &[]).await? {
            out.push(Row::read(unit, &line)?);
        }
        Ok(out)
    }
}

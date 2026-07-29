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

fn moor(back: &Back, row: &Row) -> Result<Vec<(String, String)>, Error> {
    let held = row.cell("unit").map(Cell::show).unwrap_or_default();
    let key = held.parse::<i64>().ok().and_then(|at| back.units.get(&at));
    let mut out = vec![(
        "unit".to_string(),
        key.cloned()
            .ok_or_else(|| Error::Adapt(format!("schema unit {held}")))?,
    )];
    if let Some(held) = row.cell("bond").map(Cell::show) {
        let tied = held.parse::<i64>().ok().and_then(|at| back.bonds.get(&at));
        let tied = tied.ok_or_else(|| Error::Adapt(format!("schema bond {held}")))?;
        out.push(("bond".to_string(), tied.clone()));
    }
    Ok(out)
}

impl<W: Wire> Work<'_, W> {
    pub(crate) async fn glean(&mut self) -> Result<Vec<Line>, Error> {
        let mut back = Back::default();
        let mut out = Vec::new();
        for row in self.sift(UNIT).await? {
            back.units.insert(
                row.key(),
                row.cell("key").map(Cell::show).unwrap_or_default(),
            );
            out.push(Line {
                unit: UNIT,
                cells: draw(&row, &["key", "name", "veil", "frozen"]),
            });
        }
        for row in self.sift(BOND).await? {
            let mut cells = moor(&back, &row)?;
            let name = row.cell("name").map(Cell::show).unwrap_or_default();
            back.bonds.insert(row.key(), name);
            cells.extend(draw(
                &row,
                &["name", "kind", "target", "need", "root", "crew"],
            ));
            out.push(Line { unit: BOND, cells });
        }
        for row in self.sift(FIELD).await? {
            let mut cells = moor(&back, &row)?;
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
            for row in self.sift(name).await? {
                out.push(Line {
                    unit: name,
                    cells: leaf(&back, &row, name)?,
                });
            }
        }
        Ok(out)
    }

    async fn sift(&mut self, name: &'static str) -> Result<Vec<Row>, Error> {
        let unit = self.plan.find(name)?;
        self.scan(unit).await
    }
}

fn leaf(back: &Back, row: &Row, name: &'static str) -> Result<Vec<(String, String)>, Error> {
    let held = row.cell("field").map(Cell::show).unwrap_or_default();
    let at = held.parse::<i64>().ok().and_then(|at| back.fields.get(&at));
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

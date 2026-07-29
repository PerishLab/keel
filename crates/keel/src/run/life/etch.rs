use super::Work;
use crate::adapt::Error;
use crate::life::Row;
use crate::model::manifest::rows::{BOND, FIELD, Line, SCOPE, UNIT, VALUE};
use crate::wire::Wire;
use std::collections::BTreeMap;

#[derive(Default)]
struct Seen {
    units: BTreeMap<String, i64>,
    bonds: BTreeMap<String, i64>,
    fields: BTreeMap<String, i64>,
}

impl Seen {
    fn keep(&mut self, line: &Line, key: i64) {
        match line.unit {
            UNIT => {
                self.units.insert(line.look("key"), key);
            }
            BOND => {
                self.bonds.insert(under(line, "name"), key);
            }
            FIELD => {
                self.fields.insert(under(line, "name"), key);
            }
            _ => (),
        }
    }

    fn find(&self, hold: &BTreeMap<String, i64>, at: &str) -> Result<String, Error> {
        hold.get(at)
            .map(i64::to_string)
            .ok_or_else(|| Error::Adapt(format!("schema row {at}")))
    }
}

fn under(line: &Line, head: &str) -> String {
    format!(
        "{}\u{1}{}\u{1}{}",
        line.look("unit"),
        line.look("bond"),
        line.look(head)
    )
}

fn owner(line: &Line) -> String {
    format!("{}\u{1}\u{1}{}", line.look("unit"), line.look("bond"))
}

fn tag(line: &Line, seen: &Seen) -> Result<Vec<(String, String)>, Error> {
    if line.unit == UNIT {
        return Ok(line.cells.clone());
    }
    let mut cells = line.cells.clone();
    if line.unit == SCOPE || line.unit == VALUE {
        let held = seen.find(&seen.fields, &under(line, "name"))?;
        cells.retain(|(head, _)| head != "unit" && head != "bond" && head != "name");
        cells.push(("field".into(), held));
        return Ok(cells);
    }
    let owns = seen.find(&seen.units, &line.look("unit"))?;
    let tied = match line.look("bond").is_empty() {
        true => None,
        false => Some(seen.find(&seen.bonds, &owner(line))?),
    };
    cells.retain(|(head, _)| head != "unit" && head != "bond");
    cells.push(("unit".into(), owns));
    if let Some(tied) = tied {
        cells.push(("bond".into(), tied));
    }
    Ok(cells)
}

impl<W: Wire> Work<'_, W> {
    pub(crate) async fn erase(&mut self) -> Result<(), Error> {
        for name in [VALUE, SCOPE, FIELD, BOND, UNIT] {
            let unit = self.plan.find(name)?;
            let held: Vec<i64> = self.scan(unit).await?.iter().map(Row::key).collect();
            for key in held {
                self.end(name, key).await?;
            }
        }
        Ok(())
    }

    pub(crate) async fn etch(&mut self, lines: &[Line]) -> Result<(), Error> {
        let mut seen = Seen::default();
        for line in lines {
            let cells = tag(line, &seen)?;
            let pairs: Vec<(&str, &str)> = cells
                .iter()
                .map(|(head, value)| (head.as_str(), value.as_str()))
                .collect();
            let unit = self.plan.find(line.unit)?;
            let key = self.craft(unit, &pairs).await?;
            seen.keep(line, key);
        }
        Ok(())
    }
}

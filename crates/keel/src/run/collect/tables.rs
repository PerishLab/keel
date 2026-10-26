use crate::adapt::Error;
use crate::bond::Kind;
use crate::ddl;
use crate::plan::{Plan, Unit};

pub(crate) struct Table {
    pub(crate) path: String,
    pub(crate) place: String,
    pub(crate) test: String,
}

pub(super) fn build(plan: &Plan) -> Result<Vec<Table>, Error> {
    let mut out = Vec::new();
    for unit in units(plan) {
        for edge in unit
            .bonds()
            .iter()
            .filter(|edge| edge.kind() == Kind::Many2many)
        {
            let mate = plan.find(edge.target())?;
            let left = ddl::col(&ddl::side(unit.name()));
            let right = ddl::col(&ddl::mate(unit.name(), edge.name(), edge.target()));
            out.push(Table {
                path: format!("{}.{}", unit.key(), edge.name()),
                place: ddl::joint(unit, edge.name()),
                test: format!(
                    "(c.expires_at <= ?1 OR EXISTS (SELECT 1 FROM {} AS h WHERE h.id = c.{left} AND h.expires_at <= ?1) OR EXISTS (SELECT 1 FROM {} AS t WHERE t.id = c.{right} AND t.expires_at <= ?1))",
                    ddl::seat(unit), ddl::seat(mate)
                ),
            });
        }
    }
    for unit in units(plan) {
        out.push(Table {
            path: unit.key(),
            place: ddl::seat(unit),
            test: guard(plan, unit)?,
        });
    }
    Ok(out)
}

fn guard(plan: &Plan, target: &Unit) -> Result<String, Error> {
    let mut test = String::from("c.expires_at <= ?1");
    for holder in plan.units().values() {
        for edge in holder.bonds() {
            let mate = plan.find(edge.target())?;
            let points = mate.key() == target.key();
            if edge.kind().point() && points {
                test.push_str(&format!(
                    " AND NOT EXISTS (SELECT 1 FROM {} AS r WHERE r.{} = c.id AND (r.expires_at IS NULL OR r.expires_at > ?1))",
                    ddl::seat(holder), ddl::col(&ddl::side(edge.name()))
                ));
            }
            if edge.kind() != Kind::Many2many {
                continue;
            }
            if holder.key() == target.key() {
                test.push_str(&free(
                    &ddl::joint(holder, edge.name()),
                    &ddl::side(holder.name()),
                ));
            }
            if points {
                test.push_str(&free(
                    &ddl::joint(holder, edge.name()),
                    &ddl::mate(holder.name(), edge.name(), edge.target()),
                ));
            }
        }
    }
    Ok(test)
}

fn free(table: &str, column: &str) -> String {
    format!(
        " AND NOT EXISTS (SELECT 1 FROM {table} AS r WHERE r.{} = c.id)",
        ddl::col(column)
    )
}

fn units(plan: &Plan) -> Vec<&Unit> {
    plan.units()
        .values()
        .filter(|unit| !unit.name().starts_with('@') || unit.name() == crate::cap::GRANT)
        .collect()
}

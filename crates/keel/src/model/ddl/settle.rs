use super::{Grain, col, join, project, script, stage};
use crate::bond;
use crate::plan::{Plan, Unit};

pub(crate) fn settled(plan: &Plan, grain: Grain, generation: i64) -> Vec<String> {
    if generation == 1 {
        return script(plan, grain);
    }
    let engines: Vec<&Unit> = plan
        .units()
        .values()
        .filter(|unit| unit.name().starts_with('@'))
        .collect();
    let mut out = project(&engines, grain, None);
    out.extend(super::candidate(plan, grain, generation));
    let tables = plan.staged().into_iter().flat_map(|node| {
        std::iter::once(node.table()).chain(
            node.bonds()
                .iter()
                .filter(|edge| edge.kind() == bond::Kind::Many2many)
                .map(move |edge| join(node, edge.name())),
        )
    });
    for table in tables {
        out.push(format!(
            "ALTER TABLE {} RENAME TO {};",
            col(&stage(generation, &table)),
            col(&table)
        ));
    }
    out
}

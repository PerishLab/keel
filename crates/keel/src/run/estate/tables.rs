use crate::model::manifest::{Bond, Manifest};
use std::collections::BTreeSet;

pub(super) fn of(manifest: &Manifest) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for unit in manifest.units() {
        out.insert(unit.table());
        for edge in unit
            .bonds
            .iter()
            .filter(|edge| edge.kind == Bond::Many2many)
        {
            out.insert(crate::ddl::joiner(&unit.table(), &edge.name));
            if edge.closure {
                let name = format!("{}_closure", edge.name);
                out.insert(crate::ddl::joiner(&unit.table(), &name));
            }
        }
    }
    out
}

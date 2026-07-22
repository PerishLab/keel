use crate::plan::Plan;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Path {
    unit: String,
    route: String,
}

impl Path {
    pub fn unit(&self) -> &str {
        &self.unit
    }

    pub fn route(&self) -> &str {
        &self.route
    }
}

pub fn paths(plan: &Plan) -> Vec<Path> {
    let mut count: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for unit in plan.units().values() {
        *count.entry(unit.name().to_ascii_lowercase()).or_default() += 1;
    }
    let mut paths = Vec::new();
    for unit in plan.units().values() {
        let short = unit.name().to_ascii_lowercase();
        if count[&short] > 1 {
            continue;
        }
        paths.push(Path {
            unit: unit.name().to_string(),
            route: format!("/{short}"),
        });
    }
    paths.sort_by(|a, b| a.route.cmp(&b.route));
    paths
}

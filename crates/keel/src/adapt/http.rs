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
    let mut paths = Vec::new();
    for unit in plan.units().values() {
        paths.push(Path {
            unit: unit.name().to_string(),
            route: format!("/{}", crate::name::key(unit.name())),
        });
    }
    paths.sort_by(|a, b| a.route.cmp(&b.route));
    paths
}

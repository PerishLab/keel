use crate::spec::{Resource, Spec};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default)]
pub struct Graph {
    nodes: BTreeMap<String, Spec>,
}

impl Graph {
    pub fn new() -> Self {
        Self {
            nodes: BTreeMap::new(),
        }
    }

    pub fn plug<R: Resource>(&mut self) -> &mut Self {
        let spec = R::spec();
        self.nodes.insert(spec.name().to_string(), spec);
        self
    }

    pub fn nodes(&self) -> &BTreeMap<String, Spec> {
        &self.nodes
    }
}

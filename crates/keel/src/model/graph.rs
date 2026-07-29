use crate::spec::{Resource, Spec};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default)]
pub struct Graph {
    nodes: BTreeMap<String, Spec>,
    conflicts: std::collections::BTreeSet<String>,
}

impl Graph {
    pub fn new() -> Self {
        Self {
            nodes: BTreeMap::new(),
            conflicts: std::collections::BTreeSet::new(),
        }
    }

    pub fn plug<R: Resource>(&mut self) -> &mut Self {
        self.add(R::spec())
    }

    pub fn add(&mut self, spec: Spec) -> &mut Self {
        let key = spec.key();
        if self.nodes.insert(key.clone(), spec).is_some() {
            self.conflicts.insert(key);
        }
        self
    }

    pub fn read(text: &str) -> Result<Self, crate::adapt::Error> {
        crate::model::manifest::hydrate::read(text)
    }

    pub fn nodes(&self) -> &BTreeMap<String, Spec> {
        &self.nodes
    }

    pub(crate) fn conflicts(&self) -> &std::collections::BTreeSet<String> {
        &self.conflicts
    }
}

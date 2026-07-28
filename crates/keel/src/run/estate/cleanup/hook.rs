use std::future::Future;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Gone {
    path: String,
    key: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Purge {
    generation: i64,
    gone: Vec<Gone>,
}

pub trait Hook: Send {
    fn purge(&mut self, event: Purge) -> impl Future<Output = Result<(), String>> + Send;
}

impl Gone {
    pub(crate) fn new(path: String, key: i64) -> Self {
        Self { path, key }
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn key(&self) -> i64 {
        self.key
    }
}

impl Purge {
    pub(crate) fn new(generation: i64, gone: Vec<Gone>) -> Self {
        Self { generation, gone }
    }

    pub fn generation(&self) -> i64 {
        self.generation
    }

    pub fn gone(&self) -> &[Gone] {
        &self.gone
    }
}

impl Hook for () {
    async fn purge(&mut self, event: Purge) -> Result<(), String> {
        drop(event);
        Ok(())
    }
}

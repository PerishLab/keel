use super::*;
use crate::face::{Core, Face};
use crate::wire::Wire;
use axum::Extension;
use axum::extract::State;
use axum::http::HeaderMap;
use std::sync::Arc;

pub(crate) type Seat<W> = (State<Arc<Core<W>>>, HeaderMap, Option<Extension<Operator>>);

pub(crate) struct Plea<W: Wire> {
    pub(super) core: Arc<Core<W>>,
    headers: HeaderMap,
    op: Option<Operator>,
}

impl<W: Wire> From<Seat<W>> for Plea<W> {
    fn from((State(core), headers, op): Seat<W>) -> Self {
        Self {
            core,
            headers,
            op: op.map(|Extension(op)| op),
        }
    }
}

impl<W: Wire> Plea<W> {
    pub(super) async fn face(&self, verb: &str) -> Result<Face<'_, W>, Fault> {
        front(self.core.as_ref(), &self.headers, self.op.as_ref(), verb).await
    }
}

use super::engine::Engine;
use super::plan::Plan;
use super::{Output, Query, Stats};
use crate::query::meter::Meter;
use crate::{Error, Store};
use keel::Wire;

impl<W: Wire> Store<W> {
    pub async fn join(&self, query: &Query, output: &mut Output<'_>) -> Result<Stats, Error> {
        let plan = Plan::new(&self.table, query)?;
        let mut scan = crate::Stats::default();
        let (left, right) = {
            let state = self.state.lock().await;
            let left = self.capture(&state, (&plan.left.plan, &plan.left.query), &mut scan)?;
            let right = self.capture(&state, (&plan.right.plan, &plan.right.query), &mut scan)?;
            (left, right)
        };
        let meter = Meter::new(query.limit.scan.bytes);
        let mut engine = Engine::new(&plan, query);
        self.execute(
            right,
            (&plan.right.plan, &plan.right.query),
            (&meter, &mut scan, &mut |batch| engine.build(batch)),
        )?;
        self.execute(
            left,
            (&plan.left.plan, &plan.left.query),
            (&meter, &mut scan, &mut |batch| engine.probe(batch, output)),
        )?;
        scan.bytes = meter.bytes();
        engine.finish(scan, output)
    }
}

use super::{Core, Seat};
use crate::adapt::Error;
use crate::config::{Collection, Retain};
use crate::estate::{Gone, Hook};
use crate::run::collect::{self, Work};
use crate::wire::Wire;
use std::sync::{Arc, Weak};
use std::time::Duration;
use tokio::sync::{Mutex, mpsc};
use tokio::task::JoinHandle;

#[derive(Default)]
pub(super) struct Sweep {
    jobs: Vec<JoinHandle<()>>,
}

impl Drop for Sweep {
    fn drop(&mut self) {
        for job in &self.jobs {
            job.abort();
        }
    }
}

impl<W: Wire + 'static> Core<W> {
    pub(crate) fn collect<H: Hook + 'static>(
        mut self,
        policy: Collection,
        hook: Option<H>,
    ) -> Self {
        let Retain::Span(age) = policy.retain else {
            return self;
        };
        let Ok(age) = i64::try_from(age) else {
            return self;
        };
        let (send, worker) = observe(hook);
        self.sweep.jobs.extend(worker);
        let seat = Arc::downgrade(&self.seat);
        let plan = self.plan.clone();
        self.sweep
            .jobs
            .push(tokio::spawn(run(seat, plan, (policy, age), send)));
        self
    }
}

async fn run<W: Wire>(
    seat: Weak<Mutex<Seat<W>>>,
    plan: crate::plan::Plan,
    policy: (Collection, i64),
    send: Option<mpsc::Sender<Vec<Gone>>>,
) {
    let tables = match collect::tables(&plan) {
        Ok(tables) => tables,
        Err(err) => {
            eprintln!("keel: collection: {err}");
            return;
        }
    };
    let digest = collect::digest(&plan);
    loop {
        tokio::time::sleep(Duration::from_secs(policy.0.period)).await;
        let cutoff = crate::life::tick().saturating_sub(policy.1);
        for table in &tables {
            let work = Work {
                table,
                cutoff,
                limit: policy.0.batch,
                digest: &digest,
            };
            if !pass(&seat, work, send.as_ref()).await {
                return;
            }
            tokio::task::yield_now().await;
        }
    }
}

fn observe<H: Hook + 'static>(
    hook: Option<H>,
) -> (Option<mpsc::Sender<Vec<Gone>>>, Option<JoinHandle<()>>) {
    let Some(mut hook) = hook else {
        return (None, None);
    };
    let (send, mut receive) = mpsc::channel(8);
    let job = tokio::spawn(async move {
        while let Some(gone) = receive.recv().await {
            report(tokio::time::timeout(Duration::from_secs(1), hook.collect(gone)).await);
        }
    });
    (Some(send), Some(job))
}

fn report(result: Result<Result<(), String>, tokio::time::error::Elapsed>) {
    match result {
        Ok(Ok(())) => {}
        Ok(Err(note)) => eprintln!("keel: collection hook: {note}"),
        Err(_) => eprintln!("keel: collection hook timed out"),
    }
}

async fn pass<W: Wire>(
    seat: &Weak<Mutex<Seat<W>>>,
    work: Work<'_>,
    send: Option<&mpsc::Sender<Vec<Gone>>>,
) -> bool {
    let Some(seat) = seat.upgrade() else {
        return false;
    };
    let gone = {
        let mut seat = seat.lock().await;
        batch(&mut seat, work).await
    };
    match gone {
        Ok(gone) if !gone.is_empty() => {
            if let Some(send) = send {
                let _ = send.try_send(gone);
            }
        }
        Err(err) => eprintln!("keel: collection: {err}"),
        _ => {}
    }
    true
}

async fn batch<W: Wire>(seat: &mut Seat<W>, work: Work<'_>) -> Result<Vec<Gone>, Error> {
    seat.wire.revive().await?;
    if seat.dirty {
        seat.wire.script("ROLLBACK").await?;
        seat.dirty = false;
        seat.stale = true;
    }
    seat.open().await?;
    let out = collect::run(work, &mut seat.wire).await;
    seat.close(out.is_ok()).await?;
    out
}

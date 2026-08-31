//! Execution drivers: the only seam between the deterministic scheduling core and real work.
//!
//! [`Driver`] is deliberately tiny — dispatch, poll, clock, shutdown. The core never spawns
//! anything and never blocks; drivers decide what "running a task" means. [`InlineDriver`]
//! executes synchronously inside `dispatch` (deterministic; used by simulation and benches).
//! [`ThreadDriver`] runs a *fixed* pool of worker threads fed by a bounded channel: when the
//! channel is full, `dispatch` reports backpressure and the scheduler defers — the concurrency
//! bound is structural, not aspirational.
//!
//! Every driver path funnels through [`run_bound_job`], which enforces the two pre-execution
//! checks in one place: cancellation (generation comparison) and payload-digest binding. A
//! handler therefore cannot run for a cancelled task or against bytes that no longer match the
//! envelope's digest — the failure modes are [`Outcome::CancelledBeforeStart`] and a failed
//! attempt, never silent execution.

use crate::cancel::CancelState;
use crate::envelope::{Completion, DispatchJob, Outcome};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;

pub trait Handler: Send + Sync {
    /// Executes one attempt. Implementations must not panic on bad payloads; returning
    /// [`Outcome::Failed`] is how work reports "I could not do this".
    fn execute(&self, job: &DispatchJob) -> Outcome;
}

/// Simple function-backed handler.
pub struct FnHandler<F>(pub F);

impl<F> Handler for FnHandler<F>
where
    F: Fn(&DispatchJob) -> Outcome + Send + Sync,
{
    fn execute(&self, job: &DispatchJob) -> Outcome {
        (self.0)(job)
    }
}

/// Shared pre-execution gate: cancellation check, then payload verification, then the handler.
/// Returns the completion for one attempt.
pub fn run_bound_job(
    handler: &dyn Handler,
    job: &DispatchJob,
    cancels: &CancelState,
) -> Completion {
    let task = job.envelope.task();
    let agent = job.agent;
    let epoch = job.lease_epoch;
    let attempt = job.attempt;

    if cancels.is_cancelled_since(task, job.cancel_gen) {
        return Completion {
            task,
            agent,
            attempt,
            lease_epoch: epoch,
            outcome: Outcome::CancelledBeforeStart,
        };
    }

    if !job.envelope.verify_payload(job.envelope.payload()) {
        // Payload bytes disagree with their bound digest: refuse to execute rather than run an
        // unattested input and guess.
        return Completion {
            task,
            agent,
            attempt,
            lease_epoch: epoch,
            outcome: Outcome::Failed {
                reason: "payload digest mismatch before execution".into(),
            },
        };
    }

    let outcome = handler.execute(job);
    Completion {
        task,
        agent,
        attempt,
        lease_epoch: epoch,
        outcome,
    }
}

/// How a driver accepted or refused one dispatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Enqueue {
    Accepted,
    /// The driver's inbound queue is full; the scheduler keeps the task and retries later.
    Full,
}

pub trait Driver: Send {
    fn dispatch(&mut self, job: DispatchJob) -> Enqueue;

    /// Drains completed attempts since the last call. Called by the scheduler each step.
    fn poll(&mut self) -> Vec<Completion>;

    /// Informs the driver of the fabric's current tick (simulation uses this to release
    /// completions on schedule). No-op for inline/threaded drivers.
    fn advance_clock(&mut self, _now: u64) {}

    /// Joins workers where applicable.
    fn shutdown(&mut self) {}
}

/// Synchronous driver: runs jobs immediately during `dispatch`. Determinism-friendly and the
/// fastest path for benches; there is no queue to fill so it never applies backpressure.
#[derive(Clone)]
pub struct InlineDriver {
    handler: Arc<dyn Handler>,
    cancels: CancelState,
    pending: Vec<Completion>,
}

impl InlineDriver {
    pub fn new(handler: Arc<dyn Handler>, cancels: CancelState) -> Self {
        Self {
            handler,
            cancels,
            pending: Vec::new(),
        }
    }
}

impl Driver for InlineDriver {
    fn dispatch(&mut self, job: DispatchJob) -> Enqueue {
        let completion = run_bound_job(self.handler.as_ref(), &job, &self.cancels);
        self.pending.push(completion);
        Enqueue::Accepted
    }

    fn poll(&mut self) -> Vec<Completion> {
        std::mem::take(&mut self.pending)
    }
}

/// Fixed-size bounded thread pool driver. Workers share one bounded hand-off channel; when all
/// slots are full `dispatch` returns `Enqueue::Full` and the scheduler defers the task. Worker
/// count is the fabric's real concurrency ceiling.
pub struct ThreadDriver {
    tx: Option<mpsc::SyncSender<DispatchJob>>,
    shared: Arc<Mutex<Vec<Completion>>>,
    joins: Vec<JoinHandle<()>>,
    overflow: Vec<Completion>,
}

impl ThreadDriver {
    /// Spawns `workers` threads. `channel_cap` bounds queued-but-unstarted jobs: together these
    /// two numbers are the whole memory story of the driver.
    pub fn new(
        handler: Arc<dyn Handler>,
        cancels: CancelState,
        workers: usize,
        channel_cap: usize,
    ) -> Self {
        assert!(workers > 0, "at least one worker");
        let (tx, rx) = mpsc::sync_channel::<DispatchJob>(channel_cap);
        let rx = Arc::new(Mutex::new(rx));
        let shared: Arc<Mutex<Vec<Completion>>> = Arc::new(Mutex::new(Vec::new()));
        let mut joins = Vec::with_capacity(workers);
        for _ in 0..workers {
            let rx = Arc::clone(&rx);
            let shared = Arc::clone(&shared);
            let handler = Arc::clone(&handler);
            let cancels = cancels.clone();
            joins.push(std::thread::spawn(move || loop {
                let job = {
                    let guard = rx.lock().expect("rx lock");
                    match guard.recv() {
                        Ok(j) => j,
                        Err(_) => break,
                    }
                };
                let c = run_bound_job(handler.as_ref(), &job, &cancels);
                shared.lock().expect("shared lock").push(c);
            }));
        }
        Self {
            tx: Some(tx),
            shared,
            joins,
            overflow: Vec::new(),
        }
    }
}

impl Driver for ThreadDriver {
    fn dispatch(&mut self, job: DispatchJob) -> Enqueue {
        match &self.tx {
            Some(tx) => match tx.try_send(job) {
                Ok(()) => Enqueue::Accepted,
                Err(mpsc::TrySendError::Full(_)) => Enqueue::Full,
                Err(mpsc::TrySendError::Disconnected(_)) => Enqueue::Full,
            },
            None => Enqueue::Full,
        }
    }

    fn poll(&mut self) -> Vec<Completion> {
        let drained: Vec<Completion> =
            std::mem::take(&mut *self.shared.lock().expect("shared lock"));
        let mut out = std::mem::take(&mut self.overflow);
        out.extend(drained);
        out
    }

    fn shutdown(&mut self) {
        self.tx.take();
        for j in self.joins.drain(..) {
            let _ = j.join();
        }
    }
}

impl Drop for ThreadDriver {
    fn drop(&mut self) {
        self.shutdown();
    }
}

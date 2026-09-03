//! The scheduling core: a deterministic state machine over virtual ticks.
//!
//! [`Fabric`] owns every scheduling decision and none of the execution. Per step it, in order:
//! polls driver completions, expires lapsed leases (the crash detector), releases due retries,
//! then dispatches from shard queues under four bounds at once — total in-flight, per-agent
//! single-tenancy, per-agent quota, and queue capacity. Every rejection path is observable
//! (metrics) or terminal (receipt); nothing is dropped silently.
//!
//! Determinism: all maps are ordered (`BTreeMap`), retry timing breaks ties by submission
//! sequence, shard preference comes from rendezvous hashing, and dispatch rotates across shards
//! with an explicit cursor. Given the same call sequence, two runs produce byte-identical
//! receipt streams — asserted by tests, not claimed.

use crate::cancel::CancelState;
use crate::capability::{Capability, CapabilitySet};
use crate::digest::sha256;
use crate::envelope::{Completion, DispatchJob, Outcome, Receipt, TaskEnvelope, Terminal};
use crate::exec::{Driver, Enqueue};
use crate::ids::{AgentId, IdempotencyKey, ShardId, TaskId};
use crate::lease::{ExpiredLease, LeaseError, LeaseHandle, LeaseTable};
use crate::queue::{Backpressure, BoundedQueue};
use crate::quota::{QuotaLedger, QuotaSpec};
use crate::retry::{RetryDecision, RetryPolicy};
use crate::router::{AgentState, Router};
use crate::shard;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Tuning knobs; every field is a bound the scheduler actually enforces.
#[derive(Clone, Debug)]
pub struct FabricConfig {
    pub shards: u64,
    /// Capacity of each shard's ready queue.
    pub per_shard_queue_cap: usize,
    /// Global ceiling on simultaneously executing attempts.
    pub max_in_flight: usize,
    pub default_lease_ttl_ticks: u64,
    pub retry: RetryPolicy,
    pub quota: QuotaSpec,
    /// Receipt ledger retention; oldest receipts are evicted (and counted) beyond this.
    pub receipt_retention: usize,
}

impl Default for FabricConfig {
    fn default() -> Self {
        Self {
            shards: 8,
            per_shard_queue_cap: 256,
            max_in_flight: 128,
            default_lease_ttl_ticks: 32,
            retry: RetryPolicy::default(),
            quota: QuotaSpec::unlimited(),
            receipt_retention: 4096,
        }
    }
}

pub enum Submission {
    Accepted { task: TaskId },
    Duplicate { task: TaskId },
    Rejected { pressure: Backpressure },
}

impl fmt::Debug for Submission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Submission::Accepted { task } => write!(f, "accepted({task})"),
            Submission::Duplicate { task } => write!(f, "duplicate({task})"),
            Submission::Rejected { pressure } => write!(f, "rejected({pressure})"),
        }
    }
}

struct TaskMeta {
    env: TaskEnvelope,
    submitted_tick: u64,
    attempts_done: u32,
    last_agent: Option<AgentId>,
    cancel_requested: bool,
    key: IdempotencyKey,
}

struct InFlight {
    agent: AgentId,
    attempt: u32,
    epoch: crate::ids::LeaseEpoch,
}

#[derive(Debug, Default, Clone)]
pub struct Metrics {
    pub submitted: u64,
    pub duplicate_submissions: u64,
    pub backpressure_rejections: u64,
    pub admitted: u64,
    pub dispatched: u64,
    pub retried: u64,
    pub deferred_by_quota: u64,
    pub deferred_by_driver_full: u64,
    pub unroutable_parks: u64,
    pub lease_expiries: u64,
    pub succeeded: u64,
    pub failed_terminal: u64,
    pub cancelled_terminal: u64,
    pub dropped_terminal: u64,
    pub corrupted_settlements: u64,
    pub receipts_evicted: u64,
    /// Idempotency entries dropped at settlement: the dedupe window is task-lifetime, and this
    /// counter makes every window close visible instead of silent.
    pub idempotency_evictions: u64,
    pub cancellations_requested: u64,
    pub ready_high_water: usize,
    pub in_flight_high_water: usize,
}

impl Metrics {
    pub fn settled(&self) -> u64 {
        self.succeeded + self.failed_terminal + self.cancelled_terminal + self.dropped_terminal
    }
}

/// One (task, agent) assignment interval for audit-style assertions in tests. Recording is
/// opt-in because it grows with dispatches; production runs keep it off.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssignmentSpan {
    pub task: TaskId,
    pub agent: AgentId,
    pub started_tick: u64,
    /// `None` while the attempt is still live.
    pub ended_tick: Option<u64>,
}

enum QueueEntry {
    Ready(TaskId),
}

/// The fabric itself.
pub struct Fabric {
    cfg: FabricConfig,
    router: Router,
    queues: Vec<BoundedQueue<QueueEntry>>,
    leases: LeaseTable,
    quotas: QuotaLedger,
    cancels: CancelState,
    idem: BTreeMap<IdempotencyKey, TaskId>,
    tasks: BTreeMap<TaskId, TaskMeta>,
    retry_heap: std::collections::BinaryHeap<std::cmp::Reverse<(u64, u64, TaskId)>>,
    in_flight: BTreeMap<TaskId, InFlight>,
    busy_agents: BTreeSet<AgentId>,
    receipts: std::collections::VecDeque<Receipt>,
    assignments: Vec<AssignmentSpan>,
    driver: Box<dyn Driver>,
    metrics: Metrics,
    clock: u64,
    next_task: u64,
    seq: u64,
    shard_cursor: usize,
    record_assignments: bool,
}

impl Fabric {
    pub fn new(cfg: FabricConfig, driver: Box<dyn Driver>, cancels: CancelState) -> Self {
        let n = cfg.shards;
        let cap = cfg.per_shard_queue_cap;
        Self {
            quotas: QuotaLedger::new(cfg.quota),
            queues: (0..n).map(|_| BoundedQueue::new(cap)).collect(),
            cfg,
            router: Router::new(n),
            leases: LeaseTable::new(),
            cancels,
            idem: BTreeMap::new(),
            tasks: BTreeMap::new(),
            retry_heap: std::collections::BinaryHeap::new(),
            in_flight: BTreeMap::new(),
            busy_agents: BTreeSet::new(),
            receipts: std::collections::VecDeque::new(),
            assignments: Vec::new(),
            driver,
            metrics: Metrics::default(),
            clock: 0,
            next_task: 0,
            seq: 0,
            shard_cursor: 0,
            record_assignments: false,
        }
    }

    /// Turns on assignment-interval recording (tests only; see [`AssignmentSpan`]).
    pub fn set_record_assignments(&mut self, yes: bool) {
        self.record_assignments = yes;
    }

    // ---------------------------------------------------------------- registration

    pub fn register_agent(&mut self, name: &str, caps: CapabilitySet) -> AgentId {
        let id = self.router.register(name, caps);
        self.quotas.register(id);
        id
    }

    pub fn mark_agent(
        &mut self,
        id: AgentId,
        state: AgentState,
    ) -> Result<(), crate::router::RouterError> {
        self.router.set_state(id, state)
    }

    // ---------------------------------------------------------------- admission

    /// Admits one task. Admission checks idempotency first (a replayed key returns
    /// [`Submission::Duplicate`] without touching any queue), then tries the task's rendezvous
    /// preference order over *capable* shard queues until one accepts; if all capable queues are
    /// full the task is rejected with backpressure — visible to the caller, never buffered
    /// invisibly.
    pub fn submit(
        &mut self,
        payload: Vec<u8>,
        caps: CapabilitySet,
        key: Option<IdempotencyKey>,
    ) -> Submission {
        assert!(
            !caps.is_empty(),
            "tasks must declare at least one capability"
        );
        self.metrics.submitted += 1;

        let derived = IdempotencyKey::derive(
            crate::ids::mix64(u64::from_be_bytes(
                sha256(&payload).as_bytes()[..8]
                    .try_into()
                    .expect("8 bytes"),
            )),
            crate::ids::mix64(caps.primary().map(Capability::as_str).unwrap_or("").len() as u64),
        );
        let key = key.unwrap_or(derived);
        if let Some(existing) = self.idem.get(&key) {
            self.metrics.duplicate_submissions += 1;
            return Submission::Duplicate { task: *existing };
        }

        self.next_task += 1;
        let task = TaskId::new(self.next_task);
        let env = TaskEnvelope::compose(
            task,
            payload,
            caps,
            Some(key),
            self.clock,
            self.cfg.retry.max_attempts,
        );
        let meta = TaskMeta {
            env,
            submitted_tick: self.clock,
            attempts_done: 0,
            last_agent: None,
            cancel_requested: false,
            key,
        };
        let caps_for_placement = meta.env.capabilities().clone();
        let placement_key = task.raw();

        match self.enqueue_to_capable_shard(&caps_for_placement, placement_key, task) {
            Ok(()) => {
                self.idem.insert(key, task);
                self.tasks.insert(task, meta);
                self.metrics.admitted += 1;
                let queued_total: usize = self.queues.iter().map(BoundedQueue::len).sum();
                self.metrics.ready_high_water = self.metrics.ready_high_water.max(queued_total);
                Submission::Accepted { task }
            }
            Err(pressure) => {
                self.next_task -= 1;
                self.metrics.backpressure_rejections += 1;
                Submission::Rejected { pressure }
            }
        }
    }

    fn enqueue_to_capable_shard(
        &mut self,
        caps: &CapabilitySet,
        placement_key: u64,
        task: TaskId,
    ) -> Result<(), Backpressure> {
        let pref = shard::preference_order(self.cfg.shards, placement_key);
        let mut capable_shards: BTreeSet<ShardId> = BTreeSet::new();
        for id in self.router.capable(caps) {
            if let Some(s) = self.router.shard_of(id) {
                capable_shards.insert(s);
            }
        }
        // Preference order first; if no capable shard exists anywhere, fall back to pure
        // rendezvous so the task parks as *unroutable* (visible metric) instead of being lost.
        let mut order: Vec<ShardId> = pref
            .iter()
            .copied()
            .filter(|s| capable_shards.contains(s))
            .collect();
        if order.is_empty() {
            order = pref;
        }
        for s in order {
            let q = &mut self.queues[s.raw() as usize];
            if q.push(QueueEntry::Ready(task)).is_ok() {
                return Ok(());
            }
        }
        Err(Backpressure {
            capacity: self.cfg.per_shard_queue_cap,
        })
    }

    pub fn cancel(&mut self, task: TaskId) -> bool {
        if !self.tasks.contains_key(&task) && !self.in_flight.contains_key(&task) {
            return false;
        }
        if let Some(meta) = self.tasks.get_mut(&task) {
            meta.cancel_requested = true;
        }
        self.cancels.cancel(task);
        self.metrics.cancellations_requested += 1;
        true
    }

    // ---------------------------------------------------------------- clock & stepping

    pub fn now(&self) -> u64 {
        self.clock
    }

    /// Advances to `tick` and runs all bookkeeping due there. Completions are polled after the
    /// driver is told the new time; lease expiry follows; retry-due tasks requeue; dispatch runs
    /// last so freed capacity is used immediately.
    pub fn step_to(&mut self, tick: u64) {
        assert!(tick >= self.clock, "time may not run backwards");
        self.clock = tick.max(self.clock);
        self.driver.advance_clock(self.clock);

        for c in self.driver.poll() {
            self.settle_completion(c);
        }
        self.expire_leases();
        self.release_due_retries();
        self.dispatch_ready();
    }

    fn expire_leases(&mut self) {
        let expired: Vec<ExpiredLease> = self.leases.expire_before(self.clock + 1);
        for e in expired {
            self.metrics.lease_expiries += 1;
            if let Some(inf) = self.in_flight.remove(&e.task) {
                self.busy_agents.remove(&inf.agent);
                if self.record_assignments {
                    self.close_assignment(e.task, inf.epoch, e.ended_at_tick);
                }
                if let Some(meta) = self.tasks.get_mut(&e.task) {
                    meta.attempts_done = inf.attempt;
                    self.schedule_retry_or_settle(e.task, Outcome::Crashed);
                }
            }
            // A lease with no in-flight attempt cannot exist: leases are granted only at
            // dispatch and released at settlement, both under this same single-threaded core.
        }
    }

    fn release_due_retries(&mut self) {
        while let Some(top) = self.retry_heap.peek() {
            if top.0 .0 > self.clock {
                break;
            }
            let std::cmp::Reverse((_, _, task)) = self.retry_heap.pop().expect("peeked");
            // Cancelled while waiting: settle now instead of re-entering a queue.
            if self
                .tasks
                .get(&task)
                .map(|m| m.cancel_requested)
                .unwrap_or(false)
                && self.cancels.is_cancelled_since(task, 0)
            {
                let agent = self.tasks.get(&task).and_then(|m| m.last_agent);
                self.settle_now(task, agent, Terminal::Cancelled);
                continue;
            }
            let Some(meta) = self.tasks.get(&task) else {
                continue;
            };
            let caps = meta.env.capabilities().clone();
            match self.enqueue_to_capable_shard(&caps, task.raw(), task) {
                Ok(()) => {}
                Err(_) => {
                    // Queues still saturated: come back one tick later. Sequence tiebreak keeps
                    // ordering stable.
                    self.seq += 1;
                    self.retry_heap
                        .push(std::cmp::Reverse((self.clock + 1, self.seq, task)));
                    self.metrics.deferred_by_driver_full += 1;
                }
            }
        }
    }

    fn dispatch_ready(&mut self) {
        let shard_count = self.cfg.shards as usize;
        while self.in_flight.len() < self.cfg.max_in_flight {
            // Rotate start position across shards so no shard starves another (global fairness).
            let mut picked: Option<TaskId> = None;
            for off in 0..shard_count {
                let idx = (self.shard_cursor + off) % shard_count;
                if let Some(t) = self.queues[idx].pop() {
                    self.shard_cursor = (idx + 1) % shard_count;
                    picked = Some(match t {
                        QueueEntry::Ready(task) => task,
                    });
                    break;
                }
            }
            let Some(task) = picked else { return };

            // A queued task with a requested cancellation settles as Cancelled without ever
            // being dispatched; in-flight tasks settle by their actual outcome.
            if self
                .tasks
                .get(&task)
                .map(|m| m.cancel_requested)
                .unwrap_or(false)
                && self.cancels.is_cancelled_since(task, 0)
            {
                let agent = self.tasks.get(&task).and_then(|m| m.last_agent);
                self.settle_now(task, agent, Terminal::Cancelled);
                continue;
            }
            self.try_dispatch_one(task);
        }
    }

    fn try_dispatch_one(&mut self, task: TaskId) {
        let Some(meta) = self.tasks.get(&task) else {
            return;
        };
        let env_caps = meta.env.capabilities().clone();
        let pref = shard::preference_order(self.cfg.shards, task.raw());
        match self.router.pick(&env_caps, &pref, &self.busy_agents) {
            None => {
                // Nothing eligible right now: park briefly rather than spin.
                self.metrics.unroutable_parks += 1;
                self.seq += 1;
                self.retry_heap
                    .push(std::cmp::Reverse((self.clock + 2, self.seq, task)));
            }
            Some(agent) => {
                match self.quotas.take(agent, self.clock) {
                    crate::quota::Take::Deferred { resume_tick } => {
                        self.metrics.deferred_by_quota += 1;
                        self.seq += 1;
                        self.retry_heap.push(std::cmp::Reverse((
                            resume_tick.max(self.clock + 1),
                            self.seq,
                            task,
                        )));
                    }
                    crate::quota::Take::Allowed => {
                        let ttl = self.cfg.default_lease_ttl_ticks;
                        match self.leases.grant(task, agent, self.clock, ttl) {
                            Ok(handle) => {
                                self.launch(agent, handle);
                            }
                            Err(LeaseError::HeldByOther { .. }) => {
                                // Someone else holds it (cannot arise in this single-threaded
                                // core, but the type system still demands a stated path):
                                // wait past that lease's expiry.
                                self.metrics.deferred_by_driver_full += 1;
                                let resume = self
                                    .leases
                                    .expiry_of(task)
                                    .unwrap_or(self.clock + 1)
                                    .max(self.clock + 1);
                                self.seq += 1;
                                self.retry_heap
                                    .push(std::cmp::Reverse((resume, self.seq, task)));
                            }
                            Err(other) => {
                                let _ = other;
                                unreachable!("grant cannot fail otherwise for a fresh lease");
                            }
                        }
                    }
                }
            }
        }
    }

    fn launch(&mut self, agent: AgentId, handle: LeaseHandle) {
        let task = handle.task();
        let epoch = handle.epoch();
        let Some(meta) = self.tasks.get(&task) else {
            return;
        };
        let job = DispatchJob {
            envelope: meta.env.clone(),
            agent,
            attempt: meta.attempts_done + 1,
            lease_epoch: epoch,
            cancel_gen: self.cancels.snapshot(task),
        };
        let attempt = job.attempt;

        match self.driver.dispatch(job) {
            Enqueue::Accepted => {
                if let Some(meta) = self.tasks.get_mut(&task) {
                    meta.last_agent = Some(agent);
                }
                self.in_flight.insert(
                    task,
                    InFlight {
                        agent,
                        attempt,
                        epoch,
                    },
                );
                self.busy_agents.insert(agent);
                self.metrics.dispatched += 1;
                self.metrics.in_flight_high_water =
                    self.metrics.in_flight_high_water.max(self.in_flight.len());
                if self.record_assignments {
                    self.assignments.push(AssignmentSpan {
                        task,
                        agent,
                        started_tick: self.clock,
                        ended_tick: None,
                    });
                }
            }
            Enqueue::Full => {
                // Driver saturated: release the just-granted lease and retry next tick. The
                // handle is consumed by release — the affine-token design paying off.
                let _ = self.leases.release(handle);
                self.metrics.deferred_by_driver_full += 1;
                self.seq += 1;
                self.retry_heap
                    .push(std::cmp::Reverse((self.clock + 1, self.seq, task)));
            }
        }
    }

    fn settle_completion(&mut self, c: Completion) {
        let Some(inf) = self.in_flight.remove(&c.task) else {
            // A completion for something not in flight: ignore it — the task either settled
            // already (late duplicate) or was never dispatched. Never applied blindly.
            return;
        };
        self.busy_agents.remove(&inf.agent);
        if self.record_assignments {
            self.close_assignment(c.task, inf.epoch, self.clock);
        }
        // The attempt is over: free its lease now rather than letting it linger to TTL.
        let _ = self.leases.release_by(c.task, c.lease_epoch);

        // Result binding is verified HERE, centrally: success claims are re-hashed against the
        // bytes they carry.
        let expected_digest = self.tasks.get(&c.task).map(|m| m.env.payload_digest());
        let verified_outcome = match c.outcome {
            Outcome::Succeeded { result } => {
                if Some(sha256(&result)) == expected_digest {
                    Outcome::Succeeded { result }
                } else {
                    self.metrics.corrupted_settlements += 1;
                    Outcome::Failed {
                        reason: "result digest mismatch at settlement".into(),
                    }
                }
            }
            other => other,
        };

        let Some(meta) = self.tasks.get_mut(&c.task) else {
            return;
        };
        meta.attempts_done = c.attempt.max(meta.attempts_done);
        self.schedule_retry_or_settle(c.task, verified_outcome);
    }

    fn schedule_retry_or_settle(&mut self, task: TaskId, outcome: Outcome) {
        let Some(meta) = self.tasks.get(&task) else {
            return;
        };
        let attempts = meta.attempts_done;
        let max_attempts = meta.env.max_attempts();

        match &outcome {
            Outcome::CancelledBeforeStart => {
                self.settle_now(task, meta.last_agent, Terminal::Cancelled);
            }
            Outcome::Succeeded { .. } => {
                self.settle_now(task, meta.last_agent, Terminal::Succeeded);
            }
            Outcome::Crashed | Outcome::Failed { .. } => {
                match self.cfg.retry.after_failure(attempts) {
                    RetryDecision::RetryAfter(delay) if attempts < max_attempts => {
                        self.metrics.retried += 1;
                        self.seq += 1;
                        self.retry_heap.push(std::cmp::Reverse((
                            self.clock + delay,
                            self.seq,
                            task,
                        )));
                    }
                    _ => {
                        let terminal = outcome.terminal();
                        self.settle_now(task, meta.last_agent, terminal);
                    }
                }
            }
        }
    }

    fn settle_now(&mut self, task: TaskId, agent: Option<AgentId>, terminal: Terminal) {
        let Some(meta) = self.tasks.remove(&task) else {
            return;
        };
        // The dedupe window closes with settlement: bounded memory beats an unbounded key map,
        // and the closure is counted so it is never silent.
        if self.idem.remove(&meta.key).is_some() {
            self.metrics.idempotency_evictions += 1;
        }
        match &terminal {
            Terminal::Succeeded => self.metrics.succeeded += 1,
            Terminal::Cancelled => self.metrics.cancelled_terminal += 1,
            Terminal::Dropped => self.metrics.dropped_terminal += 1,
            _ => self.metrics.failed_terminal += 1,
        }
        let receipt = Receipt::new(
            task,
            agent.or(meta.last_agent),
            meta.attempts_done.max(1),
            terminal,
            meta.env.payload_digest(),
            meta.submitted_tick,
            self.clock,
            meta.cancel_requested || self.cancels.is_cancelled_since(task, 0),
        );
        self.receipts.push_back(receipt);
        while self.receipts.len() > self.cfg.receipt_retention {
            self.receipts.pop_front();
            self.metrics.receipts_evicted += 1;
        }
    }

    fn close_assignment(&mut self, task: TaskId, _epoch: crate::ids::LeaseEpoch, end_tick: u64) {
        if let Some(open) = self
            .assignments
            .iter_mut()
            .rev()
            .find(|a| a.task == task && a.ended_tick.is_none())
        {
            open.ended_tick = Some(end_tick);
        }
    }

    // ---------------------------------------------------------------- driving loops

    /// True while any work remains anywhere (queues, retries, in-flight).
    pub fn has_pending_work(&self) -> bool {
        self.queues.iter().any(|q| !q.is_empty())
            || !self.retry_heap.is_empty()
            || !self.in_flight.is_empty()
    }

    /// Runs until idle or `max_ticks` elapsed; returns ticks consumed.
    pub fn run_until_idle(&mut self, max_ticks: u64) -> u64 {
        let start = self.clock;
        while self.has_pending_work() && self.clock - start < max_ticks {
            self.step_to(self.next_wake());
        }
        self.clock - start
    }

    fn next_wake(&self) -> u64 {
        let mut wake = self.clock + 1;
        if let Some(top) = self.retry_heap.peek() {
            wake = wake.min(top.0 .0.max(self.clock + 1));
        }
        // Lease expiries bound how long a silent worker can hold a slot.
        if let Some(min_exp) = self.earliest_expiry() {
            wake = wake.min(min_exp.max(self.clock + 1));
        }
        wake
    }

    fn earliest_expiry(&self) -> Option<u64> {
        self.in_flight
            .keys()
            .filter_map(|t| self.leases.expiry_of(*t))
            .min()
    }

    // ---------------------------------------------------------------- observation

    pub fn metrics(&self) -> &Metrics {
        &self.metrics
    }

    pub fn receipts(&self) -> impl Iterator<Item = &Receipt> {
        self.receipts.iter()
    }

    pub fn receipts_for_agent(&self, agent: AgentId) -> Vec<&Receipt> {
        self.receipts
            .iter()
            .filter(|r| r.agent() == Some(agent))
            .collect()
    }

    pub fn receipt_count(&self) -> usize {
        self.receipts.len()
    }

    pub fn assignments(&self) -> &[AssignmentSpan] {
        &self.assignments
    }

    pub fn router(&self) -> &Router {
        &self.router
    }

    pub fn leases_live(&self) -> usize {
        self.leases.live()
    }

    /// Snapshot of bounded memory usage for the memory-bound tests.
    pub fn memory_stats(&self) -> MemStats {
        MemStats {
            queued_ready: self.queues.iter().map(BoundedQueue::len).sum(),
            queue_capacity_total: self.cfg.per_shard_queue_cap * self.queues.len(),
            queue_high_water: self.metrics.ready_high_water,
            in_flight: self.in_flight.len(),
            in_flight_high_water: self.metrics.in_flight_high_water,
            retained_receipts: self.receipts.len(),
        }
    }

    pub fn shutdown_driver(&mut self) {
        self.driver.shutdown();
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemStats {
    pub queued_ready: usize,
    pub queue_capacity_total: usize,
    pub queue_high_water: usize,
    pub in_flight: usize,
    pub in_flight_high_water: usize,
    pub retained_receipts: usize,
}

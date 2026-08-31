//! Deterministic simulation: virtual clock, seeded PRNG, and scripted fault injection.
//!
//! The simulator drives the *same* [`crate::scheduler::Fabric`] the production drivers do;
//! only the driver differs. Time is explicit ticks. Faults are either scripted one-shot rules
//! ([`SimDriver::inject`]) or a seeded probabilistic rate whose draws come from [`SplitMix64`] —
//! so every run with the same seed and call sequence replays identically. `Crash` and
//! `SilentDrop` produce **no** completion at all: recovery happens exclusively through lease
//! expiry, which is exactly how a real dead worker behaves.

use crate::cancel::CancelState;
use crate::envelope::{Completion, DispatchJob, Outcome};
use crate::exec::{run_bound_job, Driver, Enqueue, Handler};
use crate::ids::{AgentId, TaskId};
use crate::scheduler::{Fabric, FabricConfig};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap};
use std::sync::{Arc, Mutex};

/// Seedable PRNG (splitmix64). Public because tests pin seeds and replay them.
#[derive(Clone, Debug)]
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        crate::ids::mix64(self.state)
    }

    /// Uniform draw below `bound` without modulo bias (rejection sampling).
    pub fn next_below(&mut self, bound: u64) -> u64 {
        assert!(bound > 0);
        loop {
            let x = self.next_u64();
            let limit = u64::MAX - u64::MAX % bound;
            if x < limit {
                return x % bound;
            }
        }
    }
}

/// What goes wrong. Names matter here: each variant is a different *claim* about the world.
#[derive(Clone, Debug)]
pub enum Fault {
    /// Worker dies before starting. No completion ever arrives; lease expiry is the detector.
    Crash,
    /// Worker accepts then goes silent. Observably identical to [`Fault::Crash`] from outside;
    /// kept distinct so injected-fault reports can say which was scripted.
    SilentDrop,
    /// Worker runs and reports failure with this reason.
    Fail(String),
    /// Worker "succeeds" against wrong bytes — exercises the settlement digest check, not the
    /// worker-side payload binding.
    CorruptResult,
}

/// One-shot fault script entry. Every set field must match for the fault to fire; `None`
/// matches anything. Consumed on use.
#[derive(Clone, Debug)]
pub struct InjectionRule {
    pub task: Option<TaskId>,
    pub agent: Option<AgentId>,
    pub attempt: Option<u32>,
    pub fault: Fault,
}

struct Due {
    seq: u64,
    completion: Completion,
}

/// The simulated execution driver.
pub struct SimDriver {
    handler: Arc<dyn Handler>,
    cancels: CancelState,
    default_latency_ticks: u64,
    per_agent_latency: BTreeMap<AgentId, u64>,
    rules: Vec<InjectionRule>,
    rng: SplitMix64,
    /// Random-fault probability in parts-per-million per dispatch; 0 disables.
    fault_rate_ppm: u64,
    fault_cycle_index: u64,
    scheduled: BinaryHeap<Reverse<(u64, u64)>>,
    dues: Vec<Due>,
    clock: u64,
    seq: u64,
}

fn random_menu(i: u64) -> Fault {
    match i % 3 {
        0 => Fault::Crash,
        1 => Fault::Fail("simulated provider error".into()),
        _ => Fault::CorruptResult,
    }
}

impl SimDriver {
    pub fn new(
        handler: Arc<dyn Handler>,
        cancels: CancelState,
        seed: u64,
        latency_ticks: u64,
    ) -> Self {
        Self {
            handler,
            cancels,
            default_latency_ticks: latency_ticks.max(1),
            per_agent_latency: BTreeMap::new(),
            rules: Vec::new(),
            rng: SplitMix64::new(seed),
            fault_rate_ppm: 0,
            fault_cycle_index: 0,
            scheduled: BinaryHeap::new(),
            dues: Vec::new(),
            clock: 0,
            seq: 0,
        }
    }

    pub fn set_latency(&mut self, agent: AgentId, ticks: u64) {
        assert!(
            ticks > 0,
            "zero-latency simulated agents would complete before their lease could expire"
        );
        self.per_agent_latency.insert(agent, ticks);
    }

    pub fn set_fault_rate_ppm(&mut self, ppm: u64) {
        assert!(ppm <= 1_000_000);
        self.fault_rate_ppm = ppm;
    }

    pub fn inject(&mut self, rule: InjectionRule) {
        self.rules.push(rule);
    }

    pub fn pending_fault_rules(&self) -> usize {
        self.rules.len()
    }

    fn latency_for(&self, agent: AgentId) -> u64 {
        self.per_agent_latency
            .get(&agent)
            .copied()
            .unwrap_or(self.default_latency_ticks)
    }

    /// Decides whether a fault applies to this job, consuming one-shot rules as they fire.
    fn fault_for(&mut self, job: &DispatchJob) -> Option<Fault> {
        let rule_hit = self.rules.iter().position(|r| {
            r.task.map(|t| t == job.envelope.task()).unwrap_or(true)
                && r.agent.map(|a| a == job.agent).unwrap_or(true)
                && r.attempt.map(|a| a == job.attempt).unwrap_or(true)
        });
        if let Some(idx) = rule_hit {
            return Some(self.rules.remove(idx).fault);
        }
        if self.fault_rate_ppm > 0 && self.rng.next_below(1_000_000) < self.fault_rate_ppm {
            let f = random_menu(self.fault_cycle_index);
            self.fault_cycle_index += 1;
            return Some(f);
        }
        None
    }

    fn schedule(&mut self, tick: u64, completion: Completion) {
        self.seq += 1;
        self.scheduled.push(Reverse((tick, self.seq)));
        self.dues.push(Due {
            seq: self.seq,
            completion,
        });
    }

    fn release_due(&mut self) -> Vec<Completion> {
        let mut out = Vec::new();
        while let Some(top) = self.scheduled.peek() {
            if top.0 .0 > self.clock {
                break;
            }
            let Reverse((_, seq)) = self.scheduled.pop().expect("peeked");
            let idx = self
                .dues
                .iter()
                .position(|d| d.seq == seq)
                .expect("scheduled entries always have a matching due");
            out.push(self.dues.remove(idx).completion);
        }
        out
    }
}

impl Driver for SimDriver {
    fn dispatch(&mut self, job: DispatchJob) -> Enqueue {
        let task = job.envelope.task();
        let agent = job.agent;
        let epoch = job.lease_epoch;
        let attempt = job.attempt;

        // Cancellation is observed before any fault logic: a cancelled attempt never starts,
        // and it reports immediately (no latency) because nothing was ever running.
        if self.cancels.is_cancelled_since(task, job.cancel_gen) {
            self.schedule(
                self.clock,
                Completion {
                    task,
                    agent,
                    attempt,
                    lease_epoch: epoch,
                    outcome: Outcome::CancelledBeforeStart,
                },
            );
            return Enqueue::Accepted;
        }

        let outcome = match self.fault_for(&job) {
            Some(Fault::Crash) | Some(Fault::SilentDrop) => {
                // Nothing is scheduled. The lease will expire and the scheduler will treat the
                // attempt as Crashed — there is no faster notification in a real system either.
                return Enqueue::Accepted;
            }
            Some(Fault::Fail(reason)) => Outcome::Failed { reason },
            Some(Fault::CorruptResult) => Outcome::Succeeded {
                result: b"corrupted-by-injection".to_vec(),
            },
            None => run_bound_job(self.handler.as_ref(), &job, &self.cancels).outcome,
        };

        let due_tick = self.clock + self.latency_for(agent);
        self.schedule(
            due_tick,
            Completion {
                task,
                agent,
                attempt,
                lease_epoch: epoch,
                outcome,
            },
        );
        Enqueue::Accepted
    }

    fn advance_clock(&mut self, now: u64) {
        self.clock = now;
    }

    fn poll(&mut self) -> Vec<Completion> {
        self.release_due()
    }
}

/// Shared handle so tests can mutate the simulator while the fabric owns a boxed driver to it.
struct SharedSim(Arc<Mutex<SimDriver>>);

impl Driver for SharedSim {
    fn dispatch(&mut self, job: DispatchJob) -> Enqueue {
        self.0.lock().expect("sim lock").dispatch(job)
    }
    fn poll(&mut self) -> Vec<Completion> {
        self.0.lock().expect("sim lock").poll()
    }
    fn advance_clock(&mut self, now: u64) {
        self.0.lock().expect("sim lock").advance_clock(now)
    }
    fn shutdown(&mut self) {}
}

/// Convenience wrapper bundling a fabric with its simulator handle.
pub struct Simulation {
    pub fabric: Fabric,
    sim: Arc<Mutex<SimDriver>>,
}

impl Simulation {
    pub fn new(
        cfg: FabricConfig,
        handler: Arc<dyn Handler>,
        cancels: CancelState,
        seed: u64,
        latency_ticks: u64,
    ) -> Self {
        let sim = Arc::new(Mutex::new(SimDriver::new(
            handler,
            cancels.clone(),
            seed,
            latency_ticks,
        )));
        let driver = Box::new(SharedSim(Arc::clone(&sim)));
        let mut fabric = Fabric::new(cfg, driver, cancels);
        fabric.set_record_assignments(true);
        Self { fabric, sim }
    }

    pub fn sim_handle(&self) -> Arc<Mutex<SimDriver>> {
        Arc::clone(&self.sim)
    }

    pub fn inject(&self, rule: InjectionRule) {
        self.sim.lock().expect("sim lock").inject(rule);
    }

    pub fn set_fault_rate_ppm(&self, ppm: u64) {
        self.sim.lock().expect("sim lock").set_fault_rate_ppm(ppm);
    }

    pub fn set_agent_latency(&self, agent: AgentId, ticks: u64) {
        self.sim.lock().expect("sim lock").set_latency(agent, ticks);
    }

    pub fn run_until_idle(&mut self, max_ticks: u64) -> u64 {
        self.fabric.run_until_idle(max_ticks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::{Capability, CapabilitySet};
    use crate::exec::FnHandler;
    use crate::scheduler::Submission;

    fn echo(job: &DispatchJob) -> Outcome {
        Outcome::Succeeded {
            result: job.envelope.payload().to_vec(),
        }
    }

    fn compute() -> CapabilitySet {
        CapabilitySet::one(Capability::parse("compute").expect("cap"))
    }

    fn sim_with(seed: u64) -> Simulation {
        Simulation::new(
            FabricConfig::default(),
            Arc::new(FnHandler(echo)),
            CancelState::new(),
            seed,
            2,
        )
    }

    #[test]
    fn splitmix_draws_are_reproducible_and_bounded() {
        let mut a = SplitMix64::new(7);
        let mut b = SplitMix64::new(7);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
        let mut c = SplitMix64::new(9);
        for _ in 0..500 {
            assert!(c.next_below(10) < 10);
        }
    }

    #[test]
    fn a_crashed_attempt_never_completes_and_lease_expiry_recovers_the_task() {
        let mut s = sim_with(11);
        let caps = compute();
        s.fabric.register_agent("w", caps.clone());
        let task = match s.fabric.submit(b"job".to_vec(), caps, None) {
            Submission::Accepted { task } => task,
            other => panic!("expected acceptance, got {other:?}"),
        };
        s.inject(InjectionRule {
            task: Some(task),
            agent: None,
            attempt: Some(1),
            fault: Fault::Crash,
        });
        s.run_until_idle(500);
        let receipts: Vec<_> = s.fabric.receipts().collect();
        assert_eq!(receipts.len(), 1, "exactly one settlement");
        assert_eq!(
            *receipts[0].terminal(),
            crate::envelope::Terminal::Succeeded,
            "the retried attempt succeeds"
        );
        assert_eq!(receipts[0].attempts_used(), 2, "first attempt crashed");
        assert!(
            s.fabric.metrics().lease_expiries >= 1,
            "expiry was the detector"
        );
    }

    #[test]
    fn same_seed_same_script_replays_identically_different_seed_diverges() {
        let receipts_digest = |seed: u64| -> String {
            let mut s = sim_with(seed);
            let caps = compute();
            s.fabric.register_agent("a", caps.clone());
            s.fabric.register_agent("b", caps.clone());
            s.set_fault_rate_ppm(200_000);
            for i in 0..40u8 {
                let _ = s.fabric.submit(vec![i; 16], caps.clone(), None);
            }
            s.run_until_idle(4_000);
            let mut digest = String::new();
            for r in s.fabric.receipts() {
                digest.push_str(&format!(
                    "{}:{}:{};",
                    r.task(),
                    r.attempts_used(),
                    r.terminal()
                ));
            }
            digest
        };
        assert_eq!(
            receipts_digest(42),
            receipts_digest(42),
            "deterministic replay"
        );
    }

    #[test]
    fn one_shot_rules_are_consumed_on_use_not_re_applied() {
        let mut s = sim_with(3);
        let caps = compute();
        s.fabric.register_agent("w", caps.clone());
        let task = match s.fabric.submit(b"j".to_vec(), caps, None) {
            Submission::Accepted { task } => task,
            other => panic!("expected acceptance, got {other:?}"),
        };
        s.inject(InjectionRule {
            task: Some(task),
            agent: None,
            attempt: Some(1),
            fault: Fault::SilentDrop,
        });
        assert_eq!(
            s.sim_handle()
                .lock()
                .expect("sim lock")
                .pending_fault_rules(),
            1
        );
        s.run_until_idle(500);
        assert_eq!(
            s.sim_handle()
                .lock()
                .expect("sim lock")
                .pending_fault_rules(),
            0,
            "rule consumed"
        );
        assert_eq!(s.fabric.receipt_count(), 1);
    }
}

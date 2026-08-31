//! Per-agent token-bucket quotas over the fabric's virtual clock.
//!
//! Quotas are admission control: an exhausted bucket defers dispatch to the tick of the next
//! refill rather than dropping the task. Refill is integer-exact (whole tokens at whole period
//! boundaries), which keeps simulation runs bit-reproducible — no floating point anywhere in
//! scheduling arithmetic. Quotas throttle rate within this process; they are not a security
//! boundary and say nothing about remote providers.

use crate::ids::AgentId;
use std::collections::BTreeMap;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuotaSpec {
    /// Maximum burst size in tasks; also the cap on accumulated refill.
    pub burst: u32,
    /// Refill grants `refill_amount` tokens every `refill_every` ticks.
    pub refill_every: u64,
    pub refill_amount: u32,
}

impl Default for QuotaSpec {
    fn default() -> Self {
        Self {
            burst: 8,
            refill_every: 4,
            refill_amount: 2,
        }
    }
}

impl QuotaSpec {
    pub fn unlimited() -> Self {
        Self {
            burst: u32::MAX,
            refill_every: 1,
            refill_amount: u32::MAX,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Take {
    Allowed,
    /// Dispatch must wait until this tick; the task is deferred, never dropped.
    Deferred {
        resume_tick: u64,
    },
}

impl fmt::Display for Take {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Take::Allowed => write!(f, "allowed"),
            Take::Deferred { resume_tick } => write!(f, "deferred until tick {resume_tick}"),
        }
    }
}

#[derive(Debug)]
struct Bucket {
    tokens: u64,
    last_synced_tick: u64,
}

/// Ledger of one bucket per registered agent. Lazily refilled on `take` by advancing each
/// touched bucket to the current tick in exact period steps.
#[derive(Debug)]
pub struct QuotaLedger {
    spec: QuotaSpec,
    buckets: BTreeMap<AgentId, Bucket>,
}

impl QuotaLedger {
    pub fn new(spec: QuotaSpec) -> Self {
        assert!(spec.refill_every > 0, "refill period must be positive");
        Self {
            spec,
            buckets: BTreeMap::new(),
        }
    }

    pub fn register(&mut self, agent: AgentId) {
        let burst = u64::from(self.spec.burst);
        self.buckets.entry(agent).or_insert(Bucket {
            tokens: burst,
            last_synced_tick: 0,
        });
    }

    pub fn unregister(&mut self, agent: AgentId) {
        self.buckets.remove(&agent);
    }

    /// Attempts to consume one token for `agent` as of `now`.
    pub fn take(&mut self, agent: AgentId, now: u64) -> Take {
        let spec = self.spec;
        let Some(bucket) = self.buckets.get_mut(&agent) else {
            return Take::Allowed;
        };

        if spec.refill_amount != u32::MAX {
            let elapsed = now.saturating_sub(bucket.last_synced_tick);
            let periods = elapsed / spec.refill_every;
            if periods > 0 {
                let gain = periods.saturating_mul(u64::from(spec.refill_amount));
                let burst = u64::from(spec.burst);
                bucket.tokens = burst.min(bucket.tokens.saturating_add(gain));
                // Advance by whole periods only; partial periods carry over so refill timing is
                // exact regardless of when takes land between boundaries.
                bucket.last_synced_tick += periods * spec.refill_every;
            }
        }

        if bucket.tokens > 0 {
            bucket.tokens -= 1;
            return Take::Allowed;
        }

        let resume = bucket.last_synced_tick + spec.refill_every;
        Take::Deferred {
            resume_tick: resume.max(now),
        }
    }

    /// Current token count after lazy sync to `now` — observability only, consumes nothing.
    pub fn available(&mut self, agent: AgentId, now: u64) -> u64 {
        let spec = self.spec;
        let Some(bucket) = self.buckets.get_mut(&agent) else {
            return u64::from(spec.burst);
        };
        let elapsed = now.saturating_sub(bucket.last_synced_tick);
        let periods = elapsed / spec.refill_every;
        let burst = u64::from(spec.burst);
        let tokens = burst.min(
            bucket
                .tokens
                .saturating_add(periods * u64::from(spec.refill_amount)),
        );
        tokens.min(burst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent(n: u64) -> AgentId {
        AgentId::from_raw(n).expect("nonzero")
    }

    #[test]
    fn burst_is_enforced_then_dispatch_defers_to_refill_not_drops() {
        let mut l = QuotaLedger::new(QuotaSpec {
            burst: 2,
            refill_every: 10,
            refill_amount: 1,
        });
        let a = agent(1);
        l.register(a);
        assert_eq!(l.take(a, 0), Take::Allowed);
        assert_eq!(l.take(a, 0), Take::Allowed);
        assert_eq!(l.take(a, 0), Take::Deferred { resume_tick: 10 });
    }

    #[test]
    fn refill_accrues_in_whole_periods_and_carries_partial_periods_forward() {
        let mut l = QuotaLedger::new(QuotaSpec {
            burst: 1,
            refill_every: 10,
            refill_amount: 1,
        });
        let a = agent(1);
        l.register(a);
        assert_eq!(l.take(a, 0), Take::Allowed);
        // Tick 5: half a period has passed — no token yet, resume stays on the boundary.
        assert_eq!(l.take(a, 5), Take::Deferred { resume_tick: 10 });
        // Tick 12: one full period since sync at 0 → one token; next boundary at 20.
        assert_eq!(l.take(a, 12), Take::Allowed);
        assert_eq!(l.take(a, 12), Take::Deferred { resume_tick: 20 });
    }

    #[test]
    fn accumulation_is_capped_at_burst_so_idle_agents_do_not_bank_unbounded_credit() {
        let mut l = QuotaLedger::new(QuotaSpec {
            burst: 3,
            refill_every: 1,
            refill_amount: 5,
        });
        let a = agent(1);
        l.register(a);
        assert_eq!(l.available(a, 1_000), 3, "capped at burst");
        for _ in 0..3 {
            assert_eq!(l.take(a, 1_000), Take::Allowed);
        }
        assert_eq!(l.take(a, 1_000), Take::Deferred { resume_tick: 1_001 });
    }

    #[test]
    fn unregistered_agents_are_not_throttled_by_the_ledger() {
        let mut l = QuotaLedger::new(QuotaSpec::default());
        assert_eq!(l.take(agent(99), 0), Take::Allowed);
    }

    #[test]
    fn agents_quota_independently() {
        let mut l = QuotaLedger::new(QuotaSpec {
            burst: 1,
            refill_every: 100,
            refill_amount: 1,
        });
        let (a, b) = (agent(1), agent(2));
        l.register(a);
        l.register(b);
        assert_eq!(l.take(a, 0), Take::Allowed);
        assert_eq!(l.take(a, 0), Take::Deferred { resume_tick: 100 });
        assert_eq!(l.take(b, 0), Take::Allowed, "b's bucket is separate");
    }
}

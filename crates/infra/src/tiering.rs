//! Hot, warm and cold storage under a stated rule, planned before it is applied.
//!
//! Blueprint 40.06 gives the layout but not the movement between tiers; 12.10 asks for
//! "cost-aware and pin-aware eviction with access recency, rebuild cost, dependency fan-out, and
//! release pins" without giving a rule. This module supplies a rule and states it, which is the
//! part usually left to a background job whose behaviour nobody can reproduce.
//!
//! # The rule, in full
//!
//! Let `idle = now - last_access`, in caller-supplied epochs.
//!
//! - **Demote** [`Tier::Hot`] to [`Tier::Warm`] when `idle >= demote_to_warm_after`, and
//!   [`Tier::Warm`] to [`Tier::Cold`] when `idle >= demote_to_cold_after`. An object idle past
//!   the cold threshold goes straight from hot to cold in one transition, and the transition
//!   records that it skipped a tier rather than pretending it passed through.
//! - **Promote** anything to [`Tier::Hot`] when `recent_accesses >= promote_after_accesses` and
//!   `idle <= promote_within`. Both conditions, because a count alone promotes an object that
//!   was busy last year and recency alone promotes an object touched once.
//! - **Pinned objects never fall below [`Tier::Warm`].** A pin in 40.06 is what keeps an object
//!   reachable through garbage collection; the tiering analogue is that a pinned object stays
//!   retrievable without a rehydration step, because the reason it is pinned is that something
//!   published depends on it.
//!
//! `recent_accesses` is supplied by the caller, not maintained here. A sliding window needs a
//! clock and a retention policy of its own, and inventing one would put a second, quieter
//! lifecycle inside the lifecycle module. What "recent" means is the caller's declaration.
//!
//! # Planning is separate from applying
//!
//! [`TieringPolicy::plan`] is pure and returns every transition with its reason;
//! [`TieringPlan::apply_to`] performs them. `bioprism-ledger` splits compaction the same way and
//! for the same reason: an operator must be able to read what a lifecycle job will do before it
//! does it, and 12.22 makes "dry run is default" a requirement.
//!
//! # Deliberately not implemented
//!
//! No storage backend, so nothing physically moves — a demotion updates a record. No rehydration
//! latency or cost model, no per-tier pricing, no size-aware policy, no batching. Tier
//! assignments are not persisted and no event is emitted; a caller that wants an audit trail
//! should write the returned transitions to `bioprism-ledger`.

use crate::epoch::Epoch;
use crate::error::LifecycleError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// Where an object currently lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Tier {
    Hot,
    Warm,
    Cold,
}

impl Tier {
    pub fn name(self) -> &'static str {
        match self {
            Tier::Hot => "hot",
            Tier::Warm => "warm",
            Tier::Cold => "cold",
        }
    }

    /// Distance from hot, so a transition can report how many tiers it crossed.
    pub fn depth(self) -> u8 {
        match self {
            Tier::Hot => 0,
            Tier::Warm => 1,
            Tier::Cold => 2,
        }
    }
}

impl fmt::Display for Tier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// What is known about one object's access pattern.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessRecord {
    pub object: String,
    pub tier: Tier,
    pub last_access: Epoch,
    /// Accesses the caller counts as recent. See the module docs: the window is the caller's
    /// declaration, not this crate's.
    pub recent_accesses: u64,
    pub bytes: u64,
    /// Pinned by a release, a result, a review, an incident or a legal hold — 12.22's retention
    /// graph. Never demoted below warm.
    pub pinned: bool,
}

impl AccessRecord {
    pub fn new(object: impl Into<String>, tier: Tier, last_access: Epoch) -> Self {
        AccessRecord {
            object: object.into(),
            tier,
            last_access,
            recent_accesses: 0,
            bytes: 0,
            pinned: false,
        }
    }

    pub fn with_recent_accesses(mut self, accesses: u64) -> Self {
        self.recent_accesses = accesses;
        self
    }

    pub fn with_bytes(mut self, bytes: u64) -> Self {
        self.bytes = bytes;
        self
    }

    pub fn pinned(mut self) -> Self {
        self.pinned = true;
        self
    }
}

/// Why a transition was planned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TierReason {
    /// Idle past a demotion threshold.
    Idle { epochs: u64, threshold: u64 },
    /// Busy enough and recent enough to promote.
    Recent { accesses: u64, idle_epochs: u64 },
    /// Idle past the cold threshold but held at warm by a pin.
    HeldByPin { epochs: u64 },
}

/// One planned move.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TierTransition {
    pub object: String,
    pub from: Tier,
    pub to: Tier,
    pub reason: TierReason,
    /// True when the move skips a tier, which is legitimate for a long-idle object but is worth
    /// reporting: a plan full of skipped tiers means the warm threshold is not doing any work.
    pub skipped_a_tier: bool,
}

/// The thresholds, checked for coherence at construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TieringPolicy {
    demote_to_warm_after: u64,
    demote_to_cold_after: u64,
    promote_after_accesses: u64,
    promote_within: u64,
}

impl TieringPolicy {
    /// Refuses a policy whose cold threshold is not strictly later than its warm threshold: such
    /// a policy claims to use three tiers and uses two.
    pub fn new(
        demote_to_warm_after: u64,
        demote_to_cold_after: u64,
        promote_after_accesses: u64,
        promote_within: u64,
    ) -> Result<Self, LifecycleError> {
        if demote_to_cold_after <= demote_to_warm_after {
            return Err(LifecycleError::IncoherentTieringPolicy {
                warm: demote_to_warm_after,
                cold: demote_to_cold_after,
            });
        }
        Ok(TieringPolicy {
            demote_to_warm_after,
            demote_to_cold_after,
            promote_after_accesses,
            promote_within,
        })
    }

    pub fn demote_to_warm_after(&self) -> u64 {
        self.demote_to_warm_after
    }

    pub fn demote_to_cold_after(&self) -> u64 {
        self.demote_to_cold_after
    }

    /// Computes every transition implied by the records at `now`. Changes nothing.
    ///
    /// An object last accessed after `now` is [`LifecycleError::AccessInFuture`] rather than a
    /// zero-idle default: the inconsistency is the caller's and a plan that swallowed it would
    /// look correct while being computed from an impossible state.
    pub fn plan<'a>(
        &self,
        records: impl IntoIterator<Item = &'a AccessRecord>,
        now: Epoch,
    ) -> Result<TieringPlan, LifecycleError> {
        let mut transitions = Vec::new();
        for record in records {
            let Some(idle) = now.elapsed_since(record.last_access) else {
                return Err(LifecycleError::AccessInFuture {
                    object: record.object.clone(),
                    last_access: record.last_access,
                    now,
                });
            };

            if record.recent_accesses >= self.promote_after_accesses
                && self.promote_after_accesses > 0
                && idle <= self.promote_within
            {
                if record.tier != Tier::Hot {
                    transitions.push(TierTransition {
                        object: record.object.clone(),
                        from: record.tier,
                        to: Tier::Hot,
                        reason: TierReason::Recent {
                            accesses: record.recent_accesses,
                            idle_epochs: idle,
                        },
                        skipped_a_tier: record.tier == Tier::Cold,
                    });
                }
                continue;
            }

            let target = if idle >= self.demote_to_cold_after {
                Tier::Cold
            } else if idle >= self.demote_to_warm_after {
                Tier::Warm
            } else {
                record.tier
            };

            let (target, reason) = if record.pinned && target == Tier::Cold {
                (Tier::Warm, TierReason::HeldByPin { epochs: idle })
            } else {
                (
                    target,
                    TierReason::Idle {
                        epochs: idle,
                        threshold: if target == Tier::Cold {
                            self.demote_to_cold_after
                        } else {
                            self.demote_to_warm_after
                        },
                    },
                )
            };

            if target.depth() > record.tier.depth() {
                transitions.push(TierTransition {
                    object: record.object.clone(),
                    from: record.tier,
                    to: target,
                    reason,
                    skipped_a_tier: target.depth() - record.tier.depth() > 1,
                });
            }
        }
        Ok(TieringPlan { now, transitions })
    }
}

/// Every transition a policy would make, and the epoch it was computed for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TieringPlan {
    pub now: Epoch,
    pub transitions: Vec<TierTransition>,
}

impl TieringPlan {
    pub fn is_empty(&self) -> bool {
        self.transitions.is_empty()
    }

    pub fn len(&self) -> usize {
        self.transitions.len()
    }

    /// Bytes moving into each tier, for a capacity planner.
    pub fn bytes_by_target<'a>(
        &self,
        records: impl IntoIterator<Item = &'a AccessRecord>,
    ) -> BTreeMap<Tier, u64> {
        let sizes: BTreeMap<&str, u64> = records
            .into_iter()
            .map(|record| (record.object.as_str(), record.bytes))
            .collect();
        let mut totals: BTreeMap<Tier, u64> = BTreeMap::new();
        for transition in &self.transitions {
            let bytes = sizes
                .get(transition.object.as_str())
                .copied()
                .unwrap_or_default();
            *totals.entry(transition.to).or_insert(0) += bytes;
        }
        totals
    }

    /// Performs the plan against a set of records, returning how many were changed.
    ///
    /// A transition naming an object not in `records` is skipped and counted separately, so a
    /// plan applied to a population that has since changed reports the discrepancy rather than
    /// silently doing less than it says.
    pub fn apply_to(&self, records: &mut [AccessRecord]) -> (usize, usize) {
        let mut applied = 0usize;
        let mut absent = 0usize;
        for transition in &self.transitions {
            match records
                .iter_mut()
                .find(|record| record.object == transition.object)
            {
                Some(record) => {
                    record.tier = transition.to;
                    applied += 1;
                }
                None => absent += 1,
            }
        }
        (applied, absent)
    }
}

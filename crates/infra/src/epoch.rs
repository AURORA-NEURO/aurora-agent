//! The logical clock everything in this crate advances on.
//!
//! Blueprint 12.10, 12.21 and 40.06 all describe lifecycle behaviour in wall-clock terms —
//! eviction by "access recency", quotas "per day/month", garbage collection after a "grace
//! period". None of that is implemented with a clock here, for the same reason
//! `bioprism-ledger` refuses to read one: a cache that consults `SystemTime` produces a
//! different eviction plan on every machine, and then the plan cannot be tested, replayed or
//! compared across two runs of the same pipeline.
//!
//! So every lifecycle decision in this crate takes an [`Epoch`] supplied by the caller. An epoch
//! is an opaque monotone counter; what one tick *means* — a day, a compaction round, a release
//! — is the caller's business, and the crate never assumes.
//!
//! [`Epoch`] is distinct from `bioprism_ledger::RecordTime` on purpose. Record time answers
//! "when did we learn this", is bitemporal, and governs what the ledger still promises to
//! answer; it is the axis [`crate::lifecycle`] coordinates on. An epoch answers only "how many
//! ticks of the caller's own schedule have passed", and governs tier demotion and cache
//! staleness, where no bitemporal claim is being made. Conflating them would let a tiering
//! policy silently reinterpret a retention boundary.

use serde::{Deserialize, Serialize};
use std::fmt;

/// A caller-supplied monotone tick.
///
/// No conversion to or from a wall clock exists, and none should be added: the moment an epoch
/// can be derived from `SystemTime`, every plan this crate produces stops being reproducible.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct Epoch(u64);

impl Epoch {
    pub const ZERO: Epoch = Epoch(0);

    pub const fn new(tick: u64) -> Self {
        Epoch(tick)
    }

    pub const fn tick(self) -> u64 {
        self.0
    }

    pub const fn next(self) -> Self {
        Epoch(self.0.saturating_add(1))
    }

    /// Ticks elapsed since `earlier`, or `None` if `earlier` is in fact later.
    ///
    /// Returns an option rather than saturating at zero because "this object was last accessed
    /// after the epoch you are planning for" is a caller bug — a demotion planner that quietly
    /// read it as "idle for 0 ticks" would hide the inconsistency and still produce a plan.
    pub fn elapsed_since(self, earlier: Epoch) -> Option<u64> {
        self.0.checked_sub(earlier.0)
    }
}

impl fmt::Display for Epoch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "e{}", self.0)
    }
}

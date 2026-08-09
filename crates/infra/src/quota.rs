//! A storage budget that cannot be duplicated by copying.
//!
//! Blueprint 12.21 requires budgets "per operation, project, organization, provider, day/month,
//! and benchmark campaign", with a hard budget that "stops safe work" and a "reserve [that]
//! protects cleanup/evidence finalization". 12.20 adds per-org storage quotas. Neither says how a
//! budget is passed to a subsystem without being copied, and that omission is the whole problem.
//!
//! # Why this type is not `Clone`
//!
//! `bioprism-weave`'s `Budget` refuses `Clone` because a coordination budget that can be copied
//! is not a budget: two agents each holding "the" budget each believe they may spend it all, and
//! the total spent is twice the limit with no error raised anywhere.
//!
//! **The same argument applies here, and the reason is worth being precise about.** A storage
//! quota is a claim on a single shared physical resource — bytes on one volume, in one bucket, in
//! one account. If an ingest subsystem and a snapshot subsystem each hold a copy of a
//! 100 GB quota, each can charge 100 GB and each will report itself within budget, while the
//! volume holds 200 GB. The failure is silent at exactly the layer that was supposed to prevent
//! it, and it surfaces as ENOSPC in an unrelated component.
//!
//! So [`StorageQuota`] does not implement `Clone` or `Copy`, and it does not implement
//! `Deserialize` either — deserializing one is copying it through a file. Allowance moves by
//! [`StorageQuota::delegate`], which removes bytes from the parent as it creates the child, and
//! returns by [`StorageQuota::absorb`], which consumes the child. Two quotas that sum to the
//! original are the only way to have two.
//!
//! `Serialize` is implemented, because reporting a budget is not duplicating one — the same
//! asymmetry `bioprism-weave`'s `View` uses.
//!
//! # Deliberately not implemented
//!
//! No time windows: 12.21's "per day/month" needs a calendar, and this crate has no clock. No
//! cost model, no currency, no provider rates — [`StorageQuota`] counts bytes, and
//! `bioprism-scale`'s cost accounting is where money belongs. No fair-share scheduling across
//! campaigns, no concurrency limits, no rate limiting. No enforcement: nothing here can stop a
//! write, it can only refuse to authorize one.

use crate::error::QuotaError;
use serde::Serialize;
use std::collections::BTreeMap;
use std::fmt;

/// What a charge is for. Determines whether the reserve may be drawn on.
///
/// 12.21's reserve exists so that a project which exhausts its budget can still finish writing
/// the evidence for the work it already did. A system that hard-stops at the limit strands
/// half-written results, which costs more storage than it saves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub enum Purpose {
    /// Ordinary work. Stops at the limit less the reserve.
    Ingest,
    /// Writing out the evidence for work already done. May draw on the reserve.
    EvidenceFinalization,
    /// Compaction, tombstoning, garbage collection. May draw on the reserve, because reclaiming
    /// space sometimes costs space first.
    Cleanup,
}

impl Purpose {
    pub fn may_use_reserve(self) -> bool {
        match self {
            Purpose::Ingest => false,
            Purpose::EvidenceFinalization | Purpose::Cleanup => true,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Purpose::Ingest => "ingest",
            Purpose::EvidenceFinalization => "evidence-finalization",
            Purpose::Cleanup => "cleanup",
        }
    }
}

/// What kind of data the bytes are, for attribution.
///
/// 12.21 requires that "every result includes resource policy and realized usage" and that "raw
/// totals remain available". Charges are therefore attributed by class *and* summed, so an
/// allocation rule applied later cannot destroy the underlying numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub enum StorageClass {
    /// Immutable content-addressed objects.
    Objects,
    /// The append-only event log.
    Events,
    /// Rebuildable indexes and projections.
    Indexes,
    /// Analytical result tables.
    Results,
    /// Cache entries and other reconstructible working data.
    Cache,
}

impl StorageClass {
    pub fn name(self) -> &'static str {
        match self {
            StorageClass::Objects => "objects",
            StorageClass::Events => "events",
            StorageClass::Indexes => "indexes",
            StorageClass::Results => "results",
            StorageClass::Cache => "cache",
        }
    }

    /// Whether losing this class costs evidence or only time.
    ///
    /// 12.08: rebuilding search indexes and dashboards "is inconvenient, not evidentiary
    /// corruption". A quota planner facing pressure should shed the reconstructible classes
    /// first, and this is the statement of which those are.
    pub fn is_reconstructible(self) -> bool {
        match self {
            StorageClass::Indexes | StorageClass::Cache => true,
            StorageClass::Objects | StorageClass::Events | StorageClass::Results => false,
        }
    }
}

impl fmt::Display for StorageClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// A non-copyable claim on a fixed number of bytes.
///
/// Deliberately not `Clone`, not `Copy`, not `Deserialize`. See the module documentation for why;
/// the short version is that a storage quota names a single shared physical resource, and every
/// copy of it is a second authorization to fill the same disk.
#[derive(Debug, Serialize)]
pub struct StorageQuota {
    limit: u64,
    reserve: u64,
    charged: BTreeMap<StorageClass, u64>,
}

impl StorageQuota {
    /// Creates a quota with a reserve that only cleanup and evidence finalization may draw on.
    pub fn new(limit: u64, reserve: u64) -> Result<Self, QuotaError> {
        if reserve >= limit {
            return Err(QuotaError::ReserveExceedsLimit { reserve, limit });
        }
        Ok(StorageQuota {
            limit,
            reserve,
            charged: BTreeMap::new(),
        })
    }

    pub fn limit(&self) -> u64 {
        self.limit
    }

    pub fn reserve(&self) -> u64 {
        self.reserve
    }

    pub fn used(&self) -> u64 {
        self.charged.values().sum()
    }

    /// Bytes left before the hard limit, including the reserve.
    pub fn remaining(&self) -> u64 {
        self.limit.saturating_sub(self.used())
    }

    /// Bytes left before the reserve, which is what ordinary work may use.
    pub fn remaining_for(&self, purpose: Purpose) -> u64 {
        let floor = if purpose.may_use_reserve() {
            0
        } else {
            self.reserve
        };
        self.limit.saturating_sub(self.used()).saturating_sub(floor)
    }

    /// Realized usage per class. Raw, never allocated or apportioned.
    pub fn charged_by_class(&self) -> &BTreeMap<StorageClass, u64> {
        &self.charged
    }

    /// Authorizes `bytes` for `purpose` in `class`, returning the bytes still available to that
    /// purpose.
    ///
    /// Two distinct refusals: crossing the hard limit is [`QuotaError::Exceeded`], and crossing
    /// into the reserve with an ordinary purpose is [`QuotaError::ReserveIsProtected`]. They are
    /// separate because the remedies are different — the first needs more space, the second needs
    /// the caller to say what the write is actually for.
    pub fn charge(
        &mut self,
        class: StorageClass,
        purpose: Purpose,
        bytes: u64,
    ) -> Result<u64, QuotaError> {
        let used = self.used();
        if used.saturating_add(bytes) > self.limit {
            return Err(QuotaError::Exceeded {
                class: class.name(),
                requested: bytes,
                available: self.limit.saturating_sub(used),
                limit: self.limit,
            });
        }
        if !purpose.may_use_reserve()
            && used.saturating_add(bytes) > self.limit.saturating_sub(self.reserve)
        {
            return Err(QuotaError::ReserveIsProtected {
                purpose: purpose.name(),
                requested: bytes,
                reserve: self.reserve,
            });
        }
        *self.charged.entry(class).or_insert(0) += bytes;
        Ok(self.remaining_for(purpose))
    }

    /// Returns bytes to the quota after a delete.
    ///
    /// Refuses to release more than was charged to the class, because that would mint allowance
    /// out of nothing and is how a quota drifts upward over a long-running process.
    pub fn release(&mut self, class: StorageClass, bytes: u64) -> Result<u64, QuotaError> {
        let charged = self.charged.get(&class).copied().unwrap_or(0);
        if bytes > charged {
            return Err(QuotaError::ReleaseExceedsCharge {
                class: class.name(),
                requested: bytes,
                charged,
            });
        }
        let remaining = charged - bytes;
        if remaining == 0 {
            self.charged.remove(&class);
        } else {
            self.charged.insert(class, remaining);
        }
        Ok(self.remaining())
    }

    /// Moves `bytes` of unspent allowance out of this quota and into a new one.
    ///
    /// The parent's limit shrinks by exactly what the child receives, so the two always sum to
    /// the original. This is the only way to obtain a second quota, and it is why `Clone` is
    /// absent: delegation is subtraction, copying is not.
    ///
    /// The child gets no reserve. A reserve protects the finalization of work at *this* level,
    /// and splitting it would give a subsystem the right to eat into a protection it did not
    /// establish; a delegate that needs its own reserve should be given one explicitly by
    /// constructing it from the delegated amount.
    pub fn delegate(&mut self, bytes: u64) -> Result<StorageQuota, QuotaError> {
        let available = self.limit.saturating_sub(self.used());
        if bytes > available {
            return Err(QuotaError::DelegationExceedsRemaining {
                requested: bytes,
                available,
            });
        }
        self.limit -= bytes;
        Ok(StorageQuota {
            limit: bytes,
            reserve: 0,
            charged: BTreeMap::new(),
        })
    }

    /// Takes a delegated quota back, consuming it.
    ///
    /// The child's realized charges come back with it, so the parent's per-class attribution
    /// stays true after the subsystem finishes. Consuming the child by value is what makes
    /// double-return impossible.
    pub fn absorb(&mut self, child: StorageQuota) {
        self.limit += child.limit;
        for (class, bytes) in child.charged {
            *self.charged.entry(class).or_insert(0) += bytes;
        }
    }
}

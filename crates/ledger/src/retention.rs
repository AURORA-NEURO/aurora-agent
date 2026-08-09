//! Retention, compaction and lawful deletion.
//!
//! Blueprint 40.06 and 12.22 both insist that lifecycle be *stated* rather than performed
//! quietly: garbage collection is "snapshot metadata, mark reachable, quarantine candidates,
//! grace period, delete, verify no broken closures, publish audit", with "dry run is default".
//! An append-only log that silently forgets its first year is worse than one that never
//! compacted, because every query it answers still looks authoritative.
//!
//! So compaction here produces two artefacts. A [`CompactionReport`] says exactly what was
//! removed and why each survivor survived, and a [`RetentionWindow`] becomes part of the
//! ledger's permanent state, causing as-of queries that reach behind the boundary to fail
//! loudly ([`crate::error::LedgerError::OutsideRetainedWindow`]) instead of returning a
//! confidently truncated past.
//!
//! Deliberately not implemented: grace periods, quarantine staging and legal-hold expiry. Those
//! are operational concerns that need a clock and durable storage, and this ledger has neither.
//! Pins stand in for legal holds.

use crate::error::LedgerError;
use crate::time::{RecordTime, TemporalCut, ValidTime};
use bioprism_ids::{ContentHash, EventId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// What a compacted ledger still claims to be able to answer.
///
/// The bounds are per axis because compaction by record time damages the two axes differently:
/// dropping everything learned before March removes old *beliefs* wholesale, but the facts
/// those beliefs were about may have been valid at any time at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RetentionWindow {
    /// Record-time cuts at or after this instant are exact. Earlier ones are refused.
    pub answerable_from_record: Option<RecordTime>,
    /// Valid-time cuts at or after this instant are exact. Earlier ones are refused.
    pub answerable_from_valid: Option<ValidTime>,
    /// Lowest sequence number still held. Checkpoints older than this cannot be resumed.
    pub earliest_retained_seq: u64,
}

impl RetentionWindow {
    /// A ledger that has never compacted answers everything.
    pub fn unrestricted() -> Self {
        RetentionWindow::default()
    }

    pub fn is_unrestricted(&self) -> bool {
        self.answerable_from_record.is_none() && self.answerable_from_valid.is_none()
    }

    /// Takes the more restrictive of two windows, so repeated compaction never appears to
    /// recover history it already destroyed.
    pub fn tighten(self, other: RetentionWindow) -> Self {
        RetentionWindow {
            answerable_from_record: later_record(
                self.answerable_from_record,
                other.answerable_from_record,
            ),
            answerable_from_valid: later_valid(
                self.answerable_from_valid,
                other.answerable_from_valid,
            ),
            earliest_retained_seq: self.earliest_retained_seq.max(other.earliest_retained_seq),
        }
    }

    /// Refuses a cut that reaches behind the boundary on either axis.
    pub fn admits(&self, cut: &TemporalCut) -> Result<(), LedgerError> {
        if let (Some(requested), Some(from)) = (cut.as_of_record, self.answerable_from_record) {
            if requested < from {
                return Err(LedgerError::OutsideRetainedWindow {
                    axis: RecordTime::AXIS,
                    requested: requested.to_string(),
                    retained_from: from.to_string(),
                });
            }
        }
        if let (Some(requested), Some(from)) = (cut.as_of_valid, self.answerable_from_valid) {
            if requested < from {
                return Err(LedgerError::OutsideRetainedWindow {
                    axis: ValidTime::AXIS,
                    requested: requested.to_string(),
                    retained_from: from.to_string(),
                });
            }
        }
        Ok(())
    }
}

fn later_record(a: Option<RecordTime>, b: Option<RecordTime>) -> Option<RecordTime> {
    match (a, b) {
        (Some(x), Some(y)) => Some(if x >= y { x } else { y }),
        (Some(x), None) => Some(x),
        (None, y) => y,
    }
}

fn later_valid(a: Option<ValidTime>, b: Option<ValidTime>) -> Option<ValidTime> {
    match (a, b) {
        (Some(x), Some(y)) => Some(if x >= y { x } else { y }),
        (Some(x), None) => Some(x),
        (None, y) => y,
    }
}

/// What to compact away, and what must survive regardless.
///
/// `dry_run` defaults to true, matching 12.22's "dry run is default". A caller who has not
/// read the report has not asked for deletion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetentionPolicy {
    /// Entries whose record time is strictly before this are candidates for removal.
    pub compact_before_record: RecordTime,
    /// Legal holds, incident references and public-release pins (12.22's retention graph).
    /// A pinned entry is never removed, and neither is anything it causally depends on.
    pub pinned: BTreeSet<EventId>,
    pub dry_run: bool,
}

impl RetentionPolicy {
    pub fn before(compact_before_record: RecordTime) -> Self {
        RetentionPolicy {
            compact_before_record,
            pinned: BTreeSet::new(),
            dry_run: true,
        }
    }

    pub fn pinning(mut self, pins: impl IntoIterator<Item = EventId>) -> Self {
        self.pinned.extend(pins);
        self
    }

    /// Opts out of the dry run. Named for what it does rather than taking a bare boolean.
    pub fn applying(mut self) -> Self {
        self.dry_run = false;
        self
    }
}

/// The permanent record that a compaction happened, committing to the state it destroyed.
///
/// `head_digest_before` is the point of it: the anchor commits to the digest of the whole
/// pre-compaction log, so a compacted ledger still carries a cryptographic statement about the
/// history it no longer holds. An operator who kept a copy can prove the two agree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompactionAnchor {
    pub bound: RecordTime,
    pub removed_count: u64,
    pub head_seq_before: u64,
    pub head_digest_before: String,
    pub window: RetentionWindow,
    pub digest: String,
}

impl CompactionAnchor {
    pub(crate) fn seal(
        bound: RecordTime,
        removed_count: u64,
        head_seq_before: u64,
        head_digest_before: String,
        window: RetentionWindow,
    ) -> Self {
        let digest = anchor_digest(
            bound,
            removed_count,
            head_seq_before,
            &head_digest_before,
            &window,
        );
        CompactionAnchor {
            bound,
            removed_count,
            head_seq_before,
            head_digest_before,
            window,
            digest,
        }
    }

    pub fn recompute_digest(&self) -> String {
        anchor_digest(
            self.bound,
            self.removed_count,
            self.head_seq_before,
            &self.head_digest_before,
            &self.window,
        )
    }
}

fn anchor_digest(
    bound: RecordTime,
    removed_count: u64,
    head_seq_before: u64,
    head_digest_before: &str,
    window: &RetentionWindow,
) -> String {
    let body = serde_json::json!({
        "bound": bound.to_string(),
        "removed_count": removed_count,
        "head_seq_before": head_seq_before,
        "head_digest_before": head_digest_before,
        "answerable_from_record": window.answerable_from_record.map(|t| t.to_string()),
        "answerable_from_valid": window.answerable_from_valid.map(|t| t.to_string()),
        "earliest_retained_seq": window.earliest_retained_seq,
    });
    ContentHash::of_value(&body)
        .expect("anchor body is strings and integers, which always canonicalize")
        .as_str()
        .to_string()
}

/// Why each survivor survived, and what the ledger will admit to answering afterwards.
///
/// The four retention reasons are not decoration. 12.04 requires GC to "verify no broken
/// closures", and a compaction that silently orphaned a causal parent or dropped the original
/// half of a supersession pair would leave the log self-inconsistent in a way no later query
/// could detect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompactionReport {
    pub dry_run: bool,
    /// Entries whose record time fell before the bound.
    pub examined: u64,
    pub removed: u64,
    /// Kept because they are the newest thing known about their subject at the bound; without
    /// them, state queries just inside the window would silently lose the subject entirely.
    pub retained_by_carry_forward: BTreeSet<EventId>,
    /// Kept because a surviving entry names them as a causal parent.
    pub retained_by_causal_closure: BTreeSet<EventId>,
    /// Kept because a surviving correction supersedes them.
    pub retained_by_supersession_closure: BTreeSet<EventId>,
    /// Kept because of a legal hold or an explicit pin.
    pub retained_by_pin: BTreeSet<EventId>,
    pub window: RetentionWindow,
    /// Absent on a dry run, because nothing was destroyed to commit to.
    pub anchor: Option<CompactionAnchor>,
}

impl CompactionReport {
    /// Total survivors among the examined candidates.
    pub fn retained(&self) -> u64 {
        self.examined - self.removed
    }
}

/// The tombstone left by a lawful deletion.
///
/// 12.22: destroy the protected payload, retain a "minimal non-identifying tombstone and impact
/// notice". The digest is the non-identifying part — it proves the entry is unchanged without
/// revealing what it said.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Redaction {
    pub id: EventId,
    pub reason: String,
    pub payload_digest: String,
    /// Entry digest before and after, which are equal. Reported rather than asserted, so a
    /// caller can check the claim instead of believing the documentation.
    pub entry_digest: String,
}

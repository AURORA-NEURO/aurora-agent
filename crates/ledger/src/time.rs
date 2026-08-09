//! The three time axes, kept apart by the type system.
//!
//! Blueprint 40.09 invariant 3: "Valid time is distinct from record and release time." That
//! sentence is cheap to agree with and expensive to enforce, because all three are instants and
//! a single `Timestamp` field will happily accept whichever one the caller had to hand.
//!
//! So each axis is its own newtype over [`bioprism_scope::Timestamp`], with no `From`
//! conversions between them. Passing a record time where a valid time belongs is a compile
//! error, not a subtly wrong answer six months later.
//!
//! The distinction is not pedantry. `bioprism-fiber`'s temporal cut decides which evidence a
//! decision was *allowed* to see; conflating "when the tumour actually progressed" (valid) with
//! "when the registry told us" (record) with "when the cohort was unblinded" (release) is
//! precisely the leakage this platform exists to detect. A model scored against evidence whose
//! release time is after the decision point has been given the answer.
//!
//! Deliberately not implemented: any way to *read* a clock. Every time on every event is passed
//! in by the caller, so a ledger built from the same inputs is byte-identical on every machine.

use crate::error::LedgerError;
use bioprism_scope::{TimeError, Timestamp};
use serde::{Deserialize, Serialize};
use std::fmt;

macro_rules! time_axis {
    ($(#[$meta:meta])* $name:ident, $axis:literal) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(Timestamp);

        impl $name {
            /// Names this axis in diagnostics, so an out-of-window error says which clock it means.
            pub const AXIS: &'static str = $axis;

            pub const fn new(instant: Timestamp) -> Self {
                $name(instant)
            }

            pub fn parse(text: &str) -> Result<Self, TimeError> {
                Timestamp::parse(text).map($name)
            }

            /// Drops back to the untyped instant. Explicit, because every use of it is a place
            /// where the axis distinction stops being enforced.
            pub const fn instant(self) -> Timestamp {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }
    };
}

time_axis!(
    /// When the fact was true in the world.
    ///
    /// May lie in the future (a forecast), may lie far in the past (a backfilled observation),
    /// and bears no required ordering against record time.
    ValidTime,
    "valid"
);
time_axis!(
    /// When the system learned the fact.
    ///
    /// This is the axis that makes "what did we believe last March" answerable. It is never
    /// rewritten: a correction gets its own, later record time.
    RecordTime,
    "record"
);
time_axis!(
    /// When the fact became readable to consumers.
    ///
    /// Distinct from record time because of embargo, blinding and staged publication. A ledger
    /// can hold a fact it must not yet show anyone, and a reader restricted to a release cut
    /// must not see it.
    ReleaseTime,
    "release"
);

/// All three axes for one event, checked for the single ordering that must hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventTimes {
    pub valid: ValidTime,
    pub record: RecordTime,
    pub release: ReleaseTime,
}

impl EventTimes {
    /// Rejects a release that precedes its own record: nothing can be published before it is
    /// known. No constraint is imposed between valid time and the other two, because forecasts
    /// and backfills are both legitimate.
    pub fn new(
        valid: ValidTime,
        record: RecordTime,
        release: ReleaseTime,
    ) -> Result<Self, LedgerError> {
        if release.instant() < record.instant() {
            return Err(LedgerError::ReleaseBeforeRecord {
                record: record.to_string(),
                release: release.to_string(),
            });
        }
        Ok(EventTimes {
            valid,
            record,
            release,
        })
    }

    /// The common case where a fact is readable the moment it is recorded.
    pub fn published_on_record(valid: ValidTime, record: RecordTime) -> Self {
        EventTimes {
            valid,
            record,
            release: ReleaseTime::new(record.instant()),
        }
    }
}

/// A question about the log, phrased as an upper bound on each axis.
///
/// An unset bound means "no restriction on this axis", not "now" — the ledger never consults a
/// clock, so there is no "now" for it to substitute.
///
/// The three bounds compose: `as_of_record` alone asks *what did we know then*, `as_of_valid`
/// alone asks *what was true then*, and both together ask the bitemporal question *what did we
/// then believe was true then*. On a log containing a late correction these have different
/// answers, which is the whole point.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalCut {
    pub as_of_valid: Option<ValidTime>,
    pub as_of_record: Option<RecordTime>,
    pub as_of_release: Option<ReleaseTime>,
}

impl TemporalCut {
    /// Everything the ledger holds, on every axis.
    pub const EVERYTHING: TemporalCut = TemporalCut {
        as_of_valid: None,
        as_of_record: None,
        as_of_release: None,
    };

    /// What was known at this record time, regardless of when it was true.
    pub fn known_at(record: RecordTime) -> Self {
        TemporalCut {
            as_of_record: Some(record),
            ..TemporalCut::EVERYTHING
        }
    }

    /// What was true at this valid time, in the light of everything learned since.
    pub fn true_at(valid: ValidTime) -> Self {
        TemporalCut {
            as_of_valid: Some(valid),
            ..TemporalCut::EVERYTHING
        }
    }

    /// The bitemporal question: what we believed at `record` about the world at `valid`.
    pub fn believed_at(valid: ValidTime, record: RecordTime) -> Self {
        TemporalCut {
            as_of_valid: Some(valid),
            as_of_record: Some(record),
            as_of_release: None,
        }
    }

    /// What a reader was permitted to see. This is the cut a scored agent gets.
    pub fn readable_at(release: ReleaseTime) -> Self {
        TemporalCut {
            as_of_release: Some(release),
            ..TemporalCut::EVERYTHING
        }
    }

    pub fn with_valid(mut self, valid: ValidTime) -> Self {
        self.as_of_valid = Some(valid);
        self
    }

    pub fn with_record(mut self, record: RecordTime) -> Self {
        self.as_of_record = Some(record);
        self
    }

    pub fn with_release(mut self, release: ReleaseTime) -> Self {
        self.as_of_release = Some(release);
        self
    }

    /// Whether an event's stamps fall inside every bound this cut sets.
    pub fn admits(&self, times: &EventTimes) -> bool {
        self.as_of_valid.is_none_or(|bound| times.valid <= bound)
            && self.as_of_record.is_none_or(|bound| times.record <= bound)
            && self
                .as_of_release
                .is_none_or(|bound| times.release <= bound)
    }
}

//! Typed failures for the event ledger.
//!
//! Blueprint 40.09 names five failure classes — duplicate idempotency key, causal parent
//! missing, clock inconsistency, projection lag, and schema unknown — and requires each to be
//! *observable* rather than silently repaired. Two of them are deliberately not errors here:
//! a missing causal parent quarantines the event (invariant 4 of 40.09 says "resolve or
//! quarantined", not "reject"), and projection lag is a measurement, not a fault. The rest are
//! variants below.

use bioprism_ids::{CanonicalError, EventId, IdError};
use thiserror::Error;

/// Every way an append, a query, a projection resume or a compaction can refuse.
///
/// `Clone + PartialEq` so tests can assert on the exact failure rather than on a message
/// substring, which is how error taxonomies rot.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum LedgerError {
    /// The same idempotency key was offered with a different body (40.09, "duplicate
    /// idempotency key"). Re-offering the *identical* body is not an error; it is a no-op.
    #[error("idempotency key {key:?} was already used by {existing} with a different body")]
    IdempotencyConflict { key: String, existing: EventId },

    /// Release time precedes record time: a fact was published before it was learned. This is
    /// the "clock inconsistency" class, and it is the exact shape of the leakage bug that
    /// availability-versus-occurrence separation exists to catch.
    #[error("release time {release} precedes record time {record}")]
    ReleaseBeforeRecord { record: String, release: String },

    /// A correction claims to have been learned before the entry it corrects.
    #[error("correction recorded at {correction} precedes the entry {target} it supersedes, recorded at {original}")]
    CorrectionPrecedesOriginal {
        target: EventId,
        original: String,
        correction: String,
    },

    /// The event kind is not in a closed schema catalog (40.09, "schema unknown").
    #[error("unknown event schema {kind:?}; the catalog is closed")]
    UnknownSchema { kind: String },

    /// An actor, subject or kind was blank or contained control characters.
    #[error("malformed {field}: {value:?}")]
    MalformedField { field: &'static str, value: String },

    /// A query, a supersession or a pin named an event the ledger does not hold. After
    /// compaction this can mean "removed", not "never existed"; the retained window says which.
    #[error("no event {0} in this ledger")]
    UnknownEvent(EventId),

    /// Supersession forks: two corrections claiming the same target leave no single current
    /// version, so the second is refused rather than guessed at.
    #[error("event {target} is already superseded by {by}")]
    AlreadySuperseded { target: EventId, by: EventId },

    /// A checkpoint was taken against a different log than the one being resumed against.
    #[error(
        "checkpoint at seq {seq} commits to entry digest {expected}, but this ledger has {found}"
    )]
    CheckpointDivergence {
        seq: u64,
        expected: String,
        found: String,
    },

    /// The checkpoint's state does not hash to the digest it carries.
    #[error("checkpoint state digest {carried} does not match the recomputed {recomputed}")]
    CheckpointStateMismatch { carried: String, recomputed: String },

    /// A checkpoint's resume point was compacted away, so the entries after it are gone.
    #[error(
        "checkpoint resumes at seq {seq}, which is before the earliest retained seq {earliest}"
    )]
    CheckpointOutsideRetention { seq: u64, earliest: u64 },

    /// An as-of query reached behind the compaction boundary. Compaction states what it kept;
    /// answering anyway would return a confidently wrong past.
    #[error("as-of {axis} query at {requested} is outside the retained window, which begins at {retained_from}")]
    OutsideRetainedWindow {
        axis: &'static str,
        requested: String,
        retained_from: String,
    },

    /// The payload was cryptographically erased under a deletion policy (12.22). The digest
    /// and the tombstone remain; the body does not.
    #[error("payload of {id} is redacted: {reason}")]
    PayloadRedacted { id: EventId, reason: String },

    /// A projection state could not be canonically serialized, so it cannot be checkpointed.
    #[error("projection state for {projection:?} cannot be serialized: {detail}")]
    StateNotSerializable { projection: String, detail: String },

    #[error(transparent)]
    Id(#[from] IdError),

    #[error(transparent)]
    Canonical(#[from] CanonicalError),
}

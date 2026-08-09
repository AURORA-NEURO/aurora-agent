//! Recorded entries and the hash chain over them.
//!
//! The chaining idea is borrowed from `bioprism-weave`'s act ledger, which chains coordination
//! acts for the same reason: any edit to history invalidates every later link. What is
//! different here is *what* the digest commits to. Weave hashes the act inline; this ledger
//! hashes the event's content digest, which in turn commits to the payload digest rather than
//! the payload. Two consequences follow, and both are deliberate:
//!
//! - a payload can be destroyed under a deletion policy without breaking the chain
//!   (see [`crate::retention`]), and
//! - an entry's digest is stable under re-serialisation of a payload, because it never depends
//!   on JSON key order or float formatting beyond what `bioprism-ids` already canonicalises.

use crate::event::Event;
use bioprism_ids::{ContentHash, EventId};
use serde::{Deserialize, Serialize};

/// One immutable record in the log.
///
/// There is no method that mutates an entry's event, times, parents or digest, and there never
/// should be: 40.09 invariant 1 is "events are append-only". The single exception is payload
/// redaction, which replaces the body with a tombstone while leaving the digest — the thing the
/// chain commits to — untouched.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LedgerEntry {
    /// Monotonic within a ledger, assigned on admission, never reused. Survives compaction, so
    /// a gap in the sequence is evidence that something was removed.
    pub seq: u64,
    pub id: EventId,
    pub event: Event,
    /// Digest of the entry that preceded this one at the time of writing. Empty for the first.
    pub previous: String,
    pub digest: String,
}

impl LedgerEntry {
    pub(crate) fn seal(seq: u64, id: EventId, event: Event, previous: String) -> Self {
        let digest = digest_of(seq, &id, &event.content_digest(), &previous);
        LedgerEntry {
            seq,
            id,
            event,
            previous,
            digest,
        }
    }

    /// Recomputes this entry's digest from its own contents, the way an auditor must before
    /// trusting anything derived from it.
    pub fn recompute_digest(&self) -> String {
        digest_of(
            self.seq,
            &self.id,
            &self.event.content_digest(),
            &self.previous,
        )
    }
}

pub(crate) fn digest_of(seq: u64, id: &EventId, content_digest: &str, previous: &str) -> String {
    let body = serde_json::json!({
        "seq": seq,
        "id": id.as_str(),
        "content": content_digest,
        "previous": previous,
    });
    ContentHash::of_value(&body)
        .expect("entry digest body is strings and an integer, which always canonicalize")
        .as_str()
        .to_string()
}

/// The result of re-deriving the chain.
///
/// `Broken` carries the sequence number and a human reason rather than a bare boolean, because
/// the first question an auditor asks after "is it intact" is "where".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum ChainStatus {
    Intact,
    Broken { at_seq: u64, reason: String },
}

impl ChainStatus {
    pub fn is_intact(&self) -> bool {
        matches!(self, ChainStatus::Intact)
    }

    pub(crate) fn broken(at_seq: u64, reason: impl Into<String>) -> Self {
        ChainStatus::Broken {
            at_seq,
            reason: reason.into(),
        }
    }
}

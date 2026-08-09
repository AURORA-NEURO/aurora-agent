//! Input provenance: unknown is not untrusted, and untrusted is not unknown.
//!
//! Blueprint 13.15 (supply chain and artifact security) requires that dependencies be pinned and
//! verified, that assets be addressed "by digest", that model and dataset revisions be immutable,
//! and that there be "no floating `latest` in published runs".
//!
//! # The distinction this module exists to protect
//!
//! The workspace's first non-negotiable is that "provably cannot matter" and "nobody checked" never
//! share a representation. Supply chain has the same shape. An input whose provenance was never
//! recorded and an input whose provenance was recorded and found wanting are different states:
//!
//! - [`ProvenanceState::Unrecorded`] — nobody looked. This is an *absence of evidence*. It is not a
//!   trust judgement, and code that renders it as "untrusted" has invented a judgement no one made.
//! - [`ProvenanceState::Rejected`] — someone looked and it failed. This is *evidence of a problem*,
//!   with a named [`RejectionReason`].
//! - [`ProvenanceState::Recorded`] — a source and a digest were recorded. Note what this does and
//!   does not mean below.
//!
//! There is deliberately no `is_trusted()`, no `unwrap_or_untrusted()` and no ordering over the
//! three, because an ordering invites `state >= Threshold` and that is how the distinction dies.
//!
//! # What "Recorded" does not mean
//!
//! [`ProvenanceState::Recorded`] means a caller wrote down where an artifact came from and what it
//! hashed to. It does not mean this crate fetched the artifact, re-hashed it, checked a signature on
//! it, scanned it, or verified that the recorded source serves that digest. This crate performs no
//! I/O. 13.15's §Scanning and §Build isolation are not implemented here at all.
//!
//! # Deliberately not implemented
//!
//! No SBOM or benchmark-bill-of-materials generation, no vulnerability or malware scanning, no
//! license conflict detection, no quarantine workflow, no key rotation on compromise, and no lineage
//! walk to find dependent results. 13.15 §Response describes an operational process; this module
//! supplies the record it would read from.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::fmt;

/// What is known about where an input came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ProvenanceState {
    /// A source and digest were written down. See the module docs for what that does not mean.
    Recorded(RecordedProvenance),
    /// Nobody recorded provenance for this input. An absence of evidence, not a verdict.
    Unrecorded,
    /// Provenance was evaluated and failed. A verdict, with a reason.
    Rejected(RejectedProvenance),
}

impl ProvenanceState {
    /// True only for [`ProvenanceState::Unrecorded`].
    ///
    /// Paired with [`Self::is_rejected`] rather than with an `is_trusted`, so that a caller writing
    /// a UI has to handle three cases and cannot reach for a boolean that flattens two of them.
    pub fn is_unknown(&self) -> bool {
        matches!(self, ProvenanceState::Unrecorded)
    }

    /// True only for [`ProvenanceState::Rejected`].
    pub fn is_rejected(&self) -> bool {
        matches!(self, ProvenanceState::Rejected(_))
    }

    /// The one-line rendering a surface must use, which never says "untrusted" for unrecorded.
    pub fn honest_label(&self) -> String {
        match self {
            ProvenanceState::Recorded(recorded) => format!(
                "recorded: {} at {} (recorded, not independently re-fetched or re-hashed)",
                recorded.source, recorded.digest
            ),
            ProvenanceState::Unrecorded => {
                "no provenance recorded — nobody checked; this is not a trust judgement".to_string()
            }
            ProvenanceState::Rejected(rejected) => {
                format!("rejected: {} ({})", rejected.source, rejected.reason)
            }
        }
    }
}

/// Where an artifact was said to come from, and what it was said to hash to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordedProvenance {
    /// A registry reference, path or URI. Opaque to this crate, which never dereferences it.
    pub source: String,
    /// The artifact's content hash as recorded by whoever pinned it.
    pub digest: ContentHash,
    /// The immutable revision 13.15 §Pinning demands. `None` means the source was recorded without
    /// a revision, which is a weaker record and is why this is an option rather than an empty string.
    pub pinned_revision: Option<String>,
}

impl RecordedProvenance {
    pub fn new(source: impl Into<String>, digest: ContentHash) -> Self {
        RecordedProvenance {
            source: source.into(),
            digest,
            pinned_revision: None,
        }
    }

    pub fn pinned_at(mut self, revision: impl Into<String>) -> Self {
        self.pinned_revision = Some(revision.into());
        self
    }

    /// True when a source was recorded without the immutable revision 13.15 §Pinning requires.
    pub fn lacks_immutable_revision(&self) -> bool {
        self.pinned_revision.is_none()
    }
}

/// Provenance that was evaluated and found wanting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RejectedProvenance {
    pub source: String,
    pub reason: RejectionReason,
}

impl RejectedProvenance {
    pub fn new(source: impl Into<String>, reason: RejectionReason) -> Self {
        RejectedProvenance {
            source: source.into(),
            reason,
        }
    }
}

/// Why recorded provenance was rejected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum RejectionReason {
    /// The artifact's content did not hash to the pinned digest — 13.15's substitution case.
    DigestMismatch { pinned: String, observed: String },
    /// The reference resolves differently over time. 13.15 §Pinning: "no floating `latest`".
    FloatingRevision { reference: String },
    /// A policy outside this crate declined the source. The rationale is carried verbatim.
    PolicyDenied { rationale: String },
    /// The artifact was withdrawn or quarantined after the run that used it.
    Quarantined { rationale: String },
}

impl fmt::Display for RejectionReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RejectionReason::DigestMismatch { pinned, observed } => {
                write!(f, "digest mismatch: pinned {pinned}, observed {observed}")
            }
            RejectionReason::FloatingRevision { reference } => {
                write!(f, "floating revision `{reference}` does not pin an immutable artifact")
            }
            RejectionReason::PolicyDenied { rationale } => write!(f, "policy denied: {rationale}"),
            RejectionReason::Quarantined { rationale } => write!(f, "quarantined: {rationale}"),
        }
    }
}

/// The supply-chain picture across a whole bundle, with the three states kept apart.
///
/// Deliberately three lists rather than a single worst-case verdict. A bundle with two unrecorded
/// inputs and one rejected input is not "rejected", and it is not "unrecorded" either; it is both,
/// and a reader needs to see both to decide anything.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupplyChainPosture {
    pub recorded: Vec<String>,
    pub unrecorded: Vec<String>,
    pub rejected: Vec<String>,
}

impl SupplyChainPosture {
    /// True only when every input has recorded provenance and none was rejected.
    pub fn is_fully_recorded(&self) -> bool {
        self.unrecorded.is_empty() && self.rejected.is_empty()
    }

    /// The sentence a result card must print next to any provenance summary.
    pub fn honest_label(&self) -> String {
        format!(
            "{} input(s) with recorded provenance, {} with none recorded (nobody checked), \
             {} rejected (checked and failed)",
            self.recorded.len(),
            self.unrecorded.len(),
            self.rejected.len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(seed: &str) -> ContentHash {
        ContentHash::of_bytes(seed.as_bytes())
    }

    #[test]
    fn unrecorded_provenance_is_not_reported_as_untrusted() {
        let unknown = ProvenanceState::Unrecorded;
        assert!(unknown.is_unknown());
        assert!(!unknown.is_rejected());
        let label = unknown.honest_label();
        assert!(label.contains("nobody checked"), "{label}");
        assert!(!label.contains("untrusted"), "{label}");
    }

    #[test]
    fn rejected_provenance_is_not_reported_as_unknown() {
        let rejected = ProvenanceState::Rejected(RejectedProvenance::new(
            "registry://pack@v1",
            RejectionReason::DigestMismatch {
                pinned: "aa".into(),
                observed: "bb".into(),
            },
        ));
        assert!(rejected.is_rejected());
        assert!(!rejected.is_unknown());
        assert!(rejected.honest_label().contains("digest mismatch"));
    }

    #[test]
    fn a_recorded_source_without_a_revision_is_flagged_rather_than_treated_as_pinned() {
        let loose = RecordedProvenance::new("registry://pack", digest("x"));
        assert!(loose.lacks_immutable_revision());
        assert!(!loose.pinned_at("sha:deadbeef").lacks_immutable_revision());
    }

    #[test]
    fn a_posture_with_both_unrecorded_and_rejected_inputs_reports_both() {
        let posture = SupplyChainPosture {
            recorded: vec!["world".into()],
            unrecorded: vec!["oracle".into(), "tokenizer".into()],
            rejected: vec!["pack".into()],
        };
        assert!(!posture.is_fully_recorded());
        let label = posture.honest_label();
        assert!(label.contains("2 with none recorded"), "{label}");
        assert!(label.contains("1 rejected"), "{label}");
    }

    #[test]
    fn a_posture_is_fully_recorded_only_when_nothing_is_unrecorded_or_rejected() {
        let clean = SupplyChainPosture {
            recorded: vec!["world".into()],
            ..SupplyChainPosture::default()
        };
        assert!(clean.is_fully_recorded());
        let with_unknown = SupplyChainPosture {
            unrecorded: vec!["oracle".into()],
            ..clean.clone()
        };
        assert!(!with_unknown.is_fully_recorded());
    }

    #[test]
    fn every_rejection_reason_explains_itself_without_using_the_word_unknown() {
        let reasons = [
            RejectionReason::DigestMismatch {
                pinned: "aa".into(),
                observed: "bb".into(),
            },
            RejectionReason::FloatingRevision {
                reference: "latest".into(),
            },
            RejectionReason::PolicyDenied {
                rationale: "unlicensed dataset".into(),
            },
            RejectionReason::Quarantined {
                rationale: "upstream compromise".into(),
            },
        ];
        for reason in reasons {
            let rendered = reason.to_string();
            assert!(!rendered.is_empty());
            assert!(
                !rendered.contains("unknown"),
                "a rejection is a verdict and must never read as an absence of one: {rendered}"
            );
        }
    }

    #[test]
    fn provenance_states_survive_a_json_round_trip_with_their_tag() {
        let states = vec![
            ProvenanceState::Recorded(
                RecordedProvenance::new("registry://pack", digest("p")).pinned_at("v1.2.3"),
            ),
            ProvenanceState::Unrecorded,
            ProvenanceState::Rejected(RejectedProvenance::new(
                "registry://loose",
                RejectionReason::FloatingRevision {
                    reference: "latest".into(),
                },
            )),
        ];
        let json = serde_json::to_string(&states).expect("serialises");
        assert!(json.contains("\"unrecorded\""), "{json}");
        assert!(json.contains("\"rejected\""), "{json}");
        let back: Vec<ProvenanceState> = serde_json::from_str(&json).expect("deserialises");
        assert_eq!(back, states);
    }
}

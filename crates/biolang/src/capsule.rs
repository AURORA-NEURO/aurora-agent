//! BioContext Capsule IR — blueprint 25.16.
//!
//! What one recipient is given: an objective, a success contract, the evidence sorted by stance, the
//! assumptions still open, the actions reachable, the authority and budget held, and — the part the
//! blueprint keeps returning to — the omissions, each with a reason.
//!
//! # Where the IR and the implementing crate disagree
//!
//! `bioprism-weave::ContextCapsule` is the running implementation and it is narrower than 25.16 in a
//! specific, defensible way: it is a *projection with a withholding record*. It carries the
//! recipient, the role, the layer, the content, the withheld items with the labels that withheld
//! them, and whether the upstream compiler's sufficiency claim survives. It does not carry an
//! objective, a success contract, an evidence tri-partition, an assumption set, an action list, a
//! budget or a staleness declaration.
//!
//! That is seven of 25.16's ten required field groups absent from the implementation. The reading
//! that makes both correct: weave's capsule is a *transport* — it answers "what may this recipient
//! see" — while 25.16 describes a *briefing*, which answers "what is this recipient supposed to do
//! with it". This IR represents the briefing and treats a weave capsule as its evidence-and-omission
//! substrate, which is why [`crate::projection::ProjectionGap`] exists: a projection from a weave
//! capsule fills three field groups and declares seven unfilled, rather than inventing an objective.
//!
//! One field group weave has that 25.16 does not name is worth recording in the other direction:
//! `upstream_supports_sufficiency`. A capsule built from a compiled Decision Section inherits the
//! compiler's certificate, and whether the omissions could have changed the decision is exactly the
//! honest-labelling property this workspace refuses to lose. [`BioContextCapsule`] keeps it.

use crate::error::CapsuleError;
use crate::ids::{ActionId, ObligationId};
use bioprism_bioir::EvidenceId;
use bioprism_scope::Timestamp;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// How the capsule's author stands towards a piece of evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stance {
    /// Checked against a stronger oracle and held.
    Verified,
    /// Present and unchallenged, but not verified.
    Provisional,
    /// Contradicted by other evidence, and retained anyway.
    Contradicted,
}

impl fmt::Display for Stance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Stance::Verified => "verified",
            Stance::Provisional => "provisional",
            Stance::Contradicted => "contradicted",
        })
    }
}

/// Something the recipient is being asked to take as given.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Assumption {
    pub statement: String,
    /// What would settle it. An assumption nobody could ever discharge is worth spotting.
    pub discharged_by: Option<String>,
}

/// Something withheld, and why. 25.16: "Omissions are explicit."
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Omission {
    pub item: String,
    /// Why. Empty is refused by [`BioContextCapsule::validate`].
    pub reason: String,
    /// Labels the recipient would have needed. Empty means it was omitted for a non-label reason.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub required_labels: BTreeSet<String>,
}

/// A derived statement, and the evidence it came from. 25.16: "Derived summaries point to source
/// evidence."
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Summary {
    pub summary_id: String,
    pub text: String,
    pub sources: BTreeSet<EvidenceId>,
}

/// How stale the capsule is allowed to be.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "staleness", rename_all = "snake_case")]
pub enum Staleness {
    /// Built at this instant from a world at this digest, and valid until the world changes.
    AsOf {
        built_at: Timestamp,
        world_digest: String,
    },
    /// Built at this instant and explicitly not tracked against the world afterwards.
    Untracked { built_at: Timestamp },
}

/// The briefing 25.16 describes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BioContextCapsule {
    pub recipient: String,
    pub recipient_role: String,
    /// What the recipient is being asked to do.
    pub objective: String,
    /// What counts as done. 25.16 requires a "success contract".
    pub success_contract: String,
    /// Evidence by stance. A single item may not appear under two stances.
    pub evidence: BTreeMap<Stance, BTreeSet<EvidenceId>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assumptions: Vec<Assumption>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub accessible_actions: BTreeSet<ActionId>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub authority: BTreeSet<String>,
    /// Remaining budget, by resource name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub budget: BTreeMap<String, f64>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub open_obligations: BTreeSet<ObligationId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub omissions: Vec<Omission>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub summaries: Vec<Summary>,
    pub staleness: Staleness,
    /// Whether the upstream compiler's sufficiency claim survives this projection.
    ///
    /// Carried from `bioprism-weave`. Not a 25.16 field, and the one place this IR is *wider* than
    /// the blueprint: a capsule that withheld something material cannot inherit a sufficiency claim,
    /// and dropping the flag on the way into the IR would lose that.
    pub upstream_supports_sufficiency: bool,
}

impl BioContextCapsule {
    /// The invariants 25.16 states.
    ///
    /// The clearance check needs what the recipient holds, which is not part of the capsule: a
    /// capsule that recorded its own recipient's clearance could be made to agree with itself.
    pub fn validate(&self, recipient_labels: &BTreeSet<String>) -> Result<(), CapsuleError> {
        for omission in &self.omissions {
            if omission.reason.trim().is_empty() {
                return Err(CapsuleError::OmissionWithoutReason {
                    recipient: self.recipient.clone(),
                    item: omission.item.clone(),
                });
            }
        }

        for summary in &self.summaries {
            if summary.sources.is_empty() {
                return Err(CapsuleError::SummaryWithoutSource {
                    summary: summary.summary_id.clone(),
                });
            }
        }

        let mut seen: BTreeMap<&EvidenceId, Stance> = BTreeMap::new();
        for (stance, items) in &self.evidence {
            for item in items {
                if let Some(previous) = seen.insert(item, *stance) {
                    return Err(CapsuleError::ContradictoryStance {
                        item: item.to_string(),
                        left: previous.to_string(),
                        right: stance.to_string(),
                    });
                }
            }
        }

        for omission in &self.omissions {
            for label in &omission.required_labels {
                if recipient_labels.contains(label) {
                    continue;
                }
                if self.mentions(&omission.item) {
                    return Err(CapsuleError::LabelNotHeld {
                        recipient: self.recipient.clone(),
                        item: omission.item.clone(),
                        label: label.clone(),
                    });
                }
            }
        }

        Ok(())
    }

    /// Whether an item the capsule claims to have omitted is nonetheless present in it.
    fn mentions(&self, item: &str) -> bool {
        self.evidence
            .values()
            .any(|items| items.iter().any(|id| id.as_str() == item))
            || self
                .summaries
                .iter()
                .any(|summary| summary.sources.iter().any(|id| id.as_str() == item))
    }

    /// Evidence at a given stance.
    pub fn at(&self, stance: Stance) -> impl Iterator<Item = &EvidenceId> {
        self.evidence.get(&stance).into_iter().flatten()
    }
}

//! What a projection dropped, stated out loud.
//!
//! Every view in this crate is lossy. A graph cannot express a five-way factor; a timeline cannot
//! express concurrency; a table cannot express typed edges. 43.01 accepts that — projections exist
//! for navigation, not execution — but it does not accept silence about it: "no visualization is
//! treated as proof of evidence completeness", and 40.28 requires that a "projection discloses
//! filters and omissions".
//!
//! This module is the adapter-style loss ledger for that requirement. A projection writes down
//! what it flattened while it renders, and the ledger is sealed into a [`FidelityReport`] that
//! ships inside the view.
//!
//! One class of loss is *not* permitted and is enforced here rather than reported: obstructions.
//! [`FidelityLedger::seal`] refuses to close if the view failed to carry every unresolved
//! obligation and every oracle witness the section holds. Losing detail is a projection; losing an
//! obstruction is a misrepresentation.

use crate::error::ProjectionError;
use crate::identity::{conflict_id, obligation_id};
use crate::view::ProjectionKind;
use bioprism_section::DecisionSection;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Why a feature of the compiled region did not survive into the view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DropReason {
    /// The target notation has no construct for it at all.
    NotRepresentable,
    /// A multiway relation was rendered as a set of pairwise edges, which cannot express that the
    /// arguments are constrained jointly (43.01 failure mode: "a graph view hides multiway
    /// semantics").
    FlattenedToBinaryEdges,
    /// A partial order was laid out on a line, imposing an order between events the world does not
    /// order (43.09: "preserve concurrent events without arbitrary total ordering").
    TotallyOrderedForDisplay,
    /// A typed relation was rendered as text in a cell.
    FlattenedToText,
    /// Identity and structure were kept; the payload values were not.
    ValuesElided,
}

impl DropReason {
    pub fn as_str(self) -> &'static str {
        match self {
            DropReason::NotRepresentable => "not_representable",
            DropReason::FlattenedToBinaryEdges => "flattened_to_binary_edges",
            DropReason::TotallyOrderedForDisplay => "totally_ordered_for_display",
            DropReason::FlattenedToText => "flattened_to_text",
            DropReason::ValuesElided => "values_elided",
        }
    }

    /// Whether the loss changes what the view *means* rather than only how much of it is shown.
    ///
    /// A reader who needs joint semantics or true concurrency must open the factor inspector or
    /// the section; a reader who only lost payload values can still trust the shape.
    pub fn is_semantic(self) -> bool {
        matches!(
            self,
            DropReason::NotRepresentable
                | DropReason::FlattenedToBinaryEdges
                | DropReason::TotallyOrderedForDisplay
        )
    }
}

/// One named feature the projection could not carry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DroppedFeature {
    pub feature: String,
    pub reason: DropReason,
    pub count: usize,
    /// Representative members, never the whole list. Matches the omission-manifest convention of
    /// 43.26: enough for a human to recognise the class, not a second copy of the data.
    pub examples: Vec<String>,
    /// Where the reader goes to recover what was lost.
    pub recover_from: String,
}

/// Accumulates losses and carried obstructions during a render.
#[derive(Debug, Clone, Default)]
pub struct FidelityLedger {
    dropped: Vec<DroppedFeature>,
    obligations: BTreeSet<String>,
    conflicts: BTreeSet<String>,
}

impl FidelityLedger {
    /// Records a feature the view could not represent.
    pub fn drop_feature(
        &mut self,
        feature: impl Into<String>,
        reason: DropReason,
        count: usize,
        examples: Vec<String>,
        recover_from: impl Into<String>,
    ) {
        self.dropped.push(DroppedFeature {
            feature: feature.into(),
            reason,
            count,
            examples,
            recover_from: recover_from.into(),
        });
    }

    /// Declares that the view rendered the obligation with this handle.
    pub fn carry_obligation(&mut self, id: impl Into<String>) {
        self.obligations.insert(id.into());
    }

    /// Declares that the view rendered the oracle witness with this handle.
    pub fn carry_conflict(&mut self, id: impl Into<String>) {
        self.conflicts.insert(id.into());
    }

    /// Closes the ledger, refusing if any obstruction failed to reach the view.
    ///
    /// The check is by handle, not by count, so a projection that renders one obligation twice
    /// cannot pass by arithmetic.
    pub(crate) fn seal(
        self,
        kind: ProjectionKind,
        section: &DecisionSection,
    ) -> Result<FidelityReport, ProjectionError> {
        let expected_obligations: BTreeSet<String> = section
            .unresolved_obligations
            .iter()
            .enumerate()
            .map(|(index, obligation)| obligation_id(index, obligation))
            .collect();
        let carried_obligations = expected_obligations.intersection(&self.obligations).count();
        if carried_obligations != expected_obligations.len() {
            return Err(ProjectionError::ObligationDropped {
                kind,
                expected: expected_obligations.len(),
                carried: carried_obligations,
            });
        }

        let expected_conflicts: BTreeSet<String> = section
            .oracle
            .witnesses
            .iter()
            .enumerate()
            .map(|(index, witness)| conflict_id(index, witness))
            .collect();
        let carried_conflicts = expected_conflicts.intersection(&self.conflicts).count();
        if carried_conflicts != expected_conflicts.len() {
            return Err(ProjectionError::ConflictDropped {
                kind,
                expected: expected_conflicts.len(),
                carried: carried_conflicts,
            });
        }

        Ok(FidelityReport {
            kind,
            dropped: self.dropped,
            carried_obligations: expected_obligations.into_iter().collect(),
            carried_conflicts: expected_conflicts.into_iter().collect(),
        })
    }
}

/// The sealed loss ledger that ships inside a view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FidelityReport {
    pub kind: ProjectionKind,
    pub dropped: Vec<DroppedFeature>,
    /// Handles of the unresolved obligations this view rendered. Equal, by construction, to the
    /// section's full obligation set.
    pub carried_obligations: Vec<String>,
    /// Handles of the oracle witnesses this view rendered.
    pub carried_conflicts: Vec<String>,
}

impl FidelityReport {
    /// True when nothing at all was flattened. Rare, and never assumed.
    pub fn is_lossless(&self) -> bool {
        self.dropped.is_empty()
    }

    /// True when some loss changes meaning rather than only volume, so a reader must not treat the
    /// view as a self-sufficient account.
    pub fn has_semantic_loss(&self) -> bool {
        self.dropped.iter().any(|d| d.reason.is_semantic())
    }

    pub fn dropped_for(&self, reason: DropReason) -> impl Iterator<Item = &DroppedFeature> {
        self.dropped.iter().filter(move |d| d.reason == reason)
    }

    /// Total number of individual objects affected by a flattening, across all classes.
    pub fn total_dropped(&self) -> usize {
        self.dropped.iter().map(|d| d.count).sum()
    }
}

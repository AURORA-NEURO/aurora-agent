//! The latent/observation/decision stack, L0 to L8.
//!
//! Blueprint 24.04 exists because of four sentences: a segmentation mask is not the tumour, a
//! methylation classifier score is not the diagnosis, a differential-expression table is not a
//! mechanism, a paper's conclusion is not the underlying evidence. Each is a level confusion,
//! and each is invisible once the objects are all "results" in one bag.
//!
//! [`StackLevel`] gives every object a level. [`LeveledObject::report_as`] refuses to present an
//! object at a level it does not occupy, which is the anti-collapse invariant of 24.04 in its
//! smallest enforceable form. [`LineageEdge`] restricts the eleven required edge types to the
//! level pairs where they mean anything.
//!
//! **Where the blueprint is silent:** 24.04 lists the eleven edge names but never states a
//! direction convention, and several names read in opposite directions in English
//! (`sampled_from` points backwards, `interpreted_as` forwards). This crate fixes the
//! convention as *subject then object, read as an English sentence* — `X sampled_from Y`,
//! `X interpreted_as Y` — and the legality table below is written against that reading. If a
//! later blueprint revision states a different convention, the table is what changes.
//!
//! Not implemented: the schema and assumption fields of the cross-layer contract. This crate
//! checks that a transition declares an uncertainty transformation and a mapping character,
//! because both are yes/no. Whether the declared assumptions are *correct* is an oracle's job.

use crate::error::StackError;
use serde::{Deserialize, Serialize};

/// The nine levels of blueprint 24.04.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StackLevel {
    /// L0 — latent biological state. Never directly observed; only ever inferred.
    LatentBiology,
    /// L1 — material sampling and specimen state.
    Material,
    /// L2 — assay and acquisition process.
    Assay,
    /// L3 — raw observed signal.
    RawSignal,
    /// L4 — processing and feature representation.
    Representation,
    /// L5 — statistical estimate or model output.
    Estimate,
    /// L6 — biological interpretation or claim.
    Claim,
    /// L7 — decision, experiment, or workflow action.
    Decision,
    /// L8 — downstream outcome and later evidence.
    Outcome,
}

impl StackLevel {
    pub const ALL: [StackLevel; 9] = [
        StackLevel::LatentBiology,
        StackLevel::Material,
        StackLevel::Assay,
        StackLevel::RawSignal,
        StackLevel::Representation,
        StackLevel::Estimate,
        StackLevel::Claim,
        StackLevel::Decision,
        StackLevel::Outcome,
    ];

    /// The `L`-number. Ordering by index is the stack's reading order, not a quality or
    /// confidence order: an L6 claim is not "more" than an L3 signal, it is a different kind
    /// of thing that can be wrong in different ways.
    pub fn index(self) -> u8 {
        match self {
            StackLevel::LatentBiology => 0,
            StackLevel::Material => 1,
            StackLevel::Assay => 2,
            StackLevel::RawSignal => 3,
            StackLevel::Representation => 4,
            StackLevel::Estimate => 5,
            StackLevel::Claim => 6,
            StackLevel::Decision => 7,
            StackLevel::Outcome => 8,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            StackLevel::LatentBiology => "L0 latent biology",
            StackLevel::Material => "L1 material",
            StackLevel::Assay => "L2 assay",
            StackLevel::RawSignal => "L3 raw signal",
            StackLevel::Representation => "L4 representation",
            StackLevel::Estimate => "L5 estimate",
            StackLevel::Claim => "L6 claim",
            StackLevel::Decision => "L7 decision",
            StackLevel::Outcome => "L8 outcome",
        }
    }
}

/// The eleven lineage edge types blueprint 24.04 requires.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineageEdge {
    SampledFrom,
    MeasuredBy,
    ProcessedBy,
    EstimatedBy,
    Supports,
    Contradicts,
    InterpretedAs,
    Motivated,
    ActedUponBy,
    ValidatedBy,
    SupersededBy,
}

impl LineageEdge {
    pub const ALL: [LineageEdge; 11] = [
        LineageEdge::SampledFrom,
        LineageEdge::MeasuredBy,
        LineageEdge::ProcessedBy,
        LineageEdge::EstimatedBy,
        LineageEdge::Supports,
        LineageEdge::Contradicts,
        LineageEdge::InterpretedAs,
        LineageEdge::Motivated,
        LineageEdge::ActedUponBy,
        LineageEdge::ValidatedBy,
        LineageEdge::SupersededBy,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            LineageEdge::SampledFrom => "sampled_from",
            LineageEdge::MeasuredBy => "measured_by",
            LineageEdge::ProcessedBy => "processed_by",
            LineageEdge::EstimatedBy => "estimated_by",
            LineageEdge::Supports => "supports",
            LineageEdge::Contradicts => "contradicts",
            LineageEdge::InterpretedAs => "interpreted_as",
            LineageEdge::Motivated => "motivated",
            LineageEdge::ActedUponBy => "acted_upon_by",
            LineageEdge::ValidatedBy => "validated_by",
            LineageEdge::SupersededBy => "superseded_by",
        }
    }

    /// Levels this edge admits as the sentence's subject.
    pub fn subject_levels(self) -> &'static [StackLevel] {
        match self {
            LineageEdge::SampledFrom => &[StackLevel::Material],
            LineageEdge::MeasuredBy => &[StackLevel::RawSignal],
            LineageEdge::ProcessedBy => &[StackLevel::Representation],
            LineageEdge::EstimatedBy => &[StackLevel::Estimate],
            LineageEdge::Supports | LineageEdge::Contradicts => &[
                StackLevel::RawSignal,
                StackLevel::Representation,
                StackLevel::Estimate,
                StackLevel::Outcome,
            ],
            LineageEdge::InterpretedAs => &[StackLevel::Estimate],
            LineageEdge::Motivated => &[StackLevel::Claim],
            LineageEdge::ActedUponBy => &[StackLevel::Claim, StackLevel::Decision],
            LineageEdge::ValidatedBy => &[StackLevel::Estimate, StackLevel::Claim],
            LineageEdge::SupersededBy => &StackLevel::ALL,
        }
    }

    /// Levels this edge admits as the sentence's object.
    pub fn object_levels(self) -> &'static [StackLevel] {
        match self {
            LineageEdge::SampledFrom => &[StackLevel::LatentBiology, StackLevel::Material],
            LineageEdge::MeasuredBy => &[StackLevel::Assay],
            LineageEdge::ProcessedBy => &[StackLevel::RawSignal, StackLevel::Representation],
            LineageEdge::EstimatedBy => &[StackLevel::Representation, StackLevel::Estimate],
            LineageEdge::Supports | LineageEdge::Contradicts => {
                &[StackLevel::Claim, StackLevel::Decision]
            }
            LineageEdge::InterpretedAs => &[StackLevel::Claim],
            LineageEdge::Motivated => &[StackLevel::Decision],
            LineageEdge::ActedUponBy => &[StackLevel::Decision],
            LineageEdge::ValidatedBy => &[StackLevel::Outcome],
            LineageEdge::SupersededBy => &StackLevel::ALL,
        }
    }

    /// Whether `subject <edge> object` is a sentence the stack admits.
    ///
    /// `superseded_by` is the one edge that additionally requires both ends to sit at the same
    /// level: a claim can be superseded by a better claim, never by a raw file.
    pub fn admits(self, subject: StackLevel, object: StackLevel) -> Result<(), StackError> {
        let same_level_required = self == LineageEdge::SupersededBy;
        let legal = self.subject_levels().contains(&subject)
            && self.object_levels().contains(&object)
            && (!same_level_required || subject == object);
        if legal {
            Ok(())
        } else {
            Err(StackError::IllegalEdge {
                edge: self.as_str(),
                from: subject.as_str(),
                to: object.as_str(),
            })
        }
    }
}

/// Any object in a trace, tagged with the level it occupies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeveledObject {
    pub id: String,
    pub level: StackLevel,
}

impl LeveledObject {
    pub fn new(id: impl Into<String>, level: StackLevel) -> Self {
        LeveledObject {
            id: id.into(),
            level,
        }
    }

    /// The anti-collapse invariant of 24.04: an object may only be presented at its own level.
    ///
    /// A mask is a representation and not the tumour; a classifier score is an estimate and not
    /// a diagnosis. Both of those are this one call returning an error.
    pub fn report_as(&self, claimed: StackLevel) -> Result<(), StackError> {
        if claimed == self.level {
            Ok(())
        } else {
            Err(StackError::LevelCollapse {
                object: self.id.clone(),
                actual: self.level.as_str(),
                claimed: claimed.as_str(),
            })
        }
    }

    /// A simplified rendering that keeps the level attached.
    ///
    /// 24.04 permits summaries; what it forbids is a summary that drops the level. So the
    /// summary type cannot be constructed without one.
    pub fn summarize(&self, text: impl Into<String>) -> LeveledSummary {
        LeveledSummary {
            object: self.id.clone(),
            level: self.level,
            text: text.into(),
        }
    }
}

/// A human-readable summary that still says what kind of thing it summarizes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeveledSummary {
    pub object: String,
    pub level: StackLevel,
    pub text: String,
}

/// How a cross-layer mapping behaves, per the last bullet of 24.04's cross-layer contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MappingCharacter {
    Deterministic,
    Statistical,
    Heuristic,
    ExpertMediated,
}

/// A declared transition between two stack levels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossLayerContract {
    pub from: StackLevel,
    pub to: StackLevel,
    /// How uncertainty at `from` becomes uncertainty at `to`. Free text, because the honest
    /// answer is often "not propagated" and that must be sayable.
    pub uncertainty_transformation: String,
    pub character: Option<MappingCharacter>,
    /// Non-identifiabilities the mapping is known to carry. Empty is legal — many mappings have
    /// none stated — so this field is recorded but not enforced.
    #[serde(default)]
    pub known_non_identifiabilities: Vec<String>,
}

impl CrossLayerContract {
    pub fn check(&self) -> Result<(), StackError> {
        if self.uncertainty_transformation.trim().is_empty() {
            return Err(StackError::UndeclaredUncertainty {
                from: self.from.as_str(),
                to: self.to.as_str(),
            });
        }
        if self.character.is_none() {
            return Err(StackError::UndeclaredMappingCharacter {
                from: self.from.as_str(),
                to: self.to.as_str(),
            });
        }
        Ok(())
    }
}

/// The six failure examples blueprint 24.04 gives, as a closed set.
///
/// The point of the list is that these failures live at different levels and must not all be
/// reported as "wrong answer". [`FailureKind::layer`] is the blueprint's own attribution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureKind {
    WrongRawFilePairing,
    WrongReferenceGenome,
    InvalidStatisticalModel,
    OverclaimedCausalMechanism,
    UninformativeAssayOrdered,
    FailureToReviseAfterLaterEvidence,
}

impl FailureKind {
    pub fn layer(self) -> StackLevel {
        match self {
            FailureKind::WrongRawFilePairing => StackLevel::Material,
            FailureKind::WrongReferenceGenome => StackLevel::Assay,
            FailureKind::InvalidStatisticalModel => StackLevel::Estimate,
            FailureKind::OverclaimedCausalMechanism => StackLevel::Claim,
            FailureKind::UninformativeAssayOrdered => StackLevel::Decision,
            FailureKind::FailureToReviseAfterLaterEvidence => StackLevel::Outcome,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_segmentation_mask_cannot_be_reported_as_the_tumour() {
        let mask = LeveledObject::new("mask:0001", StackLevel::Representation);
        let err = mask.report_as(StackLevel::LatentBiology).unwrap_err();
        assert_eq!(
            err,
            StackError::LevelCollapse {
                object: "mask:0001".to_string(),
                actual: "L4 representation",
                claimed: "L0 latent biology"
            }
        );
    }

    #[test]
    fn a_classifier_score_cannot_be_reported_as_a_diagnosis() {
        let score = LeveledObject::new("mgmt-score", StackLevel::Estimate);
        assert!(score.report_as(StackLevel::Claim).is_err());
        assert!(score.report_as(StackLevel::Estimate).is_ok());
    }

    #[test]
    fn a_summary_cannot_be_produced_without_carrying_the_level_it_summarizes() {
        let table = LeveledObject::new("de-table", StackLevel::Estimate);
        let summary = table.summarize("142 genes differentially expressed");
        assert_eq!(summary.level, StackLevel::Estimate);
        let encoded = serde_json::to_string(&summary).unwrap();
        assert!(encoded.contains("estimate"));
    }

    #[test]
    fn a_specimen_is_sampled_from_biology_but_biology_is_not_sampled_from_a_specimen() {
        assert!(LineageEdge::SampledFrom
            .admits(StackLevel::Material, StackLevel::LatentBiology)
            .is_ok());
        assert!(LineageEdge::SampledFrom
            .admits(StackLevel::LatentBiology, StackLevel::Material)
            .is_err());
    }

    #[test]
    fn an_estimate_is_interpreted_as_a_claim_and_never_the_other_way_round() {
        assert!(LineageEdge::InterpretedAs
            .admits(StackLevel::Estimate, StackLevel::Claim)
            .is_ok());
        let err = LineageEdge::InterpretedAs
            .admits(StackLevel::Claim, StackLevel::Estimate)
            .unwrap_err();
        assert_eq!(
            err,
            StackError::IllegalEdge {
                edge: "interpreted_as",
                from: "L6 claim",
                to: "L5 estimate"
            }
        );
    }

    #[test]
    fn a_claim_can_only_be_superseded_by_another_claim() {
        assert!(LineageEdge::SupersededBy
            .admits(StackLevel::Claim, StackLevel::Claim)
            .is_ok());
        assert!(LineageEdge::SupersededBy
            .admits(StackLevel::Claim, StackLevel::RawSignal)
            .is_err());
    }

    #[test]
    fn a_claim_is_validated_by_later_outcomes_not_by_the_estimate_that_produced_it() {
        assert!(LineageEdge::ValidatedBy
            .admits(StackLevel::Claim, StackLevel::Outcome)
            .is_ok());
        assert!(LineageEdge::ValidatedBy
            .admits(StackLevel::Claim, StackLevel::Estimate)
            .is_err());
    }

    #[test]
    fn every_edge_type_admits_at_least_one_legal_sentence() {
        for edge in LineageEdge::ALL {
            let legal = StackLevel::ALL.iter().any(|subject| {
                StackLevel::ALL
                    .iter()
                    .any(|object| edge.admits(*subject, *object).is_ok())
            });
            assert!(legal, "{} admits nothing", edge.as_str());
        }
    }

    #[test]
    fn a_cross_layer_transition_without_a_declared_uncertainty_transformation_is_refused() {
        let contract = CrossLayerContract {
            from: StackLevel::RawSignal,
            to: StackLevel::Representation,
            uncertainty_transformation: String::new(),
            character: Some(MappingCharacter::Deterministic),
            known_non_identifiabilities: vec![],
        };
        assert!(matches!(
            contract.check().unwrap_err(),
            StackError::UndeclaredUncertainty { .. }
        ));
    }

    #[test]
    fn a_cross_layer_transition_must_say_whether_it_is_deterministic_or_expert_mediated() {
        let contract = CrossLayerContract {
            from: StackLevel::Estimate,
            to: StackLevel::Claim,
            uncertainty_transformation: "not propagated".to_string(),
            character: None,
            known_non_identifiabilities: vec![],
        };
        assert!(matches!(
            contract.check().unwrap_err(),
            StackError::UndeclaredMappingCharacter { .. }
        ));
    }

    #[test]
    fn the_six_blueprint_failures_are_attributed_to_six_different_layers() {
        let kinds = [
            FailureKind::WrongRawFilePairing,
            FailureKind::WrongReferenceGenome,
            FailureKind::InvalidStatisticalModel,
            FailureKind::OverclaimedCausalMechanism,
            FailureKind::UninformativeAssayOrdered,
            FailureKind::FailureToReviseAfterLaterEvidence,
        ];
        let mut layers: Vec<u8> = kinds.iter().map(|k| k.layer().index()).collect();
        layers.sort_unstable();
        layers.dedup();
        assert_eq!(layers.len(), 6);
    }
}

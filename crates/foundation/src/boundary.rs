//! The clinical boundary, carried in the type system.
//!
//! Blueprint 24.15 is the module most likely to be honoured with a paragraph in a README and
//! then quietly ignored by code. A README cannot stop a result bundle from being rendered as a
//! diagnosis. [`ResearchUseOnly`] can: it wraps an artifact so that every downstream crate
//! which wants the value has to name the restriction to get at it, and there is no method on
//! this type that returns the inner value without saying "for research".
//!
//! The second half of 24.15 — "the Weave Kernel may model authority, but it cannot manufacture
//! it" — is encoded as two unrelated types. [`ModelledAuthority`] is what a simulator produces;
//! [`GrantedAuthorization`] is what an institution produces. There is deliberately no `From`
//! impl between them, in either direction. Nothing in this crate can turn a simulation into a
//! permission.
//!
//! Not implemented here: any check that a *use* is actually clinical. That judgement is human
//! and jurisdictional. What is implemented is the refusal to relabel, and the refusal to emit a
//! result whose card omits the gap to clinical validation.

use crate::error::BoundaryError;
use serde::{Deserialize, Serialize};

/// The eight things blueprint 24.15 states BioPRISM does not do.
///
/// Enumerated rather than prose so that a surface which is about to do one of them can name
/// which one, and so that adding a ninth is a visible change to a public type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NonGoal {
    DiagnoseOrRuleOutDisease,
    RecommendIndividualTreatment,
    MakeTriageOrSurgicalDecisions,
    ReplaceExpertReview,
    AutonomouslyOrderAssaysOrConsumeSpecimens,
    ExecuteWetLaboratoryProtocols,
    ClaimMedicalDeviceCertification,
    ConvertBenchmarkScoreIntoClinicalReadiness,
}

impl NonGoal {
    pub const ALL: [NonGoal; 8] = [
        NonGoal::DiagnoseOrRuleOutDisease,
        NonGoal::RecommendIndividualTreatment,
        NonGoal::MakeTriageOrSurgicalDecisions,
        NonGoal::ReplaceExpertReview,
        NonGoal::AutonomouslyOrderAssaysOrConsumeSpecimens,
        NonGoal::ExecuteWetLaboratoryProtocols,
        NonGoal::ClaimMedicalDeviceCertification,
        NonGoal::ConvertBenchmarkScoreIntoClinicalReadiness,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            NonGoal::DiagnoseOrRuleOutDisease => "diagnose or rule out disease",
            NonGoal::RecommendIndividualTreatment => "recommend individual treatment",
            NonGoal::MakeTriageOrSurgicalDecisions => "make triage or surgical decisions",
            NonGoal::ReplaceExpertReview => {
                "replace pathology, radiology, molecular or tumour-board review"
            }
            NonGoal::AutonomouslyOrderAssaysOrConsumeSpecimens => {
                "autonomously order assays or consume specimens"
            }
            NonGoal::ExecuteWetLaboratoryProtocols => "execute wet-laboratory protocols",
            NonGoal::ClaimMedicalDeviceCertification => "claim medical-device certification",
            NonGoal::ConvertBenchmarkScoreIntoClinicalReadiness => {
                "convert a benchmark score into clinical readiness"
            }
        }
    }
}

/// The only use tier BioPRISM output currently carries, plus the tiers it refuses to be
/// relabelled as. Kept as an enum with a single admissible variant so that the day a second
/// admissible tier is contemplated, it is a reviewed change to this file rather than a string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UseTier {
    /// Research and developer infrastructure. The only tier this crate will produce.
    ResearchEvaluationOnly,
    /// Requested by callers, never granted.
    ClinicalDecisionSupport,
    /// Requested by callers, never granted.
    DiagnosticUse,
    /// Requested by callers, never granted.
    TreatmentSelection,
}

impl UseTier {
    pub fn as_str(self) -> &'static str {
        match self {
            UseTier::ResearchEvaluationOnly => "research_evaluation_only",
            UseTier::ClinicalDecisionSupport => "clinical_decision_support",
            UseTier::DiagnosticUse => "diagnostic_use",
            UseTier::TreatmentSelection => "treatment_selection",
        }
    }

    pub fn is_admissible(self) -> bool {
        matches!(self, UseTier::ResearchEvaluationOnly)
    }
}

/// An artifact carrying the research-use restriction of blueprint 24.15.
///
/// Downstream crates that require this wrapper in their signatures get the boundary for free:
/// a value that never passed through [`ResearchUseOnly::seal`] cannot be handed to them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchUseOnly<T> {
    artifact: String,
    value: T,
}

impl<T> ResearchUseOnly<T> {
    pub fn seal(artifact: impl Into<String>, value: T) -> Self {
        ResearchUseOnly {
            artifact: artifact.into(),
            value,
        }
    }

    pub fn artifact(&self) -> &str {
        &self.artifact
    }

    /// Borrow the value. Named for the restriction so that a reader of the call site sees it.
    pub fn for_research(&self) -> &T {
        &self.value
    }

    /// Take the value, keeping the restriction visible at the call site. There is no
    /// `into_inner`: unwrapping should never read as neutral.
    pub fn into_inner_for_research(self) -> T {
        self.value
    }

    pub fn tier(&self) -> UseTier {
        UseTier::ResearchEvaluationOnly
    }

    /// Refuses any relabelling. The only tier a sealed artifact can be is the one it was
    /// sealed with, and requesting another is an error rather than a no-op, because a silent
    /// no-op lets a caller believe it succeeded.
    pub fn relabel_as(&self, requested: UseTier) -> Result<(), BoundaryError> {
        if requested.is_admissible() {
            Ok(())
        } else {
            Err(BoundaryError::UseEscalation {
                artifact: self.artifact.clone(),
                requested: requested.as_str(),
            })
        }
    }

    /// Refuses a stated use case that matches a declared non-goal.
    ///
    /// The match is supplied by the caller, not inferred from the text: this crate does not
    /// pretend to classify intent from a string.
    pub fn refuse_non_goal(&self, use_case: impl Into<String>, non_goal: NonGoal) -> BoundaryError {
        BoundaryError::NonGoal {
            use_case: use_case.into(),
            non_goal: non_goal.as_str(),
        }
    }
}

/// The five things blueprint 24.15 requires a result card to state before a result leaves the
/// system. Absent fields are refused individually so a contributor is told which one is missing.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ResultCard {
    pub artifact: String,
    pub data_provenance: String,
    pub use_case: String,
    pub reference_standard: String,
    pub limitations: String,
    /// The distance between what was demonstrated and prospective or clinical validation.
    /// Its absence is the specific failure 24.15 exists to prevent, so it is required even when
    /// the honest answer is "not assessed".
    pub gap_to_clinical_validation: String,
}

impl ResultCard {
    pub fn check(&self) -> Result<(), BoundaryError> {
        let required: [(&'static str, &String); 5] = [
            ("data provenance", &self.data_provenance),
            ("use case", &self.use_case),
            ("reference standard", &self.reference_standard),
            ("limitations", &self.limitations),
            (
                "the gap to prospective or clinical validation",
                &self.gap_to_clinical_validation,
            ),
        ];
        for (field, value) in required {
            if value.trim().is_empty() {
                return Err(BoundaryError::IncompleteResultCard {
                    artifact: self.artifact.clone(),
                    field,
                });
            }
        }
        Ok(())
    }
}

/// Authority as a simulator represents it. Carries no permission whatsoever.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelledAuthority {
    pub role: String,
    pub world: String,
}

/// Authority as an institution grants it.
///
/// There is no conversion from [`ModelledAuthority`], by design. A simulated tumour board
/// approving a simulated action is a fact about the simulation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrantedAuthorization {
    pub institution: String,
    pub approver: String,
    /// External reference (protocol number, ticket, signed record) that a human can check.
    pub reference: String,
}

impl GrantedAuthorization {
    /// Refuses a high-impact action when no authorization is present.
    ///
    /// Callers pass `None` when they have only a [`ModelledAuthority`], which is the whole
    /// point: the type system will not let a modelled authority be passed here at all.
    pub fn authorize(
        grant: Option<&GrantedAuthorization>,
        action: impl Into<String>,
        world: &'static str,
    ) -> Result<(), BoundaryError> {
        match grant {
            Some(grant)
                if !grant.institution.trim().is_empty()
                    && !grant.approver.trim().is_empty()
                    && !grant.reference.trim().is_empty() =>
            {
                Ok(())
            }
            _ => Err(BoundaryError::UnauthorizedHighImpactAction {
                action: action.into(),
                world,
            }),
        }
    }
}

/// A modelled authority cannot be used where an institutional grant is required.
///
/// ```compile_fail
/// use bioprism_foundation::boundary::{GrantedAuthorization, ModelledAuthority};
/// let modelled = ModelledAuthority { role: "tumour-board".into(), world: "sim".into() };
/// let _ = GrantedAuthorization::authorize(Some(&modelled), "consume-aliquot", "federated");
/// ```
#[cfg(doctest)]
pub struct AuthorityCannotBeManufactured;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sealed_artifact_cannot_be_relabelled_for_clinical_decision_support() {
        let sealed = ResearchUseOnly::seal("bundle:0001", 42u32);
        let err = sealed
            .relabel_as(UseTier::ClinicalDecisionSupport)
            .unwrap_err();
        assert_eq!(
            err,
            BoundaryError::UseEscalation {
                artifact: "bundle:0001".to_string(),
                requested: "clinical_decision_support"
            }
        );
    }

    #[test]
    fn no_use_tier_other_than_research_evaluation_is_admissible() {
        let admissible: Vec<UseTier> = [
            UseTier::ResearchEvaluationOnly,
            UseTier::ClinicalDecisionSupport,
            UseTier::DiagnosticUse,
            UseTier::TreatmentSelection,
        ]
        .into_iter()
        .filter(|tier| tier.is_admissible())
        .collect();
        assert_eq!(admissible, vec![UseTier::ResearchEvaluationOnly]);
    }

    #[test]
    fn a_result_card_omitting_the_gap_to_clinical_validation_is_refused() {
        let card = ResultCard {
            artifact: "bundle:0001".to_string(),
            data_provenance: "TCGA-GBM retrospective".to_string(),
            use_case: "segmentation benchmark".to_string(),
            reference_standard: "two-reader consensus".to_string(),
            limitations: "single site".to_string(),
            gap_to_clinical_validation: String::new(),
        };
        let err = card.check().unwrap_err();
        assert_eq!(
            err,
            BoundaryError::IncompleteResultCard {
                artifact: "bundle:0001".to_string(),
                field: "the gap to prospective or clinical validation"
            }
        );
    }

    #[test]
    fn a_complete_result_card_passes_and_names_all_five_required_statements() {
        let card = ResultCard {
            artifact: "bundle:0001".to_string(),
            data_provenance: "TCGA-GBM retrospective".to_string(),
            use_case: "segmentation benchmark".to_string(),
            reference_standard: "two-reader consensus".to_string(),
            limitations: "single site".to_string(),
            gap_to_clinical_validation: "not assessed prospectively".to_string(),
        };
        assert!(card.check().is_ok());
    }

    #[test]
    fn a_high_impact_action_without_an_institutional_grant_is_refused() {
        let err = GrantedAuthorization::authorize(None, "consume-last-aliquot", "federated")
            .unwrap_err();
        assert!(matches!(
            err,
            BoundaryError::UnauthorizedHighImpactAction { .. }
        ));
    }

    #[test]
    fn a_grant_missing_its_external_reference_is_not_an_authorization() {
        let grant = GrantedAuthorization {
            institution: "Example Hospital".to_string(),
            approver: "IRB".to_string(),
            reference: "  ".to_string(),
        };
        assert!(GrantedAuthorization::authorize(Some(&grant), "reveal-outcome", "prospective")
            .is_err());
    }

    #[test]
    fn every_non_goal_renders_as_a_sentence_a_reviewer_can_read() {
        for non_goal in NonGoal::ALL {
            assert!(!non_goal.as_str().is_empty());
            assert!(!non_goal.as_str().contains('_'));
        }
    }
}

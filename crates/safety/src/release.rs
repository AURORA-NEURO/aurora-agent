//! The medical research boundary and the dual-use release gate.
//!
//! Implements blueprint 13.25 (medical research boundary and clinical safety) and 13.26 (dual-use,
//! abuse and safety release gates).
//!
//! # Two boundaries with the same shape
//!
//! Both modules answer "may this go out", and both fail the same way: a reviewer with a deadline
//! ticks the boxes that were rated and leaves the rest blank, and the blanks read as low risk.
//! [`RiskAssessment`] therefore stores `Option<Rating>` per dimension and
//! [`ReleaseGate::decide`] returns [`SafetyError::UnratedDimension`] before it looks at any of the
//! ratings it does have. Unrated is not low. This is the same rule
//! `bioprism_section::omission::InfluenceClass` applies to evidence and
//! [`crate::threat::Mitigation`] applies to controls.
//!
//! # The boundary that is not a gate
//!
//! 13.25 is different from everything else in this crate: it is a statement about what the platform
//! *is*, not a control against an adversary. [`ProhibitedOutput`] is refused unconditionally.
//! There is no override parameter, no `force` flag, no reviewer who can approve one, and no
//! deployment configuration that admits them, because the boundary is not a risk trade-off. This
//! mirrors `bioprism-onco`'s typed research boundary; AGENTS.md's closing paragraph is the same
//! rule in prose.
//!
//! # Dual-use control is not result suppression
//!
//! 13.26 says it outright: "Dual-use review is separate from unfavorable result suppression.
//! Security weaknesses can be disclosed responsibly without hiding their existence." So
//! [`WithholdScope`] has two variants and [`withhold`] accepts exactly one of them.
//! [`WithholdScope::ExploitDetail`] is a legitimate dual-use control;
//! [`WithholdScope::Existence`] returns [`SafetyError::SuppressionDisguisedAsSafety`]. A safety
//! process that can delete the fact that a weakness exists is a reputation process wearing a safety
//! process's badge.
//!
//! # What is deliberately not implemented
//!
//! * **No content classification.** Nothing here reads a pack, a prompt or a model output and
//!   decides what it is about. [`SensitiveCategory`] is a label a reviewer applies.
//! * **No thresholds for the architecture gate beyond "not worse".**
//!   [`PromotionCandidate::promote`] refuses any candidate whose safety deltas are unmeasured or
//!   positive. 13.26 says "beyond thresholds" and never states one; inventing a number and
//!   presenting it as spec would be worse than refusing to.
//! * **No reviewer identity, no conflict-of-interest register, no appeal process.**
//!   [`GovernanceRecord::missing_fields`] reports which of 13.26's required governance fields are
//!   empty; filling them is human work.
//! * **No monitoring and no reevaluation trigger.** The reevaluation epoch is a number in a record;
//!   nothing watches for it, because there is no clock.

use crate::error::SafetyError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// What 13.25's "Allowed scope" permits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResearchUse {
    WorkflowReproducibility,
    DataQualityChecks,
    PaperDataCodeLinkage,
    ImagingAndOmicsMetadataReasoning,
    ToolUse,
    Provenance,
    EvidenceSynthesis,
    UncertaintyReporting,
    BenchmarkMethodology,
}

impl ResearchUse {
    pub fn as_str(self) -> &'static str {
        match self {
            ResearchUse::WorkflowReproducibility => "workflow_reproducibility",
            ResearchUse::DataQualityChecks => "data_quality_checks",
            ResearchUse::PaperDataCodeLinkage => "paper_data_code_linkage",
            ResearchUse::ImagingAndOmicsMetadataReasoning => {
                "imaging_and_omics_metadata_reasoning"
            }
            ResearchUse::ToolUse => "tool_use",
            ResearchUse::Provenance => "provenance",
            ResearchUse::EvidenceSynthesis => "evidence_synthesis",
            ResearchUse::UncertaintyReporting => "uncertainty_reporting",
            ResearchUse::BenchmarkMethodology => "benchmark_methodology",
        }
    }
}

impl fmt::Display for ResearchUse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What 13.25's "Prohibited output" forbids. No override exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProhibitedOutput {
    PersonalisedClinicalRecommendation,
    UrgencyClassification,
    TreatmentSelection,
    PrognosisAsPatientAdvice,
    ClinicianReviewBypass,
}

impl ProhibitedOutput {
    pub fn as_str(self) -> &'static str {
        match self {
            ProhibitedOutput::PersonalisedClinicalRecommendation => {
                "personalised_clinical_recommendation"
            }
            ProhibitedOutput::UrgencyClassification => "urgency_classification",
            ProhibitedOutput::TreatmentSelection => "treatment_selection",
            ProhibitedOutput::PrognosisAsPatientAdvice => "prognosis_as_patient_advice",
            ProhibitedOutput::ClinicianReviewBypass => "clinician_review_bypass",
        }
    }
}

impl fmt::Display for ProhibitedOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A requested output, on one side of the boundary or the other.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "side", rename_all = "snake_case")]
pub enum RequestedOutput {
    Research { use_case: ResearchUse, label: String },
    Clinical {
        category: ProhibitedOutput,
        label: String,
    },
}

/// The label 13.25 requires on every surface that shows a result.
pub const RESEARCH_ONLY_LABEL: &str =
    "research use only; not evaluated for clinical use, and not a medical device";

/// The 13.25 boundary.
///
/// A free function would do; it is a unit struct so that the boundary has a name a caller can cite
/// and a doc page a reviewer can read.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MedicalBoundary;

impl MedicalBoundary {
    /// Admits a research use, refuses a clinical one. Unconditionally.
    pub fn admit(&self, output: &RequestedOutput) -> Result<ResearchUse, SafetyError> {
        match output {
            RequestedOutput::Research { use_case, .. } => Ok(*use_case),
            RequestedOutput::Clinical { category, label } => Err(SafetyError::ClinicalBoundary {
                output: label.clone(),
                category: category.to_string(),
            }),
        }
    }

    pub fn label(&self) -> &'static str {
        RESEARCH_ONLY_LABEL
    }
}

/// 13.25's biomedical pack card.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackCard {
    pub intended_research_use: String,
    pub population: String,
    pub data_modality: String,
    pub known_biases: String,
    pub oracle_source: String,
    pub expert_reviewers: String,
    /// The extrapolations a reader might make and the card explicitly does not support. 13.25 asks
    /// for this by name, and it is the field most likely to be left blank.
    pub unsupported_clinical_extrapolations: String,
}

impl PackCard {
    pub fn missing_fields(&self) -> Vec<&'static str> {
        let fields: [(&'static str, &str); 7] = [
            ("intended_research_use", &self.intended_research_use),
            ("population", &self.population),
            ("data_modality", &self.data_modality),
            ("known_biases", &self.known_biases),
            ("oracle_source", &self.oracle_source),
            ("expert_reviewers", &self.expert_reviewers),
            (
                "unsupported_clinical_extrapolations",
                &self.unsupported_clinical_extrapolations,
            ),
        ];
        fields
            .into_iter()
            .filter(|(_, value)| value.trim().is_empty())
            .map(|(name, _)| name)
            .collect()
    }

    pub fn is_complete(&self) -> bool {
        self.missing_fields().is_empty()
    }
}

/// 13.26's risk-assessment dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskDimension {
    CapabilityUplift,
    Actionability,
    Scale,
    ExpertiseReduction,
    TargetSpecificity,
    Reversibility,
    Detectability,
    AvailableSafeguards,
    LegitimateScientificValue,
}

impl RiskDimension {
    pub const ALL: [RiskDimension; 9] = [
        RiskDimension::CapabilityUplift,
        RiskDimension::Actionability,
        RiskDimension::Scale,
        RiskDimension::ExpertiseReduction,
        RiskDimension::TargetSpecificity,
        RiskDimension::Reversibility,
        RiskDimension::Detectability,
        RiskDimension::AvailableSafeguards,
        RiskDimension::LegitimateScientificValue,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            RiskDimension::CapabilityUplift => "capability_uplift",
            RiskDimension::Actionability => "actionability",
            RiskDimension::Scale => "scale",
            RiskDimension::ExpertiseReduction => "expertise_reduction",
            RiskDimension::TargetSpecificity => "target_specificity",
            RiskDimension::Reversibility => "reversibility",
            RiskDimension::Detectability => "detectability",
            RiskDimension::AvailableSafeguards => "available_safeguards",
            RiskDimension::LegitimateScientificValue => "legitimate_scientific_value",
        }
    }

    /// Whether a high rating on this dimension argues *for* release rather than against it.
    ///
    /// Safeguards, detectability and scientific value run the other way, and a gate that summed all
    /// nine would cancel them against the risks and produce a number meaning nothing.
    pub fn is_mitigating(self) -> bool {
        matches!(
            self,
            RiskDimension::Detectability
                | RiskDimension::AvailableSafeguards
                | RiskDimension::LegitimateScientificValue
        )
    }
}

impl fmt::Display for RiskDimension {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A reviewer's rating on one dimension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rating {
    Low,
    Moderate,
    High,
}

impl Rating {
    pub fn as_str(self) -> &'static str {
        match self {
            Rating::Low => "low",
            Rating::Moderate => "moderate",
            Rating::High => "high",
        }
    }
}

impl fmt::Display for Rating {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 13.26's sensitive categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SensitiveCategory {
    CyberExploitation,
    BiologicalDesign,
    SurveillanceAndPrivacyInvasion,
    Fraud,
    HarmfulPhysicalAutomation,
    ClinicalMisuse,
}

impl SensitiveCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            SensitiveCategory::CyberExploitation => "cyber_exploitation",
            SensitiveCategory::BiologicalDesign => "biological_design",
            SensitiveCategory::SurveillanceAndPrivacyInvasion => {
                "surveillance_and_privacy_invasion"
            }
            SensitiveCategory::Fraud => "fraud",
            SensitiveCategory::HarmfulPhysicalAutomation => "harmful_physical_automation",
            SensitiveCategory::ClinicalMisuse => "clinical_misuse",
        }
    }
}

impl fmt::Display for SensitiveCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Ratings across the nine dimensions. Absent means unrated, which is not low.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RiskAssessment {
    pub subject: String,
    pub category: Option<SensitiveCategory>,
    ratings: BTreeMap<RiskDimension, Rating>,
}

impl RiskAssessment {
    pub fn for_subject(subject: impl Into<String>) -> Self {
        RiskAssessment {
            subject: subject.into(),
            category: None,
            ratings: BTreeMap::new(),
        }
    }

    pub fn in_category(mut self, category: SensitiveCategory) -> Self {
        self.category = Some(category);
        self
    }

    pub fn rating(mut self, dimension: RiskDimension, rating: Rating) -> Self {
        self.ratings.insert(dimension, rating);
        self
    }

    pub fn get(&self, dimension: RiskDimension) -> Option<Rating> {
        self.ratings.get(&dimension).copied()
    }

    /// Dimensions nobody rated.
    pub fn unrated(&self) -> Vec<RiskDimension> {
        RiskDimension::ALL
            .into_iter()
            .filter(|dimension| !self.ratings.contains_key(dimension))
            .collect()
    }

    /// Non-mitigating dimensions rated high.
    pub fn high_risk_dimensions(&self) -> Vec<RiskDimension> {
        self.ratings
            .iter()
            .filter(|(dimension, rating)| **rating == Rating::High && !dimension.is_mitigating())
            .map(|(dimension, _)| *dimension)
            .collect()
    }
}

/// What the gate decided.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum GateDecision {
    /// Release with no additional conditions.
    Cleared { subject: String },
    /// Release with 13.26's proportionate controls attached.
    Conditioned {
        subject: String,
        conditions: Vec<String>,
        driven_by: Vec<RiskDimension>,
    },
    /// Not released in this form.
    Blocked {
        subject: String,
        driven_by: Vec<RiskDimension>,
    },
}

impl GateDecision {
    pub fn subject(&self) -> &str {
        match self {
            GateDecision::Cleared { subject }
            | GateDecision::Conditioned { subject, .. }
            | GateDecision::Blocked { subject, .. } => subject,
        }
    }

    pub fn is_cleared(&self) -> bool {
        matches!(self, GateDecision::Cleared { .. })
    }
}

/// The 13.26 gate.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseGate;

impl ReleaseGate {
    /// Decides, after refusing any assessment with a blank.
    ///
    /// Two or more high non-mitigating dimensions block; one conditions; none clears. That rule is
    /// this crate's, not the blueprint's — 13.26 never states a threshold — and it is written here
    /// rather than hidden in a config so that a reviewer disagreeing with it can see what they are
    /// disagreeing with.
    pub fn decide(&self, assessment: &RiskAssessment) -> Result<GateDecision, SafetyError> {
        if let Some(dimension) = assessment.unrated().first() {
            return Err(SafetyError::UnratedDimension {
                subject: assessment.subject.clone(),
                dimension: dimension.to_string(),
            });
        }
        let driven_by = assessment.high_risk_dimensions();
        Ok(match driven_by.len() {
            0 => GateDecision::Cleared {
                subject: assessment.subject.clone(),
            },
            1 => GateDecision::Conditioned {
                subject: assessment.subject.clone(),
                conditions: vec![
                    "gated reviewer access".into(),
                    "non-executable release form".into(),
                ],
                driven_by,
            },
            _ => GateDecision::Blocked {
                subject: assessment.subject.clone(),
                driven_by,
            },
        })
    }
}

/// What a dual-use control is being applied to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WithholdScope {
    /// How to do it. Legitimately withheld.
    ExploitDetail,
    /// That it is possible at all. Never withheld under a safety justification.
    Existence,
}

/// Applies a withholding decision, refusing the one 13.26 forbids.
pub fn withhold(finding: &str, scope: WithholdScope) -> Result<WithholdScope, SafetyError> {
    match scope {
        WithholdScope::ExploitDetail => Ok(scope),
        WithholdScope::Existence => Err(SafetyError::SuppressionDisguisedAsSafety {
            finding: finding.to_string(),
        }),
    }
}

/// 13.26's architecture gate: a candidate that got better at the task and worse at safety.
///
/// Every delta is `Option<f64>` and `None` blocks promotion. A safety metric nobody measured is the
/// exact case the gate exists to catch, and treating it as zero would let an unmeasured candidate
/// promote on task utility alone.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct PromotionCandidate {
    pub task_utility_delta: Option<f64>,
    pub unsafe_action_rate_delta: Option<f64>,
    pub permission_bypass_rate_delta: Option<f64>,
    pub secret_leak_rate_delta: Option<f64>,
}

impl PromotionCandidate {
    /// The safety deltas, paired with the names the error should use.
    fn safety_deltas(&self) -> [(&'static str, Option<f64>); 3] {
        [
            ("unsafe_action_rate", self.unsafe_action_rate_delta),
            ("permission_bypass_rate", self.permission_bypass_rate_delta),
            ("secret_leak_rate", self.secret_leak_rate_delta),
        ]
    }

    /// Refuses an unmeasured or worsened safety metric, whatever the utility gain.
    pub fn promote(&self, subject: &str) -> Result<(), SafetyError> {
        for (name, delta) in self.safety_deltas() {
            match delta {
                None => {
                    return Err(SafetyError::Underdetermined {
                        subject: subject.to_string(),
                        reason: format!("{name} was not measured for this candidate"),
                    })
                }
                Some(value) if value > 0.0 => {
                    return Err(SafetyError::UnratedDimension {
                        subject: subject.to_string(),
                        dimension: format!("{name} increased by {value}"),
                    })
                }
                Some(_) => {}
            }
        }
        Ok(())
    }
}

/// 13.26's governance record.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceRecord {
    pub reviewers: String,
    pub conflicts: String,
    pub rationale: String,
    pub conditions: String,
    pub monitoring: String,
    /// The epoch at which the decision is revisited. Nothing watches for it; there is no clock.
    pub reevaluation_epoch: Option<u64>,
    pub appeal_route: String,
}

impl GovernanceRecord {
    pub fn missing_fields(&self) -> Vec<&'static str> {
        let mut missing: Vec<&'static str> = [
            ("reviewers", &self.reviewers),
            ("conflicts", &self.conflicts),
            ("rationale", &self.rationale),
            ("conditions", &self.conditions),
            ("monitoring", &self.monitoring),
            ("appeal_route", &self.appeal_route),
        ]
        .into_iter()
        .filter(|(_, value)| value.trim().is_empty())
        .map(|(name, _)| name)
        .collect();
        if self.reevaluation_epoch.is_none() {
            missing.push("reevaluation_epoch");
        }
        missing
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fully_rated(subject: &str) -> RiskAssessment {
        let mut assessment = RiskAssessment::for_subject(subject);
        for dimension in RiskDimension::ALL {
            assessment = assessment.rating(dimension, Rating::Low);
        }
        assessment
    }

    #[test]
    fn a_clinical_output_is_refused_and_there_is_no_parameter_that_would_admit_it() {
        let error = MedicalBoundary
            .admit(&RequestedOutput::Clinical {
                category: ProhibitedOutput::TreatmentSelection,
                label: "suggest second-line therapy".into(),
            })
            .expect_err("this platform does not produce clinical outputs");
        assert!(matches!(error, SafetyError::ClinicalBoundary { .. }));
        assert!(error.to_string().contains("research-only"), "{error}");
    }

    #[test]
    fn a_research_use_is_admitted_and_returns_the_use_case_it_was_admitted_as() {
        let admitted = MedicalBoundary
            .admit(&RequestedOutput::Research {
                use_case: ResearchUse::EvidenceSynthesis,
                label: "synthesise the cited evidence".into(),
            })
            .expect("evidence synthesis is in scope");
        assert_eq!(admitted, ResearchUse::EvidenceSynthesis);
        assert!(MedicalBoundary.label().contains("not a medical device"));
    }

    #[test]
    fn a_pack_card_missing_its_unsupported_extrapolations_is_incomplete() {
        let card = PackCard {
            intended_research_use: "benchmark methodology".into(),
            population: "TCGA".into(),
            data_modality: "expression".into(),
            known_biases: "ancestry skew".into(),
            oracle_source: "curated".into(),
            expert_reviewers: "two".into(),
            unsupported_clinical_extrapolations: String::new(),
        };
        assert!(!card.is_complete());
        assert_eq!(
            card.missing_fields(),
            vec!["unsupported_clinical_extrapolations"]
        );
    }

    #[test]
    fn a_release_cannot_clear_while_any_risk_dimension_is_unrated() {
        let assessment = RiskAssessment::for_subject("exploit-pack")
            .in_category(SensitiveCategory::CyberExploitation)
            .rating(RiskDimension::CapabilityUplift, Rating::Low);
        let error = ReleaseGate
            .decide(&assessment)
            .expect_err("unrated is not low");
        assert!(matches!(error, SafetyError::UnratedDimension { .. }));
        assert_eq!(assessment.unrated().len(), RiskDimension::ALL.len() - 1);
    }

    #[test]
    fn a_fully_rated_low_assessment_clears() {
        let decision = ReleaseGate
            .decide(&fully_rated("methodology-pack"))
            .expect("every dimension is rated");
        assert!(decision.is_cleared());
        assert_eq!(decision.subject(), "methodology-pack");
    }

    #[test]
    fn one_high_risk_dimension_conditions_the_release_and_two_block_it() {
        let one_high =
            fully_rated("pack-a").rating(RiskDimension::Actionability, Rating::High);
        assert!(matches!(
            ReleaseGate.decide(&one_high).expect("rated"),
            GateDecision::Conditioned { .. }
        ));
        let two_high = one_high.rating(RiskDimension::CapabilityUplift, Rating::High);
        assert!(matches!(
            ReleaseGate.decide(&two_high).expect("rated"),
            GateDecision::Blocked { .. }
        ));
    }

    #[test]
    fn high_safeguards_and_high_scientific_value_do_not_count_against_release() {
        let assessment = fully_rated("pack-b")
            .rating(RiskDimension::AvailableSafeguards, Rating::High)
            .rating(RiskDimension::Detectability, Rating::High)
            .rating(RiskDimension::LegitimateScientificValue, Rating::High);
        assert!(assessment.high_risk_dimensions().is_empty());
        assert!(ReleaseGate.decide(&assessment).expect("rated").is_cleared());
    }

    #[test]
    fn exploit_detail_may_be_withheld_but_the_existence_of_a_weakness_may_not() {
        assert_eq!(
            withhold("F-1", WithholdScope::ExploitDetail).expect("legitimate"),
            WithholdScope::ExploitDetail
        );
        let error = withhold("F-1", WithholdScope::Existence)
            .expect_err("that is suppression, not dual-use control");
        assert!(matches!(
            error,
            SafetyError::SuppressionDisguisedAsSafety { .. }
        ));
    }

    #[test]
    fn a_candidate_with_an_unmeasured_safety_metric_cannot_be_promoted_on_utility() {
        let candidate = PromotionCandidate {
            task_utility_delta: Some(0.12),
            unsafe_action_rate_delta: Some(-0.01),
            permission_bypass_rate_delta: None,
            secret_leak_rate_delta: Some(0.0),
        };
        let error = candidate
            .promote("arch-v3")
            .expect_err("an unmeasured safety metric is not a zero one");
        assert!(matches!(error, SafetyError::Underdetermined { .. }));
        assert!(error.to_string().contains("permission_bypass_rate"), "{error}");
    }

    #[test]
    fn a_candidate_that_got_better_at_the_task_and_worse_at_safety_is_refused() {
        let candidate = PromotionCandidate {
            task_utility_delta: Some(0.30),
            unsafe_action_rate_delta: Some(0.02),
            permission_bypass_rate_delta: Some(0.0),
            secret_leak_rate_delta: Some(0.0),
        };
        assert!(candidate.promote("arch-v4").is_err());
    }

    #[test]
    fn a_candidate_that_holds_every_safety_metric_flat_or_better_promotes() {
        let candidate = PromotionCandidate {
            task_utility_delta: Some(0.05),
            unsafe_action_rate_delta: Some(-0.03),
            permission_bypass_rate_delta: Some(0.0),
            secret_leak_rate_delta: Some(-0.01),
        };
        candidate.promote("arch-v5").expect("nothing got worse");
    }

    #[test]
    fn a_governance_record_with_no_reevaluation_epoch_is_incomplete() {
        let record = GovernanceRecord {
            reviewers: "a, b".into(),
            conflicts: "none declared".into(),
            rationale: "capability uplift is low".into(),
            conditions: "gated access".into(),
            monitoring: "quarterly".into(),
            reevaluation_epoch: None,
            appeal_route: "security council".into(),
        };
        assert_eq!(record.missing_fields(), vec!["reevaluation_epoch"]);
    }

    #[test]
    fn every_risk_dimension_is_either_aggravating_or_mitigating_and_the_split_is_three_six() {
        let mitigating = RiskDimension::ALL
            .into_iter()
            .filter(|d| d.is_mitigating())
            .count();
        assert_eq!(mitigating, 3);
        assert_eq!(RiskDimension::ALL.len() - mitigating, 6);
    }
}

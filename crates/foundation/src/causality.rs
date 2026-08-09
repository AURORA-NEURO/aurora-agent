//! Causal contracts and the prohibited shortcut.
//!
//! Blueprint 24.13 ends with three sentences that are the entire enforceable content of the
//! module: "A high predictive score does not establish a mechanism. A mechanistic simulator does
//! not establish real-world effect. A retrospective exposure association does not establish
//! treatment benefit."
//!
//! Each is a statement about which intervention class licenses which claim, and
//! [`InterventionClass::licenses_real_world_effect`] is that statement as a table. Two of the
//! seven classes pass, and they pass only for the outcome they actually intervened on: a
//! workflow intervention licenses a claim about the workflow, not about the biology.
//!
//! [`CausalContract::check`] enforces the eleven declarations 24.13 requires of a causal task.
//! Most are presence checks, which is the right strength: whether a stated identification
//! condition *holds* is an empirical question no type can answer, but whether it was stated at
//! all is exactly the thing that goes missing.
//!
//! Not implemented: identification itself. Deciding whether an estimand is identified from a
//! declared graph is a real algorithm and it belongs to a causal-inference crate, not to a
//! section-24 foundation.

use crate::error::CausalError;
use crate::maturity::EvidenceMaturity;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// The seven intervention classes of blueprint 24.13.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterventionClass {
    ComputationalTransformation,
    EvidenceAcquisition,
    InSilicoPerturbation,
    ExVivoOrInVitroPerturbation,
    HistoricalTreatmentExposure,
    RandomizedOrQuasiExperimental,
    WorkflowOrImplementation,
}

impl InterventionClass {
    pub const ALL: [InterventionClass; 7] = [
        InterventionClass::ComputationalTransformation,
        InterventionClass::EvidenceAcquisition,
        InterventionClass::InSilicoPerturbation,
        InterventionClass::ExVivoOrInVitroPerturbation,
        InterventionClass::HistoricalTreatmentExposure,
        InterventionClass::RandomizedOrQuasiExperimental,
        InterventionClass::WorkflowOrImplementation,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            InterventionClass::ComputationalTransformation => {
                "computational transformation or analysis choice"
            }
            InterventionClass::EvidenceAcquisition => "evidence acquisition or assay selection",
            InterventionClass::InSilicoPerturbation => "in-silico perturbation in a declared model",
            InterventionClass::ExVivoOrInVitroPerturbation => "ex-vivo or in-vitro perturbation",
            InterventionClass::HistoricalTreatmentExposure => {
                "historical treatment exposure in observational data"
            }
            InterventionClass::RandomizedOrQuasiExperimental => {
                "randomized or quasi-experimental intervention evidence"
            }
            InterventionClass::WorkflowOrImplementation => "workflow or implementation intervention",
        }
    }

    /// Whether this class licenses a claim about a real-world effect.
    ///
    /// `ExVivoOrInVitroPerturbation` is false on purpose. It is genuine interventional evidence
    /// — about the model system. Carrying it to a patient-level effect is a translation, and
    /// translations go through the spine with a certificate, not through this predicate.
    pub fn licenses_real_world_effect(self) -> bool {
        matches!(
            self,
            InterventionClass::RandomizedOrQuasiExperimental
                | InterventionClass::WorkflowOrImplementation
        )
    }
}

/// What kind of causal work the task is. 24.13 requires the contract to say which, because
/// these four fail in different ways and reward different systems.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CausalTaskKind {
    Identification,
    Estimation,
    Design,
    Interpretation,
}

/// What is being claimed, which determines what evidence is needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CausalClaimStrength {
    /// "These co-vary in this population." Needs no intervention.
    Associational,
    /// "Under the declared model, this follows." Needs a declared model, nothing more.
    ModelInternal,
    /// "Intervening changes the outcome, in the world." Needs intervention evidence.
    RealWorldEffect,
}

/// A causal task's declarations (24.13).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CausalContract {
    pub id: String,
    pub kind: Option<CausalTaskKind>,
    pub intervention_class: Option<InterventionClass>,
    pub target_estimand: String,
    pub intervention: String,
    pub comparator: String,
    pub unit_and_population: String,
    pub outcome_and_horizon: String,
    /// Assumed causal graph or identification conditions.
    pub identification_conditions: String,
    #[serde(default)]
    pub available_covariates: BTreeSet<String>,
    pub interference_and_selection_assumptions: String,
    pub missingness_model: String,
    /// Estimators the contract will accept. Empty means any estimator passes, which is not a
    /// permissive contract but an unscoreable one.
    #[serde(default)]
    pub estimator_acceptance_set: BTreeSet<String>,
    #[serde(default)]
    pub sensitivity_checks: BTreeSet<String>,
    pub claim_strength: Option<CausalClaimStrength>,
    /// Maturity of the evidence actually standing behind the claim.
    pub evidence_maturity: Option<EvidenceMaturity>,
}

impl CausalContract {
    /// Presence checks for the eleven declarations, then the prohibited shortcut.
    pub fn check(&self) -> Result<(), CausalError> {
        let required: [(&'static str, &String); 7] = [
            ("target estimand", &self.target_estimand),
            ("intervention", &self.intervention),
            ("comparator", &self.comparator),
            ("unit and population", &self.unit_and_population),
            ("outcome and horizon", &self.outcome_and_horizon),
            (
                "assumed causal graph or identification conditions",
                &self.identification_conditions,
            ),
            (
                "interference and selection assumptions",
                &self.interference_and_selection_assumptions,
            ),
        ];
        for (field, value) in required {
            if value.trim().is_empty() {
                return Err(CausalError::IncompleteContract {
                    contract: self.id.clone(),
                    field,
                });
            }
        }
        if self.missingness_model.trim().is_empty() {
            return Err(CausalError::IncompleteContract {
                contract: self.id.clone(),
                field: "missingness model",
            });
        }
        if self.kind.is_none() {
            return Err(CausalError::IncompleteContract {
                contract: self.id.clone(),
                field: "whether the task is identification, estimation, design or interpretation",
            });
        }
        if self.estimator_acceptance_set.is_empty() {
            return Err(CausalError::NoEstimatorAcceptanceSet {
                contract: self.id.clone(),
            });
        }
        if self.sensitivity_checks.is_empty() {
            return Err(CausalError::NoSensitivityCheck {
                contract: self.id.clone(),
            });
        }
        self.check_claim_strength()
    }

    /// The prohibited shortcut of 24.13.
    ///
    /// A real-world effect claim needs both an intervention class that licenses it and evidence
    /// at a rung that carries intervention evidence. Requiring both is not redundancy: the
    /// class says what kind of study it was, the maturity says whether that study has actually
    /// been done or is merely intended.
    pub fn check_claim_strength(&self) -> Result<(), CausalError> {
        if self.claim_strength != Some(CausalClaimStrength::RealWorldEffect) {
            return Ok(());
        }
        let class = self.intervention_class.ok_or(CausalError::IncompleteContract {
            contract: self.id.clone(),
            field: "intervention class",
        })?;
        if !class.licenses_real_world_effect() {
            return Err(CausalError::ProhibitedShortcut {
                contract: self.id.clone(),
                basis: class.as_str(),
            });
        }
        let maturity = self.evidence_maturity.ok_or(CausalError::IncompleteContract {
            contract: self.id.clone(),
            field: "evidence maturity",
        })?;
        if !maturity.carries_intervention_evidence() {
            return Err(CausalError::ProhibitedShortcut {
                contract: self.id.clone(),
                basis: maturity.as_str(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contract() -> CausalContract {
        CausalContract {
            id: "causal:tmz:0001".to_string(),
            kind: Some(CausalTaskKind::Estimation),
            intervention_class: Some(InterventionClass::RandomizedOrQuasiExperimental),
            target_estimand: "ATE of temozolomide on 12-month survival".to_string(),
            intervention: "temozolomide plus radiotherapy".to_string(),
            comparator: "radiotherapy alone".to_string(),
            unit_and_population: "adult newly diagnosed glioblastoma".to_string(),
            outcome_and_horizon: "overall survival at 12 months".to_string(),
            identification_conditions: "randomization; no unmeasured confounding".to_string(),
            available_covariates: ["age".to_string(), "performance-status".to_string()].into(),
            interference_and_selection_assumptions: "no interference between patients".to_string(),
            missingness_model: "administrative censoring, assumed independent".to_string(),
            estimator_acceptance_set: ["kaplan-meier".to_string(), "cox-ph".to_string()].into(),
            sensitivity_checks: ["E-value for unmeasured confounding".to_string()].into(),
            claim_strength: Some(CausalClaimStrength::RealWorldEffect),
            evidence_maturity: Some(EvidenceMaturity::ClinicalStudyOrImplementation),
        }
    }

    #[test]
    fn a_retrospective_exposure_association_cannot_claim_a_real_world_treatment_effect() {
        let mut contract = contract();
        contract.intervention_class = Some(InterventionClass::HistoricalTreatmentExposure);
        assert_eq!(
            contract.check_claim_strength().unwrap_err(),
            CausalError::ProhibitedShortcut {
                contract: "causal:tmz:0001".to_string(),
                basis: "historical treatment exposure in observational data"
            }
        );
    }

    #[test]
    fn a_mechanistic_simulator_cannot_claim_a_real_world_effect() {
        let mut contract = contract();
        contract.intervention_class = Some(InterventionClass::InSilicoPerturbation);
        assert!(matches!(
            contract.check_claim_strength().unwrap_err(),
            CausalError::ProhibitedShortcut { .. }
        ));
    }

    #[test]
    fn an_in_vitro_perturbation_is_real_intervention_evidence_about_the_model_system_only() {
        assert!(!InterventionClass::ExVivoOrInVitroPerturbation.licenses_real_world_effect());
    }

    #[test]
    fn only_randomized_and_workflow_interventions_license_a_real_world_effect_claim() {
        let licensing: Vec<InterventionClass> = InterventionClass::ALL
            .into_iter()
            .filter(|class| class.licenses_real_world_effect())
            .collect();
        assert_eq!(
            licensing,
            vec![
                InterventionClass::RandomizedOrQuasiExperimental,
                InterventionClass::WorkflowOrImplementation
            ]
        );
    }

    #[test]
    fn a_licensed_class_still_needs_evidence_at_a_rung_that_carries_intervention_evidence() {
        let mut contract = contract();
        contract.evidence_maturity = Some(EvidenceMaturity::InternallyValidatedRetrospective);
        assert!(matches!(
            contract.check_claim_strength().unwrap_err(),
            CausalError::ProhibitedShortcut { .. }
        ));
    }

    #[test]
    fn an_associational_claim_needs_no_intervention_licence_at_all() {
        let mut contract = contract();
        contract.claim_strength = Some(CausalClaimStrength::Associational);
        contract.intervention_class = Some(InterventionClass::HistoricalTreatmentExposure);
        contract.evidence_maturity = Some(EvidenceMaturity::ExploratorySingleDataset);
        assert!(contract.check_claim_strength().is_ok());
    }

    #[test]
    fn a_contract_with_no_estimator_acceptance_set_would_pass_any_estimator_and_is_refused() {
        let mut contract = contract();
        contract.estimator_acceptance_set.clear();
        assert_eq!(
            contract.check().unwrap_err(),
            CausalError::NoEstimatorAcceptanceSet {
                contract: "causal:tmz:0001".to_string()
            }
        );
    }

    #[test]
    fn a_contract_with_no_sensitivity_check_is_refused() {
        let mut contract = contract();
        contract.sensitivity_checks.clear();
        assert!(matches!(
            contract.check().unwrap_err(),
            CausalError::NoSensitivityCheck { .. }
        ));
    }

    #[test]
    fn a_contract_that_does_not_say_which_of_the_four_causal_tasks_it_is_is_refused() {
        let mut contract = contract();
        contract.kind = None;
        assert!(matches!(
            contract.check().unwrap_err(),
            CausalError::IncompleteContract { .. }
        ));
    }

    #[test]
    fn a_contract_omitting_its_missingness_model_is_refused() {
        let mut contract = contract();
        contract.missingness_model = "   ".to_string();
        assert_eq!(
            contract.check().unwrap_err(),
            CausalError::IncompleteContract {
                contract: "causal:tmz:0001".to_string(),
                field: "missingness model"
            }
        );
    }

    #[test]
    fn a_fully_declared_randomized_contract_passes() {
        assert!(contract().check().is_ok());
    }
}

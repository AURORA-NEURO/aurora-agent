//! Federated, policy-bounded experiment-design control plane.
//!
//! Atlas feature: `AFA-adapter-P09-F30`.
//!
//! This product compiles a preclinical objective into a typed multi-site design without executing
//! instruments or moving raw data. Site capability matching, modality replication, protected
//! closure, authorization, and instrument-profile comparability are explicit gates; omissions
//! stay in the design receipt for downstream protocol simulation.

use bioprism_foundation::{
    LossSeverity, ProvenanceLink, SemanticLoss, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P09-F30";
pub const CONTRACT_VERSION: &str = "federated-experiment-design-control-plane/1.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentObjective {
    pub objective_id: String,
    pub study_id: String,
    pub estimand: String,
    pub required_modalities: Vec<String>,
    pub required_controls: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DesignSite {
    pub site_id: String,
    pub supported_modalities: Vec<String>,
    pub instrument_profile: String,
    pub available_budget: f64,
    pub raw_data_local: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FederatedExperimentDesignRequest {
    pub request_id: String,
    pub objective: ExperimentObjective,
    pub sites: Vec<DesignSite>,
    pub minimum_sites_per_modality: u32,
    pub policy_allow: bool,
    pub authorization_reference: Option<String>,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DesignDecision {
    Admitted,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExperimentAssignment {
    pub site_id: String,
    pub modality: String,
    pub instrument_profile: String,
    pub budget: f64,
    pub authorized: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExperimentDesignReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub objective_id: String,
    pub decision: DesignDecision,
    pub site_order: Vec<String>,
    pub assignments: Vec<ExperimentAssignment>,
    pub modality_coverage: BTreeMap<String, u32>,
    pub omitted_modalities: Vec<String>,
    pub comparability_conflicts: Vec<String>,
    pub semantic_loss: Vec<SemanticLoss>,
    pub reasons: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

impl ExperimentDesignReceipt {
    pub fn validate(&self) -> Result<(), ExperimentDesignError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
        {
            return Err(ExperimentDesignError::Contract(
                "experiment design contract identity mismatch".into(),
            ));
        }
        if self.request_id.trim().is_empty()
            || self.objective_id.trim().is_empty()
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.reasons.is_empty()
        {
            return Err(ExperimentDesignError::InvalidRequest(
                "design identity, reasons, boundary, and locality are required".into(),
            ));
        }
        if self.site_order.is_empty()
            || self.site_order.windows(2).any(|pair| pair[0] >= pair[1])
            || self.site_order.iter().collect::<BTreeSet<_>>().len() != self.site_order.len()
        {
            return Err(ExperimentDesignError::InvalidRequest(
                "site order must be canonical and unique".into(),
            ));
        }
        let assignment_keys = self
            .assignments
            .iter()
            .map(|assignment| format!("{}:{}", assignment.site_id, assignment.modality))
            .collect::<BTreeSet<_>>();
        if assignment_keys.len() != self.assignments.len()
            || self
                .assignments
                .iter()
                .any(|assignment| !assignment.authorized || !assignment.budget.is_finite())
        {
            return Err(ExperimentDesignError::InvalidRequest(
                "assignments must be unique, finite, and authorized".into(),
            ));
        }
        if self.decision == DesignDecision::Blocked && !self.assignments.is_empty() {
            return Err(ExperimentDesignError::InvalidRequest(
                "blocked design cannot contain authorized assignments".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ExperimentDesignError::Contract(error.to_string()))?;
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, ExperimentDesignError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ExperimentDesignError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ExperimentDesignError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum ExperimentDesignError {
    #[error("invalid federated experiment-design request: {0}")]
    InvalidRequest(String),
    #[error("experiment-design contract rejected: {0}")]
    Contract(String),
    #[error("raw design data must remain local")]
    Localization,
    #[error("duplicate design site {0}")]
    DuplicateSite(String),
    #[error("experiment-design serialization failed: {0}")]
    Serialization(String),
}

pub fn compile_experiment_design(
    request: &FederatedExperimentDesignRequest,
) -> Result<ExperimentDesignReceipt, ExperimentDesignError> {
    validate_request(request)?;
    let mut sites = request.sites.clone();
    sites.sort_by(|left, right| left.site_id.cmp(&right.site_id));
    let site_order = sites
        .iter()
        .map(|site| site.site_id.clone())
        .collect::<Vec<_>>();
    let mut assignments = Vec::new();
    let mut modality_coverage = BTreeMap::new();
    let mut omitted_modalities = Vec::new();
    let mut reasons = vec![
        "design compilation emits assignments only; instrument execution remains outside this control plane".into(),
    ];
    let mut semantic_loss = Vec::new();
    for modality in &request.objective.required_modalities {
        let candidates = sites
            .iter()
            .filter(|site| {
                site.supported_modalities
                    .iter()
                    .any(|supported| supported == modality)
            })
            .collect::<Vec<_>>();
        let count = candidates
            .len()
            .min(request.minimum_sites_per_modality as usize) as u32;
        modality_coverage.insert(modality.clone(), count);
        if count < request.minimum_sites_per_modality {
            omitted_modalities.push(modality.clone());
        } else {
            for site in candidates
                .into_iter()
                .take(request.minimum_sites_per_modality as usize)
            {
                assignments.push(ExperimentAssignment {
                    site_id: site.site_id.clone(),
                    modality: modality.clone(),
                    instrument_profile: site.instrument_profile.clone(),
                    budget: site.available_budget,
                    authorized: true,
                });
            }
        }
    }
    assignments.sort_by(|left, right| {
        left.site_id
            .cmp(&right.site_id)
            .then(left.modality.cmp(&right.modality))
    });
    if !omitted_modalities.is_empty() {
        reasons.push(format!(
            "required modality replication is incomplete: {}",
            omitted_modalities.join(", ")
        ));
    }
    let mut profiles = BTreeMap::<String, BTreeSet<String>>::new();
    for assignment in &assignments {
        profiles
            .entry(assignment.modality.clone())
            .or_default()
            .insert(assignment.instrument_profile.clone());
    }
    let comparability_conflicts = profiles
        .iter()
        .filter(|(_, profile_set)| profile_set.len() > 1)
        .map(|(modality, profile_set)| {
            format!(
                "modality {} has incompatible instrument profiles: {}",
                modality,
                profile_set.iter().cloned().collect::<Vec<_>>().join(", ")
            )
        })
        .collect::<Vec<_>>();
    if !comparability_conflicts.is_empty() {
        reasons.extend(comparability_conflicts.iter().cloned());
        semantic_loss.push(SemanticLoss {
            field: "instrument_profile".into(),
            reason: "cross-site instrument profiles are not comparable without an explicit bridge"
                .into(),
            severity: LossSeverity::DecisionRelevant,
        });
    }
    let policy_authorized = request.policy_allow && request.authorization_reference.is_some();
    if !policy_authorized {
        reasons.push("policy or independent authorization is incomplete".into());
        semantic_loss.push(SemanticLoss {
            field: "authorization".into(),
            reason: "no executable design assignment may be authorized without policy and approval"
                .into(),
            severity: LossSeverity::DecisionRelevant,
        });
    }
    if !request.protected_closure {
        reasons.push("protected design closure is incomplete".into());
        semantic_loss.push(SemanticLoss {
            field: "protected_closure".into(),
            reason: "unresolved protected constraints block executable design admission".into(),
            severity: LossSeverity::DecisionRelevant,
        });
    }
    let decision = if !policy_authorized
        || !request.protected_closure
        || !comparability_conflicts.is_empty()
    {
        DesignDecision::Blocked
    } else if !omitted_modalities.is_empty() {
        DesignDecision::Partial
    } else {
        reasons.push("objective, site capability, replication, comparability, policy, and closure gates passed".into());
        DesignDecision::Admitted
    };
    let final_assignments = if decision == DesignDecision::Blocked {
        Vec::new()
    } else {
        assignments
    };
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "objective_id": request.objective.objective_id,
        "decision": decision,
        "site_order": site_order,
        "assignments": final_assignments,
        "modality_coverage": modality_coverage,
        "omitted_modalities": omitted_modalities,
        "comparability_conflicts": comparability_conflicts,
        "semantic_loss": semantic_loss,
        "reasons": reasons,
        "raw_data_local": true,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let provenance = sites
        .iter()
        .map(|site| ProvenanceLink {
            source_id: site.site_id.clone(),
            relation: "design-from-local-site-capability".into(),
            digest: ContentHash::of_bytes(site.site_id.as_bytes()),
        })
        .collect();
    let artifact = TypedResearchArtifact::from_payload(
        format!("experiment-design:{}", request.request_id),
        "application/vnd.aurora.federated-experiment-design+json",
        &payload,
        semantic_loss.clone(),
        provenance,
    )
    .map_err(|error| ExperimentDesignError::Contract(error.to_string()))?;
    let receipt = ExperimentDesignReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        objective_id: request.objective.objective_id.clone(),
        decision,
        site_order,
        assignments: final_assignments,
        modality_coverage,
        omitted_modalities,
        comparability_conflicts,
        semantic_loss,
        reasons,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(
    request: &FederatedExperimentDesignRequest,
) -> Result<(), ExperimentDesignError> {
    if request.request_id.trim().is_empty()
        || request.objective.objective_id.trim().is_empty()
        || request.objective.study_id.trim().is_empty()
        || request.objective.estimand.trim().is_empty()
        || request.objective.required_modalities.is_empty()
        || request.sites.is_empty()
        || request.minimum_sites_per_modality == 0
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
    {
        return if !request.raw_data_local || request.boundary != PRECLINICAL_BOUNDARY {
            Err(ExperimentDesignError::Localization)
        } else {
            Err(ExperimentDesignError::InvalidRequest(
                "request, objective, modalities, sites, positive replication, and boundary are required".into(),
            ))
        };
    }
    let mut ids = BTreeSet::new();
    for site in &request.sites {
        if site.site_id.trim().is_empty()
            || site.instrument_profile.trim().is_empty()
            || !site.available_budget.is_finite()
            || site.available_budget < 0.0
            || site.supported_modalities.is_empty()
            || !site.raw_data_local
        {
            return Err(ExperimentDesignError::InvalidRequest(
                "site identity, finite non-negative budget, capabilities, instrument profile, and locality are required".into(),
            ));
        }
        if !ids.insert(site.site_id.clone()) {
            return Err(ExperimentDesignError::DuplicateSite(site.site_id.clone()));
        }
    }
    if request
        .objective
        .required_modalities
        .iter()
        .any(|modality| modality.trim().is_empty())
    {
        return Err(ExperimentDesignError::InvalidRequest(
            "required modality names cannot be empty".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn site(id: &str, profile: &str, modalities: &[&str]) -> DesignSite {
        DesignSite {
            site_id: id.into(),
            supported_modalities: modalities.iter().map(|item| (*item).into()).collect(),
            instrument_profile: profile.into(),
            available_budget: 10.0,
            raw_data_local: true,
        }
    }

    fn request() -> FederatedExperimentDesignRequest {
        FederatedExperimentDesignRequest {
            request_id: "design:1".into(),
            objective: ExperimentObjective {
                objective_id: "objective:organoid".into(),
                study_id: "study:organoid".into(),
                estimand: "effect of perturbation on phenotype".into(),
                required_modalities: vec!["imaging".into(), "transcriptomics".into()],
                required_controls: vec!["vehicle".into()],
            },
            sites: vec![
                site("site:b", "instrument-v2", &["imaging"]),
                site("site:a", "instrument-v2", &["transcriptomics"]),
            ],
            minimum_sites_per_modality: 1,
            policy_allow: true,
            authorization_reference: Some("approval:design".into()),
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn design_is_sorted_and_replayable() {
        let mut reversed = request();
        reversed.sites.reverse();
        let left = compile_experiment_design(&request()).unwrap();
        let right = compile_experiment_design(&reversed).unwrap();
        assert_eq!(left.digest().unwrap(), right.digest().unwrap());
        assert_eq!(left.decision, DesignDecision::Admitted);
        assert_eq!(left.site_order, vec!["site:a", "site:b"]);
    }

    #[test]
    fn missing_modality_is_partial_not_silent() {
        let mut request = request();
        request
            .objective
            .required_modalities
            .push("proteomics".into());
        let receipt = compile_experiment_design(&request).unwrap();
        assert_eq!(receipt.decision, DesignDecision::Partial);
        assert_eq!(receipt.omitted_modalities, vec!["proteomics"]);
    }

    #[test]
    fn missing_authorization_blocks_without_assignments() {
        let mut request = request();
        request.authorization_reference = None;
        let receipt = compile_experiment_design(&request).unwrap();
        assert_eq!(receipt.decision, DesignDecision::Blocked);
        assert!(receipt.assignments.is_empty());
    }

    #[test]
    fn duplicate_site_is_rejected() {
        let mut request = request();
        request.sites[1].site_id = request.sites[0].site_id.clone();
        assert!(matches!(
            compile_experiment_design(&request).unwrap_err(),
            ExperimentDesignError::DuplicateSite(_)
        ));
    }
}

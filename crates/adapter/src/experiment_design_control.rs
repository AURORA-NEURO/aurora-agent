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
const MAX_TEXT_BYTES: usize = 512;
const MAX_SITES: usize = 8192;
const MAX_ITEMS: usize = 16384;

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
    pub input: FederatedExperimentDesignRequest,
    pub input_digest: ContentHash,
    pub request_id: String,
    pub objective_id: String,
    pub objective: ExperimentObjective,
    pub sites: Vec<DesignSite>,
    pub minimum_sites_per_modality: u32,
    pub policy_allow: bool,
    pub authorization_reference: Option<String>,
    pub protected_closure: bool,
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
            || self.sites.is_empty()
        {
            return Err(ExperimentDesignError::InvalidRequest(
                "design identity, reasons, boundary, and locality are required".into(),
            ));
        }
        validate_text("request_id", &self.request_id)?;
        validate_text("objective_id", &self.objective_id)?;
        validate_text("boundary", &self.boundary)?;
        if self.site_order.is_empty()
            || self.site_order.len() > MAX_SITES
            || self.site_order.windows(2).any(|pair| pair[0] >= pair[1])
            || self.site_order.iter().collect::<BTreeSet<_>>().len() != self.site_order.len()
        {
            return Err(ExperimentDesignError::InvalidRequest(
                "site order must be canonical and unique".into(),
            ));
        }
        for site in &self.site_order {
            validate_text("site_order", site)?;
        }
        validate_sorted_strings("omitted_modalities", &self.omitted_modalities)?;
        validate_sorted_strings("comparability_conflicts", &self.comparability_conflicts)?;
        validate_sorted_strings("reasons", &self.reasons)?;
        if self.modality_coverage.is_empty() || self.modality_coverage.len() > MAX_ITEMS {
            return Err(ExperimentDesignError::InvalidRequest(
                "modality coverage is outside its bound".into(),
            ));
        }
        for (modality, count) in &self.modality_coverage {
            validate_text("modality", modality)?;
            let assigned = self
                .assignments
                .iter()
                .filter(|assignment| &assignment.modality == modality)
                .count() as u32;
            if self.decision != DesignDecision::Blocked && *count != assigned {
                return Err(ExperimentDesignError::InvalidRequest(
                    "modality coverage does not match assignments".into(),
                ));
            }
        }
        let assignment_keys = self
            .assignments
            .iter()
            .map(|assignment| format!("{}:{}", assignment.site_id, assignment.modality))
            .collect::<BTreeSet<_>>();
        if assignment_keys.len() != self.assignments.len()
            || self.assignments.iter().any(|assignment| {
                !assignment.authorized
                    || !assignment.budget.is_finite()
                    || assignment.budget <= 0.0
                    || !self.site_order.contains(&assignment.site_id)
            })
        {
            return Err(ExperimentDesignError::InvalidRequest(
                "assignments must be unique, finite, and authorized".into(),
            ));
        }
        for assignment in &self.assignments {
            validate_text("assignment.site_id", &assignment.site_id)?;
            validate_text("assignment.modality", &assignment.modality)?;
            validate_text(
                "assignment.instrument_profile",
                &assignment.instrument_profile,
            )?;
        }
        if self.assignments.windows(2).any(|pair| {
            (pair[0].site_id.as_str(), pair[0].modality.as_str())
                >= (pair[1].site_id.as_str(), pair[1].modality.as_str())
        }) {
            return Err(ExperimentDesignError::InvalidRequest(
                "assignments must be in canonical site and modality order".into(),
            ));
        }
        for loss in &self.semantic_loss {
            validate_text("semantic_loss.field", &loss.field)?;
            validate_text("semantic_loss.reason", &loss.reason)?;
        }
        if self
            .semantic_loss
            .windows(2)
            .any(|pair| pair[0].field >= pair[1].field)
        {
            return Err(ExperimentDesignError::InvalidRequest(
                "semantic-loss ordering is not canonical".into(),
            ));
        }
        if self.decision == DesignDecision::Blocked && !self.assignments.is_empty() {
            return Err(ExperimentDesignError::InvalidRequest(
                "blocked design cannot contain authorized assignments".into(),
            ));
        }
        let expected_provenance = design_provenance(&self.sites)?;
        if self.artifact.artifact_id != format!("experiment-design:{}", self.request_id)
            || self.artifact.content_type
                != "application/vnd.aurora.federated-experiment-design+json"
            || self.artifact.semantic_loss != self.semantic_loss
            || self.artifact.provenance != expected_provenance
        {
            return Err(ExperimentDesignError::Contract(
                "experiment design artifact is not bound to the receipt".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ExperimentDesignError::Contract(error.to_string()))?;
        self.artifact
            .verify_payload(&design_payload(self))
            .map_err(|error| ExperimentDesignError::Contract(error.to_string()))?;
        validate_request(&self.input)?;
        if self.input_digest != experiment_design_input_digest(&self.input)? {
            return Err(ExperimentDesignError::Contract(
                "experiment design retained input digest does not match the request".into(),
            ));
        }
        let expected = compile_experiment_design_internal(&self.input, false)?;
        if self != &expected {
            return Err(ExperimentDesignError::Contract(
                "experiment design receipt is not derived from its retained objective and site inputs".into(),
            ));
        }
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

fn validate_text(field: &str, value: &str) -> Result<(), ExperimentDesignError> {
    if value.is_empty() || value.trim() != value {
        return Err(ExperimentDesignError::InvalidRequest(format!(
            "{field} must be non-empty and trimmed"
        )));
    }
    if value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(ExperimentDesignError::InvalidRequest(format!(
            "{field} is outside its bounded text contract"
        )));
    }
    Ok(())
}

fn experiment_design_input_digest(
    request: &FederatedExperimentDesignRequest,
) -> Result<ContentHash, ExperimentDesignError> {
    let value = serde_json::to_value(&canonical_experiment_design_request(request))
        .map_err(|error| ExperimentDesignError::Serialization(error.to_string()))?;
    ContentHash::of_value(&value)
        .map_err(|error| ExperimentDesignError::Serialization(error.to_string()))
}

fn canonical_experiment_design_request(
    request: &FederatedExperimentDesignRequest,
) -> FederatedExperimentDesignRequest {
    let mut canonical = request.clone();
    canonical.objective = canonical_objective(&canonical.objective);
    canonical.sites = canonical_sites(&canonical.sites);
    canonical
}

fn validate_unique_strings(field: &str, values: &[String]) -> Result<(), ExperimentDesignError> {
    if values.len() > MAX_ITEMS {
        return Err(ExperimentDesignError::InvalidRequest(format!(
            "{field} exceeds its item bound"
        )));
    }
    let mut unique = BTreeSet::new();
    for value in values {
        validate_text(field, value)?;
        if !unique.insert(value) {
            return Err(ExperimentDesignError::InvalidRequest(format!(
                "{field} contains duplicate values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_strings(field: &str, values: &[String]) -> Result<(), ExperimentDesignError> {
    if values.len() > MAX_ITEMS {
        return Err(ExperimentDesignError::InvalidRequest(format!(
            "{field} exceeds its item bound"
        )));
    }
    for value in values {
        validate_text(field, value)?;
    }
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ExperimentDesignError::InvalidRequest(format!(
            "{field} ordering is not canonical"
        )));
    }
    Ok(())
}

fn canonical_objective(objective: &ExperimentObjective) -> ExperimentObjective {
    let mut objective = objective.clone();
    objective.required_modalities.sort();
    objective.required_controls.sort();
    objective
}

fn canonical_sites(sites: &[DesignSite]) -> Vec<DesignSite> {
    let mut sites = sites.to_vec();
    for site in &mut sites {
        site.supported_modalities.sort();
    }
    sites.sort_by(|left, right| left.site_id.cmp(&right.site_id));
    sites
}

fn design_provenance(sites: &[DesignSite]) -> Result<Vec<ProvenanceLink>, ExperimentDesignError> {
    sites
        .iter()
        .map(|site| {
            let value = serde_json::to_value(site)
                .map_err(|error| ExperimentDesignError::Serialization(error.to_string()))?;
            let digest = ContentHash::of_value(&value)
                .map_err(|error| ExperimentDesignError::Serialization(error.to_string()))?;
            Ok(ProvenanceLink {
                source_id: site.site_id.clone(),
                relation: "design-from-local-site-capability".into(),
                digest,
            })
        })
        .collect()
}

fn design_payload(receipt: &ExperimentDesignReceipt) -> serde_json::Value {
    design_payload_from_parts(
        &receipt.schema_version,
        &receipt.contract_version,
        &receipt.feature_id,
        &receipt.request_id,
        &receipt.objective_id,
        &receipt.objective,
        &receipt.sites,
        receipt.minimum_sites_per_modality,
        receipt.policy_allow,
        &receipt.authorization_reference,
        receipt.protected_closure,
        receipt.decision,
        &receipt.site_order,
        &receipt.assignments,
        &receipt.modality_coverage,
        &receipt.omitted_modalities,
        &receipt.comparability_conflicts,
        &receipt.semantic_loss,
        &receipt.reasons,
        &receipt.artifact.provenance,
        receipt.raw_data_local,
        &receipt.boundary,
    )
}

#[allow(clippy::too_many_arguments)]
fn design_payload_from_parts(
    schema_version: &str,
    contract_version: &str,
    feature_id: &str,
    request_id: &str,
    objective_id: &str,
    objective: &ExperimentObjective,
    sites: &[DesignSite],
    minimum_sites_per_modality: u32,
    policy_allow: bool,
    authorization_reference: &Option<String>,
    protected_closure: bool,
    decision: DesignDecision,
    site_order: &[String],
    assignments: &[ExperimentAssignment],
    modality_coverage: &BTreeMap<String, u32>,
    omitted_modalities: &[String],
    comparability_conflicts: &[String],
    semantic_loss: &[SemanticLoss],
    reasons: &[String],
    provenance: &[ProvenanceLink],
    raw_data_local: bool,
    boundary: &str,
) -> serde_json::Value {
    json!({
        "schema_version": schema_version,
        "contract_version": contract_version,
        "feature_id": feature_id,
        "request_id": request_id,
        "objective_id": objective_id,
        "objective": objective,
        "sites": sites,
        "minimum_sites_per_modality": minimum_sites_per_modality,
        "policy_allow": policy_allow,
        "authorization_reference": authorization_reference,
        "protected_closure": protected_closure,
        "decision": decision,
        "site_order": site_order,
        "assignments": assignments,
        "modality_coverage": modality_coverage,
        "omitted_modalities": omitted_modalities,
        "comparability_conflicts": comparability_conflicts,
        "semantic_loss": semantic_loss,
        "reasons": reasons,
        "provenance": provenance,
        "raw_data_local": raw_data_local,
        "boundary": boundary,
    })
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
    compile_experiment_design_internal(request, true)
}

fn compile_experiment_design_internal(
    request: &FederatedExperimentDesignRequest,
    validate_output: bool,
) -> Result<ExperimentDesignReceipt, ExperimentDesignError> {
    validate_request(request)?;
    let input = canonical_experiment_design_request(request);
    let objective = canonical_objective(&request.objective);
    let sites = canonical_sites(&request.sites);
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
    let required_modalities = objective.required_modalities.clone();
    for modality in &required_modalities {
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
    omitted_modalities.sort();
    omitted_modalities.dedup();
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
    let policy_authorized = request.policy_allow
        && request
            .authorization_reference
            .as_ref()
            .is_some_and(|reference| !reference.trim().is_empty());
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
    semantic_loss.sort_by(|left, right| left.field.cmp(&right.field));
    reasons.sort();
    reasons.dedup();
    let final_assignments = if decision == DesignDecision::Blocked {
        Vec::new()
    } else {
        assignments
    };
    let provenance = design_provenance(&sites)?;
    let payload = design_payload_from_parts(
        RESEARCH_CONTRACT_SCHEMA_VERSION,
        CONTRACT_VERSION,
        FEATURE_ID,
        &request.request_id,
        &objective.objective_id,
        &objective,
        &sites,
        request.minimum_sites_per_modality,
        request.policy_allow,
        &request.authorization_reference,
        request.protected_closure,
        decision,
        &site_order,
        &final_assignments,
        &modality_coverage,
        &omitted_modalities,
        &comparability_conflicts,
        &semantic_loss,
        &reasons,
        &provenance,
        true,
        PRECLINICAL_BOUNDARY,
    );
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
        input_digest: experiment_design_input_digest(&input)?,
        input,
        request_id: request.request_id.clone(),
        objective_id: objective.objective_id.clone(),
        objective,
        sites,
        minimum_sites_per_modality: request.minimum_sites_per_modality,
        policy_allow: request.policy_allow,
        authorization_reference: request.authorization_reference.clone(),
        protected_closure: request.protected_closure,
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
    if validate_output {
        receipt.validate()?;
    }
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
        || request.objective.required_controls.is_empty()
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
    validate_text("request_id", &request.request_id)?;
    validate_text("objective_id", &request.objective.objective_id)?;
    validate_text("study_id", &request.objective.study_id)?;
    validate_text("estimand", &request.objective.estimand)?;
    validate_unique_strings(
        "required_modalities",
        &request.objective.required_modalities,
    )?;
    validate_unique_strings("required_controls", &request.objective.required_controls)?;
    if request.sites.len() > MAX_SITES {
        return Err(ExperimentDesignError::InvalidRequest(
            "site count exceeds its bound".into(),
        ));
    }
    if let Some(reference) = &request.authorization_reference {
        validate_text("authorization_reference", reference)?;
    }
    let mut ids = BTreeSet::new();
    for site in &request.sites {
        if site.site_id.trim().is_empty()
            || site.instrument_profile.trim().is_empty()
            || !site.available_budget.is_finite()
            || site.available_budget <= 0.0
            || site.supported_modalities.is_empty()
            || !site.raw_data_local
        {
            return Err(ExperimentDesignError::InvalidRequest(
                "site identity, finite non-negative budget, capabilities, instrument profile, and locality are required".into(),
            ));
        }
        validate_text("site_id", &site.site_id)?;
        validate_text("instrument_profile", &site.instrument_profile)?;
        validate_unique_strings("supported_modalities", &site.supported_modalities)?;
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

    #[test]
    fn required_modality_order_does_not_change_design_digest() {
        let mut reversed = request();
        reversed.objective.required_modalities.reverse();
        assert_eq!(
            compile_experiment_design(&request())
                .unwrap()
                .digest()
                .unwrap(),
            compile_experiment_design(&reversed)
                .unwrap()
                .digest()
                .unwrap()
        );
    }

    #[test]
    fn whitespace_authorization_is_not_admission() {
        let mut request = request();
        request.authorization_reference = Some("   ".into());
        let error = compile_experiment_design(&request).unwrap_err();
        assert!(error.to_string().contains("authorization_reference"));
    }

    #[test]
    fn receipt_rejects_tampered_artifact_payload_binding() {
        let mut receipt = compile_experiment_design(&request()).unwrap();
        receipt.objective_id = "tampered-objective".into();
        let error = receipt.validate().unwrap_err();
        assert!(error.to_string().contains("digest mismatch"));
    }

    #[test]
    fn retained_site_capability_tampering_is_rejected() {
        let mut receipt = compile_experiment_design(&request()).unwrap();
        receipt.sites[0].available_budget = 1.0;
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn retained_authorization_gate_tampering_is_rejected() {
        let mut receipt = compile_experiment_design(&request()).unwrap();
        receipt.authorization_reference = None;
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn site_capability_provenance_tampering_is_rejected() {
        let mut receipt = compile_experiment_design(&request()).unwrap();
        receipt.artifact.provenance[0].digest = ContentHash::of_bytes(b"tampered");
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn retained_request_tampering_is_rejected() {
        let mut receipt = compile_experiment_design(&request()).unwrap();
        receipt.input.objective.estimand = "tampered-estimand".into();
        assert!(receipt.validate().is_err());
    }
}

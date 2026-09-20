//! Execute the independent-site replication campaign after validation admission.
//!
//! The P10 replication campaign already performs site-level assessment, meta-analysis,
//! transportability checks, and bounded next-action execution. This feature is the guarded
//! handoff from P10-F22: a local efficacy signal cannot invoke that campaign until the validation
//! boundary is qualified and the caller supplies observations from sites other than the origin.
//! The executor remains institution-owned; MCP uses only the explicitly simulation-only worker.

use super::campaign::{
    execute_glioma_replication_campaign, GliomaReplicationCampaign,
    GliomaReplicationCampaignDisposition, GliomaReplicationCampaignError,
    GliomaReplicationCampaignExecutor, GliomaReplicationCampaignRequest,
    GliomaReplicationCampaignStopReason,
};
use super::validation_replication_gate::{
    ValidationReplicationGate, ValidationReplicationGateDisposition,
};
use crate::glioma_engine::GliomaModelSystem;
use bioprism_foundation::PRECLINICAL_BOUNDARY;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P10-F23";
pub const OUTPUT_SCHEMA: &str = "GliomaValidationReplicationCampaign1@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationReplicationCampaignRequest {
    pub gate: ValidationReplicationGate,
    pub replication: GliomaReplicationCampaignRequest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationReplicationCampaignDisposition {
    BlockedByValidation,
    HoldIndependentSites,
    Executed,
    Qualified,
    Negative,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationReplicationCampaignRun {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub validation_campaign_digest: ContentHash,
    pub origin_site_id: String,
    pub independent_site_order: Vec<String>,
    pub gate_disposition: ValidationReplicationGateDisposition,
    pub campaign: Option<GliomaReplicationCampaign>,
    pub disposition: ValidationReplicationCampaignDisposition,
    pub stop_reason: Option<GliomaReplicationCampaignStopReason>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub next_action: String,
    pub boundary: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ValidationReplicationCampaignError {
    #[error("validation-to-replication campaign request is invalid: {0}")]
    InvalidRequest(String),
    #[error("replication campaign execution failed: {0}")]
    Campaign(#[from] GliomaReplicationCampaignError),
    #[error("validation-to-replication campaign output is invalid: {0}")]
    InvalidOutput(String),
    #[error("validation-to-replication campaign digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn sorted_unique(values: impl IntoIterator<Item = String>) -> Vec<String> {
    values
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn digest_input(output: &ValidationReplicationCampaignRun) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "model_system": output.model_system,
        "validation_campaign_digest": output.validation_campaign_digest,
        "origin_site_id": output.origin_site_id,
        "independent_site_order": output.independent_site_order,
        "gate_disposition": output.gate_disposition,
        "campaign": output.campaign,
        "disposition": output.disposition,
        "stop_reason": output.stop_reason,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "next_action": output.next_action,
        "boundary": output.boundary,
    })
}

fn finish(
    request: &ValidationReplicationCampaignRequest,
    independent_site_order: Vec<String>,
    campaign: Option<GliomaReplicationCampaign>,
    disposition: ValidationReplicationCampaignDisposition,
    stop_reason: Option<GliomaReplicationCampaignStopReason>,
    negative_evidence: Vec<String>,
    uncertainty: Vec<String>,
    next_action: &str,
) -> Result<ValidationReplicationCampaignRun, ValidationReplicationCampaignError> {
    let mut output = ValidationReplicationCampaignRun {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.replication.objective.clone(),
        model_system: request.replication.model_system,
        validation_campaign_digest: request.gate.validation_campaign_digest.clone(),
        origin_site_id: request.gate.local_site_id.clone(),
        independent_site_order,
        gate_disposition: request.gate.disposition,
        campaign,
        disposition,
        stop_reason,
        negative_evidence: sorted_unique(negative_evidence),
        uncertainty: sorted_unique(uncertainty),
        next_action: next_action.into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-validation-replication-campaign"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ValidationReplicationCampaignError::Digest(error.to_string()))?;
    validate_output(&output)?;
    Ok(output)
}

fn validate_request(
    request: &ValidationReplicationCampaignRequest,
) -> Result<Vec<String>, ValidationReplicationCampaignError> {
    request
        .gate
        .validate()
        .map_err(|error| ValidationReplicationCampaignError::InvalidRequest(error.to_string()))?;
    if request.replication.model_system != request.gate.model_system
        || request.replication.objective.trim().is_empty()
        || request.gate.local_site_id.trim().is_empty()
    {
        return Err(ValidationReplicationCampaignError::InvalidRequest(
            "replication objective and model system must match the admitted validation gate".into(),
        ));
    }
    let mut sites = BTreeSet::new();
    for study in &request.replication.initial_studies {
        if study.site_id.trim().is_empty() || study.site_id == request.gate.local_site_id {
            return Err(ValidationReplicationCampaignError::InvalidRequest(
                "initial replication studies must be bound to non-origin sites".into(),
            ));
        }
        sites.insert(study.site_id.clone());
    }
    Ok(sites.into_iter().collect())
}

fn validate_output(
    output: &ValidationReplicationCampaignRun,
) -> Result<(), ValidationReplicationCampaignError> {
    if output.feature_id != FEATURE_ID
        || output.output_schema != OUTPUT_SCHEMA
        || output.objective.trim().is_empty()
        || output.origin_site_id.trim().is_empty()
        || !canonical(&output.independent_site_order)
        || !canonical(&output.negative_evidence)
        || !canonical(&output.uncertainty)
        || output.boundary != PRECLINICAL_BOUNDARY
        || output
            .campaign
            .as_ref()
            .is_some_and(|campaign| campaign.validate().is_err())
        || output.next_action.trim().is_empty()
    {
        return Err(ValidationReplicationCampaignError::InvalidOutput(
            "identity, ordering, boundary, campaign, or next-action invariants are invalid".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(output))
        .map_err(|error| ValidationReplicationCampaignError::Digest(error.to_string()))?;
    if expected != output.digest {
        return Err(ValidationReplicationCampaignError::InvalidOutput(
            "validation-to-replication campaign output is not content-addressed".into(),
        ));
    }
    Ok(())
}

impl ValidationReplicationCampaignRun {
    pub fn validate(&self) -> Result<(), ValidationReplicationCampaignError> {
        validate_output(self)
    }
}

/// Execute the existing P10 replication/interpretation campaign after the P10-F22 admission gate.
pub fn execute_glioma_validation_replication_campaign<E: GliomaReplicationCampaignExecutor>(
    request: &ValidationReplicationCampaignRequest,
    executor: &mut E,
) -> Result<ValidationReplicationCampaignRun, ValidationReplicationCampaignError> {
    let independent_site_order = validate_request(request)?;
    let mut negative_evidence = request.gate.negative_evidence.clone();
    let mut uncertainty = request.gate.uncertainty.clone();
    if !request.gate.validation_eligible
        || matches!(
            request.gate.disposition,
            ValidationReplicationGateDisposition::HoldValidation
                | ValidationReplicationGateDisposition::Blocked
                | ValidationReplicationGateDisposition::Negative
        )
    {
        negative_evidence.push("validation-replication-admission-denied".into());
        uncertainty.push(
            "the local validation gate did not establish an efficacy-qualified handoff".into(),
        );
        return finish(
            request,
            independent_site_order,
            None,
            ValidationReplicationCampaignDisposition::BlockedByValidation,
            None,
            negative_evidence,
            uncertainty,
            "complete an efficacy-qualified local validation and rerun the independent-site gate",
        );
    }
    if independent_site_order.len() < request.replication.min_sites {
        negative_evidence.push(format!(
            "independent-sites:{}<{}",
            independent_site_order.len(),
            request.replication.min_sites
        ));
        uncertainty.push(
            "the replication campaign requires paired studies from the declared minimum sites"
                .into(),
        );
        return finish(
            request,
            independent_site_order,
            None,
            ValidationReplicationCampaignDisposition::HoldIndependentSites,
            None,
            negative_evidence,
            uncertainty,
            "collect paired study summaries from additional independent sites before campaign execution",
        );
    }
    let campaign = execute_glioma_replication_campaign(&request.replication, executor)?;
    negative_evidence.extend(campaign.negative_evidence.clone());
    uncertainty.extend(campaign.uncertainty.clone());
    let (disposition, next_action) = match campaign.disposition {
        GliomaReplicationCampaignDisposition::Qualified => (
            ValidationReplicationCampaignDisposition::Qualified,
            "hold the qualified multi-site result for independent methods review and signed release evidence",
        ),
        GliomaReplicationCampaignDisposition::Negative => (
            ValidationReplicationCampaignDisposition::Negative,
            "publish the negative or null multi-site replication result and stop further spend",
        ),
        GliomaReplicationCampaignDisposition::Partial => (
            ValidationReplicationCampaignDisposition::Executed,
            "attach returned site summaries and continue only through the bounded replication campaign policy",
        ),
        GliomaReplicationCampaignDisposition::Unresolved => (
            ValidationReplicationCampaignDisposition::Blocked,
            "resolve missing, contradictory, or transportability evidence before another replication action",
        ),
        GliomaReplicationCampaignDisposition::Failed
        | GliomaReplicationCampaignDisposition::Blocked => (
            ValidationReplicationCampaignDisposition::Blocked,
            "repair the failed or blocked institution-local executor boundary before retrying",
        ),
    };
    finish(
        request,
        independent_site_order,
        Some(campaign.clone()),
        disposition,
        Some(campaign.stop_reason),
        negative_evidence,
        uncertainty,
        next_action,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_execution_when_gate_has_no_efficacy_admission() {
        let mut gate = ValidationReplicationGate {
            feature_id: super::super::validation_replication_gate::FEATURE_ID.into(),
            output_schema: super::super::validation_replication_gate::OUTPUT_SCHEMA.into(),
            objective: "local organoid validation".into(),
            model_system: GliomaModelSystem::Organoid,
            validation_campaign_digest: ContentHash::of_bytes(b"validation"),
            local_site_id: "origin-site".into(),
            independent_site_order: Vec::new(),
            validation_eligible: false,
            replication_plan: None,
            continuation: None,
            protocol: None,
            disposition: ValidationReplicationGateDisposition::HoldValidation,
            negative_evidence: vec!["validation-stop:awaiting-measurements".into()],
            uncertainty: vec!["local validation is not efficacy-qualified".into()],
            next_action: "complete validation".into(),
            boundary: PRECLINICAL_BOUNDARY.into(),
            digest: ContentHash::of_bytes(b"unsealed"),
        };
        gate.digest = ContentHash::of_value(&serde_json::json!({
            "feature_id": gate.feature_id,
            "output_schema": gate.output_schema,
            "objective": gate.objective,
            "model_system": gate.model_system,
            "validation_campaign_digest": gate.validation_campaign_digest,
            "local_site_id": gate.local_site_id,
            "independent_site_order": gate.independent_site_order,
            "validation_eligible": gate.validation_eligible,
            "replication_plan": gate.replication_plan,
            "continuation": gate.continuation,
            "protocol": gate.protocol,
            "disposition": gate.disposition,
            "negative_evidence": gate.negative_evidence,
            "uncertainty": gate.uncertainty,
            "next_action": gate.next_action,
            "boundary": gate.boundary,
        }))
        .unwrap();
        let request = ValidationReplicationCampaignRequest {
            gate,
            replication: GliomaReplicationCampaignRequest {
                objective: "replicate an organoid invasion effect".into(),
                model_system: GliomaModelSystem::Organoid,
                target_model_system: GliomaModelSystem::Organoid,
                target_signature: vec![1, 2],
                min_sites: 1,
                min_replicates_per_site: 1,
                min_studies: 1,
                min_replicates_per_study: 1,
                effect_threshold_milli: 10,
                max_heterogeneity_milli: 500,
                max_i2_milli: 900,
                min_signal_to_noise_milli: 1,
                max_leave_one_out_shift_milli: 500,
                min_quality_milli: 500,
                distance_scale_milli: 1_000,
                max_transport_gap_milli: 500,
                max_transport_heterogeneity_milli: 500,
                budget_units: 10,
                max_rounds: 1,
                max_actions_per_round: 1,
                max_retries: 0,
                initial_studies: Vec::new(),
                initial_transport_studies: Vec::new(),
                replay_identity: ContentHash::of_bytes(b"replay"),
            },
        };
        let mut executor =
            super::super::campaign::DryRunGliomaReplicationCampaignExecutor::default();
        let output =
            execute_glioma_validation_replication_campaign(&request, &mut executor).unwrap();
        assert_eq!(
            output.disposition,
            ValidationReplicationCampaignDisposition::BlockedByValidation
        );
        assert!(output.campaign.is_none());
        output.validate().unwrap();
    }
}

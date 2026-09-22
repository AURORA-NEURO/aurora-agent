//! Evidence-gated handoff from a local glioma validation campaign to independent replication.
//!
//! A local efficacy boundary is not a preclinical conclusion. This feature is the workflow
//! boundary that makes that distinction executable: it accepts a validation campaign only when
//! its typed power controller stopped for efficacy, rejects local-site observations as independent
//! evidence, replans the multi-site topology, and compiles the next bounded replication wave. It
//! preserves negative, heterogeneous, underpowered, and resource-blocked states instead of
//! allowing an autonomous worker to promote them into a claim.

use crate::glioma::programs::p06_experiment_design::validation_campaign::{
    ValidationCampaignDisposition, ValidationCampaignRun, ValidationCampaignStopReason,
};
use crate::glioma::programs::p06_experiment_design::{
    compile_glioma_replication_protocol, plan_glioma_replication_continuation,
    ReplicationContinuationDisposition, ReplicationContinuationError,
    ReplicationContinuationObservation, ReplicationContinuationPlan,
    ReplicationContinuationRequest, ReplicationPlan, ReplicationPlanError, ReplicationPlanRequest,
    ReplicationProtocolCompilation, ReplicationProtocolCompilationDisposition,
    ReplicationProtocolCompilationError, ReplicationProtocolCompileRequest,
};
use crate::glioma::programs::p07_protocol_simulation::ProtocolResource;
use crate::glioma_engine::GliomaModelSystem;
use bioprism_foundation::PRECLINICAL_BOUNDARY;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P10-F22";
pub const OUTPUT_SCHEMA: &str = "GliomaValidationReplicationGate1@1";
pub const MAX_OBSERVATIONS: usize = 4_096;
pub const MAX_ROUNDS: u32 = 1_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationReplicationGateRequest {
    pub validation_campaign: ValidationCampaignRun,
    pub local_site_id: String,
    pub replication_plan: ReplicationPlanRequest,
    pub observations: Vec<ReplicationContinuationObservation>,
    pub minimum_quality_milli: u16,
    pub negative_effect_threshold_milli: u16,
    pub current_round: u32,
    pub max_rounds: u32,
    pub protocol_resources: Vec<ProtocolResource>,
    pub max_ticks: u32,
    pub max_risk_milli: u16,
    pub allow_instrument_execution: bool,
    pub approval_reference: Option<String>,
    pub randomization_seed: ContentHash,
    pub ticks_per_replicate: u32,
    pub include_quality_task: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationReplicationGateDisposition {
    HoldValidation,
    HoldSites,
    ReadyToExecute,
    Qualified,
    Negative,
    Heterogeneous,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationReplicationGate {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub validation_campaign_digest: ContentHash,
    pub local_site_id: String,
    pub independent_site_order: Vec<String>,
    pub validation_eligible: bool,
    pub replication_plan: Option<ReplicationPlan>,
    pub continuation: Option<ReplicationContinuationPlan>,
    pub protocol: Option<ReplicationProtocolCompilation>,
    pub disposition: ValidationReplicationGateDisposition,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub next_action: String,
    pub boundary: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ValidationReplicationGateError {
    #[error("validation campaign is invalid: {0}")]
    Validation(String),
    #[error("validation-to-replication request is invalid: {0}")]
    InvalidRequest(String),
    #[error("replication topology failed: {0}")]
    Topology(#[from] ReplicationPlanError),
    #[error("replication continuation failed: {0}")]
    Continuation(#[from] ReplicationContinuationError),
    #[error("replication protocol compilation failed: {0}")]
    Protocol(#[from] ReplicationProtocolCompilationError),
    #[error("validation-to-replication output is invalid: {0}")]
    InvalidOutput(String),
    #[error("validation-to-replication digest failed: {0}")]
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

fn digest_input(output: &ValidationReplicationGate) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "model_system": output.model_system,
        "validation_campaign_digest": output.validation_campaign_digest,
        "local_site_id": output.local_site_id,
        "independent_site_order": output.independent_site_order,
        "validation_eligible": output.validation_eligible,
        "replication_plan": output.replication_plan,
        "continuation": output.continuation,
        "protocol": output.protocol,
        "disposition": output.disposition,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "next_action": output.next_action,
        "boundary": output.boundary,
    })
}

fn validate_request(
    request: &ValidationReplicationGateRequest,
) -> Result<(), ValidationReplicationGateError> {
    request
        .validation_campaign
        .validate()
        .map_err(|error| ValidationReplicationGateError::Validation(error.to_string()))?;
    if request.local_site_id.trim().is_empty()
        || request.observations.len() > MAX_OBSERVATIONS
        || request.minimum_quality_milli > 1_000
        || request.negative_effect_threshold_milli > 1_000
        || request.current_round == 0
        || request.max_rounds < request.current_round
        || request.max_rounds > MAX_ROUNDS
        || request.max_ticks == 0
        || request.max_risk_milli > 1_000
        || request.randomization_seed.as_str().len() != 64
        || request.ticks_per_replicate == 0
        || request
            .approval_reference
            .as_ref()
            .is_some_and(|value| value.trim().is_empty())
        || request.replication_plan.model_system != request.validation_campaign.model_system
    {
        return Err(ValidationReplicationGateError::InvalidRequest(
            "local site, matching model system, bounded quality/round/risk/seed/protocol limits, and a valid campaign are required".into(),
        ));
    }
    let mut pairs = BTreeSet::new();
    for item in &request.observations {
        if item.round == 0
            || item.round > request.max_rounds
            || item.quality_milli > 1_000
            || item.observation.site_id.trim().is_empty()
            || item.observation.site_id == request.local_site_id
            || item.observation.model_system != request.replication_plan.model_system
            || !pairs.insert((
                item.observation.site_id.clone(),
                item.observation.arm_id.clone(),
            ))
        {
            return Err(ValidationReplicationGateError::InvalidRequest(
                "replication observations must be unique, bounded, paired to an independent site, and match the validated model system".into(),
            ));
        }
    }
    let mut resource_ids = BTreeSet::new();
    for resource in &request.protocol_resources {
        if resource.resource_id.trim().is_empty()
            || resource.capacity_units == 0
            || !resource_ids.insert(resource.resource_id.clone())
        {
            return Err(ValidationReplicationGateError::InvalidRequest(
                "protocol resource ids must be unique and capacities positive".into(),
            ));
        }
    }
    Ok(())
}

fn validate_output(
    output: &ValidationReplicationGate,
) -> Result<(), ValidationReplicationGateError> {
    if output.feature_id != FEATURE_ID
        || output.output_schema != OUTPUT_SCHEMA
        || output.objective.trim().is_empty()
        || output.local_site_id.trim().is_empty()
        || !canonical(&output.independent_site_order)
        || !canonical(&output.negative_evidence)
        || !canonical(&output.uncertainty)
        || output.boundary != PRECLINICAL_BOUNDARY
        || output
            .replication_plan
            .as_ref()
            .is_some_and(|plan| plan.validate().is_err())
        || output
            .continuation
            .as_ref()
            .is_some_and(|plan| plan.validate().is_err())
        || output.protocol.as_ref().is_some_and(|protocol| {
            protocol.validate().is_err()
                || output
                    .continuation
                    .as_ref()
                    .is_some_and(|continuation| protocol.continuation_digest != continuation.digest)
        })
        || output.next_action.trim().is_empty()
    {
        return Err(ValidationReplicationGateError::InvalidOutput(
            "identity, ordering, boundary, nested-analysis, protocol-binding, or next-action invariants are invalid".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(output))
        .map_err(|error| ValidationReplicationGateError::Digest(error.to_string()))?;
    if expected != output.digest {
        return Err(ValidationReplicationGateError::InvalidOutput(
            "validation-to-replication output is not content-addressed".into(),
        ));
    }
    Ok(())
}

impl ValidationReplicationGate {
    pub fn validate(&self) -> Result<(), ValidationReplicationGateError> {
        validate_output(self)
    }
}

fn finish(
    request: &ValidationReplicationGateRequest,
    independent_site_order: Vec<String>,
    validation_eligible: bool,
    replication_plan: Option<ReplicationPlan>,
    continuation: Option<ReplicationContinuationPlan>,
    protocol: Option<ReplicationProtocolCompilation>,
    disposition: ValidationReplicationGateDisposition,
    negative_evidence: Vec<String>,
    uncertainty: Vec<String>,
    next_action: &str,
) -> Result<ValidationReplicationGate, ValidationReplicationGateError> {
    let mut output = ValidationReplicationGate {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.replication_plan.objective.clone(),
        model_system: request.validation_campaign.model_system,
        validation_campaign_digest: request.validation_campaign.digest.clone(),
        local_site_id: request.local_site_id.clone(),
        independent_site_order,
        validation_eligible,
        replication_plan,
        continuation,
        protocol,
        disposition,
        negative_evidence: sorted_unique(negative_evidence),
        uncertainty: sorted_unique(uncertainty),
        next_action: next_action.into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-validation-replication-gate"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ValidationReplicationGateError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

fn validation_gate(
    campaign: &ValidationCampaignRun,
) -> (
    bool,
    ValidationReplicationGateDisposition,
    String,
    Vec<String>,
    Vec<String>,
) {
    let efficacy = campaign.disposition == ValidationCampaignDisposition::Completed
        && campaign.stop_reason == ValidationCampaignStopReason::EfficacyStop
        && campaign
            .final_power_plan
            .as_ref()
            .is_some_and(|plan| {
                plan.disposition
                    == crate::glioma::programs::p06_experiment_design::PowerReestimationDisposition::Efficacy
            });
    if efficacy {
        return (
            true,
            ValidationReplicationGateDisposition::HoldSites,
            "collect paired observations from independent preclinical sites before interpreting the local efficacy signal".into(),
            Vec::new(),
            vec![
                "local validation efficacy is necessary but not sufficient for a preclinical conclusion".into(),
                "independent-site heterogeneity and leave-one-site-out influence remain unresolved".into(),
            ],
        );
    }
    let (disposition, next_action) = match campaign.stop_reason {
        ValidationCampaignStopReason::FutilityStop => (
            ValidationReplicationGateDisposition::Negative,
            "publish the validation null or futility result and retire the replication request",
        ),
        ValidationCampaignStopReason::RiskBlocked | ValidationCampaignStopReason::BudgetBlocked => (
            ValidationReplicationGateDisposition::Blocked,
            "repair the validation risk or budget boundary before requesting replication",
        ),
        ValidationCampaignStopReason::ExecutionBlocked
        | ValidationCampaignStopReason::ProtocolBlocked
        | ValidationCampaignStopReason::Unresolved => (
            ValidationReplicationGateDisposition::Blocked,
            "resolve the incomplete validation workflow before requesting replication",
        ),
        ValidationCampaignStopReason::AwaitingMeasurements | ValidationCampaignStopReason::MaxRounds => (
            ValidationReplicationGateDisposition::HoldValidation,
            "complete a measured validation look and establish an efficacy boundary before replication",
        ),
        ValidationCampaignStopReason::EfficacyStop => (
            ValidationReplicationGateDisposition::HoldValidation,
            "repair the missing or inconsistent validation efficacy plan before replication",
        ),
    };
    (
        false,
        disposition,
        next_action.into(),
        vec![format!("validation-stop:{:?}", campaign.stop_reason)],
        vec!["an unqualified local validation result cannot enter the replication workflow".into()],
    )
}

/// Gate a completed local validation campaign into an independent-site replication workflow.
pub fn plan_glioma_validation_replication_gate(
    request: &ValidationReplicationGateRequest,
) -> Result<ValidationReplicationGate, ValidationReplicationGateError> {
    validate_request(request)?;
    let independent_site_order = sorted_unique(
        request
            .observations
            .iter()
            .map(|item| item.observation.site_id.clone()),
    );
    let (
        validation_eligible,
        initial_disposition,
        initial_next_action,
        mut negative_evidence,
        mut uncertainty,
    ) = validation_gate(&request.validation_campaign);
    if !validation_eligible {
        return finish(
            request,
            independent_site_order,
            false,
            None,
            None,
            None,
            initial_disposition,
            negative_evidence,
            uncertainty,
            &initial_next_action,
        );
    }
    if request.observations.is_empty() {
        negative_evidence.push("independent-site-observations-missing".into());
        uncertainty.push(
            "the local efficacy signal has not yet been tested outside its originating site".into(),
        );
        return finish(
            request,
            independent_site_order,
            true,
            None,
            None,
            None,
            ValidationReplicationGateDisposition::HoldSites,
            negative_evidence,
            uncertainty,
            &initial_next_action,
        );
    }

    let topology_observations = request
        .observations
        .iter()
        .map(|item| item.observation.clone())
        .collect::<Vec<_>>();
    let replication_plan = crate::glioma::programs::p06_experiment_design::plan_glioma_replication(
        &request.replication_plan,
        &topology_observations,
    )?;
    negative_evidence.extend(replication_plan.negative_evidence.clone());
    uncertainty.extend(replication_plan.uncertainty.clone());
    let continuation = plan_glioma_replication_continuation(&ReplicationContinuationRequest {
        plan_request: request.replication_plan.clone(),
        current_round: request.current_round,
        max_rounds: request.max_rounds,
        minimum_quality_milli: request.minimum_quality_milli,
        negative_effect_threshold_milli: request.negative_effect_threshold_milli,
        observations: request.observations.clone(),
        previous_plan: Some(replication_plan.clone()),
    })?;
    negative_evidence.extend(continuation.negative_evidence.clone());
    uncertainty.extend(continuation.uncertainty.clone());

    let mut protocol = None;
    let disposition = match continuation.disposition {
        ReplicationContinuationDisposition::Qualified => {
            ValidationReplicationGateDisposition::Qualified
        }
        ReplicationContinuationDisposition::Negative => {
            ValidationReplicationGateDisposition::Negative
        }
        ReplicationContinuationDisposition::Heterogeneous => {
            ValidationReplicationGateDisposition::Heterogeneous
        }
        ReplicationContinuationDisposition::Blocked => {
            ValidationReplicationGateDisposition::Blocked
        }
        ReplicationContinuationDisposition::Unresolved => {
            ValidationReplicationGateDisposition::HoldSites
        }
        ReplicationContinuationDisposition::Continue => {
            if request.protocol_resources.is_empty() {
                uncertainty.push("replication-protocol-resources-missing".into());
                ValidationReplicationGateDisposition::Blocked
            } else {
                let compiled =
                    compile_glioma_replication_protocol(&ReplicationProtocolCompileRequest {
                        continuation: continuation.clone(),
                        resources: request.protocol_resources.clone(),
                        max_ticks: request.max_ticks,
                        max_risk_milli: request.max_risk_milli,
                        allow_instrument_execution: request.allow_instrument_execution,
                        approval_reference: request.approval_reference.clone(),
                        randomization_seed: request.randomization_seed.clone(),
                        ticks_per_replicate: request.ticks_per_replicate,
                        include_quality_task: request.include_quality_task,
                    })?;
                negative_evidence.extend(compiled.negative_evidence.clone());
                uncertainty.extend(compiled.uncertainty.clone());
                let disposition = match compiled.disposition {
                    ReplicationProtocolCompilationDisposition::Compiled => {
                        ValidationReplicationGateDisposition::ReadyToExecute
                    }
                    ReplicationProtocolCompilationDisposition::Held => {
                        ValidationReplicationGateDisposition::HoldSites
                    }
                    ReplicationProtocolCompilationDisposition::Unresolved => {
                        ValidationReplicationGateDisposition::Blocked
                    }
                };
                protocol = Some(compiled);
                return finish(
                    request,
                    independent_site_order,
                    true,
                    Some(replication_plan),
                    Some(continuation),
                    protocol,
                    disposition,
                    negative_evidence,
                    uncertainty,
                    "submit only the compiled, preflighted replication wave to a caller-owned local executor",
                );
            }
        }
    };
    let next_action = match disposition {
        ValidationReplicationGateDisposition::Qualified => {
            "hold the independently replicated result for methods review and signed release evidence"
        }
        ValidationReplicationGateDisposition::Negative => {
            "publish the negative or null replication result and stop spending replication budget"
        }
        ValidationReplicationGateDisposition::Heterogeneous => {
            "inspect site, batch, model-system, and protocol causes before pooling"
        }
        ValidationReplicationGateDisposition::Blocked => {
            "repair the replication risk, budget, or protocol resource boundary before execution"
        }
        ValidationReplicationGateDisposition::HoldSites => {
            "add paired independent-site observations or resolve their quality and continuation gates"
        }
        _ => initial_next_action.as_str(),
    };
    finish(
        request,
        independent_site_order,
        true,
        Some(replication_plan),
        Some(continuation),
        protocol,
        disposition,
        negative_evidence,
        uncertainty,
        next_action,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn campaign() -> ValidationCampaignRun {
        let mut output = ValidationCampaignRun {
            feature_id: "GAF-GLIOMA-P06-F06".into(),
            output_schema: "GliomaValidationCampaign1@1".into(),
            objective: "local organoid validation".into(),
            model_system: GliomaModelSystem::Organoid,
            rounds: Vec::new(),
            final_power_plan: None,
            executed_arm_order: Vec::new(),
            withheld_arm_order: Vec::new(),
            negative_evidence: Vec::new(),
            uncertainty: Vec::new(),
            disposition: ValidationCampaignDisposition::AwaitingMeasurements,
            stop_reason: ValidationCampaignStopReason::AwaitingMeasurements,
            next_action: "attach measurements".into(),
            boundary: PRECLINICAL_BOUNDARY.into(),
            digest: ContentHash::of_bytes(b"unsealed"),
        };
        let input = serde_json::json!({
            "feature_id": output.feature_id,
            "output_schema": output.output_schema,
            "objective": output.objective,
            "model_system": output.model_system,
            "rounds": output.rounds,
            "final_power_plan": output.final_power_plan,
            "executed_arm_order": output.executed_arm_order,
            "withheld_arm_order": output.withheld_arm_order,
            "negative_evidence": output.negative_evidence,
            "uncertainty": output.uncertainty,
            "disposition": output.disposition,
            "stop_reason": output.stop_reason,
            "next_action": output.next_action,
            "boundary": output.boundary,
        });
        output.digest = ContentHash::of_value(&input).unwrap();
        output
    }

    fn request() -> ValidationReplicationGateRequest {
        ValidationReplicationGateRequest {
            validation_campaign: campaign(),
            local_site_id: "origin-site".into(),
            replication_plan: ReplicationPlanRequest {
                objective: "replicate organoid invasion effect".into(),
                model_system: GliomaModelSystem::Organoid,
                endpoint: "invasion".into(),
                control_arm_id: "control".into(),
                treatment_arm_id: "treated".into(),
                target_effect_milli: 200,
                alpha_total_milli: 100,
                power_target_milli: 700,
                min_sites: 2,
                max_sites: 4,
                min_replicates_per_site: 2,
                max_replicates_per_site: 20,
                budget_units: 100,
                max_total_replicates: 80,
                max_site_heterogeneity_milli: 200,
                risk_ceiling_milli: 500,
            },
            observations: Vec::new(),
            minimum_quality_milli: 800,
            negative_effect_threshold_milli: 50,
            current_round: 1,
            max_rounds: 4,
            protocol_resources: Vec::new(),
            max_ticks: 100,
            max_risk_milli: 500,
            allow_instrument_execution: false,
            approval_reference: None,
            randomization_seed: ContentHash::of_bytes(b"seed"),
            ticks_per_replicate: 2,
            include_quality_task: true,
        }
    }

    #[test]
    fn refuses_to_promote_unqualified_local_validation() {
        let output = plan_glioma_validation_replication_gate(&request()).unwrap();
        assert_eq!(
            output.disposition,
            ValidationReplicationGateDisposition::HoldValidation
        );
        assert!(!output.validation_eligible);
        assert!(output.replication_plan.is_none());
        assert!(output
            .negative_evidence
            .iter()
            .any(|item| item.contains("validation-stop")));
        output.validate().unwrap();
    }

    #[test]
    fn rejects_origin_site_as_independent_evidence() {
        let mut request = request();
        request
            .observations
            .push(ReplicationContinuationObservation {
                round: 1,
                quality_milli: 900,
                observation:
                    crate::glioma::programs::p06_experiment_design::ReplicationObservation {
                        site_id: "origin-site".into(),
                        arm_id: "control".into(),
                        label: "origin-control".into(),
                        artifact: crate::glioma_engine::LocalArtifactRef {
                            artifact_id: "artifact".into(),
                            content_hash: ContentHash::of_bytes(b"artifact"),
                            content_type: "application/json".into(),
                            local_only: true,
                            contains_human_data: false,
                            contains_direct_identifiers: false,
                        },
                        model_system: GliomaModelSystem::Organoid,
                        mean_response_milli: 100,
                        variance_milli2: 100,
                        observations: 2,
                        cost_units_per_replicate: 1,
                        risk_milli: 100,
                    },
            });
        let error = plan_glioma_validation_replication_gate(&request).unwrap_err();
        assert!(matches!(
            error,
            ValidationReplicationGateError::InvalidRequest(_)
        ));
    }
}

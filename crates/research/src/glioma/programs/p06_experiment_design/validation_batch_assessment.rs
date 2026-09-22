//! Close the loop from an executed validation batch to the next power-aware decision.
//!
//! This feature is intentionally more than an artifact join. It pools independent local arm
//! summaries with an integer, reproducible random-effects-free aggregate, binds every new
//! observation to an executed and still-authorized arm, advances the declared interim look, and
//! re-runs the P06 stopping/power controller. The result can therefore feed the protocol compiler
//! again while preserving efficacy, futility, risk, budget, partial-execution, and underpowered
//! states. It never treats a task artifact as a biological measurement.

use super::power_reestimation::{
    plan_glioma_power_reestimation, PowerArmObservation, PowerReestimationDisposition,
    PowerReestimationPlan, PowerReestimationRequest,
};
use crate::glioma::programs::p07_protocol_simulation::mechanism_validation_execution::MechanismValidationExecution;
use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
use bioprism_foundation::PRECLINICAL_BOUNDARY;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P06-F05";
pub const OUTPUT_SCHEMA: &str = "GliomaValidationBatchAssessment1@1";
pub const MAX_OBSERVATIONS: usize = 512;
pub const MAX_ARM_REPLICATES: u32 = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationBatchAssessmentRequest {
    pub power_request: PowerReestimationRequest,
    pub execution: MechanismValidationExecution,
    pub prior_observations: Vec<PowerArmObservation>,
    pub new_observations: Vec<PowerArmObservation>,
    pub advance_look: bool,
    pub require_complete_execution: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationBatchAssessmentDisposition {
    Evaluated,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationBatchAssessment {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub endpoint: String,
    pub source_execution_digest: ContentHash,
    pub scheduled_arm_order: Vec<String>,
    pub withheld_arm_order: Vec<String>,
    pub observed_arm_order: Vec<String>,
    pub merged_observations: Vec<PowerArmObservation>,
    pub next_power_plan: Option<PowerReestimationPlan>,
    pub disposition: ValidationBatchAssessmentDisposition,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub next_action: String,
    pub boundary: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ValidationBatchAssessmentError {
    #[error("validation batch assessment request is invalid: {0}")]
    InvalidRequest(String),
    #[error("validation batch assessment observation is invalid: {0}")]
    InvalidObservation(String),
    #[error("validation batch power re-estimation failed: {0}")]
    Power(String),
    #[error("validation batch assessment output is invalid: {0}")]
    InvalidOutput(String),
    #[error("validation batch assessment digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &ValidationBatchAssessment) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "model_system": output.model_system,
        "endpoint": output.endpoint,
        "source_execution_digest": output.source_execution_digest,
        "scheduled_arm_order": output.scheduled_arm_order,
        "withheld_arm_order": output.withheld_arm_order,
        "observed_arm_order": output.observed_arm_order,
        "merged_observations": output.merged_observations,
        "next_power_plan": output.next_power_plan,
        "disposition": output.disposition,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "next_action": output.next_action,
        "boundary": output.boundary,
    })
}

fn sorted_unique(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
}

fn validate_observation(
    observation: &PowerArmObservation,
    request: &PowerReestimationRequest,
) -> Result<(), ValidationBatchAssessmentError> {
    observation
        .artifact
        .validate()
        .map_err(|error| ValidationBatchAssessmentError::InvalidObservation(error.to_string()))?;
    if observation.arm_id.trim().is_empty()
        || observation.label.trim().is_empty()
        || observation.model_system != request.model_system
        || observation.mean_response_milli.abs() > 1_000
        || observation.variance_milli2 == 0
        || observation.observations == 0
        || observation.observations > MAX_ARM_REPLICATES
        || observation.risk_milli > 1_000
        || observation.cost_units == 0
    {
        return Err(ValidationBatchAssessmentError::InvalidObservation(format!(
            "arm {} has invalid model, response, variance, replicate, risk, or cost bounds",
            observation.arm_id
        )));
    }
    Ok(())
}

fn validate_observation_set(
    observations: &[PowerArmObservation],
    request: &PowerReestimationRequest,
    label: &str,
) -> Result<(), ValidationBatchAssessmentError> {
    if observations.len() > MAX_OBSERVATIONS {
        return Err(ValidationBatchAssessmentError::InvalidObservation(format!(
            "{label} exceeds the bounded observation limit"
        )));
    }
    let mut ids = BTreeSet::new();
    for observation in observations {
        validate_observation(observation, request)?;
        if !ids.insert(observation.arm_id.clone()) {
            return Err(ValidationBatchAssessmentError::InvalidObservation(format!(
                "{label} contains duplicate arm {}",
                observation.arm_id
            )));
        }
    }
    Ok(())
}

fn merged_artifact(
    arm_id: &str,
    label: &str,
    left: &PowerArmObservation,
    right: Option<&PowerArmObservation>,
    mean_response_milli: i32,
    variance_milli2: u64,
    observations: u32,
    risk_milli: u16,
    cost_units: u32,
) -> Result<LocalArtifactRef, ValidationBatchAssessmentError> {
    let content_hash = ContentHash::of_value(&serde_json::json!({
        "kind": "glioma-validation-pooled-arm-summary",
        "arm_id": arm_id,
        "label": label,
        "left_artifact": left.artifact,
        "right_artifact": right.map(|observation| &observation.artifact),
        "mean_response_milli": mean_response_milli,
        "variance_milli2": variance_milli2,
        "observations": observations,
        "risk_milli": risk_milli,
        "cost_units": cost_units,
    }))
    .map_err(|error| ValidationBatchAssessmentError::Digest(error.to_string()))?;
    Ok(LocalArtifactRef {
        artifact_id: format!("validation-pooled:{arm_id}"),
        content_hash,
        content_type: "application/vnd.aurora.glioma-power-pooled+json".into(),
        local_only: true,
        contains_human_data: false,
        contains_direct_identifiers: false,
    })
}

fn pool_pair(
    left: &PowerArmObservation,
    right: Option<&PowerArmObservation>,
) -> Result<PowerArmObservation, ValidationBatchAssessmentError> {
    let Some(right) = right else {
        let artifact = merged_artifact(
            &left.arm_id,
            &left.label,
            left,
            None,
            left.mean_response_milli,
            left.variance_milli2,
            left.observations,
            left.risk_milli,
            left.cost_units,
        )?;
        return Ok(PowerArmObservation {
            artifact,
            ..left.clone()
        });
    };
    if left.label != right.label {
        return Err(ValidationBatchAssessmentError::InvalidObservation(format!(
            "arm {} changed labels between validation batches",
            left.arm_id
        )));
    }
    let total_n = left
        .observations
        .checked_add(right.observations)
        .ok_or_else(|| {
            ValidationBatchAssessmentError::InvalidObservation(format!(
                "arm {} replicate count overflow",
                left.arm_id
            ))
        })?;
    if total_n > MAX_ARM_REPLICATES {
        return Err(ValidationBatchAssessmentError::InvalidObservation(format!(
            "arm {} pooled replicate count exceeds the bounded limit",
            left.arm_id
        )));
    }
    let weighted_sum = i128::from(left.mean_response_milli) * i128::from(left.observations)
        + i128::from(right.mean_response_milli) * i128::from(right.observations);
    let mean = weighted_sum / i128::from(total_n);
    if !(-1_000..=1_000).contains(&mean) {
        return Err(ValidationBatchAssessmentError::InvalidObservation(format!(
            "pooled arm {} response exceeds the bounded measurement range",
            left.arm_id
        )));
    }
    let mean_i32 = mean as i32;
    let left_delta = i128::from(left.mean_response_milli) - mean;
    let right_delta = i128::from(right.mean_response_milli) - mean;
    let sum_squares = i128::from(left.observations.saturating_sub(1))
        * i128::from(left.variance_milli2)
        + i128::from(right.observations.saturating_sub(1)) * i128::from(right.variance_milli2)
        + i128::from(left.observations) * left_delta * left_delta
        + i128::from(right.observations) * right_delta * right_delta;
    let variance = (sum_squares / i128::from(total_n.saturating_sub(1))).max(1);
    let variance_u64 = u64::try_from(variance).map_err(|_| {
        ValidationBatchAssessmentError::InvalidObservation(format!(
            "pooled arm {} variance exceeds the bounded range",
            left.arm_id
        ))
    })?;
    let cost_units = left
        .cost_units
        .checked_add(right.cost_units)
        .ok_or_else(|| {
            ValidationBatchAssessmentError::InvalidObservation(format!(
                "pooled arm {} cost overflowed",
                left.arm_id
            ))
        })?;
    let artifact = merged_artifact(
        &left.arm_id,
        &left.label,
        left,
        Some(right),
        mean_i32,
        variance_u64,
        total_n,
        left.risk_milli.max(right.risk_milli),
        cost_units,
    )?;
    Ok(PowerArmObservation {
        arm_id: left.arm_id.clone(),
        label: left.label.clone(),
        artifact,
        model_system: left.model_system,
        mean_response_milli: mean_i32,
        variance_milli2: variance_u64,
        observations: total_n,
        risk_milli: left.risk_milli.max(right.risk_milli),
        cost_units,
    })
}

fn validate_output(
    output: &ValidationBatchAssessment,
) -> Result<(), ValidationBatchAssessmentError> {
    if output.feature_id != FEATURE_ID
        || output.output_schema != OUTPUT_SCHEMA
        || output.objective.trim().is_empty()
        || output.endpoint.trim().is_empty()
        || output.boundary != PRECLINICAL_BOUNDARY
        || !canonical(&output.scheduled_arm_order)
        || !canonical(&output.withheld_arm_order)
        || !canonical(&output.observed_arm_order)
        || output
            .scheduled_arm_order
            .iter()
            .any(|arm| output.withheld_arm_order.binary_search(arm).is_ok())
        || output.observed_arm_order.len() != output.merged_observations.len()
        || output
            .merged_observations
            .iter()
            .enumerate()
            .any(|(index, observation)| {
                observation.arm_id != output.observed_arm_order[index]
                    || observation.artifact.validate().is_err()
            })
        || output
            .negative_evidence
            .iter()
            .any(|item| item.trim().is_empty())
        || output.uncertainty.iter().any(|item| item.trim().is_empty())
        || output.next_action.trim().is_empty()
        || output
            .next_power_plan
            .as_ref()
            .is_some_and(|plan| plan.validate().is_err())
    {
        return Err(ValidationBatchAssessmentError::InvalidOutput(
            "identity, binding, ordering, observation, limitation, or power-plan fields are invalid"
                .into(),
        ));
    }
    if output.disposition == ValidationBatchAssessmentDisposition::Evaluated
        && output.next_power_plan.is_none()
    {
        return Err(ValidationBatchAssessmentError::InvalidOutput(
            "evaluated assessment requires a next power plan".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(output))
        .map_err(|error| ValidationBatchAssessmentError::Digest(error.to_string()))?;
    if expected != output.digest {
        return Err(ValidationBatchAssessmentError::InvalidOutput(
            "validation batch assessment digest is not content-addressed".into(),
        ));
    }
    Ok(())
}

impl ValidationBatchAssessment {
    pub fn validate(&self) -> Result<(), ValidationBatchAssessmentError> {
        validate_output(self)
    }
}

fn finish(
    request: &ValidationBatchAssessmentRequest,
    merged_observations: Vec<PowerArmObservation>,
    next_power_plan: Option<PowerReestimationPlan>,
    disposition: ValidationBatchAssessmentDisposition,
    negative_evidence: Vec<String>,
    uncertainty: Vec<String>,
    next_action: &str,
) -> Result<ValidationBatchAssessment, ValidationBatchAssessmentError> {
    let mut output = ValidationBatchAssessment {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.power_request.objective.clone(),
        model_system: request.power_request.model_system,
        endpoint: request.power_request.endpoint.clone(),
        source_execution_digest: request.execution.digest.clone(),
        scheduled_arm_order: request.execution.scheduled_arm_order.clone(),
        withheld_arm_order: request.execution.withheld_arm_order.clone(),
        observed_arm_order: merged_observations
            .iter()
            .map(|observation| observation.arm_id.clone())
            .collect(),
        merged_observations,
        next_power_plan,
        disposition,
        negative_evidence: sorted_unique(negative_evidence),
        uncertainty: sorted_unique(uncertainty),
        next_action: next_action.into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-validation-batch-assessment"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ValidationBatchAssessmentError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

/// Pool the completed batch with prior local summaries and re-run the next P06 interim look.
pub fn assess_glioma_validation_batch(
    request: &ValidationBatchAssessmentRequest,
) -> Result<ValidationBatchAssessment, ValidationBatchAssessmentError> {
    request
        .execution
        .validate()
        .map_err(|error| ValidationBatchAssessmentError::InvalidRequest(error.to_string()))?;
    if request.power_request.objective.trim().is_empty()
        || request.power_request.endpoint.trim().is_empty()
        || request.power_request.control_arm_id.trim().is_empty()
    {
        return Err(ValidationBatchAssessmentError::InvalidRequest(
            "objective, endpoint, and control arm are required".into(),
        ));
    }
    validate_observation_set(
        &request.prior_observations,
        &request.power_request,
        "prior observations",
    )?;
    validate_observation_set(
        &request.new_observations,
        &request.power_request,
        "new observations",
    )?;
    if request.new_observations.is_empty() {
        return Err(ValidationBatchAssessmentError::InvalidObservation(
            "at least one new measured arm summary is required".into(),
        ));
    }
    let scheduled = request
        .execution
        .scheduled_arm_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let withheld = request
        .execution
        .withheld_arm_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if request.new_observations.iter().any(|observation| {
        !scheduled.contains(&observation.arm_id) || withheld.contains(&observation.arm_id)
    }) {
        return Err(ValidationBatchAssessmentError::InvalidObservation(
            "new observations must bind to scheduled, non-withheld validation arms".into(),
        ));
    }
    if request.require_complete_execution
        && request.execution.disposition
            != super::super::p07_protocol_simulation::mechanism_validation_execution::
                MechanismValidationExecutionDisposition::Completed
    {
        let mut uncertainty = request.execution.uncertainty.clone();
        uncertainty.push("execution-incomplete; scientific re-estimation withheld".into());
        return finish(
            request,
            Vec::new(),
            None,
            ValidationBatchAssessmentDisposition::Blocked,
            request.execution.negative_evidence.clone(),
            uncertainty,
            "complete or repair the executed validation batch before re-estimating power",
        );
    }
    let mut observations = BTreeMap::<String, PowerArmObservation>::new();
    for observation in &request.prior_observations {
        observations.insert(observation.arm_id.clone(), observation.clone());
    }
    for observation in &request.new_observations {
        let pooled = match observations.get(&observation.arm_id) {
            Some(previous) => pool_pair(previous, Some(observation))?,
            None => pool_pair(observation, None)?,
        };
        observations.insert(observation.arm_id.clone(), pooled);
    }
    let merged_observations = observations.into_values().collect::<Vec<_>>();
    let mut next_request = request.power_request.clone();
    if request.advance_look && next_request.current_look < next_request.max_looks {
        next_request.current_look += 1;
    }
    let mut uncertainty = request.execution.uncertainty.clone();
    if request.advance_look && request.power_request.current_look >= request.power_request.max_looks
    {
        uncertainty.push("interim-look-at-final-boundary".into());
    }
    let plan = plan_glioma_power_reestimation(&next_request, &merged_observations)
        .map_err(|error| ValidationBatchAssessmentError::Power(error.to_string()))?;
    let mut negative_evidence = request.execution.negative_evidence.clone();
    negative_evidence.extend(plan.negative_evidence.clone());
    uncertainty.extend(plan.uncertainty.clone());
    let next_action = match plan.disposition {
        PowerReestimationDisposition::Efficacy | PowerReestimationDisposition::Futility => {
            "retain the typed stopping evidence and request independent preclinical replication"
        }
        PowerReestimationDisposition::Continue | PowerReestimationDisposition::Hold => {
            "compile the next bounded validation protocol for the still-open arms"
        }
        PowerReestimationDisposition::RiskBlocked | PowerReestimationDisposition::BudgetBlocked => {
            "repair the risk or budget boundary before compiling another validation protocol"
        }
        PowerReestimationDisposition::Unresolved => {
            "resolve missing or contradictory local observations before another look"
        }
    };
    finish(
        request,
        merged_observations,
        Some(plan),
        ValidationBatchAssessmentDisposition::Evaluated,
        negative_evidence,
        uncertainty,
        next_action,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p06_experiment_design::mechanism_validation_protocol::{
        MechanismValidationProtocolCompilation, MechanismValidationProtocolDisposition,
    };
    use crate::glioma::programs::p07_protocol_simulation::execution::DryRunGliomaProtocolExecutor;
    use crate::glioma::programs::p07_protocol_simulation::mechanism_validation_execution::{
        execute_glioma_mechanism_validation_protocol, MechanismValidationExecutionRequest,
    };
    use crate::glioma::programs::p07_protocol_simulation::simulator::{
        simulate_glioma_protocol, ProtocolResource, ProtocolResourceKind,
        ProtocolSimulationRequest, ProtocolTask,
    };

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn artifact(label: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: label.into(),
            content_hash: hash(label),
            content_type: "application/json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn execution() -> MechanismValidationExecution {
        let protocol = ProtocolSimulationRequest {
            objective: "assess EGFR validation".into(),
            model_system: GliomaModelSystem::Organoid,
            tasks: vec![ProtocolTask {
                task_id: "validation:control:assay".into(),
                label: "run control".into(),
                resource_kind: ProtocolResourceKind::Culture,
                resource_units: 1,
                duration_ticks: 2,
                depends_on: Vec::new(),
                model_system: GliomaModelSystem::Organoid,
                output_schema: "GliomaMechanismValidationTask1@1".into(),
                risk_milli: 100,
                requires_instrument: false,
            }],
            resources: vec![ProtocolResource {
                resource_id: "culture".into(),
                kind: ProtocolResourceKind::Culture,
                capacity_units: 1,
            }],
            max_ticks: 10,
            max_risk_milli: 1_000,
            allow_instrument_execution: false,
            approval_reference: None,
            randomization_seed: hash("seed"),
        };
        let preflight = simulate_glioma_protocol(&protocol).unwrap();
        let mut compilation = MechanismValidationProtocolCompilation {
            feature_id: "GAF-GLIOMA-P06-F16".into(),
            output_schema: "GliomaMechanismValidationProtocol1@1".into(),
            objective: protocol.objective.clone(),
            model_system: protocol.model_system,
            validation_digest: hash("validation"),
            protocol: Some(protocol),
            preflight: Some(preflight),
            scheduled_arm_order: vec!["control".into()],
            withheld_arm_order: vec!["egfr".into()],
            task_order: vec!["validation:control:assay".into()],
            disposition: MechanismValidationProtocolDisposition::Compiled,
            negative_evidence: Vec::new(),
            uncertainty: Vec::new(),
            next_action: "execute".into(),
            boundary: PRECLINICAL_BOUNDARY.into(),
            digest: hash("unsealed"),
        };
        let digest_input = serde_json::json!({
            "feature_id": compilation.feature_id,
            "output_schema": compilation.output_schema,
            "objective": compilation.objective,
            "model_system": compilation.model_system,
            "validation_digest": compilation.validation_digest,
            "protocol": compilation.protocol,
            "preflight": compilation.preflight,
            "scheduled_arm_order": compilation.scheduled_arm_order,
            "withheld_arm_order": compilation.withheld_arm_order,
            "task_order": compilation.task_order,
            "disposition": compilation.disposition,
            "negative_evidence": compilation.negative_evidence,
            "uncertainty": compilation.uncertainty,
            "next_action": compilation.next_action,
            "boundary": compilation.boundary,
        });
        compilation.digest = ContentHash::of_value(&digest_input).unwrap();
        compilation.validate().unwrap();
        let request = MechanismValidationExecutionRequest {
            compilation,
            max_retries: 1,
            require_artifacts: true,
        };
        let mut executor = DryRunGliomaProtocolExecutor;
        execute_glioma_mechanism_validation_protocol(&request, &mut executor).unwrap()
    }

    fn power_request() -> PowerReestimationRequest {
        PowerReestimationRequest {
            objective: "assess EGFR validation".into(),
            model_system: GliomaModelSystem::Organoid,
            endpoint: "invasion".into(),
            control_arm_id: "control".into(),
            target_effect_milli: 20,
            alpha_total_milli: 100,
            power_target_milli: 500,
            current_look: 1,
            max_looks: 3,
            min_replicates_per_arm: 1,
            max_replicates_per_arm: 16,
            max_new_replicates_per_arm: 4,
            budget_units: 16,
            risk_ceiling_milli: 800,
        }
    }

    fn observation(arm_id: &str, mean: i32, n: u32) -> PowerArmObservation {
        PowerArmObservation {
            arm_id: arm_id.into(),
            label: arm_id.into(),
            artifact: artifact(&format!("artifact-{arm_id}-{n}")),
            model_system: GliomaModelSystem::Organoid,
            mean_response_milli: mean,
            variance_milli2: 100,
            observations: n,
            risk_milli: 100,
            cost_units: 1,
        }
    }

    #[test]
    fn completed_validation_batches_pool_and_advance_the_power_look() {
        let request = ValidationBatchAssessmentRequest {
            power_request: power_request(),
            execution: execution(),
            prior_observations: vec![observation("control", 100, 2), observation("egfr", 110, 2)],
            new_observations: vec![observation("control", 102, 2)],
            advance_look: true,
            require_complete_execution: true,
        };
        let output = assess_glioma_validation_batch(&request).unwrap();
        assert_eq!(
            output.disposition,
            ValidationBatchAssessmentDisposition::Evaluated
        );
        assert_eq!(output.next_power_plan.as_ref().unwrap().current_look, 2);
        assert_eq!(output.merged_observations[0].observations, 4);
        assert_eq!(output.observed_arm_order, vec!["control", "egfr"]);
        output.validate().unwrap();
    }

    #[test]
    fn withheld_arm_measurement_is_rejected_before_reestimation() {
        let request = ValidationBatchAssessmentRequest {
            power_request: power_request(),
            execution: execution(),
            prior_observations: vec![observation("control", 100, 2)],
            new_observations: vec![observation("egfr", 200, 2)],
            advance_look: true,
            require_complete_execution: true,
        };
        let error = assess_glioma_validation_batch(&request).unwrap_err();
        assert!(error.to_string().contains("scheduled, non-withheld"));
    }
}

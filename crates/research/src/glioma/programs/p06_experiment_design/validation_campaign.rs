//! Closed-loop orchestration for glioma mechanism validation.
//!
//! The campaign composes the P05 robust portfolio, P06 sequential power controller, P06-to-P07
//! protocol compiler, the local protocol executor, and measured batch assessment into one bounded
//! replayable loop. It can run a dry-run protocol and then wait for measured arm summaries, or
//! consume caller-supplied observations for the next interim look. No synthetic task artifact is
//! promoted to a biological observation and no stop decision is converted into a clinical action.

use super::mechanism_validation::{
    plan_glioma_mechanism_validation, MechanismValidationPlan, MechanismValidationPlanRequest,
};
use super::mechanism_validation_protocol::{
    compile_glioma_mechanism_validation_protocol, MechanismValidationProtocolCompilation,
    MechanismValidationProtocolCompileRequest,
};
use super::power_reestimation::{
    PowerArmObservation, PowerReestimationDisposition, PowerReestimationPlan,
};
use super::validation_batch_assessment::{
    assess_glioma_validation_batch, ValidationBatchAssessment, ValidationBatchAssessmentError,
    ValidationBatchAssessmentRequest,
};
use crate::glioma::programs::p07_protocol_simulation::execution::GliomaProtocolExecutor;
use crate::glioma::programs::p07_protocol_simulation::mechanism_validation_execution::{
    execute_glioma_mechanism_validation_protocol, MechanismValidationExecution,
    MechanismValidationExecutionDisposition, MechanismValidationExecutionError,
    MechanismValidationExecutionRequest,
};
use crate::glioma::programs::p07_protocol_simulation::simulator::ProtocolResource;
use crate::glioma_engine::GliomaModelSystem;
use bioprism_foundation::PRECLINICAL_BOUNDARY;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P06-F06";
pub const OUTPUT_SCHEMA: &str = "GliomaValidationCampaign1@1";
pub const MAX_ROUNDS: u16 = 32;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationCampaignRequest {
    pub validation: MechanismValidationPlanRequest,
    pub resources: Vec<ProtocolResource>,
    pub max_ticks: u32,
    pub max_risk_milli: u16,
    pub allow_instrument_execution: bool,
    pub approval_reference: Option<String>,
    pub randomization_seed: ContentHash,
    pub ticks_per_replicate: u32,
    pub include_quality_task: bool,
    pub max_retries: u8,
    pub require_artifacts: bool,
    pub max_rounds: u16,
    pub observation_batches: Vec<Vec<PowerArmObservation>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationCampaignDisposition {
    Completed,
    AwaitingMeasurements,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationCampaignStopReason {
    EfficacyStop,
    FutilityStop,
    RiskBlocked,
    BudgetBlocked,
    AwaitingMeasurements,
    ExecutionBlocked,
    ProtocolBlocked,
    MaxRounds,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationCampaignRound {
    pub round: u16,
    pub validation: MechanismValidationPlan,
    pub compilation: MechanismValidationProtocolCompilation,
    pub execution: MechanismValidationExecution,
    pub new_observations: Vec<PowerArmObservation>,
    pub assessment: Option<ValidationBatchAssessment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationCampaignRun {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub rounds: Vec<ValidationCampaignRound>,
    pub final_power_plan: Option<PowerReestimationPlan>,
    pub executed_arm_order: Vec<String>,
    pub withheld_arm_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: ValidationCampaignDisposition,
    pub stop_reason: ValidationCampaignStopReason,
    pub next_action: String,
    pub boundary: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ValidationCampaignError {
    #[error("validation campaign request is invalid: {0}")]
    InvalidRequest(String),
    #[error("validation campaign planning failed: {0}")]
    Plan(String),
    #[error("validation campaign protocol compilation failed: {0}")]
    Compile(String),
    #[error("validation campaign execution failed: {0}")]
    Execution(String),
    #[error("validation campaign batch assessment failed: {0}")]
    Assessment(String),
    #[error("validation campaign output is invalid: {0}")]
    InvalidOutput(String),
    #[error("validation campaign digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn sorted_unique(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
}

fn digest_input(output: &ValidationCampaignRun) -> serde_json::Value {
    serde_json::json!({
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
    })
}

fn validate_request(request: &ValidationCampaignRequest) -> Result<(), ValidationCampaignError> {
    if request.max_rounds == 0
        || request.max_rounds > MAX_ROUNDS
        || request.max_ticks == 0
        || request.max_risk_milli > 1_000
        || request.randomization_seed.as_str().len() != 64
        || request.ticks_per_replicate == 0
        || request.resources.is_empty()
        || request.max_retries > super::super::p07_protocol_simulation::execution::MAX_RETRIES
        || request
            .approval_reference
            .as_ref()
            .is_some_and(|value| value.trim().is_empty())
        || request.observation_batches.len() > usize::from(request.max_rounds)
    {
        return Err(ValidationCampaignError::InvalidRequest(
            "bounded rounds, horizon, risk, seed, resources, retry, approval, and observation-batch limits are required".into(),
        ));
    }
    let mut resource_ids = BTreeSet::new();
    for resource in &request.resources {
        if resource.resource_id.trim().is_empty()
            || resource.capacity_units == 0
            || !resource_ids.insert(resource.resource_id.clone())
        {
            return Err(ValidationCampaignError::InvalidRequest(
                "resource ids must be unique and capacities positive".into(),
            ));
        }
    }
    Ok(())
}

fn validate_output(output: &ValidationCampaignRun) -> Result<(), ValidationCampaignError> {
    if output.feature_id != FEATURE_ID
        || output.output_schema != OUTPUT_SCHEMA
        || output.objective.trim().is_empty()
        || output.boundary != PRECLINICAL_BOUNDARY
        || output.rounds.len() > usize::from(MAX_ROUNDS)
        || !canonical(&output.executed_arm_order)
        || !canonical(&output.withheld_arm_order)
        || !canonical(&output.negative_evidence)
        || !canonical(&output.uncertainty)
        || output
            .executed_arm_order
            .iter()
            .any(|arm| output.withheld_arm_order.binary_search(arm).is_ok())
        || output.rounds.iter().enumerate().any(|(index, round)| {
            round.round != (index as u16 + 1)
                || round.validation.validate().is_err()
                || round.compilation.validate().is_err()
                || round.execution.validate().is_err()
                || round.execution.compilation_digest != round.compilation.digest
                || round
                    .assessment
                    .as_ref()
                    .is_some_and(|assessment| assessment.validate().is_err())
        })
        || output
            .final_power_plan
            .as_ref()
            .is_some_and(|plan| plan.validate().is_err())
        || output.next_action.trim().is_empty()
    {
        return Err(ValidationCampaignError::InvalidOutput(
            "identity, round binding, ordering, stopping, or output fields are invalid".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(output))
        .map_err(|error| ValidationCampaignError::Digest(error.to_string()))?;
    if expected != output.digest {
        return Err(ValidationCampaignError::InvalidOutput(
            "validation campaign output is not content-addressed".into(),
        ));
    }
    Ok(())
}

impl ValidationCampaignRun {
    pub fn validate(&self) -> Result<(), ValidationCampaignError> {
        validate_output(self)
    }
}

fn finish(
    request: &ValidationCampaignRequest,
    rounds: Vec<ValidationCampaignRound>,
    final_power_plan: Option<PowerReestimationPlan>,
    executed_arm_order: Vec<String>,
    withheld_arm_order: Vec<String>,
    negative_evidence: Vec<String>,
    uncertainty: Vec<String>,
    disposition: ValidationCampaignDisposition,
    stop_reason: ValidationCampaignStopReason,
    next_action: &str,
) -> Result<ValidationCampaignRun, ValidationCampaignError> {
    let mut output = ValidationCampaignRun {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.validation.objective.clone(),
        model_system: request.validation.power.model_system,
        rounds,
        final_power_plan,
        executed_arm_order: sorted_unique(executed_arm_order),
        withheld_arm_order: sorted_unique(withheld_arm_order),
        negative_evidence: sorted_unique(negative_evidence),
        uncertainty: sorted_unique(uncertainty),
        disposition,
        stop_reason,
        next_action: next_action.into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-validation-campaign"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ValidationCampaignError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

/// Run a bounded, observation-driven mechanism-validation campaign through a caller-owned local
/// protocol executor.
pub fn execute_glioma_validation_campaign<E: GliomaProtocolExecutor>(
    request: &ValidationCampaignRequest,
    executor: &mut E,
) -> Result<ValidationCampaignRun, ValidationCampaignError> {
    validate_request(request)?;
    let mut state = request.validation.clone();
    let mut rounds = Vec::new();
    let mut final_power_plan = None;
    let mut executed_arm_order = Vec::new();
    let mut withheld_arm_order = Vec::new();
    let mut negative_evidence = Vec::new();
    let mut uncertainty = Vec::new();
    let mut disposition = ValidationCampaignDisposition::Partial;
    let mut stop_reason = ValidationCampaignStopReason::MaxRounds;
    let mut next_action = "campaign reached its declared round bound";

    for round_number in 1..=request.max_rounds {
        let validation = plan_glioma_mechanism_validation(&state)
            .map_err(|error| ValidationCampaignError::Plan(error.to_string()))?;
        negative_evidence.extend(validation.negative_evidence.clone());
        uncertainty.extend(validation.uncertainty.clone());
        let compilation_request = MechanismValidationProtocolCompileRequest {
            validation: validation.clone(),
            arms: state.arms.clone(),
            resources: request.resources.clone(),
            max_ticks: request.max_ticks,
            max_risk_milli: request.max_risk_milli,
            allow_instrument_execution: request.allow_instrument_execution,
            approval_reference: request.approval_reference.clone(),
            randomization_seed: request.randomization_seed.clone(),
            ticks_per_replicate: request.ticks_per_replicate,
            include_quality_task: request.include_quality_task,
        };
        let compilation = compile_glioma_mechanism_validation_protocol(&compilation_request)
            .map_err(|error| ValidationCampaignError::Compile(error.to_string()))?;
        negative_evidence.extend(compilation.negative_evidence.clone());
        uncertainty.extend(compilation.uncertainty.clone());
        executed_arm_order.extend(compilation.scheduled_arm_order.clone());
        withheld_arm_order.extend(compilation.withheld_arm_order.clone());
        let execution_request = MechanismValidationExecutionRequest {
            compilation: compilation.clone(),
            max_retries: request.max_retries,
            require_artifacts: request.require_artifacts,
        };
        let execution = execute_glioma_mechanism_validation_protocol(&execution_request, executor)
            .map_err(|error: MechanismValidationExecutionError| {
                ValidationCampaignError::Execution(error.to_string())
            })?;
        negative_evidence.extend(execution.negative_evidence.clone());
        uncertainty.extend(execution.uncertainty.clone());
        let new_observations = request
            .observation_batches
            .get(usize::from(round_number - 1))
            .cloned()
            .unwrap_or_default();
        let mut assessment = None;
        if execution.disposition != MechanismValidationExecutionDisposition::Completed {
            disposition = ValidationCampaignDisposition::Blocked;
            stop_reason = ValidationCampaignStopReason::ExecutionBlocked;
            next_action = "repair the protocol or local executor before collecting another batch";
        } else if new_observations.is_empty() {
            disposition = ValidationCampaignDisposition::AwaitingMeasurements;
            stop_reason = ValidationCampaignStopReason::AwaitingMeasurements;
            next_action = "attach measured arm summaries before advancing the next interim look";
        } else {
            let assessment_request = ValidationBatchAssessmentRequest {
                power_request: state.power.clone(),
                execution: execution.clone(),
                prior_observations: state.observations.clone(),
                new_observations: new_observations.clone(),
                advance_look: true,
                require_complete_execution: true,
            };
            let assessed = assess_glioma_validation_batch(&assessment_request).map_err(
                |error: ValidationBatchAssessmentError| {
                    ValidationCampaignError::Assessment(error.to_string())
                },
            )?;
            negative_evidence.extend(assessed.negative_evidence.clone());
            uncertainty.extend(assessed.uncertainty.clone());
            if let Some(power_plan) = assessed.next_power_plan.clone() {
                final_power_plan = Some(power_plan.clone());
                state.power.current_look = power_plan.current_look;
                state.observations = assessed.merged_observations.clone();
                match power_plan.disposition {
                    PowerReestimationDisposition::Efficacy => {
                        disposition = ValidationCampaignDisposition::Completed;
                        stop_reason = ValidationCampaignStopReason::EfficacyStop;
                        next_action = "preserve the efficacy boundary and request independent preclinical replication";
                    }
                    PowerReestimationDisposition::Futility => {
                        disposition = ValidationCampaignDisposition::Completed;
                        stop_reason = ValidationCampaignStopReason::FutilityStop;
                        next_action =
                            "preserve the futility evidence and retire the weak mechanism arm";
                    }
                    PowerReestimationDisposition::RiskBlocked => {
                        disposition = ValidationCampaignDisposition::Blocked;
                        stop_reason = ValidationCampaignStopReason::RiskBlocked;
                        next_action =
                            "repair the declared risk boundary before another validation look";
                    }
                    PowerReestimationDisposition::BudgetBlocked => {
                        disposition = ValidationCampaignDisposition::Blocked;
                        stop_reason = ValidationCampaignStopReason::BudgetBlocked;
                        next_action =
                            "repair the declared budget boundary before another validation look";
                    }
                    PowerReestimationDisposition::Continue | PowerReestimationDisposition::Hold => {
                        disposition = ValidationCampaignDisposition::Partial;
                        stop_reason = ValidationCampaignStopReason::MaxRounds;
                        next_action = "compile and execute another bounded look while the mechanism remains open";
                    }
                    PowerReestimationDisposition::Unresolved => {
                        disposition = ValidationCampaignDisposition::Blocked;
                        stop_reason = ValidationCampaignStopReason::Unresolved;
                        next_action =
                            "resolve missing or contradictory observations before another look";
                    }
                }
            } else {
                disposition = ValidationCampaignDisposition::Blocked;
                stop_reason = ValidationCampaignStopReason::Unresolved;
                next_action = "resolve the assessment boundary before another validation look";
            }
            assessment = Some(assessed);
        }
        rounds.push(ValidationCampaignRound {
            round: round_number,
            validation,
            compilation,
            execution,
            new_observations,
            assessment,
        });
        if !matches!(disposition, ValidationCampaignDisposition::Partial)
            || round_number == request.max_rounds
        {
            break;
        }
    }
    finish(
        request,
        rounds,
        final_power_plan,
        executed_arm_order,
        withheld_arm_order,
        negative_evidence,
        uncertainty,
        disposition,
        stop_reason,
        next_action,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p05_mechanism_exploration::counterfactual::CounterfactualIntervention;
    use crate::glioma::programs::p05_mechanism_exploration::ensemble_counterfactual::CounterfactualModel;
    use crate::glioma::programs::p05_mechanism_exploration::graph_propagation::{
        MechanismGraphEdge, MechanismGraphNode, MechanismGraphRelation,
    };
    use crate::glioma::programs::p05_mechanism_exploration::robust_portfolio::{
        plan_glioma_robust_intervention_portfolio, PortfolioDirection, RobustInterventionCandidate,
        RobustInterventionRequest,
    };
    use crate::glioma::programs::p07_protocol_simulation::execution::DryRunGliomaProtocolExecutor;
    use crate::glioma_engine::LocalArtifactRef;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: id.into(),
            content_hash: hash(id),
            content_type: "application/json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn request() -> ValidationCampaignRequest {
        let node = |id: &str, modality| MechanismGraphNode {
            node_id: id.into(),
            label: id.into(),
            modality,
            prior_milli: 0,
            support_milli: 800,
            contradiction_milli: 0,
        };
        let model = |id: &str| CounterfactualModel {
            model_id: id.into(),
            prior_milli: 500,
            nodes: vec![
                node("egfr", crate::glioma_engine::GliomaModality::Genomics),
                node("invasion", crate::glioma_engine::GliomaModality::Imaging),
            ],
            edges: vec![MechanismGraphEdge {
                edge_id: format!("{id}-edge"),
                source_node_id: "egfr".into(),
                target_node_id: "invasion".into(),
                relation: MechanismGraphRelation::Activates,
                confidence_milli: 900,
                evidence_order: vec![format!("evidence-{id}")],
            }],
        };
        let portfolio = plan_glioma_robust_intervention_portfolio(
            &RobustInterventionRequest {
                objective: "run an EGFR validation campaign".into(),
                model_system: GliomaModelSystem::Organoid,
                max_iterations: 100,
                convergence_tolerance_milli: 1,
                damping_milli: 600,
                min_edge_confidence_milli: 500,
                direction: PortfolioDirection::Decrease,
                budget_units: 3,
                max_selected: 1,
                min_robust_effect_milli: 10,
                min_agreement_milli: 750,
                risk_ceiling_milli: 800,
                effect_weight_milli: 500,
                tail_weight_milli: 300,
                worst_case_weight_milli: 200,
                feasibility_weight_milli: 1_000,
                risk_penalty_milli: 1,
            },
            &[model("a"), model("b")],
            &[RobustInterventionCandidate {
                candidate_id: "egfr-invasion".into(),
                label: "EGFR perturbation".into(),
                intervention: CounterfactualIntervention {
                    intervention_id: "egfr-intervention".into(),
                    node_id: "egfr".into(),
                    delta_milli: -600,
                    rationale: "test EGFR invasion mechanism".into(),
                    evidence_order: vec!["evidence-egfr".into()],
                },
                target_node_id: "invasion".into(),
                redundancy_group: "egfr".into(),
                feasibility_milli: 1_000,
                cost_units: 1,
                risk_milli: 100,
            }],
        )
        .unwrap();
        let control = PowerArmObservation {
            arm_id: "control".into(),
            label: "control".into(),
            artifact: artifact("control-observation"),
            model_system: GliomaModelSystem::Organoid,
            mean_response_milli: 100,
            variance_milli2: 100,
            observations: 2,
            risk_milli: 100,
            cost_units: 1,
        };
        let egfr = PowerArmObservation {
            arm_id: "egfr-arm".into(),
            label: "EGFR".into(),
            artifact: artifact("egfr-observation"),
            model_system: GliomaModelSystem::Organoid,
            mean_response_milli: 110,
            variance_milli2: 100,
            observations: 2,
            risk_milli: 100,
            cost_units: 1,
        };
        ValidationCampaignRequest {
            validation: MechanismValidationPlanRequest {
                objective: "run an EGFR validation campaign".into(),
                portfolio,
                power: super::super::power_reestimation::PowerReestimationRequest {
                    objective: "run an EGFR validation campaign".into(),
                    model_system: GliomaModelSystem::Organoid,
                    endpoint: "invasion".into(),
                    control_arm_id: "control".into(),
                    target_effect_milli: 20,
                    alpha_total_milli: 100,
                    power_target_milli: 500,
                    current_look: 1,
                    max_looks: 2,
                    min_replicates_per_arm: 1,
                    max_replicates_per_arm: 16,
                    max_new_replicates_per_arm: 4,
                    budget_units: 16,
                    risk_ceiling_milli: 800,
                },
                arms: vec![
                    super::super::mechanism_validation::MechanismValidationArm {
                        arm_id: "control".into(),
                        candidate_id: None,
                        label: "control".into(),
                        target_node_id: "invasion".into(),
                        protocol_id: "validation-v1".into(),
                        role: super::super::mechanism_validation::ValidationArmRole::Control,
                        artifact: artifact("control-arm"),
                    },
                    super::super::mechanism_validation::MechanismValidationArm {
                        arm_id: "egfr-arm".into(),
                        candidate_id: Some("egfr-invasion".into()),
                        label: "EGFR".into(),
                        target_node_id: "invasion".into(),
                        protocol_id: "validation-v1".into(),
                        role: super::super::mechanism_validation::ValidationArmRole::Intervention,
                        artifact: artifact("egfr-arm"),
                    },
                ],
                observations: vec![control.clone(), egfr],
                require_qualified_portfolio: true,
            },
            resources: vec![
                ProtocolResource {
                    resource_id: "culture".into(),
                    kind: super::super::super::p07_protocol_simulation::simulator::
                        ProtocolResourceKind::Culture,
                    capacity_units: 1,
                },
                ProtocolResource {
                    resource_id: "compute".into(),
                    kind: super::super::super::p07_protocol_simulation::simulator::
                        ProtocolResourceKind::Compute,
                    capacity_units: 1,
                },
            ],
            max_ticks: 100,
            max_risk_milli: 1_000,
            allow_instrument_execution: false,
            approval_reference: None,
            randomization_seed: hash("campaign-seed"),
            ticks_per_replicate: 2,
            include_quality_task: true,
            max_retries: 1,
            require_artifacts: true,
            max_rounds: 1,
            observation_batches: vec![vec![PowerArmObservation {
                arm_id: "control".into(),
                label: "control".into(),
                artifact: artifact("control-new"),
                model_system: GliomaModelSystem::Organoid,
                mean_response_milli: 101,
                variance_milli2: 100,
                observations: 2,
                risk_milli: 100,
                cost_units: 1,
            }]],
        }
    }

    #[test]
    fn campaign_executes_and_returns_a_next_look() {
        let request = request();
        let mut executor = DryRunGliomaProtocolExecutor;
        let first = execute_glioma_validation_campaign(&request, &mut executor).unwrap();
        let second = execute_glioma_validation_campaign(&request, &mut executor).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.rounds.len(), 1);
        assert!(first.rounds[0].assessment.is_some());
        assert_eq!(first.final_power_plan.as_ref().unwrap().current_look, 2);
        first.validate().unwrap();
    }
}

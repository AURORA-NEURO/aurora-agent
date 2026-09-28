//! Compile completed phase-resolved glioma image captures into a reproducible P09 workflow.
//!
//! This is the P08-to-P09 research seam: it consumes only action-aligned, local, de-identified
//! capture artifacts and expands them into a typed imaging computation DAG. It never interprets
//! pixels, calls a worker, or turns a dry-run artifact into biological evidence. Incomplete,
//! negative, or unresolved acquisition remains a first-class handoff state instead of becoming an
//! empty input list or a false computation-ready claim.

use super::execution::ComputationOperation;
use super::workflow::{
    compile_glioma_computation_workflow, GliomaComputationWorkflow, GliomaComputationWorkflowError,
    GliomaComputationWorkflowRequest,
};
use crate::glioma::programs::p08_instrument_robotics::execution::InstrumentExecutionRun;
use crate::glioma::programs::p08_instrument_robotics::phase_resolved_invasion_schedule::PhaseResolvedInvasionActionPlan;
use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P09-F27";
pub const OUTPUT_SCHEMA: &str = "GliomaPhaseResolvedImagingHandoff1@1";
pub const MAX_CAPTURE_ARTIFACTS: usize = 1_024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PhaseResolvedHandoffExecutionMode {
    DryRun,
    LocalGateway,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PhaseResolvedImagingHandoffDisposition {
    ReadyForComputation,
    SimulationOnly,
    Blocked,
    Negative,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhaseResolvedImagingHandoffRequest {
    pub objective: String,
    pub action_plan: PhaseResolvedInvasionActionPlan,
    pub execution: InstrumentExecutionRun,
    pub execution_mode: PhaseResolvedHandoffExecutionMode,
    pub budget_units: u64,
    pub duration_ticks: u64,
    pub max_tasks: usize,
    pub replay_identity: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhaseResolvedImagingComputationHandoff {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub instrument_id: String,
    pub replay_identity: ContentHash,
    pub input_artifact_order: Vec<String>,
    pub workflow_request: Option<GliomaComputationWorkflowRequest>,
    pub workflow: Option<GliomaComputationWorkflow>,
    pub simulation_only: bool,
    pub disposition: PhaseResolvedImagingHandoffDisposition,
    pub next_operator_action: String,
    pub uncertainty: Vec<String>,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PhaseResolvedImagingHandoffError {
    #[error("phase-resolved imaging handoff request is invalid: {0}")]
    InvalidRequest(String),
    #[error("phase-resolved imaging acquisition is not ready for computation: {0}")]
    Acquisition(String),
    #[error("phase-resolved computation workflow failed: {0}")]
    Workflow(#[from] GliomaComputationWorkflowError),
    #[error("phase-resolved imaging handoff output is invalid: {0}")]
    InvalidOutput(String),
    #[error("phase-resolved imaging handoff digest failed: {0}")]
    Digest(String),
}

fn valid_identifier(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:-".contains(&byte))
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &PhaseResolvedImagingComputationHandoff) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "study_id": output.study_id,
        "model_system": output.model_system,
        "instrument_id": output.instrument_id,
        "replay_identity": output.replay_identity,
        "input_artifact_order": output.input_artifact_order,
        "workflow_request": output.workflow_request,
        "workflow": output.workflow,
        "simulation_only": output.simulation_only,
        "disposition": output.disposition,
        "next_operator_action": output.next_operator_action,
        "uncertainty": output.uncertainty,
    })
}

fn validate_request(
    request: &PhaseResolvedImagingHandoffRequest,
) -> Result<(), PhaseResolvedImagingHandoffError> {
    if request.objective.trim().is_empty()
        || request.budget_units == 0
        || request.duration_ticks == 0
        || request.max_tasks == 0
        || request.replay_identity.as_str().len() != 64
    {
        return Err(PhaseResolvedImagingHandoffError::InvalidRequest(
            "objective, resource bounds, and a content-addressed replay identity are required"
                .into(),
        ));
    }
    request.action_plan.verify_digest().map_err(|error| {
        PhaseResolvedImagingHandoffError::InvalidRequest(format!(
            "phase-resolved action plan is invalid: {error}"
        ))
    })?;
    request.execution.validate().map_err(|error| {
        PhaseResolvedImagingHandoffError::InvalidRequest(format!(
            "instrument execution run is invalid: {error}"
        ))
    })?;
    if request.execution.objective != request.objective
        || request.execution.instrument_id != request.action_plan.instrument_id
        || request.execution.action_order != request.action_plan.action_order
    {
        return Err(PhaseResolvedImagingHandoffError::InvalidRequest(
            "objective, instrument, and action identities do not reconcile across P08 outputs"
                .into(),
        ));
    }
    Ok(())
}

fn completed_artifacts(
    request: &PhaseResolvedImagingHandoffRequest,
) -> Result<Vec<String>, PhaseResolvedImagingHandoffError> {
    if request.execution.results.len() > MAX_CAPTURE_ARTIFACTS {
        return Err(PhaseResolvedImagingHandoffError::InvalidRequest(
            "capture artifact count exceeds the bounded handoff limit".into(),
        ));
    }
    let mut result_ids = BTreeSet::new();
    let mut artifacts = BTreeSet::new();
    for result in &request.execution.results {
        if !result_ids.insert(result.action_id.clone()) {
            return Err(PhaseResolvedImagingHandoffError::InvalidRequest(
                "execution contains duplicate action results".into(),
            ));
        }
        if let Some(artifact) = &result.artifact {
            artifact.validate().map_err(|error| {
                PhaseResolvedImagingHandoffError::InvalidRequest(format!(
                    "capture artifact {} is not a local de-identified artifact: {error}",
                    artifact.artifact_id
                ))
            })?;
            if !valid_identifier(&artifact.artifact_id)
                || !artifacts.insert(artifact.artifact_id.clone())
            {
                return Err(PhaseResolvedImagingHandoffError::InvalidRequest(
                    "capture artifact identities must be unique and path-safe".into(),
                ));
            }
        }
    }
    if result_ids
        != request
            .action_plan
            .action_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>()
    {
        return Err(PhaseResolvedImagingHandoffError::InvalidRequest(
            "execution results do not cover the phase-resolved action plan".into(),
        ));
    }
    Ok(artifacts.into_iter().collect())
}

fn incomplete_disposition(
    execution: &InstrumentExecutionRun,
) -> PhaseResolvedImagingHandoffDisposition {
    if !execution.negative_order.is_empty() {
        PhaseResolvedImagingHandoffDisposition::Negative
    } else if !execution.unresolved_order.is_empty() {
        PhaseResolvedImagingHandoffDisposition::Unresolved
    } else {
        PhaseResolvedImagingHandoffDisposition::Blocked
    }
}

fn seal(
    mut output: PhaseResolvedImagingComputationHandoff,
) -> Result<PhaseResolvedImagingComputationHandoff, PhaseResolvedImagingHandoffError> {
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| PhaseResolvedImagingHandoffError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

impl PhaseResolvedImagingComputationHandoff {
    pub fn validate(&self) -> Result<(), PhaseResolvedImagingHandoffError> {
        let disposition_mode_valid = match (self.workflow.is_some(), self.disposition) {
            (true, PhaseResolvedImagingHandoffDisposition::ReadyForComputation) => {
                !self.simulation_only
            }
            (true, PhaseResolvedImagingHandoffDisposition::SimulationOnly) => self.simulation_only,
            (true, PhaseResolvedImagingHandoffDisposition::Blocked) => true,
            (false, PhaseResolvedImagingHandoffDisposition::Blocked)
            | (false, PhaseResolvedImagingHandoffDisposition::Negative)
            | (false, PhaseResolvedImagingHandoffDisposition::Unresolved) => true,
            _ => false,
        };
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || !valid_identifier(&self.study_id)
            || self.instrument_id.trim().is_empty()
            || self.replay_identity.as_str().len() != 64
            || !canonical(&self.input_artifact_order)
            || self
                .input_artifact_order
                .iter()
                .any(|id| !valid_identifier(id))
            || !canonical(&self.uncertainty)
            || self.next_operator_action.trim().is_empty()
            || !disposition_mode_valid
        {
            return Err(PhaseResolvedImagingHandoffError::InvalidOutput(
                "handoff identity, ordering, resource, or operator fields are invalid".into(),
            ));
        }
        match (&self.workflow_request, &self.workflow) {
            (Some(request), Some(workflow)) => {
                workflow.validate().map_err(|error| {
                    PhaseResolvedImagingHandoffError::InvalidOutput(error.to_string())
                })?;
                if request.objective != self.objective
                    || request.study_id != self.study_id
                    || request.model_system != self.model_system
                    || request.input_artifact_ids != self.input_artifact_order
                    || request.replay_identity != self.replay_identity
                    || workflow.objective != self.objective
                    || workflow.study_id != self.study_id
                    || workflow.input_artifact_ids != self.input_artifact_order
                {
                    return Err(PhaseResolvedImagingHandoffError::InvalidOutput(
                        "workflow request, workflow, and capture artifact identities do not reconcile"
                            .into(),
                    ));
                }
            }
            (None, None)
                if matches!(
                    self.disposition,
                    PhaseResolvedImagingHandoffDisposition::Blocked
                        | PhaseResolvedImagingHandoffDisposition::Negative
                        | PhaseResolvedImagingHandoffDisposition::Unresolved
                ) => {}
            _ => {
                return Err(PhaseResolvedImagingHandoffError::InvalidOutput(
                    "workflow presence must match the handoff disposition".into(),
                ));
            }
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| PhaseResolvedImagingHandoffError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(PhaseResolvedImagingHandoffError::InvalidOutput(
                "handoff digest does not cover its inputs, workflow, and operator state".into(),
            ));
        }
        Ok(())
    }

    pub fn verify_digest(&self) -> Result<(), PhaseResolvedImagingHandoffError> {
        self.validate()
    }
}

/// Compile completed phase-resolved captures into the standard P09 imaging workflow.
pub fn compile_glioma_phase_resolved_imaging_computation_handoff(
    request: &PhaseResolvedImagingHandoffRequest,
) -> Result<PhaseResolvedImagingComputationHandoff, PhaseResolvedImagingHandoffError> {
    validate_request(request)?;
    let artifacts = completed_artifacts(request)?;
    let simulation_only = matches!(
        request.execution_mode,
        PhaseResolvedHandoffExecutionMode::DryRun
    );
    let all_completed = request.execution.results.iter().all(|result| {
        result.disposition == super::super::p08_instrument_robotics::execution::InstrumentExecutionDisposition::Completed
            && result.artifact.is_some()
    });
    if !all_completed {
        let disposition = incomplete_disposition(&request.execution);
        return seal(PhaseResolvedImagingComputationHandoff {
            feature_id: FEATURE_ID.into(),
            output_schema: OUTPUT_SCHEMA.into(),
            objective: request.objective.clone(),
            study_id: request.action_plan.study_id.clone(),
            model_system: request.action_plan.model_system,
            instrument_id: request.action_plan.instrument_id.clone(),
            replay_identity: request.replay_identity.clone(),
            input_artifact_order: artifacts,
            workflow_request: None,
            workflow: None,
            simulation_only,
            disposition,
            next_operator_action:
                "repair or explicitly review the incomplete instrument run before computation"
                    .into(),
            uncertainty: vec!["capture execution did not produce a complete artifact set".into()],
            digest: ContentHash::of_bytes(b"unsealed-phase-resolved-imaging-handoff"),
        });
    }

    let workflow_request = GliomaComputationWorkflowRequest {
        objective: request.objective.clone(),
        study_id: request.action_plan.study_id.clone(),
        model_system: request.action_plan.model_system,
        modalities: vec![
            GliomaModality::Instrument,
            GliomaModality::Imaging,
            GliomaModality::OrganoidAssay,
        ],
        operations: vec![
            ComputationOperation::Ingest,
            ComputationOperation::Normalize,
            ComputationOperation::Register,
            ComputationOperation::Segment,
            ComputationOperation::Quantify,
            ComputationOperation::Integrate,
            ComputationOperation::Validate,
            ComputationOperation::Export,
        ],
        input_artifact_ids: artifacts.clone(),
        budget_units: request.budget_units,
        duration_ticks: request.duration_ticks,
        max_tasks: request.max_tasks,
        max_modalities: 4,
        min_modalities: 2,
        information_weight_milli: 8,
        uncertainty_weight_milli: 7,
        coverage_weight_milli: 6,
        cost_penalty_milli: 2,
        duration_penalty_milli: 2,
        require_deterministic: true,
        max_rounds: 1,
        max_retries: 1,
        allow_cache: true,
        require_local_artifacts: true,
        cache: Vec::new(),
        replay_identity: request.replay_identity.clone(),
    };
    let workflow = compile_glioma_computation_workflow(&workflow_request)?;
    let within_resources = workflow.within_declared_resources;
    let disposition = if !within_resources {
        PhaseResolvedImagingHandoffDisposition::Blocked
    } else if simulation_only {
        PhaseResolvedImagingHandoffDisposition::SimulationOnly
    } else {
        PhaseResolvedImagingHandoffDisposition::ReadyForComputation
    };
    let mut uncertainty = workflow.uncertainty.clone();
    if simulation_only {
        uncertainty.push(
            "capture artifacts are synthetic dry-run outputs, not biological measurements".into(),
        );
    }
    uncertainty.sort();
    uncertainty.dedup();
    let next_operator_action = if !within_resources {
        "increase the declared computation budget or duration before dispatch".into()
    } else if simulation_only {
        "replace synthetic captures with an admitted local acquisition before scientific interpretation".into()
    } else {
        "submit the compiled workflow to the governed local computation executor".into()
    };
    seal(PhaseResolvedImagingComputationHandoff {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        study_id: request.action_plan.study_id.clone(),
        model_system: request.action_plan.model_system,
        instrument_id: request.action_plan.instrument_id.clone(),
        replay_identity: request.replay_identity.clone(),
        input_artifact_order: artifacts,
        workflow_request: Some(workflow_request),
        workflow: Some(workflow),
        simulation_only,
        disposition,
        next_operator_action,
        uncertainty,
        digest: ContentHash::of_bytes(b"unsealed-phase-resolved-imaging-handoff"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p08_instrument_robotics::execution::{
        InstrumentExecutionDisposition, InstrumentExecutionResult,
    };
    use crate::glioma::programs::p08_instrument_robotics::preflight::InstrumentOperation;
    use crate::glioma_engine::LocalArtifactRef;

    fn digest(label: &str) -> ContentHash {
        ContentHash::of_bytes(label.as_bytes())
    }

    fn action_plan() -> PhaseResolvedInvasionActionPlan {
        let action =
            crate::glioma::programs::p08_instrument_robotics::preflight::InstrumentAction {
                action_id: "capture-capture-0001".into(),
                instrument_id: "organoid-imager-1".into(),
                operation: InstrumentOperation::AcquireImage,
                model_system: GliomaModelSystem::Organoid,
                requested_start_tick: 0,
                duration_ticks: 10,
                risk_milli: 10,
                requires_operator: true,
                output_schema: "GliomaTimeLapseFrame1@1".into(),
                parameters: Vec::new(),
            };
        let mut second_action = action.clone();
        second_action.action_id = "capture-capture-0002".into();
        second_action.requested_start_tick = 100;
        let mut plan = PhaseResolvedInvasionActionPlan {
            feature_id: crate::glioma::programs::p08_instrument_robotics::phase_resolved_invasion_schedule::FEATURE_ID.into(),
            output_schema: crate::glioma::programs::p08_instrument_robotics::phase_resolved_invasion_schedule::ACTION_PLAN_SCHEMA
                .into(),
            schedule_digest: digest("schedule"),
            study_id: "glioma-phase-study-1".into(),
            profile_id: "cadence-100ms".into(),
            action_id_prefix: "capture".into(),
            capability_manifest_digest: digest("capabilities"),
            dose_calibration_digest: digest("dose-calibration"),
            instrument_id: "organoid-imager-1".into(),
            model_system: GliomaModelSystem::Organoid,
            action_order: vec![action.action_id.clone(), second_action.action_id.clone()],
            actions: vec![action, second_action],
            capture_interval_ticks: 100,
            assay_duration_ticks: 100,
            first_capture_tick: 0,
            final_capture_tick: 100,
            completion_tick: 110,
            dose_units_per_capture: 2,
            total_dose_units: 4,
            digest: digest("unsealed-plan"),
        };
        plan.digest = ContentHash::of_value(
            &crate::glioma::programs::p08_instrument_robotics::phase_resolved_invasion_schedule::action_plan_digest_input(&plan),
        )
        .unwrap();
        plan
    }

    fn execution(plan: &PhaseResolvedInvasionActionPlan) -> InstrumentExecutionRun {
        let results = plan
            .action_order
            .iter()
            .enumerate()
            .map(|(index, action_id)| InstrumentExecutionResult {
                action_id: action_id.clone(),
                disposition: InstrumentExecutionDisposition::Completed,
                attempt_count: 1,
                started_tick: Some(index as u64 * 100),
                completed_tick: Some(index as u64 * 100 + 10),
                artifact: Some(LocalArtifactRef {
                    artifact_id: format!("dry-run-instrument:{action_id}"),
                    content_hash: digest(&format!("frame-{index}")),
                    content_type: "application/vnd.aurora.glioma.instrument-operation+json".into(),
                    local_only: true,
                    contains_human_data: false,
                    contains_direct_identifiers: false,
                }),
                note: "synthetic dry-run capture".into(),
                uncertainty: Vec::new(),
                negative_evidence: Vec::new(),
            })
            .collect::<Vec<_>>();
        let mut run = InstrumentExecutionRun {
            feature_id: super::super::super::p08_instrument_robotics::execution::FEATURE_ID.into(),
            output_schema: super::super::super::p08_instrument_robotics::execution::OUTPUT_SCHEMA.into(),
            objective: "phase-resolved imaging".into(),
            plan_digest: digest("preflight"),
            instrument_id: plan.instrument_id.clone(),
            action_order: plan.action_order.clone(),
            results,
            completed_order: plan.action_order.clone(),
            negative_order: Vec::new(),
            partial_order: Vec::new(),
            failed_order: Vec::new(),
            unresolved_order: Vec::new(),
            skipped_order: Vec::new(),
            retry_count: 0,
            emergency_stop_requested: false,
            emergency_stop_succeeded: false,
            uncertainty: Vec::new(),
            negative_evidence: Vec::new(),
            disposition: InstrumentExecutionDisposition::Completed,
            stop_reason: super::super::super::p08_instrument_robotics::execution::InstrumentExecutionStopReason::Completed,
            digest: digest("unsealed-execution"),
        };
        run.digest = ContentHash::of_value(
            &super::super::super::p08_instrument_robotics::execution::digest_input(&run),
        )
        .unwrap();
        run
    }

    fn request(mode: PhaseResolvedHandoffExecutionMode) -> PhaseResolvedImagingHandoffRequest {
        let plan = action_plan();
        let mut execution = execution(&plan);
        execution.objective = "phase-resolved imaging".into();
        PhaseResolvedImagingHandoffRequest {
            objective: "phase-resolved imaging".into(),
            action_plan: plan,
            execution,
            execution_mode: mode,
            budget_units: 10_000,
            duration_ticks: 10_000,
            max_tasks: 128,
            replay_identity: digest("handoff-replay"),
        }
    }

    #[test]
    fn completed_dry_run_captures_compile_a_simulation_labeled_imaging_workflow() {
        let output = compile_glioma_phase_resolved_imaging_computation_handoff(&request(
            PhaseResolvedHandoffExecutionMode::DryRun,
        ))
        .unwrap();
        output.verify_digest().unwrap();
        assert_eq!(
            output.disposition,
            PhaseResolvedImagingHandoffDisposition::SimulationOnly
        );
        assert!(output.simulation_only);
        assert_eq!(output.input_artifact_order.len(), 2);
        let workflow = output.workflow.as_ref().unwrap();
        assert!(workflow
            .operation_order
            .contains(&ComputationOperation::Segment));
        assert!(workflow
            .operation_order
            .contains(&ComputationOperation::Quantify));
        assert!(output
            .uncertainty
            .iter()
            .any(|item| item.contains("synthetic")));
    }

    #[test]
    fn computation_resource_shortfall_keeps_the_workflow_but_blocks_dispatch() {
        let mut request = request(PhaseResolvedHandoffExecutionMode::LocalGateway);
        request.budget_units = 1;
        request.duration_ticks = 1;
        let output = compile_glioma_phase_resolved_imaging_computation_handoff(&request).unwrap();
        assert_eq!(
            output.disposition,
            PhaseResolvedImagingHandoffDisposition::Blocked
        );
        assert!(output.workflow.is_some());
        assert!(!output.workflow.as_ref().unwrap().within_declared_resources);
        output.verify_digest().unwrap();
    }

    #[test]
    fn incomplete_acquisition_is_preserved_as_blocked_without_an_empty_workflow() {
        let mut request = request(PhaseResolvedHandoffExecutionMode::LocalGateway);
        request.execution.results[0].disposition = InstrumentExecutionDisposition::Partial;
        request.execution.partial_order = vec![request.execution.action_order[0].clone()];
        request.execution.completed_order = vec![request.execution.action_order[1].clone()];
        request.execution.disposition = InstrumentExecutionDisposition::Partial;
        request.execution.digest = ContentHash::of_value(
            &super::super::super::p08_instrument_robotics::execution::digest_input(
                &request.execution,
            ),
        )
        .unwrap();
        let output = compile_glioma_phase_resolved_imaging_computation_handoff(&request).unwrap();
        assert_eq!(
            output.disposition,
            PhaseResolvedImagingHandoffDisposition::Blocked
        );
        assert!(output.workflow.is_none());
        output.verify_digest().unwrap();
    }

    #[test]
    fn human_or_remote_capture_artifacts_are_rejected_before_workflow_expansion() {
        let mut request = request(PhaseResolvedHandoffExecutionMode::LocalGateway);
        request.execution.results[0]
            .artifact
            .as_mut()
            .unwrap()
            .local_only = false;
        request.execution.digest = ContentHash::of_value(
            &super::super::super::p08_instrument_robotics::execution::digest_input(
                &request.execution,
            ),
        )
        .unwrap();
        assert!(matches!(
            compile_glioma_phase_resolved_imaging_computation_handoff(&request),
            Err(PhaseResolvedImagingHandoffError::InvalidRequest(_))
        ));
    }
}

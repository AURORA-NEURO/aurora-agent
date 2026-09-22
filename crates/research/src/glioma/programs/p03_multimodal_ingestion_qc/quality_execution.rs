//! Schedule-bound execution for quality-aware preclinical glioma acquisition plans.
//!
//! The quality scheduler chooses a modality order, but a production research engine also needs
//! an approval-bound execution seam that can retry local work, preserve failed and below-floor
//! outcomes, and stop before an unsafe or scientifically unusable continuation. This module
//! executes exactly the selected schedule; it does not re-plan silently, impute assays, move raw
//! data, or turn a QC observation into a biological or clinical conclusion.

use super::quality_scheduler::{QualityScheduleItem, QualitySchedulePlan};
use crate::glioma_engine::GliomaModality;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P03-F27";
pub const OUTPUT_SCHEMA: &str = "GliomaMultimodalQualityExecution1@1";
pub const MAX_RETRIES: u8 = 8;
pub const MAX_ACTIONS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityExecutionMode {
    DryRun,
    InstitutionLocal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityExecutionApproval {
    pub approval_id: String,
    pub approver_id: String,
    pub scope: String,
    pub issued_epoch: u64,
    pub expires_epoch: u64,
    pub revoked: bool,
    pub approval_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityExecutionRequest {
    pub schedule: QualitySchedulePlan,
    pub approval: QualityExecutionApproval,
    pub current_epoch: u64,
    pub max_retries: u8,
    pub stop_on_failure: bool,
    pub stop_on_quality_floor: bool,
    pub require_all_required: bool,
    pub quality_acceptance_floor_milli: u16,
    pub execution_mode: QualityExecutionMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityExecutionObservation {
    pub observation_id: String,
    pub modality: GliomaModality,
    pub quality_milli: u16,
    pub expected_feature_count: u32,
    pub observed_feature_count: u32,
    pub artifact_hash: ContentHash,
    pub local_only: bool,
    pub contains_human_data: bool,
    pub contains_direct_identifiers: bool,
    pub simulated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualityExecutionFailure {
    pub reason: String,
    pub retryable: bool,
}

/// Institution-local gateways implement this seam. They receive one already-approved schedule
/// item and return only a typed, de-identified QC observation; the controller never receives raw
/// payloads and never decides whether an assay is biologically meaningful.
pub trait QualityScheduleExecutor {
    fn execute_quality_acquisition(
        &mut self,
        item: &QualityScheduleItem,
        attempt: u8,
    ) -> Result<QualityExecutionObservation, QualityExecutionFailure>;
}

/// Deterministic metadata-only executor used by MCP and tests. It models the workflow boundary;
/// no instrument, specimen, network, or biological material is touched.
#[derive(Debug, Default)]
pub struct DryRunQualityScheduleExecutor;

impl QualityScheduleExecutor for DryRunQualityScheduleExecutor {
    fn execute_quality_acquisition(
        &mut self,
        item: &QualityScheduleItem,
        attempt: u8,
    ) -> Result<QualityExecutionObservation, QualityExecutionFailure> {
        let artifact_hash = ContentHash::of_value(&serde_json::json!({
            "feature_id": FEATURE_ID,
            "modality": item.modality,
            "attempt": attempt,
            "simulation_only": true,
        }))
        .map_err(|error| QualityExecutionFailure {
            reason: format!("dry-run quality artifact digest failed: {error}"),
            retryable: false,
        })?;
        Ok(QualityExecutionObservation {
            observation_id: format!("dry-run-quality:{:?}", item.modality).to_lowercase(),
            modality: item.modality,
            quality_milli: item.forecast_quality_milli,
            expected_feature_count: 100,
            observed_feature_count: 100,
            artifact_hash,
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
            simulated: true,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityRunDisposition {
    Completed,
    Failed,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityExecutionRun {
    pub modality: GliomaModality,
    pub required: bool,
    pub attempts: u8,
    pub disposition: QualityRunDisposition,
    pub observation_id: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityExecutionDisposition {
    Completed,
    Conditional,
    Partial,
    Blocked,
    Failed,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityExecutionStopReason {
    Completed,
    QualityFloorFailed,
    ExecutorFailed,
    RequiredFailure,
    NoActions,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityExecutionResult {
    pub feature_id: String,
    pub output_schema: String,
    pub study_id: String,
    pub schedule_digest: ContentHash,
    pub approval_digest: ContentHash,
    pub execution_mode: QualityExecutionMode,
    pub action_order: Vec<GliomaModality>,
    pub completed_order: Vec<GliomaModality>,
    pub failed_order: Vec<GliomaModality>,
    pub blocked_order: Vec<GliomaModality>,
    pub below_floor_order: Vec<GliomaModality>,
    pub observations: Vec<QualityExecutionObservation>,
    pub runs: Vec<QualityExecutionRun>,
    pub retry_count: u32,
    pub attempted_cost_units: u32,
    pub quality_acceptance_floor_milli: u16,
    pub quality_gate_passed: bool,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub simulation_only: bool,
    pub disposition: QualityExecutionDisposition,
    pub stop_reason: QualityExecutionStopReason,
    pub next_action: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum QualityExecutionError {
    #[error("quality execution request is invalid: {0}")]
    InvalidRequest(String),
    #[error("quality execution failed: {0}")]
    Execution(String),
    #[error("quality execution output is invalid: {0}")]
    InvalidOutput(String),
    #[error("quality execution digest failed: {0}")]
    Digest(String),
}

fn approval_digest_input(approval: &QualityExecutionApproval) -> serde_json::Value {
    serde_json::json!({
        "approval_id": approval.approval_id,
        "approver_id": approval.approver_id,
        "scope": approval.scope,
        "issued_epoch": approval.issued_epoch,
        "expires_epoch": approval.expires_epoch,
        "revoked": approval.revoked,
    })
}

fn digest_input(output: &QualityExecutionResult) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "study_id": output.study_id,
        "schedule_digest": output.schedule_digest,
        "approval_digest": output.approval_digest,
        "execution_mode": output.execution_mode,
        "action_order": output.action_order,
        "completed_order": output.completed_order,
        "failed_order": output.failed_order,
        "blocked_order": output.blocked_order,
        "below_floor_order": output.below_floor_order,
        "observations": output.observations,
        "runs": output.runs,
        "retry_count": output.retry_count,
        "attempted_cost_units": output.attempted_cost_units,
        "quality_acceptance_floor_milli": output.quality_acceptance_floor_milli,
        "quality_gate_passed": output.quality_gate_passed,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "simulation_only": output.simulation_only,
        "disposition": output.disposition,
        "stop_reason": output.stop_reason,
        "next_action": output.next_action,
    })
}

fn validate_approval(
    approval: &QualityExecutionApproval,
    study_id: &str,
    current_epoch: u64,
) -> Result<(), QualityExecutionError> {
    if approval.approval_id.trim().is_empty()
        || approval.approver_id.trim().is_empty()
        || approval.scope != study_id
        || approval.expires_epoch <= approval.issued_epoch
        || current_epoch < approval.issued_epoch
        || current_epoch >= approval.expires_epoch
        || approval.revoked
    {
        return Err(QualityExecutionError::InvalidRequest(
            "approval requires a matching study scope, active epoch window, and non-revoked authorization".into(),
        ));
    }
    let expected = ContentHash::of_value(&approval_digest_input(approval))
        .map_err(|error| QualityExecutionError::Digest(error.to_string()))?;
    if expected != approval.approval_digest {
        return Err(QualityExecutionError::InvalidRequest(
            "approval digest does not match its signed fields".into(),
        ));
    }
    Ok(())
}

fn validate_observation(
    observation: &QualityExecutionObservation,
    modality: GliomaModality,
) -> Result<(), QualityExecutionError> {
    if observation.observation_id.trim().is_empty()
        || observation.modality != modality
        || observation.quality_milli > 1_000
        || observation.expected_feature_count == 0
        || observation.observed_feature_count == 0
        || observation.observed_feature_count > observation.expected_feature_count
        || !observation.local_only
        || observation.contains_human_data
        || observation.contains_direct_identifiers
        || observation.artifact_hash.as_str().len() != 64
    {
        return Err(QualityExecutionError::Execution(
            "executor returned an invalid or non-local QC observation".into(),
        ));
    }
    Ok(())
}

fn validate_request(request: &QualityExecutionRequest) -> Result<(), QualityExecutionError> {
    request
        .schedule
        .validate()
        .map_err(|error| QualityExecutionError::InvalidRequest(error.to_string()))?;
    if request.schedule.selected_items.is_empty()
        || request.schedule.selected_items.len() > MAX_ACTIONS
        || !request.schedule.uncovered_required_order.is_empty()
        || matches!(
            request.schedule.disposition,
            super::quality_scheduler::QualityScheduleDisposition::Blocked
                | super::quality_scheduler::QualityScheduleDisposition::Unresolved
        )
        || request.max_retries > MAX_RETRIES
        || request.quality_acceptance_floor_milli > 1_000
    {
        return Err(QualityExecutionError::InvalidRequest(
            "execution requires a non-empty, bounded, non-blocked schedule and bounded quality/retry gates".into(),
        ));
    }
    validate_approval(
        &request.approval,
        &request.schedule.study_id,
        request.current_epoch,
    )
}

fn validate_output(output: &QualityExecutionResult) -> Result<(), QualityExecutionError> {
    if output.feature_id != FEATURE_ID
        || output.output_schema != OUTPUT_SCHEMA
        || output.study_id.trim().is_empty()
        || output.action_order.len() != output.runs.len()
        || output.runs.len() > MAX_ACTIONS
        || output.observations.iter().any(|observation| {
            observation.observation_id.trim().is_empty()
                || observation.quality_milli > 1_000
                || observation.expected_feature_count == 0
                || observation.observed_feature_count == 0
                || observation.observed_feature_count > observation.expected_feature_count
        })
        || output.quality_acceptance_floor_milli > 1_000
        || !output
            .runs
            .iter()
            .zip(&output.action_order)
            .all(|(run, modality)| run.modality == *modality)
        || !output
            .completed_order
            .iter()
            .all(|modality| output.action_order.contains(modality))
        || !output.failed_order.iter().all(|modality| {
            output.action_order.contains(modality) && !output.completed_order.contains(modality)
        })
        || !output.blocked_order.iter().all(|modality| {
            output.action_order.contains(modality)
                && !output.completed_order.contains(modality)
                && !output.failed_order.contains(modality)
        })
        || !output
            .below_floor_order
            .iter()
            .all(|modality| output.completed_order.contains(modality))
        || !output
            .negative_evidence
            .windows(2)
            .all(|pair| pair[0] < pair[1])
        || !output.uncertainty.windows(2).all(|pair| pair[0] < pair[1])
    {
        return Err(QualityExecutionError::InvalidOutput(
            "execution identity, run/action reconciliation, observation, or evidence invariants are invalid".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(output))
        .map_err(|error| QualityExecutionError::Digest(error.to_string()))?;
    if expected != output.digest {
        return Err(QualityExecutionError::InvalidOutput(
            "quality execution digest is not content-addressed".into(),
        ));
    }
    Ok(())
}

impl QualityExecutionResult {
    pub fn validate(&self) -> Result<(), QualityExecutionError> {
        validate_output(self)
    }
}

/// Execute the exact selected quality schedule through a caller-owned institution-local seam.
pub fn execute_glioma_multimodal_quality_schedule<E: QualityScheduleExecutor>(
    request: &QualityExecutionRequest,
    executor: &mut E,
) -> Result<QualityExecutionResult, QualityExecutionError> {
    validate_request(request)?;
    let mut completed = Vec::new();
    let mut failed = Vec::new();
    let mut blocked = Vec::new();
    let mut below_floor = Vec::new();
    let mut observations = Vec::new();
    let mut runs = Vec::new();
    let mut negative = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut retry_count = 0_u32;
    let mut attempted_cost = 0_u32;
    let mut stop_reason = QualityExecutionStopReason::Completed;
    let mut stop = false;

    for item in &request.schedule.selected_items {
        if stop {
            blocked.push(item.modality);
            runs.push(QualityExecutionRun {
                modality: item.modality,
                required: item.required,
                attempts: 0,
                disposition: QualityRunDisposition::Blocked,
                observation_id: None,
                reason: Some("execution halted by an earlier quality or executor gate".into()),
            });
            continue;
        }
        let mut returned = None;
        let mut attempts = 0_u8;
        let mut failure_reason = None;
        for attempt in 1..=request.max_retries.saturating_add(1) {
            attempts = attempt;
            attempted_cost = attempted_cost.saturating_add(item.cost_units);
            match executor.execute_quality_acquisition(item, attempt) {
                Ok(observation) => {
                    validate_observation(&observation, item.modality)?;
                    returned = Some(observation);
                    break;
                }
                Err(error) => {
                    if error.reason.trim().is_empty() {
                        return Err(QualityExecutionError::Execution(
                            "executor returned an empty failure reason".into(),
                        ));
                    }
                    failure_reason = Some(error.reason);
                    if error.retryable && attempt <= request.max_retries {
                        retry_count = retry_count.saturating_add(1);
                        continue;
                    }
                    break;
                }
            }
        }

        if let Some(observation) = returned {
            let below = observation.quality_milli < request.quality_acceptance_floor_milli;
            let observation_id = observation.observation_id.clone();
            observations.push(observation);
            completed.push(item.modality);
            if below {
                below_floor.push(item.modality);
                uncertainty.insert(format!("quality-below-floor:{:?}", item.modality));
                if item.required && request.stop_on_quality_floor {
                    stop = true;
                    stop_reason = QualityExecutionStopReason::QualityFloorFailed;
                    negative.insert(format!("required-quality-floor-failed:{:?}", item.modality));
                }
            }
            runs.push(QualityExecutionRun {
                modality: item.modality,
                required: item.required,
                attempts,
                disposition: QualityRunDisposition::Completed,
                observation_id: Some(observation_id),
                reason: None,
            });
        } else {
            let reason =
                failure_reason.unwrap_or_else(|| "executor did not return an observation".into());
            negative.insert(format!("executor-failed:{:?}", item.modality));
            failed.push(item.modality);
            runs.push(QualityExecutionRun {
                modality: item.modality,
                required: item.required,
                attempts,
                disposition: QualityRunDisposition::Failed,
                observation_id: None,
                reason: Some(reason.clone()),
            });
            if item.required && request.require_all_required {
                stop = true;
                stop_reason = QualityExecutionStopReason::RequiredFailure;
            } else if request.stop_on_failure {
                stop = true;
                stop_reason = QualityExecutionStopReason::ExecutorFailed;
            }
        }
    }

    if stop_reason == QualityExecutionStopReason::Completed && !failed.is_empty() {
        stop_reason = QualityExecutionStopReason::ExecutorFailed;
    }
    let required_modalities = request
        .schedule
        .selected_items
        .iter()
        .filter(|item| item.required)
        .map(|item| item.modality)
        .collect::<BTreeSet<_>>();
    let completed_set = completed.iter().copied().collect::<BTreeSet<_>>();
    let quality_gate_passed = required_modalities
        .iter()
        .all(|modality| completed_set.contains(modality) && !below_floor.contains(modality));
    if !quality_gate_passed {
        negative.insert("required-quality-gate-not-passed".into());
    }
    let disposition = if runs.is_empty() {
        QualityExecutionDisposition::Unresolved
    } else if !quality_gate_passed && request.require_all_required {
        QualityExecutionDisposition::Blocked
    } else if !failed.is_empty() {
        QualityExecutionDisposition::Failed
    } else if !blocked.is_empty() {
        QualityExecutionDisposition::Partial
    } else if !below_floor.is_empty() {
        QualityExecutionDisposition::Conditional
    } else {
        QualityExecutionDisposition::Completed
    };
    let next_action = match disposition {
        QualityExecutionDisposition::Completed => {
            "ingest returned QC metadata into the downstream multimodal workflow"
        }
        QualityExecutionDisposition::Conditional => {
            "review below-floor QC before endpoint fusion or replan reacquisition"
        }
        QualityExecutionDisposition::Partial => {
            "resume blocked schedule items only after reviewing the execution gate"
        }
        QualityExecutionDisposition::Blocked => {
            "resolve required QC failures before downstream research execution"
        }
        QualityExecutionDisposition::Failed => {
            "inspect executor failure and retry through a newly approved schedule"
        }
        QualityExecutionDisposition::Unresolved => {
            "hold downstream execution for researcher adjudication"
        }
    }
    .into();
    let action_order = request
        .schedule
        .selected_items
        .iter()
        .map(|item| item.modality)
        .collect::<Vec<_>>();
    let mut output = QualityExecutionResult {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        study_id: request.schedule.study_id.clone(),
        schedule_digest: request.schedule.digest.clone(),
        approval_digest: request.approval.approval_digest.clone(),
        execution_mode: request.execution_mode,
        action_order,
        completed_order: completed,
        failed_order: failed,
        blocked_order: blocked,
        below_floor_order: below_floor,
        observations,
        runs,
        retry_count,
        attempted_cost_units: attempted_cost,
        quality_acceptance_floor_milli: request.quality_acceptance_floor_milli,
        quality_gate_passed,
        negative_evidence: negative.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        simulation_only: matches!(request.execution_mode, QualityExecutionMode::DryRun),
        disposition,
        stop_reason,
        next_action,
        digest: ContentHash::of_bytes(b"unsealed-glioma-quality-execution"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| QualityExecutionError::Digest(error.to_string()))?;
    validate_output(&output)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p03_multimodal_ingestion_qc::quality_scheduler::{
        plan_glioma_multimodal_quality_schedule, QualityAcquisitionCandidate,
        QualityScheduleRequest,
    };
    use crate::glioma_engine::GliomaModelSystem;

    fn candidate(modality: GliomaModality, required: bool) -> QualityAcquisitionCandidate {
        QualityAcquisitionCandidate {
            modality,
            forecast_quality_milli: if required { 600 } else { 900 },
            quality_risk_milli: if required { 700 } else { 200 },
            scientific_value_milli: 800,
            cost_units: 2,
            duration_units: 1,
            deadline_epoch_index: 0,
            required,
            fallback_modality: None,
        }
    }

    fn schedule() -> QualitySchedulePlan {
        plan_glioma_multimodal_quality_schedule(&QualityScheduleRequest {
            objective: "execute forecast-risk-aware acquisition".into(),
            study_id: "quality-execution-study".into(),
            model_system: GliomaModelSystem::Organoid,
            epoch_order: vec!["epoch-0".into(), "epoch-1".into()],
            candidates: vec![
                candidate(GliomaModality::Genomics, true),
                candidate(GliomaModality::Imaging, false),
            ],
            budget_units: 4,
            horizon_units: 2,
            min_forecast_quality_milli: 700,
            max_selected: 2,
            max_alternatives: 2,
        })
        .expect("schedule")
    }

    fn request(schedule: QualitySchedulePlan) -> QualityExecutionRequest {
        let mut approval = QualityExecutionApproval {
            approval_id: "approval-1".into(),
            approver_id: "researcher-1".into(),
            scope: schedule.study_id.clone(),
            issued_epoch: 10,
            expires_epoch: 20,
            revoked: false,
            approval_digest: ContentHash::of_bytes(b"unsealed-approval"),
        };
        approval.approval_digest =
            ContentHash::of_value(&approval_digest_input(&approval)).unwrap();
        QualityExecutionRequest {
            schedule,
            approval,
            current_epoch: 12,
            max_retries: 1,
            stop_on_failure: true,
            stop_on_quality_floor: false,
            require_all_required: true,
            quality_acceptance_floor_milli: 700,
            execution_mode: QualityExecutionMode::DryRun,
        }
    }

    #[test]
    fn executes_exact_schedule_and_preserves_quality_gate() {
        let mut executor = DryRunQualityScheduleExecutor;
        let output =
            execute_glioma_multimodal_quality_schedule(&request(schedule()), &mut executor)
                .expect("quality execution");
        assert_eq!(output.action_order.len(), 2);
        assert_eq!(output.disposition, QualityExecutionDisposition::Blocked);
        assert!(!output.quality_gate_passed);
        assert!(output.below_floor_order.contains(&GliomaModality::Genomics));
        assert!(output.simulation_only);
        output.validate().expect("digest and invariants");
    }

    #[test]
    fn revoked_or_expired_approval_never_reaches_executor() {
        let mut request = request(schedule());
        request.approval.revoked = true;
        let mut executor = DryRunQualityScheduleExecutor;
        let error = execute_glioma_multimodal_quality_schedule(&request, &mut executor)
            .expect_err("revoked approval must block");
        assert!(error.to_string().contains("approval"));
    }

    #[test]
    fn required_executor_failure_blocks_remaining_schedule() {
        #[derive(Default)]
        struct FailingExecutor;
        impl QualityScheduleExecutor for FailingExecutor {
            fn execute_quality_acquisition(
                &mut self,
                item: &QualityScheduleItem,
                _attempt: u8,
            ) -> Result<QualityExecutionObservation, QualityExecutionFailure> {
                if item.required {
                    Err(QualityExecutionFailure {
                        reason: "calibration unavailable".into(),
                        retryable: false,
                    })
                } else {
                    DryRunQualityScheduleExecutor.execute_quality_acquisition(item, 1)
                }
            }
        }
        let mut executor = FailingExecutor;
        let output =
            execute_glioma_multimodal_quality_schedule(&request(schedule()), &mut executor)
                .expect("failure becomes typed output");
        assert_eq!(output.disposition, QualityExecutionDisposition::Blocked);
        assert_eq!(
            output.stop_reason,
            QualityExecutionStopReason::RequiredFailure
        );
        assert!(output
            .negative_evidence
            .iter()
            .any(|entry| entry.contains("executor-failed")));
    }
}

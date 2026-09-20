//! Closed-loop adaptive quality campaigns for preclinical glioma acquisition.
//!
//! F26 and F27 provide schedule planning and schedule-bound execution. This module composes them
//! into a bounded research workflow: execute one approved quality schedule, assimilate only the
//! returned QC metadata, remove satisfied work, re-score unresolved risk, and re-plan the next
//! round. The controller never treats a successful acquisition as biological evidence, silently
//! retries a revoked approval, or exports raw experimental payloads.

use super::quality_execution::{
    execute_glioma_multimodal_quality_schedule, QualityExecutionApproval, QualityExecutionError,
    QualityExecutionMode, QualityExecutionRequest, QualityExecutionResult, QualityScheduleExecutor,
};
use super::quality_scheduler::{
    plan_glioma_multimodal_quality_schedule, QualityAcquisitionCandidate, QualityScheduleError,
    QualitySchedulePlan, QualityScheduleRequest,
};
use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P03-F28";
pub const OUTPUT_SCHEMA: &str = "GliomaMultimodalQualityAdaptiveCampaign1@1";
pub const MAX_ROUNDS: u16 = 24;
pub const MAX_CANDIDATES: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityAdaptiveCampaignRequest {
    pub objective: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub epoch_order: Vec<String>,
    pub candidates: Vec<QualityAcquisitionCandidate>,
    pub approval: QualityExecutionApproval,
    pub current_epoch: u64,
    pub budget_units: u32,
    pub horizon_units: u32,
    pub min_forecast_quality_milli: u16,
    pub max_selected: usize,
    pub max_alternatives: usize,
    pub max_rounds: u16,
    pub max_retries: u8,
    pub stop_on_quality_floor: bool,
    pub require_all_required: bool,
    pub execution_mode: QualityExecutionMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityAdaptiveCampaignStopReason {
    RequiredQualitySatisfied,
    BudgetExhausted,
    QualityGateFailed,
    ExecutorFailed,
    NoPendingWork,
    MaxRounds,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityAdaptiveCampaignDisposition {
    Completed,
    Conditional,
    BudgetBlocked,
    Failed,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityAdaptiveCampaignRound {
    pub round: u16,
    pub pending_order: Vec<GliomaModality>,
    pub scheduled_order: Vec<GliomaModality>,
    pub newly_satisfied_required_order: Vec<GliomaModality>,
    pub carry_forward_order: Vec<GliomaModality>,
    pub schedule: QualitySchedulePlan,
    pub execution: QualityExecutionResult,
    pub budget_before_units: u32,
    pub budget_after_units: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityAdaptiveCampaign {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub rounds: Vec<QualityAdaptiveCampaignRound>,
    pub pending_order: Vec<GliomaModality>,
    pub satisfied_required_order: Vec<GliomaModality>,
    pub completed_order: Vec<GliomaModality>,
    pub failed_order: Vec<GliomaModality>,
    pub budget_spent_units: u32,
    pub remaining_budget_units: u32,
    pub retry_count: u32,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub simulation_only: bool,
    pub disposition: QualityAdaptiveCampaignDisposition,
    pub stop_reason: QualityAdaptiveCampaignStopReason,
    pub next_action: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum QualityAdaptiveCampaignError {
    #[error("adaptive quality campaign request is invalid: {0}")]
    InvalidRequest(String),
    #[error("adaptive quality campaign scheduling failed: {0}")]
    Scheduling(#[from] QualityScheduleError),
    #[error("adaptive quality campaign execution failed: {0}")]
    Execution(#[from] QualityExecutionError),
    #[error("adaptive quality campaign output is invalid: {0}")]
    InvalidOutput(String),
    #[error("adaptive quality campaign digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &QualityAdaptiveCampaign) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "study_id": output.study_id,
        "model_system": output.model_system,
        "rounds": output.rounds,
        "pending_order": output.pending_order,
        "satisfied_required_order": output.satisfied_required_order,
        "completed_order": output.completed_order,
        "failed_order": output.failed_order,
        "budget_spent_units": output.budget_spent_units,
        "remaining_budget_units": output.remaining_budget_units,
        "retry_count": output.retry_count,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "simulation_only": output.simulation_only,
        "disposition": output.disposition,
        "stop_reason": output.stop_reason,
        "next_action": output.next_action,
    })
}

fn validate_request(
    request: &QualityAdaptiveCampaignRequest,
) -> Result<(), QualityAdaptiveCampaignError> {
    if request.objective.trim().is_empty()
        || request.study_id.trim().is_empty()
        || request.epoch_order.is_empty()
        || !canonical(&request.epoch_order)
        || request.candidates.is_empty()
        || request.candidates.len() > MAX_CANDIDATES
        || request.budget_units == 0
        || request.horizon_units == 0
        || request.min_forecast_quality_milli > 1_000
        || request.max_selected == 0
        || request.max_selected > request.candidates.len()
        || request.max_rounds == 0
        || request.max_rounds > MAX_ROUNDS
        || request.max_retries > super::quality_execution::MAX_RETRIES
    {
        return Err(QualityAdaptiveCampaignError::InvalidRequest(
            "objective, canonical epochs, bounded candidates, positive budget, and bounded rounds are required".into(),
        ));
    }
    let mut modalities = BTreeSet::new();
    for candidate in &request.candidates {
        if !modalities.insert(candidate.modality)
            || candidate.forecast_quality_milli > 1_000
            || candidate.quality_risk_milli > 1_000
            || candidate.scientific_value_milli > 1_000
            || candidate.cost_units == 0
            || candidate.duration_units == 0
            || candidate.deadline_epoch_index >= request.epoch_order.len() as u32
        {
            return Err(QualityAdaptiveCampaignError::InvalidRequest(
                "candidates require unique modalities, bounded scores, positive resources, and valid deadlines".into(),
            ));
        }
    }
    Ok(())
}

fn validate_output(output: &QualityAdaptiveCampaign) -> Result<(), QualityAdaptiveCampaignError> {
    if output.feature_id != FEATURE_ID
        || output.output_schema != OUTPUT_SCHEMA
        || output.objective.trim().is_empty()
        || output.study_id.trim().is_empty()
        || output.rounds.len() > MAX_ROUNDS as usize
        || !canonical(&output.pending_order)
        || !canonical(&output.satisfied_required_order)
        || !canonical(&output.completed_order)
        || !canonical(&output.failed_order)
        || !canonical(&output.negative_evidence)
        || !canonical(&output.uncertainty)
        || output
            .budget_spent_units
            .saturating_add(output.remaining_budget_units)
            == 0
    {
        return Err(QualityAdaptiveCampaignError::InvalidOutput(
            "identity, bounded rounds, canonical orders, evidence ordering, or budget reconciliation is invalid".into(),
        ));
    }
    for round in &output.rounds {
        if round.round == 0
            || round.pending_order.is_empty()
            || round.schedule.study_id != output.study_id
            || round.execution.study_id != output.study_id
            || round.schedule.digest != round.execution.schedule_digest
            || round.execution.action_order != round.scheduled_order
            || round.budget_after_units > round.budget_before_units
        {
            return Err(QualityAdaptiveCampaignError::InvalidOutput(
                "round schedule/execution bindings or budget transition is invalid".into(),
            ));
        }
        round
            .schedule
            .validate()
            .map_err(|error| QualityAdaptiveCampaignError::InvalidOutput(error.to_string()))?;
        round
            .execution
            .validate()
            .map_err(|error| QualityAdaptiveCampaignError::InvalidOutput(error.to_string()))?;
    }
    let expected = ContentHash::of_value(&digest_input(output))
        .map_err(|error| QualityAdaptiveCampaignError::Digest(error.to_string()))?;
    if expected != output.digest {
        return Err(QualityAdaptiveCampaignError::InvalidOutput(
            "adaptive campaign digest is not content-addressed".into(),
        ));
    }
    Ok(())
}

impl QualityAdaptiveCampaign {
    pub fn validate(&self) -> Result<(), QualityAdaptiveCampaignError> {
        validate_output(self)
    }
}

fn update_pending(
    pending: &mut Vec<QualityAcquisitionCandidate>,
    execution: &QualityExecutionResult,
    floor: u16,
    satisfied_required: &mut BTreeSet<GliomaModality>,
) -> (Vec<GliomaModality>, BTreeSet<GliomaModality>) {
    let observations = execution
        .observations
        .iter()
        .map(|observation| (observation.modality, observation))
        .collect::<BTreeMap<_, _>>();
    let failed = execution
        .failed_order
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut newly_satisfied = Vec::new();
    let mut observed_modalities = BTreeSet::new();
    pending.retain_mut(|candidate| {
        if let Some(observation) = observations.get(&candidate.modality) {
            observed_modalities.insert(candidate.modality);
            if observation.quality_milli >= floor {
                if candidate.required && satisfied_required.insert(candidate.modality) {
                    newly_satisfied.push(candidate.modality);
                }
                return false;
            }
            candidate.forecast_quality_milli = observation.quality_milli;
            candidate.quality_risk_milli = candidate
                .quality_risk_milli
                .saturating_add(floor.saturating_sub(observation.quality_milli))
                .min(1_000);
        } else if failed.contains(&candidate.modality) {
            candidate.quality_risk_milli =
                candidate.quality_risk_milli.saturating_add(200).min(1_000);
            observed_modalities.insert(candidate.modality);
        }
        true
    });
    newly_satisfied.sort();
    (newly_satisfied, observed_modalities)
}

fn all_required_satisfied(
    original: &[QualityAcquisitionCandidate],
    satisfied_required: &BTreeSet<GliomaModality>,
) -> bool {
    original
        .iter()
        .filter(|candidate| candidate.required)
        .all(|candidate| satisfied_required.contains(&candidate.modality))
}

/// Execute bounded quality rounds, re-planning only from typed local QC outcomes.
pub fn execute_glioma_multimodal_quality_adaptive_campaign<E: QualityScheduleExecutor>(
    request: &QualityAdaptiveCampaignRequest,
    executor: &mut E,
) -> Result<QualityAdaptiveCampaign, QualityAdaptiveCampaignError> {
    validate_request(request)?;
    let original_candidates = request.candidates.clone();
    let mut pending = request.candidates.clone();
    let mut rounds = Vec::new();
    let mut satisfied_required = BTreeSet::new();
    let mut completed = BTreeSet::new();
    let mut failed = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut spent = 0_u32;
    let mut retry_count = 0_u32;
    let mut stop_reason = QualityAdaptiveCampaignStopReason::MaxRounds;

    for round_number in 1..=request.max_rounds {
        if all_required_satisfied(&original_candidates, &satisfied_required) {
            stop_reason = QualityAdaptiveCampaignStopReason::RequiredQualitySatisfied;
            break;
        }
        if pending.is_empty() {
            stop_reason = QualityAdaptiveCampaignStopReason::NoPendingWork;
            break;
        }
        let remaining = request.budget_units.saturating_sub(spent);
        if remaining == 0 {
            stop_reason = QualityAdaptiveCampaignStopReason::BudgetExhausted;
            break;
        }
        let max_selected = request.max_selected.min(pending.len()).max(1);
        let schedule_request = QualityScheduleRequest {
            objective: request.objective.clone(),
            study_id: request.study_id.clone(),
            model_system: request.model_system,
            epoch_order: request.epoch_order.clone(),
            candidates: pending.clone(),
            budget_units: remaining,
            horizon_units: request.horizon_units,
            min_forecast_quality_milli: request.min_forecast_quality_milli,
            max_selected,
            max_alternatives: request.max_alternatives,
        };
        let schedule = plan_glioma_multimodal_quality_schedule(&schedule_request)?;
        if schedule.selected_items.is_empty() || !schedule.uncovered_required_order.is_empty() {
            stop_reason = QualityAdaptiveCampaignStopReason::QualityGateFailed;
            negative.insert("adaptive-round-required-schedule-blocked".into());
            break;
        }
        let budget_before = remaining;
        let schedule_cost = schedule.total_cost_units.max(1);
        let retry_budget = remaining / schedule_cost;
        let allowed_retries = request
            .max_retries
            .min(retry_budget.saturating_sub(1).min(u32::from(u8::MAX)) as u8);
        let execution_request = QualityExecutionRequest {
            schedule: schedule.clone(),
            approval: request.approval.clone(),
            current_epoch: request.current_epoch + u64::from(round_number - 1),
            max_retries: allowed_retries,
            stop_on_failure: true,
            stop_on_quality_floor: request.stop_on_quality_floor,
            require_all_required: request.require_all_required,
            quality_acceptance_floor_milli: request.min_forecast_quality_milli,
            execution_mode: request.execution_mode,
        };
        let execution = execute_glioma_multimodal_quality_schedule(&execution_request, executor)?;
        spent = spent.saturating_add(execution.attempted_cost_units);
        retry_count = retry_count.saturating_add(execution.retry_count);
        for modality in &execution.completed_order {
            completed.insert(*modality);
        }
        for modality in &execution.failed_order {
            failed.insert(*modality);
        }
        negative.extend(execution.negative_evidence.iter().cloned());
        uncertainty.extend(execution.uncertainty.iter().cloned());
        let pending_before = pending
            .iter()
            .map(|candidate| candidate.modality)
            .collect::<Vec<_>>();
        let (newly_satisfied, _) = update_pending(
            &mut pending,
            &execution,
            request.min_forecast_quality_milli,
            &mut satisfied_required,
        );
        let carry_forward_order = pending
            .iter()
            .map(|candidate| candidate.modality)
            .collect::<Vec<_>>();
        rounds.push(QualityAdaptiveCampaignRound {
            round: round_number,
            pending_order: pending_before,
            scheduled_order: schedule.selected_order.clone(),
            newly_satisfied_required_order: newly_satisfied,
            carry_forward_order,
            schedule,
            execution: execution.clone(),
            budget_before_units: budget_before,
            budget_after_units: remaining.saturating_sub(execution.attempted_cost_units),
        });
        if execution.failed_order.iter().any(|modality| {
            original_candidates
                .iter()
                .any(|candidate| candidate.required && candidate.modality == *modality)
        }) {
            stop_reason = QualityAdaptiveCampaignStopReason::ExecutorFailed;
            break;
        }
        if request.stop_on_quality_floor
            && !execution.quality_gate_passed
            && execution.below_floor_order.iter().any(|modality| {
                original_candidates
                    .iter()
                    .any(|candidate| candidate.required && candidate.modality == *modality)
            })
        {
            stop_reason = QualityAdaptiveCampaignStopReason::QualityGateFailed;
            break;
        }
        if spent >= request.budget_units {
            stop_reason = QualityAdaptiveCampaignStopReason::BudgetExhausted;
            break;
        }
    }

    let all_required = all_required_satisfied(&original_candidates, &satisfied_required);
    let disposition = if all_required {
        QualityAdaptiveCampaignDisposition::Completed
    } else if spent >= request.budget_units {
        QualityAdaptiveCampaignDisposition::BudgetBlocked
    } else if !failed.is_empty() {
        QualityAdaptiveCampaignDisposition::Failed
    } else if !rounds.is_empty() && !satisfied_required.is_empty() {
        QualityAdaptiveCampaignDisposition::Conditional
    } else if rounds.is_empty() {
        QualityAdaptiveCampaignDisposition::Unresolved
    } else {
        QualityAdaptiveCampaignDisposition::Blocked
    };
    let next_action = match disposition {
        QualityAdaptiveCampaignDisposition::Completed => {
            "promote accepted QC metadata to multimodal readiness and endpoint analysis"
        }
        QualityAdaptiveCampaignDisposition::Conditional => {
            "review unresolved quality risk and approve the next adaptive round"
        }
        QualityAdaptiveCampaignDisposition::BudgetBlocked => {
            "allocate additional bounded resources or revise the quality objective"
        }
        QualityAdaptiveCampaignDisposition::Failed => {
            "inspect executor failures and issue a new study approval before retrying"
        }
        QualityAdaptiveCampaignDisposition::Blocked => {
            "resolve required quality gates before downstream research execution"
        }
        QualityAdaptiveCampaignDisposition::Unresolved => {
            "hold the adaptive campaign for researcher adjudication"
        }
    }
    .into();
    let pending_order = pending
        .iter()
        .map(|candidate| candidate.modality)
        .collect::<Vec<_>>();
    let mut output = QualityAdaptiveCampaign {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        study_id: request.study_id.clone(),
        model_system: request.model_system,
        rounds,
        pending_order,
        satisfied_required_order: satisfied_required.into_iter().collect(),
        completed_order: completed.into_iter().collect(),
        failed_order: failed.into_iter().collect(),
        budget_spent_units: spent,
        remaining_budget_units: request.budget_units.saturating_sub(spent),
        retry_count,
        negative_evidence: negative.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        simulation_only: matches!(request.execution_mode, QualityExecutionMode::DryRun),
        disposition,
        stop_reason,
        next_action,
        digest: ContentHash::of_bytes(b"unsealed-glioma-quality-adaptive-campaign"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| QualityAdaptiveCampaignError::Digest(error.to_string()))?;
    validate_output(&output)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p03_multimodal_ingestion_qc::quality_execution::{
        DryRunQualityScheduleExecutor, QualityExecutionFailure, QualityExecutionObservation,
    };
    use crate::glioma::programs::p03_multimodal_ingestion_qc::quality_scheduler::QualityScheduleItem;

    fn candidate(
        modality: GliomaModality,
        required: bool,
        forecast: u16,
    ) -> QualityAcquisitionCandidate {
        QualityAcquisitionCandidate {
            modality,
            forecast_quality_milli: forecast,
            quality_risk_milli: if forecast < 700 { 700 } else { 100 },
            scientific_value_milli: 800,
            cost_units: 2,
            duration_units: 1,
            deadline_epoch_index: 0,
            required,
            fallback_modality: None,
        }
    }

    fn approval(study_id: &str) -> QualityExecutionApproval {
        let mut approval = QualityExecutionApproval {
            approval_id: "adaptive-approval".into(),
            approver_id: "adaptive-researcher".into(),
            scope: study_id.into(),
            issued_epoch: 10,
            expires_epoch: 30,
            revoked: false,
            approval_digest: ContentHash::of_bytes(b"unsealed-adaptive-approval"),
        };
        let value = serde_json::json!({
            "approval_id": approval.approval_id,
            "approver_id": approval.approver_id,
            "scope": approval.scope,
            "issued_epoch": approval.issued_epoch,
            "expires_epoch": approval.expires_epoch,
            "revoked": approval.revoked,
        });
        approval.approval_digest = ContentHash::of_value(&value).unwrap();
        approval
    }

    fn request() -> QualityAdaptiveCampaignRequest {
        QualityAdaptiveCampaignRequest {
            objective: "adaptively acquire QC before endpoint fusion".into(),
            study_id: "adaptive-quality-study".into(),
            model_system: GliomaModelSystem::Organoid,
            epoch_order: vec!["epoch-0".into(), "epoch-1".into()],
            candidates: vec![
                candidate(GliomaModality::Genomics, true, 600),
                candidate(GliomaModality::Imaging, false, 900),
            ],
            approval: approval("adaptive-quality-study"),
            current_epoch: 12,
            budget_units: 12,
            horizon_units: 2,
            min_forecast_quality_milli: 700,
            max_selected: 2,
            max_alternatives: 2,
            max_rounds: 3,
            max_retries: 0,
            stop_on_quality_floor: false,
            require_all_required: true,
            execution_mode: QualityExecutionMode::DryRun,
        }
    }

    #[test]
    fn dry_run_campaign_reaches_explicit_blocked_quality_state() {
        let mut executor = DryRunQualityScheduleExecutor;
        let output = execute_glioma_multimodal_quality_adaptive_campaign(&request(), &mut executor)
            .expect("adaptive campaign");
        assert_eq!(
            output.disposition,
            QualityAdaptiveCampaignDisposition::Blocked
        );
        assert!(!output
            .satisfied_required_order
            .contains(&GliomaModality::Genomics));
        assert_eq!(output.rounds.len(), 3);
        output.validate().expect("digest and invariants");
    }

    #[test]
    fn adaptive_executor_can_resolve_a_previous_below_floor_result() {
        #[derive(Default)]
        struct ImprovingExecutor {
            calls: BTreeMap<GliomaModality, u8>,
        }
        impl QualityScheduleExecutor for ImprovingExecutor {
            fn execute_quality_acquisition(
                &mut self,
                item: &QualityScheduleItem,
                _attempt: u8,
            ) -> Result<QualityExecutionObservation, QualityExecutionFailure> {
                let call = self.calls.entry(item.modality).or_insert(0);
                *call += 1;
                let quality = if item.modality == GliomaModality::Genomics && *call == 1 {
                    600
                } else {
                    900
                };
                let artifact_hash = ContentHash::of_value(&serde_json::json!({
                    "modality": item.modality,
                    "call": *call,
                }))
                .unwrap();
                Ok(QualityExecutionObservation {
                    observation_id: format!("adaptive-{:?}-{}", item.modality, *call),
                    modality: item.modality,
                    quality_milli: quality,
                    expected_feature_count: 10,
                    observed_feature_count: 10,
                    artifact_hash,
                    local_only: true,
                    contains_human_data: false,
                    contains_direct_identifiers: false,
                    simulated: false,
                })
            }
        }
        let mut executor = ImprovingExecutor::default();
        let output = execute_glioma_multimodal_quality_adaptive_campaign(&request(), &mut executor)
            .expect("adaptive campaign");
        assert_eq!(
            output.disposition,
            QualityAdaptiveCampaignDisposition::Completed
        );
        assert!(output
            .satisfied_required_order
            .contains(&GliomaModality::Genomics));
        assert!(output.rounds.len() >= 2);
    }

    #[test]
    fn budget_is_never_exceeded_even_when_retries_are_requested() {
        let mut request = request();
        request.budget_units = 4;
        request.max_retries = 8;
        let mut executor = DryRunQualityScheduleExecutor;
        let output = execute_glioma_multimodal_quality_adaptive_campaign(&request, &mut executor)
            .expect("budgeted adaptive campaign");
        assert!(output.budget_spent_units <= request.budget_units);
        assert!(output.remaining_budget_units + output.budget_spent_units == request.budget_units);
    }
}

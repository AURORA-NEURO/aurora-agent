//! Adaptive recovery for failed and partial preclinical glioma computation campaigns.
//!
//! A computation campaign can stop after a worker failure, a partial artifact, a dependency
//! block, or a budget boundary.  Re-running the whole DAG wastes compute and can accidentally
//! reuse the very cache entry that caused the failure.  This feature extracts the failed frontier,
//! closes its prerequisite DAG, invalidates only affected cache entries, and launches a bounded
//! recovery campaign under a fresh replay identity.  It preserves the initial campaign and the
//! recovery attempt as separate, replayable products.

use super::campaign::{
    execute_glioma_computation_campaign, GliomaComputationCampaign,
    GliomaComputationCampaignDisposition, GliomaComputationCampaignError,
    GliomaComputationCampaignRequest, GliomaComputationPlanner,
};
use super::execution::{ComputationCacheEntry, GliomaComputationExecutor};
use super::planning::ComputationCandidate;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P09-F25";
pub const OUTPUT_SCHEMA: &str = "GliomaComputationRecoveryCampaign1@1";
pub const MAX_RECOVERY_ROUNDS: u16 = 32;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationRecoveryRequest {
    pub initial: GliomaComputationCampaignRequest,
    pub recovery_budget_units: u64,
    pub recovery_duration_ticks: u64,
    pub max_recovery_rounds: u16,
    pub require_clean_completion: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputationRecoveryDisposition {
    Completed,
    Recovered,
    Partial,
    Failed,
    Blocked,
    NoRecoveryNeeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputationRecoveryStopReason {
    InitialCompleted,
    RecoveryCompleted,
    RecoveryPartial,
    RecoveryFailed,
    NoRecoverableCandidates,
    RecoveryBudgetExhausted,
    RecoveryDurationExhausted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationRecoveryCampaign {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub initial: GliomaComputationCampaign,
    pub recovery: Option<GliomaComputationCampaign>,
    pub recovery_candidate_order: Vec<String>,
    pub invalidated_cache_order: Vec<String>,
    pub recovery_replay_identity: Option<ContentHash>,
    pub budget_spent_units: u64,
    pub duration_spent_ticks: u64,
    pub remaining_budget_units: u64,
    pub remaining_duration_ticks: u64,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub disposition: ComputationRecoveryDisposition,
    pub stop_reason: ComputationRecoveryStopReason,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ComputationRecoveryError {
    #[error("computation recovery request is invalid: {0}")]
    InvalidRequest(String),
    #[error("computation recovery campaign failed: {0}")]
    Campaign(#[from] GliomaComputationCampaignError),
    #[error("computation recovery output is invalid: {0}")]
    InvalidOutput(String),
    #[error("computation recovery digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &ComputationRecoveryCampaign) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "initial": output.initial,
        "recovery": output.recovery,
        "recovery_candidate_order": output.recovery_candidate_order,
        "invalidated_cache_order": output.invalidated_cache_order,
        "recovery_replay_identity": output.recovery_replay_identity,
        "budget_spent_units": output.budget_spent_units,
        "duration_spent_ticks": output.duration_spent_ticks,
        "remaining_budget_units": output.remaining_budget_units,
        "remaining_duration_ticks": output.remaining_duration_ticks,
        "uncertainty": output.uncertainty,
        "negative_evidence": output.negative_evidence,
        "disposition": output.disposition,
        "stop_reason": output.stop_reason,
    })
}

impl ComputationRecoveryCampaign {
    pub fn validate(&self) -> Result<(), ComputationRecoveryError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.initial.objective != self.objective
            || !canonical(&self.recovery_candidate_order)
            || !canonical(&self.invalidated_cache_order)
            || !canonical(&self.uncertainty)
            || !canonical(&self.negative_evidence)
        {
            return Err(ComputationRecoveryError::InvalidOutput(
                "identity, campaign binding, canonical recovery partitions, or resource fields are invalid".into(),
            ));
        }
        self.initial.validate()?;
        if let Some(recovery) = &self.recovery {
            recovery.validate()?;
            if recovery.objective != self.objective {
                return Err(ComputationRecoveryError::InvalidOutput(
                    "recovery campaign objective does not match initial campaign".into(),
                ));
            }
        }
        if let Some(identity) = &self.recovery_replay_identity {
            if identity.as_str().len() != 64 {
                return Err(ComputationRecoveryError::InvalidOutput(
                    "recovery replay identity is not a valid content hash".into(),
                ));
            }
        } else if self.recovery.is_some() {
            return Err(ComputationRecoveryError::InvalidOutput(
                "a recovery campaign must carry its fresh replay identity".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ComputationRecoveryError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ComputationRecoveryError::InvalidOutput(
                "recovery campaign digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(request: &ComputationRecoveryRequest) -> Result<(), ComputationRecoveryError> {
    if request.recovery_budget_units == 0
        || request.recovery_duration_ticks == 0
        || request.max_recovery_rounds == 0
        || request.max_recovery_rounds > MAX_RECOVERY_ROUNDS
    {
        return Err(ComputationRecoveryError::InvalidRequest(
            "recovery budget, duration, and bounded recovery rounds are required".into(),
        ));
    }
    if request.initial.objective.trim().is_empty()
        || request.initial.initial_candidates.is_empty()
        || request.initial.replay_identity.as_str().len() != 64
    {
        return Err(ComputationRecoveryError::InvalidRequest(
            "initial computation campaign must carry a typed objective, candidates, and replay identity".into(),
        ));
    }
    Ok(())
}

fn recoverable_ids(initial: &GliomaComputationCampaign) -> BTreeSet<String> {
    initial
        .failed_order
        .iter()
        .chain(initial.partial_order.iter())
        .chain(initial.skipped_order.iter())
        .cloned()
        .collect()
}

fn close_candidates(
    candidates: &[ComputationCandidate],
    seed_ids: &BTreeSet<String>,
) -> Vec<ComputationCandidate> {
    let registry = candidates
        .iter()
        .map(|candidate| (candidate.candidate_id.clone(), candidate.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut selected = seed_ids.clone();
    loop {
        let mut changed = false;
        for id in selected.clone() {
            if let Some(candidate) = registry.get(&id) {
                for dependency in &candidate.task.depends_on {
                    if registry.contains_key(dependency) && selected.insert(dependency.clone()) {
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
    selected
        .into_iter()
        .filter_map(|id| registry.get(&id).cloned())
        .collect()
}

/// Execute an initial campaign and, only when necessary, recover its failed frontier under a new
/// replay identity.  Successful initial artifacts remain reusable; suspect task cache entries are
/// removed before recovery so the same failure cannot be silently replayed as a hit.
pub fn execute_glioma_computation_recovery<
    P: GliomaComputationPlanner,
    E: GliomaComputationExecutor,
>(
    request: &ComputationRecoveryRequest,
    planner: &mut P,
    executor: &mut E,
) -> Result<ComputationRecoveryCampaign, ComputationRecoveryError> {
    validate_request(request)?;
    let initial = execute_glioma_computation_campaign(&request.initial, planner, executor)?;
    let recoverable = recoverable_ids(&initial);
    let candidates = close_candidates(&request.initial.initial_candidates, &recoverable);
    let mut invalidated_cache_order = recoverable.iter().cloned().collect::<Vec<_>>();
    invalidated_cache_order.sort();
    let budget_spent = initial.budget_used_units;
    let duration_spent = initial.duration_used_ticks;
    // Recovery limits are a separate allowance. The initial campaign's consumption is still
    // reported in total spend, but it must not silently consume the recovery envelope.
    let remaining_budget = request.recovery_budget_units;
    let remaining_duration = request.recovery_duration_ticks;
    let (recovery, recovery_identity, stop_reason) = if recoverable.is_empty() {
        (None, None, ComputationRecoveryStopReason::InitialCompleted)
    } else if candidates.is_empty() {
        (
            None,
            None,
            ComputationRecoveryStopReason::NoRecoverableCandidates,
        )
    } else if remaining_budget == 0 {
        (
            None,
            None,
            ComputationRecoveryStopReason::RecoveryBudgetExhausted,
        )
    } else if remaining_duration == 0 {
        (
            None,
            None,
            ComputationRecoveryStopReason::RecoveryDurationExhausted,
        )
    } else {
        let recovery_identity = ContentHash::of_value(&serde_json::json!({
            "initial_replay_identity": request.initial.replay_identity,
            "initial_digest": initial.digest,
            "recovery_candidates": candidates.iter().map(|candidate| &candidate.candidate_id).collect::<Vec<_>>(),
            "invalidated_cache": invalidated_cache_order,
        }))
        .map_err(|error| ComputationRecoveryError::Digest(error.to_string()))?;
        let invalidated = recoverable.iter().collect::<BTreeSet<_>>();
        let cache = request
            .initial
            .cache
            .iter()
            .filter(|entry: &&ComputationCacheEntry| !invalidated.contains(&entry.task_id))
            .cloned()
            .collect::<Vec<_>>();
        let recovery_request = GliomaComputationCampaignRequest {
            objective: request.initial.objective.clone(),
            model_system: request.initial.model_system,
            initial_candidates: candidates,
            budget_units: remaining_budget,
            duration_ticks: remaining_duration,
            max_rounds: request.max_recovery_rounds,
            max_retries: request.initial.max_retries,
            max_tasks: request.initial.max_tasks,
            max_modalities: request.initial.max_modalities,
            min_modalities: request.initial.min_modalities,
            information_weight_milli: request.initial.information_weight_milli,
            uncertainty_weight_milli: request.initial.uncertainty_weight_milli,
            coverage_weight_milli: request.initial.coverage_weight_milli,
            cost_penalty_milli: request.initial.cost_penalty_milli,
            duration_penalty_milli: request.initial.duration_penalty_milli,
            require_deterministic: request.initial.require_deterministic,
            allow_cache: request.initial.allow_cache,
            require_local_artifacts: request.initial.require_local_artifacts,
            cache,
            replay_identity: recovery_identity.clone(),
        };
        let recovery = execute_glioma_computation_campaign(&recovery_request, planner, executor)?;
        let stop_reason = match recovery.disposition {
            GliomaComputationCampaignDisposition::Completed => {
                ComputationRecoveryStopReason::RecoveryCompleted
            }
            GliomaComputationCampaignDisposition::Partial => {
                ComputationRecoveryStopReason::RecoveryPartial
            }
            GliomaComputationCampaignDisposition::Failed => {
                ComputationRecoveryStopReason::RecoveryFailed
            }
            GliomaComputationCampaignDisposition::Blocked => {
                if recovery.budget_used_units >= remaining_budget {
                    ComputationRecoveryStopReason::RecoveryBudgetExhausted
                } else {
                    ComputationRecoveryStopReason::RecoveryDurationExhausted
                }
            }
        };
        (Some(recovery), Some(recovery_identity), stop_reason)
    };
    let budget_recovery = recovery
        .as_ref()
        .map(|campaign| campaign.budget_used_units)
        .unwrap_or_default();
    let duration_recovery = recovery
        .as_ref()
        .map(|campaign| campaign.duration_used_ticks)
        .unwrap_or_default();
    let budget_total = budget_spent.saturating_add(budget_recovery);
    let duration_total = duration_spent.saturating_add(duration_recovery);
    let uncertainty = initial
        .uncertainty
        .iter()
        .chain(
            recovery
                .as_ref()
                .into_iter()
                .flat_map(|campaign| campaign.uncertainty.iter()),
        )
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let negative_evidence = initial
        .negative_evidence
        .iter()
        .chain(
            recovery
                .as_ref()
                .into_iter()
                .flat_map(|campaign| campaign.negative_evidence.iter()),
        )
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let disposition = match stop_reason {
        ComputationRecoveryStopReason::InitialCompleted => {
            if request.require_clean_completion {
                ComputationRecoveryDisposition::Completed
            } else {
                ComputationRecoveryDisposition::NoRecoveryNeeded
            }
        }
        ComputationRecoveryStopReason::RecoveryCompleted => {
            ComputationRecoveryDisposition::Recovered
        }
        ComputationRecoveryStopReason::RecoveryPartial => ComputationRecoveryDisposition::Partial,
        ComputationRecoveryStopReason::RecoveryFailed => ComputationRecoveryDisposition::Failed,
        ComputationRecoveryStopReason::NoRecoverableCandidates
        | ComputationRecoveryStopReason::RecoveryBudgetExhausted
        | ComputationRecoveryStopReason::RecoveryDurationExhausted => {
            ComputationRecoveryDisposition::Blocked
        }
    };
    let recovery_candidate_order = recovery
        .as_ref()
        .map(|campaign| {
            campaign
                .rounds
                .iter()
                .flat_map(|round| round.candidate_order.iter().cloned())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut output = ComputationRecoveryCampaign {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.initial.objective.clone(),
        initial,
        recovery,
        recovery_candidate_order,
        invalidated_cache_order,
        recovery_replay_identity: recovery_identity,
        budget_spent_units: budget_total,
        duration_spent_ticks: duration_total,
        remaining_budget_units: request
            .recovery_budget_units
            .saturating_sub(budget_recovery),
        remaining_duration_ticks: request
            .recovery_duration_ticks
            .saturating_sub(duration_recovery),
        uncertainty,
        negative_evidence,
        disposition,
        stop_reason,
        digest: ContentHash::of_bytes(b"unsealed-glioma-computation-recovery"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ComputationRecoveryError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::super::execution::{
        ComputationExecutionFailure, ComputationOperation, ComputationTask, ComputationTaskResult,
        DryRunGliomaComputationExecutor,
    };
    use super::super::planning::ComputationCandidate;
    use super::*;
    use crate::glioma_engine::{GliomaModality, GliomaModelSystem};

    fn candidate(id: &str) -> ComputationCandidate {
        ComputationCandidate {
            candidate_id: id.into(),
            task: ComputationTask {
                task_id: id.into(),
                operation: ComputationOperation::Normalize,
                model_system: GliomaModelSystem::Organoid,
                depends_on: Vec::new(),
                input_artifact_ids: vec![format!("input:{id}")],
                output_schema: format!("{id}@1"),
                estimated_cost_units: 2,
                estimated_duration_ticks: 1,
                deterministic: true,
            },
            modality: GliomaModality::Transcriptomics,
            information_gain_milli: 700,
            uncertainty_reduction_milli: 600,
            coverage_debt_milli: 400,
            redundancy_group: id.into(),
            required: true,
        }
    }

    fn request() -> ComputationRecoveryRequest {
        ComputationRecoveryRequest {
            initial: GliomaComputationCampaignRequest {
                objective: "recover a glioma organoid computation".into(),
                model_system: GliomaModelSystem::Organoid,
                initial_candidates: vec![candidate("normalize")],
                budget_units: 4,
                duration_ticks: 2,
                max_rounds: 2,
                max_retries: 0,
                max_tasks: 1,
                max_modalities: 1,
                min_modalities: 1,
                information_weight_milli: 5,
                uncertainty_weight_milli: 3,
                coverage_weight_milli: 2,
                cost_penalty_milli: 1,
                duration_penalty_milli: 1,
                require_deterministic: true,
                allow_cache: true,
                require_local_artifacts: true,
                cache: Vec::new(),
                replay_identity: ContentHash::of_bytes(b"initial-recovery-replay"),
            },
            recovery_budget_units: 4,
            recovery_duration_ticks: 2,
            max_recovery_rounds: 2,
            require_clean_completion: true,
        }
    }

    #[derive(Debug, Default)]
    struct FailOnce {
        failed: bool,
        dry_run: DryRunGliomaComputationExecutor,
    }

    impl GliomaComputationExecutor for FailOnce {
        fn execute_task(
            &mut self,
            task: &ComputationTask,
            upstream: &[ComputationTaskResult],
            attempt: u8,
        ) -> Result<ComputationTaskResult, ComputationExecutionFailure> {
            if !self.failed {
                self.failed = true;
                return Err(ComputationExecutionFailure {
                    reason: "synthetic worker interruption".into(),
                    retryable: false,
                });
            }
            self.dry_run.execute_task(task, upstream, attempt)
        }
    }

    #[test]
    fn recovery_invalidates_failed_cache_and_replays_under_a_new_identity() {
        let mut planner = super::super::campaign::StaticGliomaComputationPlanner;
        let mut executor = FailOnce::default();
        let output =
            execute_glioma_computation_recovery(&request(), &mut planner, &mut executor).unwrap();
        assert_eq!(
            output.disposition,
            ComputationRecoveryDisposition::Recovered
        );
        assert_eq!(
            output.stop_reason,
            ComputationRecoveryStopReason::RecoveryCompleted
        );
        assert_eq!(
            output.invalidated_cache_order,
            vec!["normalize".to_string()]
        );
        assert_eq!(
            output.recovery_candidate_order,
            vec!["normalize".to_string()]
        );
        assert_ne!(
            output.initial.replay_identity,
            output.recovery_replay_identity.clone().unwrap()
        );
        output.validate().unwrap();
    }
}

//! Bounded autonomous computation campaigns for preclinical glioma research.
//!
//! A single portfolio execution is useful, but a real multimodal study usually needs several
//! compute rounds: harmonise a batch, inspect its output, add a model fit, then run validation or
//! robustness work. This controller keeps that loop typed and replayable. A caller-owned planner
//! may propose new computation candidates after every returned result; the controller never runs
//! untyped code, exceeds the declared cost/time budget, retries a failed round indefinitely, or
//! promotes a negative/partial computation into a success. Raw data and compute providers remain
//! institution-local.

use super::execution::{
    ComputationCacheEntry, ComputationTaskDisposition, GliomaComputationExecutor, MAX_RETRIES,
};
use super::planning::{ComputationCandidate, ComputationPortfolioRequest};
use super::portfolio_execution::{
    execute_glioma_computation_portfolio, ComputationPortfolioExecution,
    ComputationPortfolioExecutionDisposition, ComputationPortfolioExecutionError,
    ComputationPortfolioExecutionRequest,
};
use crate::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P09-F13";
pub const OUTPUT_SCHEMA: &str = "GliomaComputationCampaign1@1";
pub const MAX_ROUNDS: u16 = 64;
pub const MAX_CANDIDATES: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaComputationCampaignRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub initial_candidates: Vec<ComputationCandidate>,
    pub budget_units: u64,
    pub duration_ticks: u64,
    pub max_rounds: u16,
    pub max_retries: u8,
    pub max_tasks: usize,
    pub max_modalities: usize,
    pub min_modalities: usize,
    pub information_weight_milli: u16,
    pub uncertainty_weight_milli: u16,
    pub coverage_weight_milli: u16,
    pub cost_penalty_milli: u16,
    pub duration_penalty_milli: u16,
    pub require_deterministic: bool,
    pub allow_cache: bool,
    pub require_local_artifacts: bool,
    pub cache: Vec<ComputationCacheEntry>,
    pub replay_identity: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaComputationPlannerContext {
    pub round: u16,
    pub completed_order: Vec<String>,
    pub terminal_order: Vec<String>,
    pub available_order: Vec<String>,
    pub budget_remaining_units: u64,
    pub duration_remaining_ticks: u64,
    pub previous_execution: Option<ComputationPortfolioExecution>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GliomaComputationPlannerFailure {
    pub reason: String,
    pub retryable: bool,
}

/// Produce typed computation candidates after observing the previous local round.
pub trait GliomaComputationPlanner {
    fn propose_candidates(
        &mut self,
        context: &GliomaComputationPlannerContext,
    ) -> Result<Vec<ComputationCandidate>, GliomaComputationPlannerFailure>;
}

/// A deterministic no-op planner. Seed candidates still execute in round one; production hosts
/// replace this with a planner that compiles new candidates from returned local artifacts.
#[derive(Debug, Default)]
pub struct StaticGliomaComputationPlanner;

impl GliomaComputationPlanner for StaticGliomaComputationPlanner {
    fn propose_candidates(
        &mut self,
        _context: &GliomaComputationPlannerContext,
    ) -> Result<Vec<ComputationCandidate>, GliomaComputationPlannerFailure> {
        Ok(Vec::new())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaComputationCampaignRound {
    pub round: u16,
    pub candidate_order: Vec<String>,
    pub budget_before_units: u64,
    pub budget_after_units: u64,
    pub duration_before_ticks: u64,
    pub duration_after_ticks: u64,
    pub execution: ComputationPortfolioExecution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GliomaComputationCampaignDisposition {
    Completed,
    Partial,
    Failed,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GliomaComputationCampaignStopReason {
    Completed,
    NoCandidates,
    SelectionBlocked,
    DependencyBlocked,
    ExecutorFailed,
    BudgetExhausted,
    DurationExhausted,
    PlannerFailed,
    MaxRounds,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaComputationCampaign {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub replay_identity: ContentHash,
    pub rounds: Vec<GliomaComputationCampaignRound>,
    pub completed_order: Vec<String>,
    pub cached_order: Vec<String>,
    pub negative_order: Vec<String>,
    pub partial_order: Vec<String>,
    pub failed_order: Vec<String>,
    pub skipped_order: Vec<String>,
    pub retry_count: u32,
    pub budget_used_units: u64,
    pub duration_used_ticks: u64,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub disposition: GliomaComputationCampaignDisposition,
    pub stop_reason: GliomaComputationCampaignStopReason,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GliomaComputationCampaignError {
    #[error("computation campaign request is invalid: {0}")]
    InvalidRequest(String),
    #[error("computation campaign planner failed: {0}")]
    Planner(String),
    #[error("computation campaign execution failed: {0}")]
    Execution(#[from] ComputationPortfolioExecutionError),
    #[error("computation campaign output is invalid: {0}")]
    InvalidOutput(String),
    #[error("computation campaign digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &GliomaComputationCampaign) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "model_system": output.model_system,
        "replay_identity": output.replay_identity,
        "rounds": output.rounds,
        "completed_order": output.completed_order,
        "cached_order": output.cached_order,
        "negative_order": output.negative_order,
        "partial_order": output.partial_order,
        "failed_order": output.failed_order,
        "skipped_order": output.skipped_order,
        "retry_count": output.retry_count,
        "budget_used_units": output.budget_used_units,
        "duration_used_ticks": output.duration_used_ticks,
        "uncertainty": output.uncertainty,
        "negative_evidence": output.negative_evidence,
        "disposition": output.disposition,
        "stop_reason": output.stop_reason,
    })
}

impl GliomaComputationCampaign {
    pub fn validate(&self) -> Result<(), GliomaComputationCampaignError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.replay_identity.as_str().len() != 64
            || self.rounds.len() > MAX_ROUNDS as usize
            || !self.rounds.iter().enumerate().all(|(index, round)| {
                round.round == index as u16 + 1
                    && canonical(&round.candidate_order)
                    && round.budget_after_units <= round.budget_before_units
                    && round.duration_after_ticks <= round.duration_before_ticks
                    && round.execution.validate().is_ok()
            })
            || !canonical(&self.completed_order)
            || !canonical(&self.cached_order)
            || !canonical(&self.negative_order)
            || !canonical(&self.partial_order)
            || !canonical(&self.failed_order)
            || !canonical(&self.skipped_order)
            || !canonical(&self.uncertainty)
            || !canonical(&self.negative_evidence)
            || self.budget_used_units
                > self
                    .rounds
                    .first()
                    .map_or(0, |round| round.budget_before_units)
            || self.duration_used_ticks
                > self
                    .rounds
                    .first()
                    .map_or(0, |round| round.duration_before_ticks)
        {
            return Err(GliomaComputationCampaignError::InvalidOutput(
                "identity, round ordering, budget, nested execution, or canonical partitions are invalid".into(),
            ));
        }
        let mut status = BTreeMap::<String, ComputationTaskDisposition>::new();
        for round in &self.rounds {
            for result in round
                .execution
                .execution
                .as_ref()
                .map(|execution| execution.task_results.as_slice())
                .unwrap_or(&[])
            {
                if status.contains_key(&result.task_id)
                    && result.disposition != ComputationTaskDisposition::Cached
                {
                    return Err(GliomaComputationCampaignError::InvalidOutput(
                        "a computation task executed more than once across campaign rounds".into(),
                    ));
                }
                status
                    .entry(result.task_id.clone())
                    .or_insert(result.disposition);
            }
        }
        let partition = |ids: &[String], disposition: ComputationTaskDisposition| {
            ids.iter()
                .filter_map(|id| status.get(id).map(|value| (id, *value)))
                .all(|(_, value)| value == disposition)
                && ids.iter().all(|id| status.contains_key(id))
        };
        if !partition(&self.completed_order, ComputationTaskDisposition::Completed)
            || !partition(&self.cached_order, ComputationTaskDisposition::Cached)
            || !partition(&self.negative_order, ComputationTaskDisposition::Negative)
            || !partition(&self.partial_order, ComputationTaskDisposition::Partial)
            || !partition(&self.failed_order, ComputationTaskDisposition::Failed)
            || !partition(&self.skipped_order, ComputationTaskDisposition::Skipped)
        {
            return Err(GliomaComputationCampaignError::InvalidOutput(
                "task status partitions do not reconcile with round results".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| GliomaComputationCampaignError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(GliomaComputationCampaignError::InvalidOutput(
                "campaign digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &GliomaComputationCampaignRequest,
) -> Result<(), GliomaComputationCampaignError> {
    if request.objective.trim().is_empty()
        || request.initial_candidates.is_empty()
        || request.initial_candidates.len() > MAX_CANDIDATES
        || request.budget_units == 0
        || request.duration_ticks == 0
        || request.max_rounds == 0
        || request.max_rounds > MAX_ROUNDS
        || request.max_retries > MAX_RETRIES
        || request.max_tasks == 0
        || request.max_modalities == 0
        || request.min_modalities > request.max_modalities
        || request.replay_identity.as_str().len() != 64
        || request
            .initial_candidates
            .iter()
            .any(|candidate| candidate.task.model_system != request.model_system)
    {
        return Err(GliomaComputationCampaignError::InvalidRequest(
            "objective, typed seed candidates, bounded rounds/retries/resources, modality limits, and replay identity are required".into(),
        ));
    }
    Ok(())
}

fn merge_candidate(
    registry: &mut BTreeMap<String, ComputationCandidate>,
    candidate: ComputationCandidate,
) -> Result<(), GliomaComputationCampaignError> {
    if candidate.candidate_id.trim().is_empty() || candidate.task.task_id != candidate.candidate_id
    {
        return Err(GliomaComputationCampaignError::InvalidRequest(
            "planner returned a candidate whose task identity is not stable".into(),
        ));
    }
    if let Some(existing) = registry.get(&candidate.candidate_id) {
        if existing != &candidate {
            return Err(GliomaComputationCampaignError::InvalidRequest(format!(
                "planner changed the typed contract for existing candidate {}",
                candidate.candidate_id
            )));
        }
        return Ok(());
    }
    if registry.len() >= MAX_CANDIDATES {
        return Err(GliomaComputationCampaignError::InvalidRequest(
            "campaign candidate registry exceeds 4096 candidates".into(),
        ));
    }
    registry.insert(candidate.candidate_id.clone(), candidate);
    Ok(())
}

fn planner_round<P: GliomaComputationPlanner>(
    planner: &mut P,
    context: &GliomaComputationPlannerContext,
    max_retries: u8,
) -> Result<Vec<ComputationCandidate>, GliomaComputationCampaignError> {
    for attempt in 1..=max_retries.saturating_add(1) {
        match planner.propose_candidates(context) {
            Ok(candidates) => return Ok(candidates),
            Err(failure) if failure.reason.trim().is_empty() => {
                return Err(GliomaComputationCampaignError::Planner(
                    "planner returned an empty failure reason".into(),
                ));
            }
            Err(failure) if failure.retryable && attempt <= max_retries => continue,
            Err(failure) => return Err(GliomaComputationCampaignError::Planner(failure.reason)),
        }
    }
    Err(GliomaComputationCampaignError::Planner(
        "planner produced no decision".into(),
    ))
}

fn sorted_unique(values: &mut Vec<String>) {
    values.sort();
    values.dedup();
}

/// Run a bounded, observation-driven computation campaign through a caller-owned worker.
pub fn execute_glioma_computation_campaign<
    P: GliomaComputationPlanner,
    E: GliomaComputationExecutor,
>(
    request: &GliomaComputationCampaignRequest,
    planner: &mut P,
    executor: &mut E,
) -> Result<GliomaComputationCampaign, GliomaComputationCampaignError> {
    validate_request(request)?;
    let mut registry = BTreeMap::<String, ComputationCandidate>::new();
    for candidate in request.initial_candidates.iter().cloned() {
        merge_candidate(&mut registry, candidate)?;
    }
    let mut completed = BTreeSet::new();
    let mut terminal = BTreeSet::new();
    let mut rounds = Vec::new();
    let mut completed_order = Vec::new();
    let mut cached_order = Vec::new();
    let mut negative_order = Vec::new();
    let mut partial_order = Vec::new();
    let mut failed_order = Vec::new();
    let mut skipped_order = Vec::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative_evidence = BTreeSet::new();
    let mut retry_count = 0_u32;
    let mut budget_remaining = request.budget_units;
    let mut duration_remaining = request.duration_ticks;
    let mut previous_execution = None;
    let mut cache = request
        .cache
        .iter()
        .cloned()
        .map(|entry| (entry.task_id.clone(), entry))
        .collect::<BTreeMap<_, _>>();
    let mut stop_reason = GliomaComputationCampaignStopReason::MaxRounds;

    for round in 1..=request.max_rounds {
        if budget_remaining == 0 {
            stop_reason = if !registry.is_empty()
                && registry
                    .values()
                    .all(|candidate| terminal.contains(&candidate.candidate_id))
            {
                GliomaComputationCampaignStopReason::Completed
            } else {
                GliomaComputationCampaignStopReason::BudgetExhausted
            };
            break;
        }
        if duration_remaining == 0 {
            stop_reason = if !registry.is_empty()
                && registry
                    .values()
                    .all(|candidate| terminal.contains(&candidate.candidate_id))
            {
                GliomaComputationCampaignStopReason::Completed
            } else {
                GliomaComputationCampaignStopReason::DurationExhausted
            };
            break;
        }
        let context = GliomaComputationPlannerContext {
            round,
            completed_order: completed.iter().cloned().collect(),
            terminal_order: terminal.iter().cloned().collect(),
            available_order: registry.keys().cloned().collect(),
            budget_remaining_units: budget_remaining,
            duration_remaining_ticks: duration_remaining,
            previous_execution: previous_execution.clone(),
        };
        let candidates = planner_round(planner, &context, request.max_retries);
        let candidates = candidates?;
        for candidate in candidates {
            merge_candidate(&mut registry, candidate)?;
        }
        let active_candidates = registry
            .values()
            .filter(|candidate| !terminal.contains(&candidate.candidate_id))
            .cloned()
            .collect::<Vec<_>>();
        if active_candidates.is_empty() {
            stop_reason = GliomaComputationCampaignStopReason::Completed;
            break;
        }
        let mut closure_ids = active_candidates
            .iter()
            .map(|candidate| candidate.candidate_id.clone())
            .collect::<BTreeSet<_>>();
        loop {
            let mut added = false;
            for candidate in registry.values() {
                if closure_ids.contains(&candidate.candidate_id) {
                    for dependency in &candidate.task.depends_on {
                        if registry.contains_key(dependency)
                            && closure_ids.insert(dependency.clone())
                        {
                            added = true;
                        }
                    }
                }
            }
            if !added {
                break;
            }
        }
        let candidate_pool = registry
            .values()
            .filter(|candidate| closure_ids.contains(&candidate.candidate_id))
            .cloned()
            .collect::<Vec<_>>();
        let budget_before = budget_remaining;
        let duration_before = duration_remaining;
        let portfolio = ComputationPortfolioRequest {
            objective: request.objective.clone(),
            model_system: request.model_system,
            budget_units: budget_remaining,
            duration_ticks: duration_remaining,
            max_tasks: request.max_tasks,
            max_modalities: request.max_modalities,
            min_modalities: request.min_modalities,
            information_weight_milli: request.information_weight_milli,
            uncertainty_weight_milli: request.uncertainty_weight_milli,
            coverage_weight_milli: request.coverage_weight_milli,
            cost_penalty_milli: request.cost_penalty_milli,
            duration_penalty_milli: request.duration_penalty_milli,
            require_deterministic: request.require_deterministic,
            completed_order: completed.iter().cloned().collect(),
        };
        let execution = execute_glioma_computation_portfolio(
            &ComputationPortfolioExecutionRequest {
                portfolio,
                candidates: candidate_pool.clone(),
                replay_identity: request.replay_identity.clone(),
                max_retries: request.max_retries,
                allow_cache: request.allow_cache,
                require_local_artifacts: request.require_local_artifacts,
                cache: cache.values().cloned().collect(),
            },
            executor,
        )?;
        retry_count = retry_count.saturating_add(
            execution
                .execution
                .as_ref()
                .map(|run| {
                    run.task_results
                        .iter()
                        .map(|result| u32::from(result.attempt_count.saturating_sub(1)))
                        .sum::<u32>()
                })
                .unwrap_or_default(),
        );
        budget_remaining = budget_remaining.saturating_sub(
            execution
                .execution
                .as_ref()
                .map(|run| run.budget_used_units)
                .unwrap_or_default(),
        );
        duration_remaining = duration_remaining.saturating_sub(
            execution
                .execution
                .as_ref()
                .map(|run| run.duration_used_ticks)
                .unwrap_or_default(),
        );
        if let Some(run) = &execution.execution {
            for result in &run.task_results {
                if result.disposition == ComputationTaskDisposition::Cached
                    && terminal.contains(&result.task_id)
                {
                    continue;
                }
                if request.allow_cache
                    && matches!(
                        result.disposition,
                        ComputationTaskDisposition::Completed | ComputationTaskDisposition::Cached
                    )
                {
                    if let Some(artifact) = &result.artifact {
                        cache.insert(
                            result.task_id.clone(),
                            ComputationCacheEntry {
                                task_id: result.task_id.clone(),
                                replay_identity: request.replay_identity.clone(),
                                output_schema: result.output_schema.clone(),
                                artifact: artifact.clone(),
                            },
                        );
                    }
                }
                match result.disposition {
                    ComputationTaskDisposition::Completed => {
                        completed.insert(result.task_id.clone());
                        completed_order.push(result.task_id.clone());
                        terminal.insert(result.task_id.clone());
                    }
                    ComputationTaskDisposition::Cached => {
                        completed.insert(result.task_id.clone());
                        cached_order.push(result.task_id.clone());
                        terminal.insert(result.task_id.clone());
                    }
                    ComputationTaskDisposition::Negative => {
                        completed.insert(result.task_id.clone());
                        negative_order.push(result.task_id.clone());
                        terminal.insert(result.task_id.clone());
                    }
                    ComputationTaskDisposition::Partial => {
                        partial_order.push(result.task_id.clone());
                        terminal.insert(result.task_id.clone());
                    }
                    ComputationTaskDisposition::Failed => {
                        failed_order.push(result.task_id.clone());
                        terminal.insert(result.task_id.clone());
                    }
                    ComputationTaskDisposition::Skipped => {
                        skipped_order.push(result.task_id.clone())
                    }
                }
            }
        }
        uncertainty.extend(execution.uncertainty.iter().cloned());
        negative_evidence.extend(execution.negative_evidence.iter().cloned());
        let action_order = execution
            .execution
            .as_ref()
            .map(|run| run.task_order.clone())
            .unwrap_or_default();
        if action_order.is_empty() {
            stop_reason = match execution.disposition {
                ComputationPortfolioExecutionDisposition::Blocked
                | ComputationPortfolioExecutionDisposition::Unresolved => {
                    GliomaComputationCampaignStopReason::SelectionBlocked
                }
                _ => GliomaComputationCampaignStopReason::NoCandidates,
            };
        }
        rounds.push(GliomaComputationCampaignRound {
            round,
            candidate_order: candidate_pool
                .iter()
                .map(|candidate| candidate.candidate_id.clone())
                .collect(),
            budget_before_units: budget_before,
            budget_after_units: budget_remaining,
            duration_before_ticks: duration_before,
            duration_after_ticks: duration_remaining,
            execution: execution.clone(),
        });
        previous_execution = Some(execution.clone());
        if !failed_order.is_empty()
            || execution.disposition == ComputationPortfolioExecutionDisposition::Failed
        {
            stop_reason = GliomaComputationCampaignStopReason::ExecutorFailed;
            break;
        }
        let execution_blocked = execution.execution.as_ref().is_none_or(|run| {
            run.disposition != super::execution::ComputationExecutionDisposition::Completed
        });
        if !partial_order.is_empty() || !skipped_order.is_empty() || execution_blocked {
            stop_reason = GliomaComputationCampaignStopReason::DependencyBlocked;
            break;
        }
        if budget_remaining == 0 {
            stop_reason = if registry
                .values()
                .all(|candidate| terminal.contains(&candidate.candidate_id))
            {
                GliomaComputationCampaignStopReason::Completed
            } else {
                GliomaComputationCampaignStopReason::BudgetExhausted
            };
            break;
        }
        if duration_remaining == 0 {
            stop_reason = if registry
                .values()
                .all(|candidate| terminal.contains(&candidate.candidate_id))
            {
                GliomaComputationCampaignStopReason::Completed
            } else {
                GliomaComputationCampaignStopReason::DurationExhausted
            };
            break;
        }
    }

    sorted_unique(&mut completed_order);
    sorted_unique(&mut cached_order);
    sorted_unique(&mut negative_order);
    sorted_unique(&mut partial_order);
    sorted_unique(&mut failed_order);
    sorted_unique(&mut skipped_order);
    let disposition = if rounds.is_empty() {
        GliomaComputationCampaignDisposition::Blocked
    } else if !failed_order.is_empty() {
        GliomaComputationCampaignDisposition::Failed
    } else if !partial_order.is_empty()
        || !skipped_order.is_empty()
        || !matches!(stop_reason, GliomaComputationCampaignStopReason::Completed)
    {
        GliomaComputationCampaignDisposition::Partial
    } else {
        GliomaComputationCampaignDisposition::Completed
    };
    let budget_used_units = request.budget_units.saturating_sub(budget_remaining);
    let duration_used_ticks = request.duration_ticks.saturating_sub(duration_remaining);
    let mut output = GliomaComputationCampaign {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        replay_identity: request.replay_identity.clone(),
        rounds,
        completed_order,
        cached_order,
        negative_order,
        partial_order,
        failed_order,
        skipped_order,
        retry_count,
        budget_used_units,
        duration_used_ticks,
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative_evidence.into_iter().collect(),
        disposition,
        stop_reason,
        digest: ContentHash::of_bytes(b"unsealed-glioma-computation-campaign"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| GliomaComputationCampaignError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p09_reproducible_computation::execution::{
        ComputationOperation, DryRunGliomaComputationExecutor,
    };
    use crate::glioma_engine::GliomaModality;

    fn candidate(
        id: &str,
        operation: ComputationOperation,
        depends_on: Vec<&str>,
    ) -> ComputationCandidate {
        ComputationCandidate {
            candidate_id: id.into(),
            task: super::super::execution::ComputationTask {
                task_id: id.into(),
                operation,
                model_system: GliomaModelSystem::Organoid,
                depends_on: depends_on.into_iter().map(str::to_string).collect(),
                input_artifact_ids: vec![format!("input:{id}")],
                output_schema: format!("{id}@1"),
                estimated_cost_units: 2,
                estimated_duration_ticks: 1,
                deterministic: true,
            },
            modality: GliomaModality::Transcriptomics,
            information_gain_milli: 700,
            uncertainty_reduction_milli: 500,
            coverage_debt_milli: 300,
            redundancy_group: id.into(),
            required: false,
        }
    }

    fn request(initial_candidates: Vec<ComputationCandidate>) -> GliomaComputationCampaignRequest {
        GliomaComputationCampaignRequest {
            objective: "autonomously refine a glioma invasion computation".into(),
            model_system: GliomaModelSystem::Organoid,
            initial_candidates,
            budget_units: 4,
            duration_ticks: 2,
            max_rounds: 4,
            max_retries: 1,
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
            replay_identity: ContentHash::of_bytes(b"campaign-replay"),
        }
    }

    #[derive(Debug, Default)]
    struct FollowupPlanner;

    impl GliomaComputationPlanner for FollowupPlanner {
        fn propose_candidates(
            &mut self,
            context: &GliomaComputationPlannerContext,
        ) -> Result<Vec<ComputationCandidate>, GliomaComputationPlannerFailure> {
            if context.round == 2 {
                Ok(vec![candidate(
                    "validate",
                    ComputationOperation::Validate,
                    vec!["normalize"],
                )])
            } else {
                Ok(Vec::new())
            }
        }
    }

    #[test]
    fn planner_adds_a_followup_round_from_typed_results() {
        let mut planner = FollowupPlanner;
        let mut executor = DryRunGliomaComputationExecutor;
        let output = execute_glioma_computation_campaign(
            &request(vec![candidate(
                "normalize",
                ComputationOperation::Normalize,
                vec![],
            )]),
            &mut planner,
            &mut executor,
        )
        .unwrap();
        assert_eq!(output.rounds.len(), 2);
        assert_eq!(
            output.completed_order,
            vec!["normalize".to_string(), "validate".to_string()]
        );
        assert_eq!(
            output.stop_reason,
            GliomaComputationCampaignStopReason::Completed
        );
        assert_eq!(
            output.disposition,
            GliomaComputationCampaignDisposition::Completed
        );
        output.validate().unwrap();
    }

    #[test]
    fn static_campaign_is_replay_stable_and_does_not_spin_after_seed_completion() {
        let request = request(vec![candidate(
            "normalize",
            ComputationOperation::Normalize,
            vec![],
        )]);
        let mut first_planner = StaticGliomaComputationPlanner;
        let mut first_executor = DryRunGliomaComputationExecutor;
        let first =
            execute_glioma_computation_campaign(&request, &mut first_planner, &mut first_executor)
                .unwrap();
        let mut second_planner = StaticGliomaComputationPlanner;
        let mut second_executor = DryRunGliomaComputationExecutor;
        let second = execute_glioma_computation_campaign(
            &request,
            &mut second_planner,
            &mut second_executor,
        )
        .unwrap();
        assert_eq!(first, second);
        assert_eq!(first.rounds.len(), 1);
        assert_eq!(
            first.stop_reason,
            GliomaComputationCampaignStopReason::Completed
        );
    }
}

//! Autonomous, closed-loop execution of a preclinical glioma research campaign.
//!
//! This is the product controller above the portfolio selector and action executor.  It keeps a
//! bounded candidate registry, asks a caller-owned planner for new assays or analyses after each
//! observed round, and replans only from results that the local executor actually returned.  A
//! failed or partial effect is a terminal safety boundary for the campaign; a negative result is
//! retained as usable evidence and may satisfy a dependency without being promoted to a success.
//! The controller never opens a socket, moves raw data, controls an instrument, or makes a
//! clinical decision.

use super::action_execution::{
    execute_glioma_action_portfolio, ActionExecutionDisposition, ActionExecutionResult,
    ActionPortfolioExecution, ActionPortfolioExecutionRequest, GliomaActionExecutor, MAX_ACTIONS,
    MAX_RETRIES,
};
use crate::glioma_engine::{
    compile_glioma_research, GliomaActionCandidate, GliomaResearchIntent, GliomaSelectionConfig,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P07-F20";
pub const OUTPUT_SCHEMA: &str = "GliomaAutonomousCampaign1@1";
pub const MAX_ROUNDS: u16 = 64;

/// A planner receives only de-identified campaign state and the typed results from the previous
/// round.  It can be implemented by a protocol library, a local model, or a researcher-approved
/// scheduler; it cannot submit an untyped effect directly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaAutonomousPlannerContext {
    pub round: u16,
    pub completed_actions: BTreeSet<String>,
    pub terminal_actions: BTreeSet<String>,
    pub available_action_ids: Vec<String>,
    pub budget_remaining_units: u32,
    pub previous_results: Vec<ActionExecutionResult>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GliomaPlannerFailure {
    pub reason: String,
    pub retryable: bool,
}

/// Propose typed local actions for the next campaign round.
pub trait GliomaActionPlanner {
    fn propose_actions(
        &mut self,
        context: &GliomaAutonomousPlannerContext,
    ) -> Result<Vec<GliomaActionCandidate>, GliomaPlannerFailure>;
}

/// A no-op planner is useful when a host wants to execute a fixed seed portfolio while retaining
/// the same campaign output contract.  A production host can replace it with a planner that
/// compiles new actions from returned observations.
#[derive(Debug, Default)]
pub struct StaticGliomaActionPlanner;

impl GliomaActionPlanner for StaticGliomaActionPlanner {
    fn propose_actions(
        &mut self,
        _context: &GliomaAutonomousPlannerContext,
    ) -> Result<Vec<GliomaActionCandidate>, GliomaPlannerFailure> {
        Ok(Vec::new())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaAutonomousCampaignRequest {
    pub intent: GliomaResearchIntent,
    pub initial_candidates: Vec<GliomaActionCandidate>,
    pub selection: GliomaSelectionConfig,
    pub max_rounds: u16,
    pub max_retries: u8,
    pub require_artifacts: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaAutonomousCampaignRound {
    pub round: u16,
    pub candidate_order: Vec<String>,
    pub budget_before_units: u32,
    pub budget_after_units: u32,
    pub execution: ActionPortfolioExecution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GliomaAutonomousCampaignDisposition {
    Completed,
    Partial,
    Failed,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GliomaAutonomousCampaignStopReason {
    Completed,
    NoCandidates,
    SelectionBlocked,
    DependencyBlocked,
    ExecutorFailed,
    BudgetExhausted,
    MaxRounds,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaAutonomousCampaign {
    pub feature_id: String,
    pub output_schema: String,
    pub research_id: String,
    pub study_id: String,
    pub objective: String,
    pub replay_identity: ContentHash,
    pub rounds: Vec<GliomaAutonomousCampaignRound>,
    pub completed_order: Vec<String>,
    pub negative_order: Vec<String>,
    pub partial_order: Vec<String>,
    pub failed_order: Vec<String>,
    pub skipped_order: Vec<String>,
    pub retry_count: u32,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub disposition: GliomaAutonomousCampaignDisposition,
    pub stop_reason: GliomaAutonomousCampaignStopReason,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GliomaAutonomousCampaignError {
    #[error("autonomous campaign request is invalid: {0}")]
    InvalidRequest(String),
    #[error("autonomous campaign planner failed: {0}")]
    Planner(String),
    #[error("autonomous campaign execution failed: {0}")]
    Execution(String),
    #[error("autonomous campaign output is invalid: {0}")]
    InvalidOutput(String),
    #[error("autonomous campaign digest failed: {0}")]
    Digest(String),
}

fn sorted_unique(values: &mut Vec<String>) {
    values.sort();
    values.dedup();
}

fn digest_input(output: &GliomaAutonomousCampaign) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "research_id": output.research_id,
        "study_id": output.study_id,
        "objective": output.objective,
        "replay_identity": output.replay_identity,
        "rounds": output.rounds,
        "completed_order": output.completed_order,
        "negative_order": output.negative_order,
        "partial_order": output.partial_order,
        "failed_order": output.failed_order,
        "skipped_order": output.skipped_order,
        "retry_count": output.retry_count,
        "uncertainty": output.uncertainty,
        "negative_evidence": output.negative_evidence,
        "disposition": output.disposition,
        "stop_reason": output.stop_reason,
    })
}

impl GliomaAutonomousCampaign {
    pub fn validate(&self) -> Result<(), GliomaAutonomousCampaignError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.research_id.trim().is_empty()
            || self.study_id.trim().is_empty()
            || self.objective.trim().is_empty()
            || self.replay_identity.as_str().len() != 64
            || self.rounds.len() > MAX_ROUNDS as usize
            || !self
                .rounds
                .iter()
                .enumerate()
                .all(|(index, round)| round.round == index as u16 + 1)
            || !self.rounds.iter().all(|round| {
                round
                    .candidate_order
                    .windows(2)
                    .all(|pair| pair[0] < pair[1])
                    && round.budget_after_units <= round.budget_before_units
                    && round.execution.validate().is_ok()
            })
            || !self
                .completed_order
                .windows(2)
                .all(|pair| pair[0] < pair[1])
            || !self.negative_order.windows(2).all(|pair| pair[0] < pair[1])
            || !self.partial_order.windows(2).all(|pair| pair[0] < pair[1])
            || !self.failed_order.windows(2).all(|pair| pair[0] < pair[1])
            || !self.skipped_order.windows(2).all(|pair| pair[0] < pair[1])
            || !self.uncertainty.windows(2).all(|pair| pair[0] < pair[1])
            || !self
                .negative_evidence
                .windows(2)
                .all(|pair| pair[0] < pair[1])
        {
            return Err(GliomaAutonomousCampaignError::InvalidOutput(
                "identity, round ordering, budget, nested execution, or canonical partitions are invalid".into(),
            ));
        }
        let mut status = BTreeMap::<String, ActionExecutionDisposition>::new();
        for round in &self.rounds {
            for result in &round.execution.results {
                if status
                    .insert(result.action_id.clone(), result.disposition)
                    .is_some()
                {
                    return Err(GliomaAutonomousCampaignError::InvalidOutput(
                        "an action executed more than once across campaign rounds".into(),
                    ));
                }
            }
        }
        let partition = |ids: &[String], disposition: ActionExecutionDisposition| {
            ids.iter()
                .filter_map(|id| status.get(id).map(|value| (id, *value)))
                .all(|(_, value)| value == disposition)
                && ids.iter().all(|id| status.contains_key(id))
        };
        if !partition(&self.completed_order, ActionExecutionDisposition::Completed)
            || !partition(&self.negative_order, ActionExecutionDisposition::Negative)
            || !partition(&self.partial_order, ActionExecutionDisposition::Partial)
            || !partition(&self.failed_order, ActionExecutionDisposition::Failed)
            || !partition(&self.skipped_order, ActionExecutionDisposition::Skipped)
        {
            return Err(GliomaAutonomousCampaignError::InvalidOutput(
                "campaign status partitions do not reconcile with round results".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| GliomaAutonomousCampaignError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(GliomaAutonomousCampaignError::InvalidOutput(
                "campaign digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &GliomaAutonomousCampaignRequest,
) -> Result<(), GliomaAutonomousCampaignError> {
    if request.max_rounds == 0
        || request.max_rounds > MAX_ROUNDS
        || request.max_retries > MAX_RETRIES
        || request.selection.budget_units == 0
        || request.selection.max_actions == 0
        || request.initial_candidates.len() > MAX_ACTIONS
    {
        return Err(GliomaAutonomousCampaignError::InvalidRequest(
            "campaign requires bounded rounds/retries, a positive action budget, max_actions, and at most 256 seed candidates".into(),
        ));
    }
    Ok(())
}

fn merge_candidate(
    registry: &mut BTreeMap<String, GliomaActionCandidate>,
    candidate: GliomaActionCandidate,
) -> Result<(), GliomaAutonomousCampaignError> {
    if candidate.action_id.trim().is_empty() {
        return Err(GliomaAutonomousCampaignError::InvalidRequest(
            "planner returned an empty action id".into(),
        ));
    }
    if let Some(existing) = registry.get(&candidate.action_id) {
        if existing != &candidate {
            return Err(GliomaAutonomousCampaignError::InvalidRequest(format!(
                "planner changed the typed contract for existing action {}",
                candidate.action_id
            )));
        }
        return Ok(());
    }
    if registry.len() >= MAX_ACTIONS {
        return Err(GliomaAutonomousCampaignError::InvalidRequest(
            "campaign candidate registry exceeds 256 actions".into(),
        ));
    }
    registry.insert(candidate.action_id.clone(), candidate);
    Ok(())
}

fn planner_round<P: GliomaActionPlanner>(
    planner: &mut P,
    context: &GliomaAutonomousPlannerContext,
    max_retries: u8,
) -> Result<Vec<GliomaActionCandidate>, GliomaAutonomousCampaignError> {
    for attempt in 1..=max_retries.saturating_add(1) {
        match planner.propose_actions(context) {
            Ok(candidates) => return Ok(candidates),
            Err(failure) if failure.reason.trim().is_empty() => {
                return Err(GliomaAutonomousCampaignError::Planner(
                    "planner returned an empty failure reason".into(),
                ));
            }
            Err(failure) if failure.retryable && attempt <= max_retries => continue,
            Err(failure) => return Err(GliomaAutonomousCampaignError::Planner(failure.reason)),
        }
    }
    Err(GliomaAutonomousCampaignError::Planner(
        "planner produced no decision".into(),
    ))
}

/// Run a bounded, observation-driven autonomous glioma research campaign.
pub fn execute_glioma_autonomous_campaign<P: GliomaActionPlanner, E: GliomaActionExecutor>(
    request: &GliomaAutonomousCampaignRequest,
    planner: &mut P,
    executor: &mut E,
) -> Result<GliomaAutonomousCampaign, GliomaAutonomousCampaignError> {
    validate_request(request)?;
    compile_glioma_research(&request.intent)
        .map_err(|error| GliomaAutonomousCampaignError::InvalidRequest(error.to_string()))?;

    let mut registry = BTreeMap::<String, GliomaActionCandidate>::new();
    for candidate in request.initial_candidates.iter().cloned() {
        merge_candidate(&mut registry, candidate)?;
    }
    let mut completed = BTreeSet::new();
    let mut terminal = BTreeSet::new();
    let mut previous_results = Vec::new();
    let mut remaining_budget = request
        .selection
        .budget_units
        .min(request.intent.budget_units);
    let mut rounds = Vec::new();
    let mut completed_order = Vec::new();
    let mut negative_order = Vec::new();
    let mut partial_order = Vec::new();
    let mut failed_order = Vec::new();
    let mut skipped_order = Vec::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative_evidence = BTreeSet::new();
    let mut retry_count = 0_u32;
    let mut stop_reason = GliomaAutonomousCampaignStopReason::MaxRounds;

    for round in 1..=request.max_rounds {
        if remaining_budget == 0 {
            stop_reason = GliomaAutonomousCampaignStopReason::BudgetExhausted;
            break;
        }
        let context = GliomaAutonomousPlannerContext {
            round,
            completed_actions: completed.clone(),
            terminal_actions: terminal.clone(),
            available_action_ids: registry.keys().cloned().collect(),
            budget_remaining_units: remaining_budget,
            previous_results: previous_results.clone(),
        };
        for candidate in planner_round(planner, &context, request.max_retries)? {
            merge_candidate(&mut registry, candidate)?;
        }
        let candidate_pool = registry
            .values()
            .filter(|candidate| {
                !completed.contains(&candidate.action_id)
                    && !terminal.contains(&candidate.action_id)
            })
            .cloned()
            .collect::<Vec<_>>();
        if candidate_pool.is_empty() {
            stop_reason = GliomaAutonomousCampaignStopReason::NoCandidates;
            break;
        }
        let budget_before = remaining_budget;
        let mut selection = request.selection.clone();
        selection.budget_units = remaining_budget;
        let execution = execute_glioma_action_portfolio(
            &ActionPortfolioExecutionRequest {
                candidates: candidate_pool.clone(),
                completed_actions: completed.clone(),
                selection,
                max_retries: request.max_retries,
                require_artifacts: request.require_artifacts,
            },
            executor,
        )
        .map_err(|error| GliomaAutonomousCampaignError::Execution(error.to_string()))?;
        let consumed = execution
            .action_order
            .iter()
            .filter_map(|id| registry.get(id).map(|candidate| candidate.cost_units))
            .sum::<u32>();
        remaining_budget = remaining_budget.saturating_sub(consumed);
        retry_count = retry_count.saturating_add(execution.retry_count);
        for result in &execution.results {
            match result.disposition {
                ActionExecutionDisposition::Completed => {
                    completed.insert(result.action_id.clone());
                    completed_order.push(result.action_id.clone());
                    terminal.insert(result.action_id.clone());
                }
                ActionExecutionDisposition::Negative => {
                    completed.insert(result.action_id.clone());
                    negative_order.push(result.action_id.clone());
                    terminal.insert(result.action_id.clone());
                }
                ActionExecutionDisposition::Partial => {
                    partial_order.push(result.action_id.clone());
                    terminal.insert(result.action_id.clone());
                }
                ActionExecutionDisposition::Failed => {
                    failed_order.push(result.action_id.clone());
                    terminal.insert(result.action_id.clone());
                }
                ActionExecutionDisposition::Skipped => skipped_order.push(result.action_id.clone()),
            }
            uncertainty.extend(result.uncertainty.iter().cloned());
            negative_evidence.extend(result.negative_evidence.iter().cloned());
        }
        uncertainty.extend(execution.uncertainty.iter().cloned());
        negative_evidence.extend(execution.negative_evidence.iter().cloned());
        previous_results = execution.results.clone();
        rounds.push(GliomaAutonomousCampaignRound {
            round,
            candidate_order: candidate_pool
                .iter()
                .map(|candidate| candidate.action_id.clone())
                .collect(),
            budget_before_units: budget_before,
            budget_after_units: remaining_budget,
            execution,
        });
        if !failed_order.is_empty() {
            stop_reason = GliomaAutonomousCampaignStopReason::ExecutorFailed;
            break;
        }
        if !partial_order.is_empty() || !skipped_order.is_empty() {
            stop_reason = GliomaAutonomousCampaignStopReason::DependencyBlocked;
            break;
        }
        if rounds
            .last()
            .is_some_and(|round| round.execution.action_order.is_empty())
        {
            stop_reason = GliomaAutonomousCampaignStopReason::SelectionBlocked;
            break;
        }
        if registry.values().all(|candidate| {
            completed.contains(&candidate.action_id) || terminal.contains(&candidate.action_id)
        }) {
            stop_reason = GliomaAutonomousCampaignStopReason::Completed;
            break;
        }
        if remaining_budget == 0 {
            stop_reason = GliomaAutonomousCampaignStopReason::BudgetExhausted;
            break;
        }
    }

    sorted_unique(&mut completed_order);
    sorted_unique(&mut negative_order);
    sorted_unique(&mut partial_order);
    sorted_unique(&mut failed_order);
    sorted_unique(&mut skipped_order);
    let disposition = if rounds.is_empty() {
        GliomaAutonomousCampaignDisposition::Blocked
    } else if !failed_order.is_empty() {
        GliomaAutonomousCampaignDisposition::Failed
    } else if !partial_order.is_empty()
        || !skipped_order.is_empty()
        || !matches!(stop_reason, GliomaAutonomousCampaignStopReason::Completed)
    {
        GliomaAutonomousCampaignDisposition::Partial
    } else {
        GliomaAutonomousCampaignDisposition::Completed
    };
    let mut output = GliomaAutonomousCampaign {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        research_id: request.intent.research_id.clone(),
        study_id: request.intent.study_id.clone(),
        objective: request.intent.objective.clone(),
        replay_identity: request.intent.replay_identity.clone(),
        rounds,
        completed_order,
        negative_order,
        partial_order,
        failed_order,
        skipped_order,
        retry_count,
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative_evidence.into_iter().collect(),
        disposition,
        stop_reason,
        digest: ContentHash::of_bytes(b"unsealed-glioma-autonomous-campaign"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| GliomaAutonomousCampaignError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p07_protocol_simulation::action_execution::DryRunGliomaActionExecutor;
    use crate::glioma_engine::{
        GliomaModality, GliomaModelSystem, GliomaStageKind, LocalArtifactRef,
    };
    use bioprism_foundation::{AutonomyTier, Effect, PRECLINICAL_BOUNDARY};
    use bioprism_onco::OutputUse;

    fn intent() -> GliomaResearchIntent {
        GliomaResearchIntent {
            research_id: "autonomous-campaign-test".into(),
            study_id: "study-001".into(),
            objective: "test preclinical glioma invasion mechanism".into(),
            output_uses: BTreeSet::from([OutputUse::MethodDevelopment]),
            model_systems: BTreeSet::from([GliomaModelSystem::Organoid]),
            modalities: BTreeSet::from([GliomaModality::Genomics, GliomaModality::Computational]),
            input_artifacts: vec![LocalArtifactRef {
                artifact_id: "local:matrix".into(),
                content_hash: ContentHash::of_bytes(b"matrix"),
                content_type: "application/octet-stream".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            }],
            requested_autonomy: AutonomyTier::A1,
            approval_reference: None,
            budget_units: 10,
            max_retries: 1,
            allow_instrument_execution: false,
            allow_federation: false,
            raw_data_local: true,
            aggregate_only: true,
            replay_identity: ContentHash::of_bytes(b"campaign-replay"),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    fn candidate(id: &str, cost: u32) -> GliomaActionCandidate {
        GliomaActionCandidate {
            action_id: id.into(),
            stage_kind: GliomaStageKind::ExperimentDesign,
            modality: GliomaModality::Genomics,
            model_system: GliomaModelSystem::Organoid,
            depends_on: Vec::new(),
            cost_units: cost,
            information_gain_milli: 800,
            frontier_novelty_milli: 700,
            workflow_leverage_milli: 700,
            cross_stage_unlock_milli: 700,
            reproducibility_safety_milli: 900,
            federation_value_milli: 400,
            feasibility_milli: 900,
            autonomy_tier: AutonomyTier::A1,
            effects: BTreeSet::from([
                Effect::ReadLocalData,
                Effect::ExecuteLocalComputation,
                Effect::WriteLocalArtifact,
            ]),
        }
    }

    #[test]
    fn campaign_executes_seed_portfolio_and_replays() {
        let request = GliomaAutonomousCampaignRequest {
            intent: intent(),
            initial_candidates: vec![candidate("a", 3), candidate("b", 3)],
            selection: GliomaSelectionConfig {
                budget_units: 6,
                max_actions: 2,
                ..GliomaSelectionConfig::default()
            },
            max_rounds: 4,
            max_retries: 1,
            require_artifacts: true,
        };
        let mut planner = StaticGliomaActionPlanner;
        let mut executor = DryRunGliomaActionExecutor;
        let first =
            execute_glioma_autonomous_campaign(&request, &mut planner, &mut executor).unwrap();
        let mut planner = StaticGliomaActionPlanner;
        let mut executor = DryRunGliomaActionExecutor;
        let second =
            execute_glioma_autonomous_campaign(&request, &mut planner, &mut executor).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            first.disposition,
            GliomaAutonomousCampaignDisposition::Completed
        );
        assert_eq!(first.completed_order, vec!["a", "b"]);
        first.validate().unwrap();
    }

    struct ObservationPlanner {
        proposed: bool,
    }

    impl GliomaActionPlanner for ObservationPlanner {
        fn propose_actions(
            &mut self,
            context: &GliomaAutonomousPlannerContext,
        ) -> Result<Vec<GliomaActionCandidate>, GliomaPlannerFailure> {
            if context.round == 1 && !self.proposed {
                self.proposed = true;
                return Ok(vec![candidate("derived", 2)]);
            }
            Ok(Vec::new())
        }
    }

    #[test]
    fn planner_can_add_a_new_round_after_observed_completion() {
        let request = GliomaAutonomousCampaignRequest {
            intent: intent(),
            initial_candidates: vec![candidate("seed", 2)],
            selection: GliomaSelectionConfig {
                budget_units: 4,
                max_actions: 1,
                ..GliomaSelectionConfig::default()
            },
            max_rounds: 4,
            max_retries: 0,
            require_artifacts: true,
        };
        let mut planner = ObservationPlanner { proposed: false };
        let mut executor = DryRunGliomaActionExecutor;
        let output =
            execute_glioma_autonomous_campaign(&request, &mut planner, &mut executor).unwrap();
        assert_eq!(output.rounds.len(), 2);
        assert_eq!(output.completed_order, vec!["derived", "seed"]);
        output.validate().unwrap();
    }

    struct FailOnce;

    impl GliomaActionPlanner for FailOnce {
        fn propose_actions(
            &mut self,
            _context: &GliomaAutonomousPlannerContext,
        ) -> Result<Vec<GliomaActionCandidate>, GliomaPlannerFailure> {
            Err(GliomaPlannerFailure {
                reason: "planner unavailable".into(),
                retryable: false,
            })
        }
    }

    #[test]
    fn planner_failure_is_explicit_and_does_not_dispatch_seed_effects() {
        let request = GliomaAutonomousCampaignRequest {
            intent: intent(),
            initial_candidates: vec![candidate("seed", 2)],
            selection: GliomaSelectionConfig {
                budget_units: 2,
                max_actions: 1,
                ..GliomaSelectionConfig::default()
            },
            max_rounds: 2,
            max_retries: 0,
            require_artifacts: true,
        };
        let mut planner = FailOnce;
        let mut executor = DryRunGliomaActionExecutor;
        let error =
            execute_glioma_autonomous_campaign(&request, &mut planner, &mut executor).unwrap_err();
        assert_eq!(
            error,
            GliomaAutonomousCampaignError::Planner("planner unavailable".into())
        );
    }
}

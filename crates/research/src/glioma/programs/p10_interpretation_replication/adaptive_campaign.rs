//! Bounded multi-round adaptive interpretation campaigns.
//!
//! A single adaptive frontier is useful, but a research engine needs to resynthesize the
//! declared evidence after each local action and decide whether another round is warranted. This
//! controller owns that loop. It never invents evidence: the planner seam must return the next
//! typed `InterpretationSynthesisRequest`, and the bundled planner deliberately stops because
//! synthetic action artifacts are not biological observations.

use super::adaptive_execution::{
    execute_glioma_adaptive_frontier, AdaptiveFrontierExecution,
    AdaptiveFrontierExecutionDisposition, AdaptiveFrontierExecutionError,
    AdaptiveFrontierExecutionRequest,
};
use super::adaptive_frontier::AdaptiveFrontierRequest;
use super::synthesis::{
    synthesize_glioma_interpretation, InterpretationSynthesis, InterpretationSynthesisDisposition,
    InterpretationSynthesisError, InterpretationSynthesisRequest,
};
use crate::glioma::programs::p07_protocol_simulation::{
    DryRunGliomaActionExecutor, GliomaActionExecutor,
};
use crate::glioma_engine::GliomaSelectionWeights;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P10-F26";
pub const OUTPUT_SCHEMA: &str = "GliomaAdaptiveInterpretationCampaign1@1";
pub const MAX_ROUNDS: u16 = 32;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveInterpretationCampaignRequest {
    pub initial_synthesis: InterpretationSynthesisRequest,
    pub completed_actions: BTreeSet<String>,
    pub budget_units: u32,
    pub max_rounds: u16,
    pub max_actions_per_round: u16,
    pub max_retries: u8,
    pub require_artifacts: bool,
    pub allow_unresolved_dispatch: bool,
    pub stop_on_qualified: bool,
    pub stop_on_negative: bool,
    pub approval_granted: bool,
    pub allow_instrument_execution: bool,
    pub allow_federation: bool,
    pub selection_weights: GliomaSelectionWeights,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveInterpretationCampaignRound {
    pub round: u16,
    pub synthesis: InterpretationSynthesis,
    pub execution: Option<AdaptiveFrontierExecution>,
    pub budget_before_units: u32,
    pub budget_after_units: u32,
    pub spent_units: u32,
    pub next_request_planned: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdaptiveInterpretationCampaignDisposition {
    Qualified,
    Negative,
    Partial,
    Held,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdaptiveInterpretationCampaignStopReason {
    Qualified,
    Negative,
    BudgetExhausted,
    MaxRounds,
    FrontierHeld,
    FrontierBlocked,
    ExecutorFailed,
    PlannerNoProgress,
    PlannerFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveInterpretationCampaign {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub hypothesis: String,
    pub rounds: Vec<AdaptiveInterpretationCampaignRound>,
    pub completed_action_order: Vec<String>,
    pub budget_spent_units: u32,
    pub remaining_budget_units: u32,
    pub final_synthesis: InterpretationSynthesis,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: AdaptiveInterpretationCampaignDisposition,
    pub stop_reason: AdaptiveInterpretationCampaignStopReason,
    pub digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdaptiveInterpretationPlanningFailure {
    pub reason: String,
    pub retryable: bool,
}

/// Production deployments provide a planner that converts returned local artifacts into the next
/// typed synthesis request. It must not infer evidence from an action result that does not carry
/// a declared, validated analysis artifact.
pub trait AdaptiveInterpretationPlanner {
    fn plan_next(
        &mut self,
        current_request: &InterpretationSynthesisRequest,
        synthesis: &InterpretationSynthesis,
        execution: &AdaptiveFrontierExecution,
        round: u16,
    ) -> Result<Option<InterpretationSynthesisRequest>, AdaptiveInterpretationPlanningFailure>;
}

/// Synthetic planner. It stops after one round because dry-run action artifacts are not evidence.
#[derive(Debug, Default)]
pub struct DryRunAdaptiveInterpretationPlanner;

impl AdaptiveInterpretationPlanner for DryRunAdaptiveInterpretationPlanner {
    fn plan_next(
        &mut self,
        _current_request: &InterpretationSynthesisRequest,
        _synthesis: &InterpretationSynthesis,
        _execution: &AdaptiveFrontierExecution,
        _round: u16,
    ) -> Result<Option<InterpretationSynthesisRequest>, AdaptiveInterpretationPlanningFailure> {
        Ok(None)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AdaptiveInterpretationCampaignError {
    #[error("adaptive interpretation campaign request is invalid: {0}")]
    InvalidRequest(String),
    #[error("adaptive interpretation synthesis failed: {0}")]
    Synthesis(#[from] InterpretationSynthesisError),
    #[error("adaptive interpretation frontier execution failed: {0}")]
    Frontier(#[from] AdaptiveFrontierExecutionError),
    #[error("adaptive interpretation planner failed: {0}")]
    Planner(String),
    #[error("adaptive interpretation campaign output is invalid: {0}")]
    InvalidOutput(String),
    #[error("adaptive interpretation campaign digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &AdaptiveInterpretationCampaign) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "hypothesis": output.hypothesis,
        "rounds": output.rounds,
        "completed_action_order": output.completed_action_order,
        "budget_spent_units": output.budget_spent_units,
        "remaining_budget_units": output.remaining_budget_units,
        "final_synthesis": output.final_synthesis,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
        "stop_reason": output.stop_reason,
    })
}

fn cost_of_execution(execution: &AdaptiveFrontierExecution) -> u32 {
    execution
        .frontier
        .candidates
        .iter()
        .filter(|candidate| {
            execution
                .dispatched_order
                .contains(&candidate.action.action_id)
        })
        .map(|candidate| candidate.action.cost_units)
        .sum()
}

fn map_disposition(
    synthesis: &InterpretationSynthesis,
    execution: Option<&AdaptiveFrontierExecution>,
) -> AdaptiveInterpretationCampaignDisposition {
    if matches!(
        synthesis.disposition,
        InterpretationSynthesisDisposition::Negative
    ) {
        return AdaptiveInterpretationCampaignDisposition::Negative;
    }
    match execution.map(|output| output.disposition) {
        Some(AdaptiveFrontierExecutionDisposition::Executed) => {
            AdaptiveInterpretationCampaignDisposition::Partial
        }
        Some(AdaptiveFrontierExecutionDisposition::Negative) => {
            AdaptiveInterpretationCampaignDisposition::Negative
        }
        Some(AdaptiveFrontierExecutionDisposition::Partial) => {
            AdaptiveInterpretationCampaignDisposition::Partial
        }
        Some(AdaptiveFrontierExecutionDisposition::Failed) => {
            AdaptiveInterpretationCampaignDisposition::Blocked
        }
        Some(AdaptiveFrontierExecutionDisposition::Held) => {
            AdaptiveInterpretationCampaignDisposition::Held
        }
        Some(AdaptiveFrontierExecutionDisposition::Blocked) => {
            AdaptiveInterpretationCampaignDisposition::Blocked
        }
        Some(AdaptiveFrontierExecutionDisposition::Unresolved) => {
            AdaptiveInterpretationCampaignDisposition::Unresolved
        }
        None => match synthesis.disposition {
            InterpretationSynthesisDisposition::Qualified => {
                AdaptiveInterpretationCampaignDisposition::Qualified
            }
            InterpretationSynthesisDisposition::Negative => {
                AdaptiveInterpretationCampaignDisposition::Negative
            }
            InterpretationSynthesisDisposition::Partial => {
                AdaptiveInterpretationCampaignDisposition::Partial
            }
            InterpretationSynthesisDisposition::Unresolved => {
                AdaptiveInterpretationCampaignDisposition::Unresolved
            }
        },
    }
}

fn validate_request(
    request: &AdaptiveInterpretationCampaignRequest,
) -> Result<(), AdaptiveInterpretationCampaignError> {
    if request.budget_units == 0
        || request.max_rounds == 0
        || request.max_rounds > MAX_ROUNDS
        || request.max_actions_per_round == 0
        || request.max_retries > 8
        || request
            .completed_actions
            .iter()
            .any(|action| action.trim().is_empty())
    {
        return Err(AdaptiveInterpretationCampaignError::InvalidRequest(
            "positive bounded budget, rounds, actions, retries, and completed-action identifiers are required".into(),
        ));
    }
    Ok(())
}

impl AdaptiveInterpretationCampaign {
    pub fn validate(&self) -> Result<(), AdaptiveInterpretationCampaignError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.hypothesis.trim().is_empty()
            || self.rounds.is_empty()
            || self.rounds.len() > usize::from(MAX_ROUNDS)
            || !canonical(&self.completed_action_order)
            || self
                .budget_spent_units
                .saturating_add(self.remaining_budget_units)
                != self
                    .rounds
                    .first()
                    .map_or(0, |round| round.budget_before_units)
            || self.final_synthesis.objective != self.objective
            || self.final_synthesis.hypothesis != self.hypothesis
        {
            return Err(AdaptiveInterpretationCampaignError::InvalidOutput(
                "identity, bounded rounds, budget accounting, ordering, or synthesis binding is invalid".into(),
            ));
        }
        for (index, round) in self.rounds.iter().enumerate() {
            if round.round != index as u16
                || round.synthesis.objective != self.objective
                || round.synthesis.hypothesis != self.hypothesis
                || round.budget_after_units.saturating_add(round.spent_units)
                    != round.budget_before_units
            {
                return Err(AdaptiveInterpretationCampaignError::InvalidOutput(
                    "round numbering, synthesis binding, or round budget accounting is invalid"
                        .into(),
                ));
            }
            round.synthesis.validate().map_err(|error| {
                AdaptiveInterpretationCampaignError::InvalidOutput(error.to_string())
            })?;
            if let Some(execution) = &round.execution {
                execution.validate().map_err(|error| {
                    AdaptiveInterpretationCampaignError::InvalidOutput(error.to_string())
                })?;
                if execution.hypothesis != self.hypothesis {
                    return Err(AdaptiveInterpretationCampaignError::InvalidOutput(
                        "round execution is not bound to the campaign hypothesis".into(),
                    ));
                }
            }
        }
        self.final_synthesis.validate().map_err(|error| {
            AdaptiveInterpretationCampaignError::InvalidOutput(error.to_string())
        })?;
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| AdaptiveInterpretationCampaignError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(AdaptiveInterpretationCampaignError::InvalidOutput(
                "adaptive interpretation campaign digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Run the synthesis → adaptive frontier → local execution → replanning loop.
pub fn execute_glioma_adaptive_interpretation_campaign<
    P: AdaptiveInterpretationPlanner,
    E: GliomaActionExecutor + ?Sized,
>(
    request: &AdaptiveInterpretationCampaignRequest,
    planner: &mut P,
    executor: &mut E,
) -> Result<AdaptiveInterpretationCampaign, AdaptiveInterpretationCampaignError> {
    validate_request(request)?;
    let mut current_request = request.initial_synthesis.clone();
    let mut completed_actions = request.completed_actions.clone();
    let mut remaining_budget = request.budget_units;
    let mut rounds = Vec::new();
    let mut negative_evidence = Vec::new();
    let mut uncertainty = Vec::new();
    let mut campaign_disposition = AdaptiveInterpretationCampaignDisposition::Unresolved;
    let mut stop_reason = AdaptiveInterpretationCampaignStopReason::MaxRounds;
    let mut final_synthesis = None;

    for round_index in 0..request.max_rounds {
        let synthesis = synthesize_glioma_interpretation(&current_request)?;
        negative_evidence.extend(synthesis.negative_evidence.iter().cloned());
        uncertainty.extend(synthesis.uncertainty.iter().cloned());
        final_synthesis = Some(synthesis.clone());
        if request.stop_on_qualified
            && synthesis.disposition == InterpretationSynthesisDisposition::Qualified
        {
            campaign_disposition = AdaptiveInterpretationCampaignDisposition::Qualified;
            stop_reason = AdaptiveInterpretationCampaignStopReason::Qualified;
            rounds.push(AdaptiveInterpretationCampaignRound {
                round: round_index,
                synthesis,
                execution: None,
                budget_before_units: remaining_budget,
                budget_after_units: remaining_budget,
                spent_units: 0,
                next_request_planned: false,
            });
            break;
        }
        if request.stop_on_negative
            && synthesis.disposition == InterpretationSynthesisDisposition::Negative
        {
            campaign_disposition = AdaptiveInterpretationCampaignDisposition::Negative;
            stop_reason = AdaptiveInterpretationCampaignStopReason::Negative;
            rounds.push(AdaptiveInterpretationCampaignRound {
                round: round_index,
                synthesis,
                execution: None,
                budget_before_units: remaining_budget,
                budget_after_units: remaining_budget,
                spent_units: 0,
                next_request_planned: false,
            });
            break;
        }
        if remaining_budget == 0 {
            campaign_disposition = AdaptiveInterpretationCampaignDisposition::Blocked;
            stop_reason = AdaptiveInterpretationCampaignStopReason::BudgetExhausted;
            rounds.push(AdaptiveInterpretationCampaignRound {
                round: round_index,
                synthesis,
                execution: None,
                budget_before_units: 0,
                budget_after_units: 0,
                spent_units: 0,
                next_request_planned: false,
            });
            break;
        }
        let frontier_request = AdaptiveFrontierRequest {
            synthesis: synthesis.clone(),
            completed_actions: completed_actions.clone(),
            budget_units: remaining_budget,
            max_actions: request.max_actions_per_round,
            approval_granted: request.approval_granted,
            allow_instrument_execution: request.allow_instrument_execution,
            allow_federation: request.allow_federation,
            selection_weights: request.selection_weights,
        };
        let execution = execute_glioma_adaptive_frontier(
            &AdaptiveFrontierExecutionRequest {
                frontier: frontier_request,
                max_retries: request.max_retries,
                require_artifacts: request.require_artifacts,
                allow_unresolved_dispatch: request.allow_unresolved_dispatch,
            },
            executor,
        )?;
        let spent_units = cost_of_execution(&execution).min(remaining_budget);
        let budget_before = remaining_budget;
        remaining_budget = remaining_budget.saturating_sub(spent_units);
        completed_actions.extend(execution.completed_order.iter().cloned());
        completed_actions.extend(execution.negative_order.iter().cloned());
        negative_evidence.extend(execution.negative_evidence.iter().cloned());
        uncertainty.extend(execution.uncertainty.iter().cloned());
        let round_disposition = map_disposition(&synthesis, Some(&execution));
        campaign_disposition = round_disposition;
        let should_stop = match execution.disposition {
            AdaptiveFrontierExecutionDisposition::Held => {
                stop_reason = AdaptiveInterpretationCampaignStopReason::FrontierHeld;
                true
            }
            AdaptiveFrontierExecutionDisposition::Blocked => {
                stop_reason = AdaptiveInterpretationCampaignStopReason::FrontierBlocked;
                true
            }
            AdaptiveFrontierExecutionDisposition::Failed => {
                stop_reason = AdaptiveInterpretationCampaignStopReason::ExecutorFailed;
                true
            }
            AdaptiveFrontierExecutionDisposition::Negative if request.stop_on_negative => {
                stop_reason = AdaptiveInterpretationCampaignStopReason::Negative;
                true
            }
            AdaptiveFrontierExecutionDisposition::Executed
            | AdaptiveFrontierExecutionDisposition::Negative
            | AdaptiveFrontierExecutionDisposition::Partial
            | AdaptiveFrontierExecutionDisposition::Unresolved => false,
        };
        if should_stop || round_index.saturating_add(1) >= request.max_rounds {
            if !should_stop {
                stop_reason = AdaptiveInterpretationCampaignStopReason::MaxRounds;
            }
            rounds.push(AdaptiveInterpretationCampaignRound {
                round: round_index,
                synthesis,
                execution: Some(execution),
                budget_before_units: budget_before,
                budget_after_units: remaining_budget,
                spent_units,
                next_request_planned: false,
            });
            break;
        }
        let next = planner
            .plan_next(&current_request, &synthesis, &execution, round_index)
            .map_err(|failure| AdaptiveInterpretationCampaignError::Planner(failure.reason))?;
        let Some(next_request) = next else {
            stop_reason = AdaptiveInterpretationCampaignStopReason::PlannerNoProgress;
            rounds.push(AdaptiveInterpretationCampaignRound {
                round: round_index,
                synthesis,
                execution: Some(execution),
                budget_before_units: budget_before,
                budget_after_units: remaining_budget,
                spent_units,
                next_request_planned: false,
            });
            break;
        };
        if next_request.objective != current_request.objective
            || next_request.hypothesis != current_request.hypothesis
            || next_request.model_system != current_request.model_system
            || next_request.replay_identity != current_request.replay_identity
        {
            return Err(AdaptiveInterpretationCampaignError::Planner(
                "planner changed the campaign objective, hypothesis, model system, or replay identity".into(),
            ));
        }
        let current_evidence = serde_json::to_value(&current_request.evidence)
            .map_err(|error| AdaptiveInterpretationCampaignError::Digest(error.to_string()))?;
        let next_evidence = serde_json::to_value(&next_request.evidence)
            .map_err(|error| AdaptiveInterpretationCampaignError::Digest(error.to_string()))?;
        let current_evidence_digest = ContentHash::of_value(&current_evidence)
            .map_err(|error| AdaptiveInterpretationCampaignError::Digest(error.to_string()))?;
        let next_evidence_digest = ContentHash::of_value(&next_evidence)
            .map_err(|error| AdaptiveInterpretationCampaignError::Digest(error.to_string()))?;
        let next_request_planned = current_evidence_digest != next_evidence_digest;
        rounds.push(AdaptiveInterpretationCampaignRound {
            round: round_index,
            synthesis,
            execution: Some(execution),
            budget_before_units: budget_before,
            budget_after_units: remaining_budget,
            spent_units,
            next_request_planned,
        });
        if !next_request_planned {
            stop_reason = AdaptiveInterpretationCampaignStopReason::PlannerNoProgress;
            break;
        }
        current_request = next_request;
    }

    let final_synthesis = final_synthesis.ok_or_else(|| {
        AdaptiveInterpretationCampaignError::InvalidOutput(
            "campaign produced no synthesis round".into(),
        )
    })?;
    if request.stop_on_qualified
        && final_synthesis.disposition == InterpretationSynthesisDisposition::Qualified
    {
        campaign_disposition = AdaptiveInterpretationCampaignDisposition::Qualified;
    } else if request.stop_on_negative
        && final_synthesis.disposition == InterpretationSynthesisDisposition::Negative
    {
        campaign_disposition = AdaptiveInterpretationCampaignDisposition::Negative;
    }
    let mut completed_action_order = completed_actions.into_iter().collect::<Vec<_>>();
    completed_action_order.sort();
    negative_evidence.sort();
    negative_evidence.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    let mut output = AdaptiveInterpretationCampaign {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.initial_synthesis.objective.clone(),
        hypothesis: request.initial_synthesis.hypothesis.clone(),
        rounds,
        completed_action_order,
        budget_spent_units: request.budget_units.saturating_sub(remaining_budget),
        remaining_budget_units: remaining_budget,
        final_synthesis,
        negative_evidence,
        uncertainty,
        disposition: campaign_disposition,
        stop_reason,
        digest: ContentHash::of_bytes(b"unsealed-glioma-adaptive-interpretation-campaign"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| AdaptiveInterpretationCampaignError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

pub fn execute_glioma_adaptive_interpretation_campaign_dry_run(
    request: &AdaptiveInterpretationCampaignRequest,
) -> Result<AdaptiveInterpretationCampaign, AdaptiveInterpretationCampaignError> {
    let mut planner = DryRunAdaptiveInterpretationPlanner;
    let mut executor = DryRunGliomaActionExecutor;
    execute_glioma_adaptive_interpretation_campaign(request, &mut planner, &mut executor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p10_interpretation_replication::{
        InterpretationEvidence, InterpretationEvidenceDirection, InterpretationEvidenceFamily,
    };
    use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};

    fn request() -> AdaptiveInterpretationCampaignRequest {
        let hash = ContentHash::of_bytes(b"adaptive-interpretation-campaign");
        let evidence = vec![
            InterpretationEvidence {
                evidence_id: "causal-a".into(),
                family: InterpretationEvidenceFamily::CausalContrast,
                independent_group: "site-a".into(),
                model_system: GliomaModelSystem::Organoid,
                direction: InterpretationEvidenceDirection::Positive,
                effect_milli: 300,
                uncertainty_milli: 50,
                quality_milli: 900,
                sample_count: 6,
                artifact: LocalArtifactRef {
                    artifact_id: "causal-a".into(),
                    content_hash: hash.clone(),
                    content_type: "application/json".into(),
                    local_only: true,
                    contains_human_data: false,
                    contains_direct_identifiers: false,
                },
                negative_evidence: Vec::new(),
            },
            InterpretationEvidence {
                evidence_id: "replication-b".into(),
                family: InterpretationEvidenceFamily::Replication,
                independent_group: "site-b".into(),
                model_system: GliomaModelSystem::Organoid,
                direction: InterpretationEvidenceDirection::Positive,
                effect_milli: 280,
                uncertainty_milli: 60,
                quality_milli: 850,
                sample_count: 8,
                artifact: LocalArtifactRef {
                    artifact_id: "replication-b".into(),
                    content_hash: hash.clone(),
                    content_type: "application/json".into(),
                    local_only: true,
                    contains_human_data: false,
                    contains_direct_identifiers: false,
                },
                negative_evidence: Vec::new(),
            },
        ];
        AdaptiveInterpretationCampaignRequest {
            initial_synthesis: InterpretationSynthesisRequest {
                objective: "adaptive interpretation campaign".into(),
                hypothesis: "a preclinical invasion mechanism is reproducible".into(),
                model_system: GliomaModelSystem::Organoid,
                min_evidence: 2,
                min_independent_groups: 2,
                min_families: 2,
                min_quality_milli: 700,
                effect_threshold_milli: 100,
                max_disagreement_milli: 700,
                max_leave_one_out_shift_milli: 700,
                require_replication_family: true,
                replay_identity: hash,
                evidence,
            },
            completed_actions: BTreeSet::new(),
            budget_units: 80,
            max_rounds: 3,
            max_actions_per_round: 3,
            max_retries: 1,
            require_artifacts: true,
            allow_unresolved_dispatch: false,
            stop_on_qualified: false,
            stop_on_negative: true,
            approval_granted: true,
            allow_instrument_execution: false,
            allow_federation: false,
            selection_weights: GliomaSelectionWeights::default(),
        }
    }

    #[test]
    fn dry_run_executes_one_round_then_refuses_to_fabricate_reanalysis() {
        let request = request();
        let first = execute_glioma_adaptive_interpretation_campaign_dry_run(&request).unwrap();
        let second = execute_glioma_adaptive_interpretation_campaign_dry_run(&request).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.rounds.len(), 1);
        assert_eq!(
            first.stop_reason,
            AdaptiveInterpretationCampaignStopReason::PlannerNoProgress
        );
        assert_eq!(
            first.rounds[0].execution.as_ref().unwrap().disposition,
            AdaptiveFrontierExecutionDisposition::Executed
        );
        assert!(first
            .negative_evidence
            .iter()
            .any(|item| item.contains("synthetic-dry-run")));
    }

    #[test]
    fn unresolved_synthesis_stops_as_a_frontier_hold() {
        let mut request = request();
        request.initial_synthesis.min_families = 4;
        let output = execute_glioma_adaptive_interpretation_campaign_dry_run(&request).unwrap();
        assert_eq!(
            output.stop_reason,
            AdaptiveInterpretationCampaignStopReason::FrontierHeld
        );
        assert_eq!(
            output.disposition,
            AdaptiveInterpretationCampaignDisposition::Held
        );
        assert!(output.rounds[0].execution.is_some());
    }
}

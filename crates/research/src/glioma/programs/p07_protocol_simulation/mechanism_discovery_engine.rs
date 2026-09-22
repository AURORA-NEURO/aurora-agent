//! Autonomous mechanism-discovery engine for preclinical glioma research.
//!
//! This vertical composes the scientific algorithms already owned by P03, P05, P06, and P07
//! into one bounded research loop.  It does not collapse a simulation into evidence: multimodal
//! graph fusion, pathway activity, signed feedback dynamics, and model-ensemble intervention
//! robustness remain separate typed gates.  Only when those gates are adequate does the engine
//! dispatch a dependency-safe local action portfolio.  Returned action outcomes retire or reopen
//! work for the next round, so the useful product is an executable mechanism-to-assay program,
//! not a receipt stream.

use super::super::p03_multimodal_ingestion_qc::{
    analyze_glioma_multimodal_graph_fusion, GraphFusionAnalysis, GraphFusionDisposition,
    GraphFusionVector,
};
use super::super::p05_mechanism_exploration::{
    analyze_glioma_pathway_activity, plan_glioma_robust_intervention_portfolio,
    simulate_glioma_mechanism_dynamics, CounterfactualModel, MechanismDynamicsDisposition,
    MechanismDynamicsEdge, MechanismDynamicsIntervention, MechanismDynamicsNode,
    MechanismDynamicsPlan, MechanismDynamicsRequest, PathwayActivityAnalysis,
    PathwayActivityDefinition, PathwayActivityDisposition, PathwayActivityObservation,
    RobustInterventionCandidate, RobustInterventionPortfolio, RobustInterventionRequest,
    RobustPortfolioDisposition,
};
use super::action_execution::GliomaActionExecutor;
use super::mechanism_campaign::{
    execute_glioma_multimodal_mechanism_campaign,
    execute_glioma_multimodal_mechanism_campaign_with_executor, MechanismCampaignDisposition,
    MechanismCampaignError, MultimodalMechanismCampaign, MultimodalMechanismCampaignExecution,
    MultimodalMechanismCampaignRequest,
};
use crate::glioma_engine::{GliomaActionCandidate, GliomaModelSystem, GliomaSelectionConfig};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P07-F16";
pub const OUTPUT_SCHEMA: &str = "GliomaMechanismDiscoveryEngine1@1";
pub const MAX_ROUNDS: u16 = 16;
pub const MAX_ACTIONS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaMechanismDiscoveryRequest {
    pub objective: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub campaign: MultimodalMechanismCampaignRequest,
    pub graph_vectors: Vec<GraphFusionVector>,
    pub pathway_definitions: Vec<PathwayActivityDefinition>,
    pub pathway_observations: Vec<PathwayActivityObservation>,
    pub dynamics: MechanismDynamicsRequest,
    pub dynamics_nodes: Vec<MechanismDynamicsNode>,
    pub dynamics_edges: Vec<MechanismDynamicsEdge>,
    pub dynamics_interventions: Vec<MechanismDynamicsIntervention>,
    pub robust: RobustInterventionRequest,
    pub robust_models: Vec<CounterfactualModel>,
    pub robust_candidates: Vec<RobustInterventionCandidate>,
    pub action_candidates: Vec<GliomaActionCandidate>,
    pub selection: GliomaSelectionConfig,
    pub completed_action_order: Vec<String>,
    pub budget_units: u32,
    pub max_actions: u16,
    pub max_rounds: u16,
    pub max_retries: u8,
    pub require_artifacts: bool,
    pub require_qualified_multimodal: bool,
    pub require_stable_dynamics: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GliomaMechanismDiscoveryDisposition {
    Completed,
    Partial,
    EvidenceBlocked,
    BudgetExhausted,
    NoRunnableActions,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GliomaMechanismDiscoveryStopReason {
    Qualified,
    MultimodalEvidenceBlocked,
    DynamicsBlocked,
    RobustPortfolioBlocked,
    BudgetExhausted,
    NoRunnableActions,
    MaxRounds,
    ExecutorFailed,
    NoProgress,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaMechanismDiscoveryRound {
    pub round: u16,
    pub budget_before_units: u32,
    pub budget_after_units: u32,
    pub graph: GraphFusionAnalysis,
    pub pathway: PathwayActivityAnalysis,
    pub dynamics: MechanismDynamicsPlan,
    pub robust_portfolio: RobustInterventionPortfolio,
    pub campaign: MultimodalMechanismCampaign,
    pub execution: Option<MultimodalMechanismCampaignExecution>,
    pub attempted_action_order: Vec<String>,
    pub executed_action_order: Vec<String>,
    pub newly_completed_order: Vec<String>,
    pub newly_negative_order: Vec<String>,
    pub newly_failed_order: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub cost_units: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaMechanismDiscoveryRun {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub rounds: Vec<GliomaMechanismDiscoveryRound>,
    pub completed_action_order: Vec<String>,
    pub negative_action_order: Vec<String>,
    pub failed_action_order: Vec<String>,
    pub retired_action_order: Vec<String>,
    pub budget_spent_units: u32,
    pub remaining_budget_units: u32,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: GliomaMechanismDiscoveryDisposition,
    pub stop_reason: GliomaMechanismDiscoveryStopReason,
    pub next_step: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GliomaMechanismDiscoveryError {
    #[error("glioma mechanism discovery request is invalid: {0}")]
    InvalidRequest(String),
    #[error("glioma mechanism discovery analysis failed: {0}")]
    Analysis(String),
    #[error("glioma mechanism discovery campaign failed: {0}")]
    Campaign(#[from] MechanismCampaignError),
    #[error("glioma mechanism discovery output is invalid: {0}")]
    InvalidOutput(String),
    #[error("glioma mechanism discovery digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(run: &GliomaMechanismDiscoveryRun) -> serde_json::Value {
    serde_json::json!({
        "feature_id": run.feature_id,
        "output_schema": run.output_schema,
        "objective": run.objective,
        "study_id": run.study_id,
        "model_system": run.model_system,
        "rounds": run.rounds,
        "completed_action_order": run.completed_action_order,
        "negative_action_order": run.negative_action_order,
        "failed_action_order": run.failed_action_order,
        "retired_action_order": run.retired_action_order,
        "budget_spent_units": run.budget_spent_units,
        "remaining_budget_units": run.remaining_budget_units,
        "negative_evidence": run.negative_evidence,
        "uncertainty": run.uncertainty,
        "disposition": run.disposition,
        "stop_reason": run.stop_reason,
        "next_step": run.next_step,
    })
}

fn validate_request(
    request: &GliomaMechanismDiscoveryRequest,
) -> Result<(), GliomaMechanismDiscoveryError> {
    if request.objective.trim().is_empty()
        || request.study_id.trim().is_empty()
        || request.model_system != request.campaign.model_system
        || request.study_id != request.campaign.study_id
        || request.objective != request.campaign.objective
        || request.campaign.graph.study_id != request.study_id
        || request.campaign.pathway.study_id != request.study_id
        || request.campaign.graph.model_system != request.model_system
        || request.campaign.pathway.model_system != request.model_system
        || request.dynamics.model_system != request.model_system
        || request.robust.model_system != request.model_system
        || request.dynamics.objective != request.objective
        || request.robust.objective != request.objective
        || request.budget_units == 0
        || request.max_rounds == 0
        || request.max_rounds > MAX_ROUNDS
        || request.max_actions == 0
        || usize::from(request.max_actions) > MAX_ACTIONS
        || request.selection.budget_units == 0
        || request.selection.max_actions == 0
        || request.max_retries > super::action_execution::MAX_RETRIES
        || !canonical(&request.completed_action_order)
        || request
            .completed_action_order
            .iter()
            .any(|id| id.trim().is_empty())
        || request.action_candidates.len() > MAX_ACTIONS
    {
        return Err(GliomaMechanismDiscoveryError::InvalidRequest(
            "objective, study/model bindings, bounded rounds/actions/budget, canonical completion, and retry bounds are required".into(),
        ));
    }
    let known = request
        .action_candidates
        .iter()
        .map(|candidate| candidate.action_id.clone())
        .collect::<BTreeSet<_>>();
    if known.len() != request.action_candidates.len()
        || request
            .completed_action_order
            .iter()
            .any(|id| !known.contains(id))
    {
        return Err(GliomaMechanismDiscoveryError::InvalidRequest(
            "action candidates and completed actions must be unique and bound to one another"
                .into(),
        ));
    }
    Ok(())
}

fn cost_by_action(candidates: &[GliomaActionCandidate]) -> std::collections::BTreeMap<String, u32> {
    candidates
        .iter()
        .map(|candidate| (candidate.action_id.clone(), candidate.cost_units))
        .collect()
}

fn set_difference(values: &[String], known: &BTreeSet<String>) -> Vec<String> {
    values
        .iter()
        .filter(|value| !known.contains(*value))
        .cloned()
        .collect()
}

impl GliomaMechanismDiscoveryRun {
    pub fn validate(&self) -> Result<(), GliomaMechanismDiscoveryError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.study_id.trim().is_empty()
            || self.rounds.len() > usize::from(MAX_ROUNDS)
            || !canonical(&self.completed_action_order)
            || !canonical(&self.negative_action_order)
            || !canonical(&self.failed_action_order)
            || !canonical(&self.retired_action_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self
                .completed_action_order
                .iter()
                .any(|id| self.retired_action_order.binary_search(id).is_ok())
            || self
                .negative_action_order
                .iter()
                .any(|id| self.failed_action_order.binary_search(id).is_ok())
            || self.next_step.trim().is_empty()
            || self
                .budget_spent_units
                .saturating_add(self.remaining_budget_units)
                == 0
        {
            return Err(GliomaMechanismDiscoveryError::InvalidOutput(
                "identity, ordering, action partitions, budget, or next-step invariants are invalid".into(),
            ));
        }
        let mut seen_rounds = BTreeSet::new();
        let mut expected_spend = 0_u32;
        for round in &self.rounds {
            if round.round == 0
                || round.round > MAX_ROUNDS
                || !seen_rounds.insert(round.round)
                || round.budget_after_units > round.budget_before_units
                || round
                    .budget_before_units
                    .saturating_sub(round.budget_after_units)
                    != round.cost_units
                || !canonical(&round.attempted_action_order)
                || !canonical(&round.executed_action_order)
                || !canonical(&round.newly_completed_order)
                || !canonical(&round.newly_negative_order)
                || !canonical(&round.newly_failed_order)
                || !canonical(&round.uncertainty)
                || !canonical(&round.negative_evidence)
                || round
                    .executed_action_order
                    .iter()
                    .any(|id| !round.attempted_action_order.contains(id))
                || round
                    .newly_completed_order
                    .iter()
                    .any(|id| round.newly_negative_order.binary_search(id).is_ok())
                || round
                    .newly_completed_order
                    .iter()
                    .any(|id| round.newly_failed_order.binary_search(id).is_ok())
            {
                return Err(GliomaMechanismDiscoveryError::InvalidOutput(
                    "round ordering, budget, and action partitions are invalid".into(),
                ));
            }
            round
                .graph
                .validate()
                .map_err(|error| GliomaMechanismDiscoveryError::InvalidOutput(error.to_string()))?;
            round
                .pathway
                .validate()
                .map_err(|error| GliomaMechanismDiscoveryError::InvalidOutput(error.to_string()))?;
            round
                .dynamics
                .validate()
                .map_err(|error| GliomaMechanismDiscoveryError::InvalidOutput(error.to_string()))?;
            round
                .robust_portfolio
                .validate()
                .map_err(|error| GliomaMechanismDiscoveryError::InvalidOutput(error.to_string()))?;
            round.campaign.validate()?;
            if let Some(execution) = &round.execution {
                execution.validate()?;
                if execution.campaign.digest != round.campaign.digest
                    || execution.campaign.objective != self.objective
                    || execution.campaign.study_id != self.study_id
                    || execution.executed_order != round.executed_action_order
                {
                    return Err(GliomaMechanismDiscoveryError::InvalidOutput(
                        "execution is not bound to the round campaign".into(),
                    ));
                }
                if let Some(portfolio) = &execution.execution {
                    if portfolio.action_order != round.attempted_action_order {
                        return Err(GliomaMechanismDiscoveryError::InvalidOutput(
                            "execution action order does not match attempted order".into(),
                        ));
                    }
                }
            } else if !round.attempted_action_order.is_empty() {
                return Err(GliomaMechanismDiscoveryError::InvalidOutput(
                    "an execution-less round cannot report attempted actions".into(),
                ));
            }
            expected_spend = expected_spend.saturating_add(round.cost_units);
        }
        if expected_spend != self.budget_spent_units
            || self
                .completed_action_order
                .iter()
                .any(|id| self.negative_action_order.binary_search(id).is_ok())
            || self
                .completed_action_order
                .iter()
                .any(|id| self.failed_action_order.binary_search(id).is_ok())
        {
            return Err(GliomaMechanismDiscoveryError::InvalidOutput(
                "round spending or action partitions do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| GliomaMechanismDiscoveryError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(GliomaMechanismDiscoveryError::InvalidOutput(
                "mechanism discovery digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn gate_ready(
    request: &GliomaMechanismDiscoveryRequest,
    graph: &GraphFusionAnalysis,
    pathway: &PathwayActivityAnalysis,
    dynamics: &MechanismDynamicsPlan,
    robust: &RobustInterventionPortfolio,
    campaign: &MultimodalMechanismCampaign,
) -> Option<GliomaMechanismDiscoveryStopReason> {
    if request.require_qualified_multimodal
        && (graph.disposition != GraphFusionDisposition::Qualified
            || pathway.disposition != PathwayActivityDisposition::Qualified)
    {
        return Some(GliomaMechanismDiscoveryStopReason::MultimodalEvidenceBlocked);
    }
    if request.require_stable_dynamics
        && !matches!(dynamics.disposition, MechanismDynamicsDisposition::Stable)
    {
        return Some(GliomaMechanismDiscoveryStopReason::DynamicsBlocked);
    }
    if robust.disposition != RobustPortfolioDisposition::Qualified {
        return Some(GliomaMechanismDiscoveryStopReason::RobustPortfolioBlocked);
    }
    if campaign.disposition == MechanismCampaignDisposition::Unresolved {
        return Some(GliomaMechanismDiscoveryStopReason::NoRunnableActions);
    }
    None
}

/// Run the bounded mechanism-discovery vertical.  The only effectful seam is the caller-owned
/// action executor; all scientific calculations remain local, deterministic, and replayable.
pub fn execute_glioma_mechanism_discovery_engine<E: GliomaActionExecutor + ?Sized>(
    request: &GliomaMechanismDiscoveryRequest,
    executor: &mut E,
) -> Result<GliomaMechanismDiscoveryRun, GliomaMechanismDiscoveryError> {
    validate_request(request)?;
    let action_costs = cost_by_action(&request.action_candidates);
    let mut completed = request
        .completed_action_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut negative_actions = BTreeSet::new();
    let mut failed_actions = BTreeSet::new();
    let mut retired = BTreeSet::new();
    let mut rounds = Vec::new();
    let mut negative_evidence = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut spent = 0_u32;
    let mut stop_reason = GliomaMechanismDiscoveryStopReason::MaxRounds;

    for round_number in 1..=request.max_rounds {
        let remaining = request.budget_units.saturating_sub(spent);
        if remaining == 0 {
            stop_reason = GliomaMechanismDiscoveryStopReason::BudgetExhausted;
            break;
        }
        let graph =
            analyze_glioma_multimodal_graph_fusion(&request.campaign.graph, &request.graph_vectors)
                .map_err(|error| GliomaMechanismDiscoveryError::Analysis(error.to_string()))?;
        let pathway = analyze_glioma_pathway_activity(
            &request.campaign.pathway,
            &request.pathway_definitions,
            &request.pathway_observations,
        )
        .map_err(|error| GliomaMechanismDiscoveryError::Analysis(error.to_string()))?;
        let dynamics = simulate_glioma_mechanism_dynamics(
            &request.dynamics,
            &request.dynamics_nodes,
            &request.dynamics_edges,
            &request.dynamics_interventions,
        )
        .map_err(|error| GliomaMechanismDiscoveryError::Analysis(error.to_string()))?;
        let robust = plan_glioma_robust_intervention_portfolio(
            &request.robust,
            &request.robust_models,
            &request.robust_candidates,
        )
        .map_err(|error| GliomaMechanismDiscoveryError::Analysis(error.to_string()))?;

        let mut campaign_request = request.campaign.clone();
        campaign_request.completed_action_order =
            completed.union(&retired).cloned().collect::<Vec<_>>();
        let mut selection = request.selection.clone();
        selection.budget_units = selection.budget_units.min(remaining);
        selection.max_actions = selection.max_actions.min(request.max_actions);
        campaign_request.selection = selection;
        let campaign = execute_glioma_multimodal_mechanism_campaign(
            &campaign_request,
            &request.graph_vectors,
            &request.pathway_definitions,
            &request.pathway_observations,
            &request.action_candidates,
        )?;
        negative_evidence.extend(graph.negative_evidence.iter().cloned());
        negative_evidence.extend(pathway.negative_evidence.iter().cloned());
        negative_evidence.extend(dynamics.negative_evidence.iter().cloned());
        negative_evidence.extend(robust.negative_evidence.iter().cloned());
        negative_evidence.extend(campaign.negative_evidence.iter().cloned());
        uncertainty.extend(graph.uncertainty.iter().cloned());
        uncertainty.extend(pathway.uncertainty.iter().cloned());
        uncertainty.extend(dynamics.uncertainty.iter().cloned());
        uncertainty.extend(robust.uncertainty.iter().cloned());
        uncertainty.extend(campaign.uncertainty.iter().cloned());

        let gate = gate_ready(request, &graph, &pathway, &dynamics, &robust, &campaign);
        if let Some(reason) = gate {
            stop_reason = reason;
            let mut round_uncertainty = uncertainty.iter().cloned().collect::<Vec<_>>();
            round_uncertainty.sort();
            let mut round_negative = negative_evidence.iter().cloned().collect::<Vec<_>>();
            round_negative.sort();
            rounds.push(GliomaMechanismDiscoveryRound {
                round: round_number,
                budget_before_units: remaining,
                budget_after_units: remaining,
                graph,
                pathway,
                dynamics,
                robust_portfolio: robust,
                campaign,
                execution: None,
                attempted_action_order: Vec::new(),
                executed_action_order: Vec::new(),
                newly_completed_order: Vec::new(),
                newly_negative_order: Vec::new(),
                newly_failed_order: Vec::new(),
                uncertainty: round_uncertainty,
                negative_evidence: round_negative,
                cost_units: 0,
            });
            break;
        }

        let execution = execute_glioma_multimodal_mechanism_campaign_with_executor(
            &campaign_request,
            &request.graph_vectors,
            &request.pathway_definitions,
            &request.pathway_observations,
            &request.action_candidates,
            request.max_retries,
            request.require_artifacts,
            executor,
        )?;
        let attempted = execution
            .execution
            .as_ref()
            .map(|value| value.action_order.clone())
            .unwrap_or_default();
        let executed = execution.executed_order.clone();
        let newly_completed = set_difference(&executed, &completed);
        let newly_negative = execution
            .execution
            .as_ref()
            .map(|value| set_difference(&value.negative_order, &negative_actions))
            .unwrap_or_default();
        let newly_failed = execution
            .execution
            .as_ref()
            .map(|value| set_difference(&value.failed_order, &failed_actions))
            .unwrap_or_default();
        let cost = attempted
            .iter()
            .filter_map(|id| action_costs.get(id))
            .copied()
            .sum::<u32>();
        let budget_after = remaining.saturating_sub(cost);
        if cost == 0 && attempted.is_empty() {
            stop_reason = if matches!(
                execution.disposition,
                super::mechanism_campaign::MechanismCampaignExecutionDisposition::Blocked
            ) {
                GliomaMechanismDiscoveryStopReason::NoRunnableActions
            } else {
                GliomaMechanismDiscoveryStopReason::NoProgress
            };
        }
        completed.extend(newly_completed.iter().cloned());
        negative_actions.extend(newly_negative.iter().cloned());
        failed_actions.extend(newly_failed.iter().cloned());
        retired.extend(newly_negative.iter().cloned());
        retired.extend(newly_failed.iter().cloned());
        negative_evidence.extend(execution.negative_evidence.iter().cloned());
        uncertainty.extend(execution.uncertainty.iter().cloned());
        rounds.push(GliomaMechanismDiscoveryRound {
            round: round_number,
            budget_before_units: remaining,
            budget_after_units: budget_after,
            graph,
            pathway,
            dynamics,
            robust_portfolio: robust,
            campaign,
            execution: Some(execution),
            attempted_action_order: attempted,
            executed_action_order: executed,
            newly_completed_order: newly_completed,
            newly_negative_order: newly_negative,
            newly_failed_order: newly_failed,
            uncertainty: uncertainty.iter().cloned().collect(),
            negative_evidence: negative_evidence.iter().cloned().collect(),
            cost_units: cost,
        });
        spent = spent.saturating_add(cost);
        if stop_reason == GliomaMechanismDiscoveryStopReason::NoRunnableActions
            || stop_reason == GliomaMechanismDiscoveryStopReason::NoProgress
        {
            break;
        }
        if completed.len() + retired.len() >= request.action_candidates.len() {
            stop_reason = GliomaMechanismDiscoveryStopReason::Qualified;
            break;
        }
        if budget_after == 0 {
            stop_reason = GliomaMechanismDiscoveryStopReason::BudgetExhausted;
            break;
        }
    }

    let disposition = match stop_reason {
        GliomaMechanismDiscoveryStopReason::Qualified => {
            GliomaMechanismDiscoveryDisposition::Completed
        }
        GliomaMechanismDiscoveryStopReason::MultimodalEvidenceBlocked
        | GliomaMechanismDiscoveryStopReason::DynamicsBlocked
        | GliomaMechanismDiscoveryStopReason::RobustPortfolioBlocked => {
            GliomaMechanismDiscoveryDisposition::EvidenceBlocked
        }
        GliomaMechanismDiscoveryStopReason::BudgetExhausted => {
            GliomaMechanismDiscoveryDisposition::BudgetExhausted
        }
        GliomaMechanismDiscoveryStopReason::NoRunnableActions => {
            GliomaMechanismDiscoveryDisposition::NoRunnableActions
        }
        GliomaMechanismDiscoveryStopReason::ExecutorFailed => {
            GliomaMechanismDiscoveryDisposition::Failed
        }
        GliomaMechanismDiscoveryStopReason::NoProgress
        | GliomaMechanismDiscoveryStopReason::MaxRounds => {
            GliomaMechanismDiscoveryDisposition::Partial
        }
    };
    let next_step = match stop_reason {
        GliomaMechanismDiscoveryStopReason::Qualified => {
            "route the completed local mechanism artifacts to independent replication and interpretation"
        }
        GliomaMechanismDiscoveryStopReason::MultimodalEvidenceBlocked => {
            "acquire the missing or contradictory modality evidence before dispatching a mechanism assay"
        }
        GliomaMechanismDiscoveryStopReason::DynamicsBlocked => {
            "review feedback instability or sensitivity debt before treating an intervention as assay-ready"
        }
        GliomaMechanismDiscoveryStopReason::RobustPortfolioBlocked => {
            "add model support or redesign the intervention portfolio until lower-tail robustness clears"
        }
        GliomaMechanismDiscoveryStopReason::BudgetExhausted => {
            "resume with an explicit additional local budget and the returned action partitions"
        }
        GliomaMechanismDiscoveryStopReason::NoRunnableActions => {
            "inspect dependency, approval, and evidence gates for the next mechanism action"
        }
        GliomaMechanismDiscoveryStopReason::MaxRounds => {
            "resume from the returned completed, negative, and failed action partitions"
        }
        GliomaMechanismDiscoveryStopReason::ExecutorFailed => {
            "inspect the local executor failure and preserve its negative and replay evidence"
        }
        GliomaMechanismDiscoveryStopReason::NoProgress => {
            "provide a new typed observation or revise the bounded mechanism candidate set"
        }
    }
    .into();
    let mut output = GliomaMechanismDiscoveryRun {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        study_id: request.study_id.clone(),
        model_system: request.model_system,
        rounds,
        completed_action_order: completed.into_iter().collect(),
        negative_action_order: negative_actions.into_iter().collect(),
        failed_action_order: failed_actions.into_iter().collect(),
        retired_action_order: retired.into_iter().collect(),
        budget_spent_units: spent,
        remaining_budget_units: request.budget_units.saturating_sub(spent),
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        stop_reason,
        next_step,
        digest: ContentHash::of_bytes(b"unsealed-glioma-mechanism-discovery-engine"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| GliomaMechanismDiscoveryError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p03_multimodal_ingestion_qc::{FeatureValue, GraphFusionRequest};
    use crate::glioma::programs::p05_mechanism_exploration::{
        CounterfactualIntervention, MechanismGraphEdge, MechanismGraphNode, MechanismGraphRelation,
        PathwayActivityNode, PathwayActivityRequest, PortfolioDirection,
    };
    use crate::glioma::programs::p07_protocol_simulation::DryRunGliomaActionExecutor;
    use crate::glioma_engine::{
        GliomaModality, GliomaSelectionWeights, GliomaStageKind, LocalArtifactRef,
    };
    use bioprism_foundation::{AutonomyTier, Effect};

    fn hash(value: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"value": value})).unwrap()
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

    fn request() -> GliomaMechanismDiscoveryRequest {
        let graph = GraphFusionRequest {
            study_id: "discovery-study".into(),
            model_system: GliomaModelSystem::Organoid,
            required_modalities: BTreeSet::from([GliomaModality::Proteomics]),
            min_samples: 2,
            min_modalities_per_sample: 1,
            min_shared_features: 1,
            neighbours: 1,
            diffusion_steps: 1,
            max_distance_milli: 1_000,
            min_consensus_support_milli: 500,
            max_disagreement_milli: 200,
            require_all_modalities: false,
        };
        let pathway = PathwayActivityRequest {
            objective: "discover invasion mechanism".into(),
            study_id: "discovery-study".into(),
            model_system: GliomaModelSystem::Organoid,
            min_pathway_nodes: 1,
            min_observed_nodes: 1,
            min_modalities: 1,
            min_confidence_milli: 100,
            max_pathways: 4,
            require_cross_modal: false,
        };
        let campaign = MultimodalMechanismCampaignRequest {
            objective: "discover invasion mechanism".into(),
            study_id: "discovery-study".into(),
            model_system: GliomaModelSystem::Organoid,
            graph: graph.clone(),
            pathway: pathway.clone(),
            selection: GliomaSelectionConfig {
                budget_units: 3,
                max_actions: 1,
                approval_granted: false,
                allow_instrument_execution: false,
                allow_federation: false,
                weights: GliomaSelectionWeights::default(),
            },
            completed_action_order: Vec::new(),
        };
        let action = GliomaActionCandidate {
            action_id: "assay-invasion".into(),
            stage_kind: GliomaStageKind::MechanismExploration,
            modality: GliomaModality::FunctionalPerturbation,
            model_system: GliomaModelSystem::Organoid,
            depends_on: Vec::new(),
            cost_units: 1,
            information_gain_milli: 900,
            frontier_novelty_milli: 800,
            workflow_leverage_milli: 800,
            cross_stage_unlock_milli: 700,
            reproducibility_safety_milli: 900,
            federation_value_milli: 400,
            feasibility_milli: 950,
            autonomy_tier: AutonomyTier::A0,
            effects: BTreeSet::from([
                Effect::ReadLocalData,
                Effect::ExecuteLocalComputation,
                Effect::WriteLocalArtifact,
            ]),
        };
        let model = CounterfactualModel {
            model_id: "model-a".into(),
            prior_milli: 1_000,
            nodes: vec![
                MechanismGraphNode {
                    node_id: "egfr".into(),
                    label: "EGFR".into(),
                    modality: GliomaModality::Proteomics,
                    prior_milli: 0,
                    support_milli: 900,
                    contradiction_milli: 0,
                },
                MechanismGraphNode {
                    node_id: "invasion".into(),
                    label: "invasion".into(),
                    modality: GliomaModality::Proteomics,
                    prior_milli: 0,
                    support_milli: 0,
                    contradiction_milli: 0,
                },
            ],
            edges: vec![MechanismGraphEdge {
                edge_id: "egfr-invasion".into(),
                source_node_id: "egfr".into(),
                target_node_id: "invasion".into(),
                relation: MechanismGraphRelation::Activates,
                confidence_milli: 900,
                evidence_order: vec!["local-evidence".into()],
            }],
        };
        GliomaMechanismDiscoveryRequest {
            objective: "discover invasion mechanism".into(),
            study_id: "discovery-study".into(),
            model_system: GliomaModelSystem::Organoid,
            campaign,
            graph_vectors: vec![
                GraphFusionVector {
                    observation_id: "v1".into(),
                    study_id: "discovery-study".into(),
                    sample_lineage: "sample-a".into(),
                    modality: GliomaModality::Proteomics,
                    model_system: GliomaModelSystem::Organoid,
                    artifact: artifact("a"),
                    reliability_milli: 900,
                    features: vec![FeatureValue {
                        feature_id: "egfr".into(),
                        value_milli: 800,
                    }],
                },
                GraphFusionVector {
                    observation_id: "v2".into(),
                    study_id: "discovery-study".into(),
                    sample_lineage: "sample-b".into(),
                    modality: GliomaModality::Proteomics,
                    model_system: GliomaModelSystem::Organoid,
                    artifact: artifact("b"),
                    reliability_milli: 900,
                    features: vec![FeatureValue {
                        feature_id: "egfr".into(),
                        value_milli: 700,
                    }],
                },
            ],
            pathway_definitions: vec![PathwayActivityDefinition {
                pathway_id: "invasion".into(),
                label: "invasion".into(),
                nodes: vec![PathwayActivityNode {
                    node_id: "egfr".into(),
                    label: "EGFR".into(),
                    modality: GliomaModality::Proteomics,
                    expected_direction: 1,
                    weight_milli: 1_000,
                }],
                edges: Vec::new(),
            }],
            pathway_observations: vec![PathwayActivityObservation {
                observation_id: "p1".into(),
                study_id: "discovery-study".into(),
                sample_lineage: "sample-a".into(),
                modality: GliomaModality::Proteomics,
                model_system: GliomaModelSystem::Organoid,
                artifact: artifact("p"),
                feature_id: "egfr".into(),
                value_milli: 800,
                reliability_milli: 900,
            }],
            dynamics: MechanismDynamicsRequest {
                objective: "discover invasion mechanism".into(),
                model_system: GliomaModelSystem::Organoid,
                max_steps: 12,
                time_step_milli: 200,
                stability_window: 2,
                stability_delta_milli: 20,
                divergence_abs_milli: 1_000,
                max_selected_interventions: 1,
                budget_units: 8,
                risk_ceiling_milli: 800,
                sensitivity_delta_milli: 5,
            },
            dynamics_nodes: vec![MechanismDynamicsNode {
                node_id: "invasion".into(),
                label: "invasion".into(),
                initial_state_milli: 100,
                drift_milli: -10,
                uncertainty_milli: 10,
            }],
            dynamics_edges: Vec::new(),
            dynamics_interventions: vec![MechanismDynamicsIntervention {
                intervention_id: "suppress-invasion".into(),
                target_node: "invasion".into(),
                delta_milli: -100,
                start_step: 0,
                duration_steps: 4,
                cost_units: 1,
                risk_milli: 100,
                confidence_milli: 900,
            }],
            robust: RobustInterventionRequest {
                objective: "discover invasion mechanism".into(),
                model_system: GliomaModelSystem::Organoid,
                max_iterations: 64,
                convergence_tolerance_milli: 1,
                damping_milli: 600,
                min_edge_confidence_milli: 500,
                direction: PortfolioDirection::Decrease,
                budget_units: 2,
                max_selected: 1,
                min_robust_effect_milli: 1,
                min_agreement_milli: 750,
                risk_ceiling_milli: 800,
                effect_weight_milli: 500,
                tail_weight_milli: 300,
                worst_case_weight_milli: 200,
                feasibility_weight_milli: 1_000,
                risk_penalty_milli: 1,
            },
            robust_models: vec![model],
            robust_candidates: vec![RobustInterventionCandidate {
                candidate_id: "egfr-invasion".into(),
                label: "EGFR inhibition".into(),
                intervention: CounterfactualIntervention {
                    intervention_id: "inhibit-egfr".into(),
                    node_id: "egfr".into(),
                    delta_milli: -600,
                    rationale: "test EGFR-to-invasion propagation".into(),
                    evidence_order: vec!["local-evidence".into()],
                },
                target_node_id: "invasion".into(),
                redundancy_group: "egfr".into(),
                feasibility_milli: 1_000,
                cost_units: 1,
                risk_milli: 100,
            }],
            action_candidates: vec![action],
            selection: GliomaSelectionConfig {
                budget_units: 2,
                max_actions: 1,
                approval_granted: false,
                allow_instrument_execution: false,
                allow_federation: false,
                weights: GliomaSelectionWeights::default(),
            },
            completed_action_order: Vec::new(),
            budget_units: 2,
            max_rounds: 2,
            max_retries: 1,
            require_artifacts: true,
            require_qualified_multimodal: true,
            require_stable_dynamics: false,
            max_actions: 1,
        }
    }

    #[test]
    fn engine_runs_mechanism_stack_and_executes_one_local_assay() {
        let mut executor = DryRunGliomaActionExecutor;
        let run = execute_glioma_mechanism_discovery_engine(&request(), &mut executor).unwrap();
        assert_eq!(run.feature_id, FEATURE_ID);
        assert_eq!(run.rounds.len(), 1);
        assert_eq!(run.completed_action_order, vec!["assay-invasion"]);
        assert_eq!(
            run.stop_reason,
            GliomaMechanismDiscoveryStopReason::Qualified
        );
        assert!(run.rounds[0]
            .robust_portfolio
            .selected_order
            .contains(&"egfr-invasion".into()));
        run.validate().unwrap();
    }

    #[test]
    fn engine_holds_before_dispatch_when_multimodal_evidence_is_unresolved() {
        let mut request = request();
        request.campaign.pathway.require_cross_modal = true;
        let mut executor = DryRunGliomaActionExecutor;
        let run = execute_glioma_mechanism_discovery_engine(&request, &mut executor).unwrap();
        assert_eq!(
            run.disposition,
            GliomaMechanismDiscoveryDisposition::EvidenceBlocked
        );
        assert_eq!(
            run.stop_reason,
            GliomaMechanismDiscoveryStopReason::MultimodalEvidenceBlocked
        );
        assert!(run.rounds[0].execution.is_none());
        run.validate().unwrap();
    }
}

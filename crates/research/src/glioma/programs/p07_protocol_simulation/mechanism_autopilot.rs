//! Closed-loop multimodal mechanism autopilot for preclinical glioma research.
//!
//! This feature turns the existing graph/pathway campaign and local action portfolio into a
//! usable autonomous research loop.  It does not manufacture measurements: each round compiles
//! the current multimodal evidence, applies the campaign's dependency and safety gates, executes
//! at most one bounded local batch, retires every returned outcome, and replans the next batch.
//! A provider can replace the dry-run worker with an institution-local assay, imaging, or
//! computation adapter without changing the scientific control logic.

use super::action_execution::{
    ActionExecutionDisposition, ActionExecutionResult, ActionPortfolioExecutionDisposition,
    GliomaActionExecutor, MAX_RETRIES,
};
use super::mechanism_campaign::{
    execute_glioma_multimodal_mechanism_campaign,
    execute_glioma_multimodal_mechanism_campaign_with_executor, MechanismCampaignDisposition,
    MechanismCampaignError, MultimodalMechanismCampaign, MultimodalMechanismCampaignExecution,
    MultimodalMechanismCampaignRequest,
};
use crate::glioma_engine::{GliomaActionCandidate, GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

use super::super::p03_multimodal_ingestion_qc::GraphFusionVector;
use super::super::p05_mechanism_exploration::{
    PathwayActivityDefinition, PathwayActivityObservation,
};

pub const FEATURE_ID: &str = "GAF-GLIOMA-P07-F30";
pub const OUTPUT_SCHEMA: &str = "GliomaMultimodalMechanismAutopilot1@2";
pub const MAX_ROUNDS: u16 = 32;
pub const MAX_CANDIDATES: usize = 256;
pub const MAX_FEEDBACK_OBSERVATIONS_PER_ACTION: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaMechanismAutopilotRequest {
    pub campaign: MultimodalMechanismCampaignRequest,
    pub graph_vectors: Vec<GraphFusionVector>,
    pub pathway_definitions: Vec<PathwayActivityDefinition>,
    pub pathway_observations: Vec<PathwayActivityObservation>,
    pub candidates: Vec<GliomaActionCandidate>,
    pub max_rounds: u16,
    pub max_retries: u8,
    pub require_artifacts: bool,
    pub require_ready_for_execution: bool,
    pub stop_on_negative: bool,
}

/// Typed value-only observations interpreted from one completed local action artifact.
/// Payload bytes remain in the institution-local artifact store and are never passed here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaMechanismAutopilotFeedback {
    pub action_id: String,
    pub source_artifact: LocalArtifactRef,
    pub graph_vectors: Vec<GraphFusionVector>,
    pub pathway_observations: Vec<PathwayActivityObservation>,
}

/// Institution-local adapter that interprets measured action artifacts as bounded typed values.
/// Implementations should read only the exact content-addressed artifact supplied by `result`.
/// `Ok(None)` means the artifact has no supported structured readout and cannot drive a new
/// scientific decision. Synthetic dry-run artifacts are never sent to this adapter.
pub trait GliomaMechanismFeedbackInterpreter {
    fn interpret_action_result(
        &mut self,
        candidate: &GliomaActionCandidate,
        result: &ActionExecutionResult,
    ) -> Result<Option<GliomaMechanismAutopilotFeedback>, String>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GliomaMechanismAutopilotDisposition {
    Completed,
    Partial,
    BudgetExhausted,
    Blocked,
    NoRunnableActions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GliomaMechanismAutopilotStopReason {
    Completed,
    CampaignEvidenceBlocked,
    BudgetExhausted,
    NoRunnableActions,
    MaxRounds,
    NegativeEvidence,
    ExecutorFailed,
    NoProgress,
    FeedbackUnavailable,
    FeedbackInterpretationFailed,
    FeedbackRejected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaMechanismAutopilotRound {
    pub round: u16,
    pub budget_before_units: u32,
    pub budget_after_units: u32,
    pub active_candidate_count: usize,
    pub campaign: MultimodalMechanismCampaign,
    pub execution: Option<MultimodalMechanismCampaignExecution>,
    pub planned_action_order: Vec<String>,
    pub executed_action_order: Vec<String>,
    pub newly_completed_order: Vec<String>,
    pub newly_negative_order: Vec<String>,
    pub newly_failed_order: Vec<String>,
    pub assimilated_feedback: Vec<GliomaMechanismAutopilotFeedback>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaMechanismAutopilotRun {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub rounds: Vec<GliomaMechanismAutopilotRound>,
    pub completed_action_order: Vec<String>,
    pub negative_action_order: Vec<String>,
    pub failed_action_order: Vec<String>,
    pub retired_action_order: Vec<String>,
    pub budget_spent_units: u32,
    pub remaining_budget_units: u32,
    pub retry_count: u32,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: GliomaMechanismAutopilotDisposition,
    pub stop_reason: GliomaMechanismAutopilotStopReason,
    pub next_step: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GliomaMechanismAutopilotError {
    #[error("glioma mechanism autopilot request is invalid: {0}")]
    InvalidRequest(String),
    #[error("glioma mechanism autopilot campaign failed: {0}")]
    Campaign(#[from] MechanismCampaignError),
    #[error("glioma mechanism autopilot output is invalid: {0}")]
    InvalidOutput(String),
    #[error("glioma mechanism autopilot digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(run: &GliomaMechanismAutopilotRun) -> serde_json::Value {
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
        "retry_count": run.retry_count,
        "negative_evidence": run.negative_evidence,
        "uncertainty": run.uncertainty,
        "disposition": run.disposition,
        "stop_reason": run.stop_reason,
        "next_step": run.next_step,
    })
}

impl GliomaMechanismAutopilotRun {
    pub fn validate(&self) -> Result<(), GliomaMechanismAutopilotError> {
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
                .any(|id| self.negative_action_order.binary_search(id).is_ok())
            || self
                .completed_action_order
                .iter()
                .any(|id| self.failed_action_order.binary_search(id).is_ok())
            || self
                .retired_action_order
                .iter()
                .any(|id| id.trim().is_empty())
        {
            return Err(GliomaMechanismAutopilotError::InvalidOutput(
                "identity, ordering, partitions, or output bounds are invalid".into(),
            ));
        }
        let mut seen_rounds = BTreeSet::new();
        let mut seen_actions = BTreeSet::new();
        let mut seen_feedback_graph_observations = BTreeSet::new();
        let mut seen_feedback_pathway_observations = BTreeSet::new();
        let mut expected_spend = 0_u32;
        let mut expected_retries = 0_u32;
        for round in &self.rounds {
            if round.round == 0
                || !seen_rounds.insert(round.round)
                || round.round > MAX_ROUNDS
                || round.budget_after_units > round.budget_before_units
                || !canonical(&round.planned_action_order)
                || !canonical(&round.executed_action_order)
                || !canonical(&round.newly_completed_order)
                || !canonical(&round.newly_negative_order)
                || !canonical(&round.newly_failed_order)
                || !round
                    .assimilated_feedback
                    .windows(2)
                    .all(|pair| pair[0].action_id < pair[1].action_id)
                || !canonical(&round.uncertainty)
                || !canonical(&round.negative_evidence)
                || round
                    .newly_completed_order
                    .iter()
                    .any(|id| round.newly_negative_order.binary_search(id).is_ok())
            {
                return Err(GliomaMechanismAutopilotError::InvalidOutput(
                    "round ordering, budget, or result partitions are invalid".into(),
                ));
            }
            if round.campaign.objective != self.objective
                || round.campaign.study_id != self.study_id
                || round.campaign.model_system != self.model_system
                || round.planned_action_order != round.campaign.next_action_order
            {
                return Err(GliomaMechanismAutopilotError::InvalidOutput(
                    "round campaign bindings or planned actions are inconsistent".into(),
                ));
            }
            round
                .campaign
                .validate()
                .map_err(|error| GliomaMechanismAutopilotError::InvalidOutput(error.to_string()))?;
            if let Some(execution) = &round.execution {
                execution.validate().map_err(|error| {
                    GliomaMechanismAutopilotError::InvalidOutput(error.to_string())
                })?;
                if execution.campaign.digest != round.campaign.digest
                    || execution.executed_order != round.executed_action_order
                {
                    return Err(GliomaMechanismAutopilotError::InvalidOutput(
                        "round execution is not bound to its campaign".into(),
                    ));
                }
                if let Some(portfolio) = &execution.execution {
                    expected_retries = expected_retries.saturating_add(portfolio.retry_count);
                    let round_spend = round
                        .budget_before_units
                        .saturating_sub(round.budget_after_units);
                    expected_spend = expected_spend.saturating_add(round_spend);
                    for action_id in &portfolio.action_order {
                        if !seen_actions.insert(action_id.clone()) {
                            return Err(GliomaMechanismAutopilotError::InvalidOutput(
                                "an action was executed in multiple autopilot rounds".into(),
                            ));
                        }
                    }
                    for feedback in &round.assimilated_feedback {
                        let Some(result) = portfolio
                            .results
                            .iter()
                            .find(|result| result.action_id == feedback.action_id)
                        else {
                            return Err(GliomaMechanismAutopilotError::InvalidOutput(
                                "assimilated evidence has no matching executed action".into(),
                            ));
                        };
                        if feedback.action_id.trim().is_empty()
                            || feedback.graph_vectors.is_empty()
                                && feedback.pathway_observations.is_empty()
                            || result.disposition != ActionExecutionDisposition::Completed
                                && result.disposition != ActionExecutionDisposition::Negative
                            || result.artifact.as_ref() != Some(&feedback.source_artifact)
                            || is_synthetic_result(result)
                            || feedback.source_artifact.validate().is_err()
                            || feedback.graph_vectors.iter().any(|vector| {
                                vector.artifact != feedback.source_artifact
                                    || vector.study_id != self.study_id
                                    || vector.model_system != self.model_system
                                    || vector.observation_id.trim().is_empty()
                                    || vector.reliability_milli > 1_000
                                    || !seen_feedback_graph_observations
                                        .insert(vector.observation_id.clone())
                            })
                            || feedback.pathway_observations.iter().any(|observation| {
                                observation.artifact != feedback.source_artifact
                                    || observation.study_id != self.study_id
                                    || observation.model_system != self.model_system
                                    || observation.observation_id.trim().is_empty()
                                    || observation.reliability_milli > 1_000
                                    || !seen_feedback_pathway_observations
                                        .insert(observation.observation_id.clone())
                            })
                        {
                            return Err(GliomaMechanismAutopilotError::InvalidOutput(
                                "assimilated evidence is not bound to a valid measured local artifact".into(),
                            ));
                        }
                    }
                } else if !round.assimilated_feedback.is_empty() {
                    return Err(GliomaMechanismAutopilotError::InvalidOutput(
                        "assimilated evidence has no action execution record".into(),
                    ));
                }
            } else if !round.executed_action_order.is_empty()
                || !round.assimilated_feedback.is_empty()
            {
                return Err(GliomaMechanismAutopilotError::InvalidOutput(
                    "execution-less round cannot report executed actions".into(),
                ));
            }
        }
        if expected_spend != self.budget_spent_units || expected_retries != self.retry_count {
            return Err(GliomaMechanismAutopilotError::InvalidOutput(
                "budget or retry totals do not reconcile with rounds".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| GliomaMechanismAutopilotError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(GliomaMechanismAutopilotError::InvalidOutput(
                "autopilot digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &GliomaMechanismAutopilotRequest,
) -> Result<(), GliomaMechanismAutopilotError> {
    if request.campaign.objective.trim().is_empty()
        || request.campaign.study_id.trim().is_empty()
        || request.campaign.graph.study_id != request.campaign.study_id
        || request.campaign.pathway.study_id != request.campaign.study_id
        || request.campaign.graph.model_system != request.campaign.model_system
        || request.campaign.pathway.model_system != request.campaign.model_system
        || request.max_rounds == 0
        || request.max_rounds > MAX_ROUNDS
        || request.max_retries > MAX_RETRIES
        || request.candidates.is_empty()
        || request.candidates.len() > MAX_CANDIDATES
        || !canonical(&request.campaign.completed_action_order)
        || request
            .campaign
            .completed_action_order
            .iter()
            .any(|id| id.trim().is_empty())
    {
        return Err(GliomaMechanismAutopilotError::InvalidRequest(
            "objective, study/model bindings, bounded rounds/retries/candidates, and canonical completed actions are required".into(),
        ));
    }
    Ok(())
}

fn is_synthetic_result(result: &ActionExecutionResult) -> bool {
    result
        .artifact
        .as_ref()
        .is_some_and(|artifact| artifact.artifact_id.starts_with("dry-run-action:"))
        || result
            .uncertainty
            .iter()
            .any(|value| value == "simulation-only-result")
        || result
            .negative_evidence
            .iter()
            .any(|value| value == "synthetic-dry-run-not-biological-evidence")
        || result.note.to_ascii_lowercase().contains("dry-run")
}

fn validate_feedback(
    request: &GliomaMechanismAutopilotRequest,
    candidate: &GliomaActionCandidate,
    result: &ActionExecutionResult,
    feedback: &GliomaMechanismAutopilotFeedback,
    graph_vectors: &[GraphFusionVector],
    pathway_observations: &[PathwayActivityObservation],
) -> Result<(), &'static str> {
    let Some(output_artifact) = result.artifact.as_ref() else {
        return Err("action output artifact is missing");
    };
    if result.action_id != candidate.action_id
        || feedback.action_id != candidate.action_id
        || feedback.source_artifact != *output_artifact
        || feedback.source_artifact.validate().is_err()
        || feedback.graph_vectors.len() + feedback.pathway_observations.len()
            > MAX_FEEDBACK_OBSERVATIONS_PER_ACTION
        || graph_vectors
            .len()
            .saturating_add(feedback.graph_vectors.len())
            > 16_384
        || pathway_observations
            .len()
            .saturating_add(feedback.pathway_observations.len())
            > 131_072
        || feedback.graph_vectors.is_empty() && feedback.pathway_observations.is_empty()
    {
        return Err("feedback identity, provenance, privacy, or size limit is invalid");
    }

    let mut graph_ids = graph_vectors
        .iter()
        .map(|vector| vector.observation_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut graph_samples = graph_vectors
        .iter()
        .map(|vector| (vector.sample_lineage.as_str(), vector.modality))
        .collect::<BTreeSet<_>>();
    for vector in &feedback.graph_vectors {
        let mut feature_ids = BTreeSet::new();
        if vector.observation_id.trim().is_empty()
            || !graph_ids.insert(vector.observation_id.as_str())
            || vector.study_id != request.campaign.study_id
            || vector.sample_lineage.trim().is_empty()
            || vector.model_system != request.campaign.model_system
            || !request
                .campaign
                .graph
                .required_modalities
                .contains(&vector.modality)
            || vector.artifact != feedback.source_artifact
            || vector.reliability_milli > 1_000
            || vector.features.is_empty()
            || vector.features.len() > 16_384
            || !graph_samples.insert((vector.sample_lineage.as_str(), vector.modality))
            || vector.features.iter().any(|feature| {
                feature.feature_id.trim().is_empty() || !feature_ids.insert(&feature.feature_id)
            })
        {
            return Err(
                "graph readout is invalid, duplicated, out of scope, or not artifact-bound",
            );
        }
    }

    let mut pathway_ids = pathway_observations
        .iter()
        .map(|observation| observation.observation_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut pathway_features = pathway_observations
        .iter()
        .map(|observation| {
            (
                observation.sample_lineage.as_str(),
                observation.modality,
                observation.feature_id.as_str(),
            )
        })
        .collect::<BTreeSet<_>>();
    for observation in &feedback.pathway_observations {
        let is_defined_node = request.pathway_definitions.iter().any(|definition| {
            definition.nodes.iter().any(|node| {
                node.node_id == observation.feature_id && node.modality == observation.modality
            })
        });
        if observation.observation_id.trim().is_empty()
            || !pathway_ids.insert(observation.observation_id.as_str())
            || observation.study_id != request.campaign.study_id
            || observation.sample_lineage.trim().is_empty()
            || observation.model_system != request.campaign.model_system
            || observation.feature_id.trim().is_empty()
            || !is_defined_node
            || observation.artifact != feedback.source_artifact
            || observation.reliability_milli > 1_000
            || !pathway_features.insert((
                observation.sample_lineage.as_str(),
                observation.modality,
                observation.feature_id.as_str(),
            ))
        {
            return Err(
                "pathway readout is invalid, duplicated, out of scope, or not artifact-bound",
            );
        }
    }
    Ok(())
}

fn sorted_insert(set: &mut BTreeSet<String>, values: impl IntoIterator<Item = String>) {
    set.extend(values);
}

/// Run a bounded multimodal mechanism campaign as an autonomous local research loop.
pub fn execute_glioma_mechanism_autopilot<E: GliomaActionExecutor + ?Sized>(
    request: &GliomaMechanismAutopilotRequest,
    executor: &mut E,
) -> Result<GliomaMechanismAutopilotRun, GliomaMechanismAutopilotError> {
    execute_mechanism_autopilot(request, executor, None)
}

/// Run the mechanism campaign with an institution-local readout interpreter. Only typed evidence
/// bound to an exact completed action artifact can update the next planning round. If an action
/// cannot yield a valid readout, the run records the reason and stops instead of replanning from
/// unchanged or synthetic observations.
pub fn execute_glioma_mechanism_autopilot_with_feedback<
    E: GliomaActionExecutor + ?Sized,
    I: GliomaMechanismFeedbackInterpreter,
>(
    request: &GliomaMechanismAutopilotRequest,
    executor: &mut E,
    interpreter: &mut I,
) -> Result<GliomaMechanismAutopilotRun, GliomaMechanismAutopilotError> {
    execute_mechanism_autopilot(request, executor, Some(interpreter))
}

fn execute_mechanism_autopilot<E: GliomaActionExecutor + ?Sized>(
    request: &GliomaMechanismAutopilotRequest,
    executor: &mut E,
    mut feedback_interpreter: Option<&mut dyn GliomaMechanismFeedbackInterpreter>,
) -> Result<GliomaMechanismAutopilotRun, GliomaMechanismAutopilotError> {
    validate_request(request)?;
    let initial_budget = request.campaign.selection.budget_units;
    let mut graph_vectors = request.graph_vectors.clone();
    let mut pathway_observations = request.pathway_observations.clone();
    let mut completed = request
        .campaign
        .completed_action_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut negative = BTreeSet::new();
    let mut failed = BTreeSet::new();
    let mut retired = completed.clone();
    let mut negative_evidence = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut rounds = Vec::new();
    let mut budget_spent = 0_u32;
    let mut retry_count = 0_u32;
    let mut stop_reason = GliomaMechanismAutopilotStopReason::MaxRounds;

    for round_number in 1..=request.max_rounds {
        let remaining_budget = initial_budget.saturating_sub(budget_spent);
        if remaining_budget == 0 {
            stop_reason = GliomaMechanismAutopilotStopReason::BudgetExhausted;
            break;
        }
        let active_candidates = request
            .candidates
            .iter()
            .filter(|candidate| !retired.contains(&candidate.action_id))
            .cloned()
            .collect::<Vec<_>>();
        if active_candidates.is_empty() {
            stop_reason = GliomaMechanismAutopilotStopReason::NoRunnableActions;
            break;
        }
        let mut campaign_request = request.campaign.clone();
        campaign_request.completed_action_order = completed.iter().cloned().collect();
        campaign_request.selection.budget_units = remaining_budget;
        let planned = execute_glioma_multimodal_mechanism_campaign(
            &campaign_request,
            &graph_vectors,
            &request.pathway_definitions,
            &pathway_observations,
            &active_candidates,
        )?;
        let budget_before = remaining_budget;
        let mut round_negative = planned
            .negative_evidence
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut round_uncertainty = planned.uncertainty.iter().cloned().collect::<BTreeSet<_>>();
        let mut newly_completed = BTreeSet::new();
        let mut newly_negative = BTreeSet::new();
        let mut newly_failed = BTreeSet::new();
        let evidence_blocked = planned.disposition == MechanismCampaignDisposition::Unresolved
            || (request.require_ready_for_execution
                && planned.disposition != MechanismCampaignDisposition::ReadyForExecution);
        if evidence_blocked {
            round_negative.insert("mechanism-campaign-evidence-gate-blocked-dispatch".into());
            sorted_insert(&mut negative_evidence, round_negative.iter().cloned());
            sorted_insert(&mut uncertainty, round_uncertainty.iter().cloned());
            let round = GliomaMechanismAutopilotRound {
                round: round_number,
                budget_before_units: budget_before,
                budget_after_units: budget_before,
                active_candidate_count: active_candidates.len(),
                planned_action_order: planned.next_action_order.clone(),
                campaign: planned,
                execution: None,
                executed_action_order: Vec::new(),
                newly_completed_order: Vec::new(),
                newly_negative_order: Vec::new(),
                newly_failed_order: Vec::new(),
                assimilated_feedback: Vec::new(),
                uncertainty: round_uncertainty.into_iter().collect(),
                negative_evidence: round_negative.into_iter().collect(),
            };
            rounds.push(round);
            stop_reason = GliomaMechanismAutopilotStopReason::CampaignEvidenceBlocked;
            break;
        }
        let executed = execute_glioma_multimodal_mechanism_campaign_with_executor(
            &campaign_request,
            &graph_vectors,
            &request.pathway_definitions,
            &pathway_observations,
            &active_candidates,
            request.max_retries,
            request.require_artifacts,
            executor,
        )?;
        let executed_order = executed.executed_order.clone();
        let portfolio = executed.execution.as_ref();
        // The portfolio controller charges each actual worker invocation, including retries.
        // Reuse that measured spend instead of summing selected action costs once; otherwise a
        // transient worker failure would create unreported budget headroom for the next round.
        let round_cost = portfolio
            .map(|portfolio| portfolio.budget_spent_units)
            .unwrap_or(0);
        budget_spent = budget_spent.saturating_add(round_cost);
        retry_count = retry_count.saturating_add(portfolio.map(|p| p.retry_count).unwrap_or(0));
        if let Some(portfolio) = portfolio {
            sorted_insert(
                &mut newly_completed,
                portfolio.completed_order.iter().cloned(),
            );
            sorted_insert(
                &mut newly_negative,
                portfolio.negative_order.iter().cloned(),
            );
            sorted_insert(
                &mut newly_failed,
                portfolio
                    .failed_order
                    .iter()
                    .chain(portfolio.partial_order.iter())
                    .chain(portfolio.blocked_order.iter())
                    .cloned(),
            );
            sorted_insert(&mut retired, portfolio.action_order.iter().cloned());
            round_negative.extend(portfolio.negative_evidence.iter().cloned());
            round_uncertainty.extend(portfolio.uncertainty.iter().cloned());
        }
        round_negative.extend(executed.negative_evidence.iter().cloned());
        round_uncertainty.extend(executed.uncertainty.iter().cloned());

        let mut assimilated_feedback = BTreeMap::new();
        let mut feedback_stop_reason = None;
        if let (Some(interpreter), Some(portfolio)) =
            (feedback_interpreter.as_deref_mut(), portfolio)
        {
            for result in &portfolio.results {
                if !matches!(
                    result.disposition,
                    ActionExecutionDisposition::Completed | ActionExecutionDisposition::Negative
                ) {
                    continue;
                }
                if is_synthetic_result(result) {
                    round_uncertainty.insert(
                        "synthetic-action-output-not-assimilated-as-biological-evidence".into(),
                    );
                    feedback_stop_reason =
                        Some(GliomaMechanismAutopilotStopReason::FeedbackUnavailable);
                    break;
                }
                if result.artifact.is_none() {
                    round_uncertainty
                        .insert("completed-action-has-no-interpretable-local-artifact".into());
                    feedback_stop_reason =
                        Some(GliomaMechanismAutopilotStopReason::FeedbackUnavailable);
                    break;
                }
                let Some(candidate) = active_candidates
                    .iter()
                    .find(|candidate| candidate.action_id == result.action_id)
                else {
                    feedback_stop_reason =
                        Some(GliomaMechanismAutopilotStopReason::FeedbackRejected);
                    round_negative.insert("feedback-action-not-in-active-candidate-set".into());
                    break;
                };
                let interpreted = interpreter.interpret_action_result(candidate, result);
                let mut feedback = match interpreted {
                    Ok(Some(feedback)) => feedback,
                    Ok(None) => {
                        round_uncertainty.insert(
                            "local-action-artifact-has-no-supported-typed-research-readout".into(),
                        );
                        feedback_stop_reason =
                            Some(GliomaMechanismAutopilotStopReason::FeedbackUnavailable);
                        break;
                    }
                    Err(_) => {
                        round_negative.insert("local-action-feedback-interpretation-failed".into());
                        feedback_stop_reason =
                            Some(GliomaMechanismAutopilotStopReason::FeedbackInterpretationFailed);
                        break;
                    }
                };
                if validate_feedback(
                    request,
                    candidate,
                    result,
                    &feedback,
                    &graph_vectors,
                    &pathway_observations,
                )
                .is_err()
                {
                    round_negative.insert(
                        "local-action-feedback-failed-scope-or-provenance-validation".into(),
                    );
                    feedback_stop_reason =
                        Some(GliomaMechanismAutopilotStopReason::FeedbackRejected);
                    break;
                }
                feedback
                    .graph_vectors
                    .sort_by(|left, right| left.observation_id.cmp(&right.observation_id));
                feedback
                    .pathway_observations
                    .sort_by(|left, right| left.observation_id.cmp(&right.observation_id));
                graph_vectors.extend(feedback.graph_vectors.iter().cloned());
                pathway_observations.extend(feedback.pathway_observations.iter().cloned());
                assimilated_feedback.insert(feedback.action_id.clone(), feedback);
            }
        }
        sorted_insert(&mut completed, newly_completed.iter().cloned());
        sorted_insert(&mut negative, newly_negative.iter().cloned());
        sorted_insert(&mut failed, newly_failed.iter().cloned());
        sorted_insert(&mut negative_evidence, round_negative.iter().cloned());
        sorted_insert(&mut uncertainty, round_uncertainty.iter().cloned());
        let round = GliomaMechanismAutopilotRound {
            round: round_number,
            budget_before_units: budget_before,
            budget_after_units: budget_before.saturating_sub(round_cost),
            active_candidate_count: active_candidates.len(),
            planned_action_order: planned.next_action_order.clone(),
            campaign: executed.campaign.clone(),
            execution: Some(executed.clone()),
            executed_action_order: executed_order,
            newly_completed_order: newly_completed.iter().cloned().collect(),
            newly_negative_order: newly_negative.iter().cloned().collect(),
            newly_failed_order: newly_failed.iter().cloned().collect(),
            assimilated_feedback: assimilated_feedback.into_values().collect(),
            uncertainty: round_uncertainty.into_iter().collect(),
            negative_evidence: round_negative.into_iter().collect(),
        };
        rounds.push(round);
        if request.stop_on_negative && !newly_negative.is_empty() {
            stop_reason = GliomaMechanismAutopilotStopReason::NegativeEvidence;
            break;
        }
        if let Some(reason) = feedback_stop_reason {
            stop_reason = reason;
            break;
        }
        if feedback_interpreter.is_none()
            && request
                .candidates
                .iter()
                .any(|candidate| !retired.contains(&candidate.action_id))
        {
            stop_reason = GliomaMechanismAutopilotStopReason::FeedbackUnavailable;
            break;
        }
        match executed.disposition {
            super::mechanism_campaign::MechanismCampaignExecutionDisposition::Completed => {}
            super::mechanism_campaign::MechanismCampaignExecutionDisposition::Blocked => {
                stop_reason = GliomaMechanismAutopilotStopReason::CampaignEvidenceBlocked;
                break;
            }
            super::mechanism_campaign::MechanismCampaignExecutionDisposition::Partial => {
                stop_reason = if portfolio
                    .is_some_and(|p| p.disposition == ActionPortfolioExecutionDisposition::Failed)
                {
                    GliomaMechanismAutopilotStopReason::ExecutorFailed
                } else {
                    GliomaMechanismAutopilotStopReason::NoProgress
                };
                break;
            }
        }
        if newly_completed.is_empty() && newly_negative.is_empty() {
            stop_reason = GliomaMechanismAutopilotStopReason::NoProgress;
            break;
        }
    }

    if stop_reason == GliomaMechanismAutopilotStopReason::MaxRounds {
        if rounds.len() < usize::from(request.max_rounds) {
            stop_reason = GliomaMechanismAutopilotStopReason::Completed;
        } else if request
            .candidates
            .iter()
            .all(|candidate| retired.contains(&candidate.action_id))
        {
            stop_reason = GliomaMechanismAutopilotStopReason::NoRunnableActions;
        }
    }
    let disposition = match stop_reason {
        GliomaMechanismAutopilotStopReason::Completed => {
            GliomaMechanismAutopilotDisposition::Completed
        }
        GliomaMechanismAutopilotStopReason::BudgetExhausted => {
            GliomaMechanismAutopilotDisposition::BudgetExhausted
        }
        GliomaMechanismAutopilotStopReason::NoRunnableActions => {
            GliomaMechanismAutopilotDisposition::NoRunnableActions
        }
        GliomaMechanismAutopilotStopReason::CampaignEvidenceBlocked
        | GliomaMechanismAutopilotStopReason::ExecutorFailed => {
            GliomaMechanismAutopilotDisposition::Blocked
        }
        GliomaMechanismAutopilotStopReason::NegativeEvidence
        | GliomaMechanismAutopilotStopReason::NoProgress
        | GliomaMechanismAutopilotStopReason::MaxRounds
        | GliomaMechanismAutopilotStopReason::FeedbackUnavailable => {
            GliomaMechanismAutopilotDisposition::Partial
        }
        GliomaMechanismAutopilotStopReason::FeedbackInterpretationFailed
        | GliomaMechanismAutopilotStopReason::FeedbackRejected => {
            GliomaMechanismAutopilotDisposition::Blocked
        }
    };
    let mut run = GliomaMechanismAutopilotRun {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.campaign.objective.clone(),
        study_id: request.campaign.study_id.clone(),
        model_system: request.campaign.model_system,
        rounds,
        completed_action_order: completed.into_iter().collect(),
        negative_action_order: negative.into_iter().collect(),
        failed_action_order: failed.into_iter().collect(),
        retired_action_order: retired.into_iter().collect(),
        budget_spent_units: budget_spent,
        remaining_budget_units: initial_budget.saturating_sub(budget_spent),
        retry_count,
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        stop_reason,
        next_step: match stop_reason {
            GliomaMechanismAutopilotStopReason::Completed => {
                "review the executed local artifacts and provide the next typed multimodal observation".into()
            }
            GliomaMechanismAutopilotStopReason::CampaignEvidenceBlocked => {
                "resolve missing, contradictory, or underpowered graph/pathway evidence before dispatch".into()
            }
            GliomaMechanismAutopilotStopReason::BudgetExhausted => {
                "allocate a new bounded budget after reviewing completed and negative action outcomes".into()
            }
            GliomaMechanismAutopilotStopReason::NegativeEvidence => {
                "review the negative result and replan a contradiction-resolving follow-up".into()
            }
            GliomaMechanismAutopilotStopReason::ExecutorFailed => {
                "repair or replace the institution-local worker before retrying the failed action".into()
            }
            GliomaMechanismAutopilotStopReason::NoRunnableActions => {
                "supply new dependency-complete multimodal candidates".into()
            }
            GliomaMechanismAutopilotStopReason::NoProgress => {
                "inspect the unchanged frontier and add an orthogonal measurement or stronger evidence".into()
            }
            GliomaMechanismAutopilotStopReason::MaxRounds => {
                "resume with the returned completed and retired action partitions".into()
            }
            GliomaMechanismAutopilotStopReason::FeedbackUnavailable => {
                "review the local action artifact and provide a supported typed research readout before replanning".into()
            }
            GliomaMechanismAutopilotStopReason::FeedbackInterpretationFailed => {
                "repair the institution-local artifact interpreter, then resume from the preserved action artifact".into()
            }
            GliomaMechanismAutopilotStopReason::FeedbackRejected => {
                "inspect the rejected readout scope and provenance; no follow-up action was dispatched".into()
            }
        },
        digest: ContentHash::of_bytes(b"unsealed-glioma-mechanism-autopilot"),
    };
    run.digest = ContentHash::of_value(&digest_input(&run))
        .map_err(|error| GliomaMechanismAutopilotError::Digest(error.to_string()))?;
    run.validate()?;
    Ok(run)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p03_multimodal_ingestion_qc::{FeatureValue, GraphFusionRequest};
    use crate::glioma::programs::p05_mechanism_exploration::{
        PathwayActivityNode, PathwayActivityRequest,
    };
    use crate::glioma::programs::p07_protocol_simulation::action_execution::DryRunGliomaActionExecutor;
    use crate::glioma_engine::{
        GliomaModality, GliomaSelectionConfig, GliomaSelectionWeights, GliomaStageKind,
        LocalArtifactRef,
    };
    use bioprism_foundation::{AutonomyTier, Effect};
    use std::collections::BTreeSet;

    fn hash(value: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"value": value})).unwrap()
    }

    #[derive(Default)]
    struct MeasuredGliomaActionExecutor;

    impl GliomaActionExecutor for MeasuredGliomaActionExecutor {
        fn execute_action(
            &mut self,
            candidate: &GliomaActionCandidate,
            attempt: u8,
        ) -> Result<ActionExecutionResult, super::super::action_execution::ActionExecutionFailure>
        {
            let artifact = LocalArtifactRef {
                artifact_id: format!("assay-output:{}", candidate.action_id),
                content_hash: hash(&candidate.action_id),
                content_type: "application/vnd.aurora.glioma.readout+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            };
            Ok(ActionExecutionResult {
                action_id: candidate.action_id.clone(),
                disposition: ActionExecutionDisposition::Completed,
                attempt_count: attempt,
                artifact: Some(artifact),
                note: "local organoid assay produced a measured value-only artifact".into(),
                uncertainty: Vec::new(),
                negative_evidence: Vec::new(),
            })
        }
    }

    #[derive(Default)]
    struct RetryOnceGliomaActionExecutor {
        attempts: u8,
    }

    impl GliomaActionExecutor for RetryOnceGliomaActionExecutor {
        fn execute_action(
            &mut self,
            candidate: &GliomaActionCandidate,
            attempt: u8,
        ) -> Result<ActionExecutionResult, super::super::action_execution::ActionExecutionFailure>
        {
            self.attempts = self.attempts.saturating_add(1);
            if attempt == 1 {
                return Err(super::super::action_execution::ActionExecutionFailure {
                    reason: "transient local mechanism worker loss".into(),
                    retryable: true,
                });
            }
            DryRunGliomaActionExecutor.execute_action(candidate, attempt)
        }
    }

    #[derive(Default)]
    struct PathwayReadoutInterpreter {
        calls: usize,
        mismatch_source_artifact: bool,
    }

    impl GliomaMechanismFeedbackInterpreter for PathwayReadoutInterpreter {
        fn interpret_action_result(
            &mut self,
            candidate: &GliomaActionCandidate,
            result: &ActionExecutionResult,
        ) -> Result<Option<GliomaMechanismAutopilotFeedback>, String> {
            self.calls += 1;
            let artifact = result
                .artifact
                .clone()
                .ok_or_else(|| "missing local artifact".to_owned())?;
            let mut source_artifact = artifact.clone();
            if self.mismatch_source_artifact {
                source_artifact.content_hash = hash("wrong-artifact");
            }
            Ok(Some(GliomaMechanismAutopilotFeedback {
                action_id: candidate.action_id.clone(),
                source_artifact,
                graph_vectors: Vec::new(),
                pathway_observations: vec![PathwayActivityObservation {
                    observation_id: format!("readout:{}", candidate.action_id),
                    study_id: "study-autopilot".into(),
                    sample_lineage: format!("organoid-after:{}", candidate.action_id),
                    modality: GliomaModality::Proteomics,
                    model_system: GliomaModelSystem::Organoid,
                    artifact,
                    feature_id: "vimentin".into(),
                    value_milli: 250,
                    reliability_milli: 1_000,
                }],
            }))
        }
    }

    fn request() -> GliomaMechanismAutopilotRequest {
        let artifact = crate::glioma_engine::LocalArtifactRef {
            artifact_id: "autopilot-input".into(),
            content_hash: hash("autopilot-input"),
            content_type: "application/json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        };
        let pathway = PathwayActivityDefinition {
            pathway_id: "invasion".into(),
            label: "invasion".into(),
            nodes: vec![PathwayActivityNode {
                node_id: "vimentin".into(),
                label: "vimentin".into(),
                modality: GliomaModality::Proteomics,
                expected_direction: 1,
                weight_milli: 1_000,
            }],
            edges: Vec::new(),
        };
        let pathway_request = PathwayActivityRequest {
            objective: "autopilot invasion".into(),
            study_id: "study-autopilot".into(),
            model_system: GliomaModelSystem::Organoid,
            min_pathway_nodes: 1,
            min_observed_nodes: 1,
            min_modalities: 1,
            min_confidence_milli: 100,
            max_pathways: 4,
            require_cross_modal: false,
            min_edge_agreement_milli: 700,
            require_edge_consistency: true,
        };
        let graph_request = GraphFusionRequest {
            study_id: "study-autopilot".into(),
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
        let candidate = GliomaActionCandidate {
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
        GliomaMechanismAutopilotRequest {
            campaign: MultimodalMechanismCampaignRequest {
                objective: "autopilot invasion".into(),
                study_id: "study-autopilot".into(),
                model_system: GliomaModelSystem::Organoid,
                graph: graph_request,
                pathway: pathway_request,
                selection: GliomaSelectionConfig {
                    budget_units: 2,
                    max_actions: 1,
                    approval_granted: false,
                    allow_instrument_execution: false,
                    allow_federation: false,
                    weights: GliomaSelectionWeights::default(),
                },
                completed_action_order: Vec::new(),
            },
            graph_vectors: vec![
                GraphFusionVector {
                    observation_id: "obs-vimentin-a".into(),
                    study_id: "study-autopilot".into(),
                    sample_lineage: "organoid-a".into(),
                    modality: GliomaModality::Proteomics,
                    model_system: GliomaModelSystem::Organoid,
                    artifact: artifact.clone(),
                    reliability_milli: 900,
                    features: vec![FeatureValue {
                        feature_id: "vimentin".into(),
                        value_milli: 800,
                    }],
                },
                GraphFusionVector {
                    observation_id: "obs-vimentin-b".into(),
                    study_id: "study-autopilot".into(),
                    sample_lineage: "organoid-b".into(),
                    modality: GliomaModality::Proteomics,
                    model_system: GliomaModelSystem::Organoid,
                    artifact: artifact.clone(),
                    reliability_milli: 900,
                    features: vec![FeatureValue {
                        feature_id: "vimentin".into(),
                        value_milli: 700,
                    }],
                },
            ],
            pathway_definitions: vec![pathway],
            pathway_observations: vec![PathwayActivityObservation {
                observation_id: "obs-vimentin".into(),
                study_id: "study-autopilot".into(),
                sample_lineage: "organoid-a".into(),
                modality: GliomaModality::Proteomics,
                model_system: GliomaModelSystem::Organoid,
                artifact,
                feature_id: "vimentin".into(),
                value_milli: 800,
                reliability_milli: 900,
            }],
            candidates: vec![candidate],
            max_rounds: 2,
            max_retries: 1,
            require_artifacts: true,
            require_ready_for_execution: false,
            stop_on_negative: false,
        }
    }

    #[test]
    fn autopilot_executes_and_retires_a_local_mechanism_batch() {
        let mut executor = DryRunGliomaActionExecutor;
        let run = execute_glioma_mechanism_autopilot(&request(), &mut executor).unwrap();
        assert_eq!(run.feature_id, FEATURE_ID);
        assert_eq!(run.rounds.len(), 1);
        assert_eq!(run.completed_action_order, vec!["assay-invasion"]);
        assert_eq!(run.retired_action_order, vec!["assay-invasion"]);
        assert_eq!(run.budget_spent_units, 1);
        assert_eq!(
            run.stop_reason,
            GliomaMechanismAutopilotStopReason::NoRunnableActions
        );
        run.validate().unwrap();
    }

    #[test]
    fn autopilot_bills_retry_attempts_against_the_campaign_budget() {
        let mut executor = RetryOnceGliomaActionExecutor::default();
        let run = execute_glioma_mechanism_autopilot(&request(), &mut executor).unwrap();

        assert_eq!(executor.attempts, 2);
        assert_eq!(run.budget_spent_units, 2);
        assert_eq!(run.remaining_budget_units, 0);
        assert_eq!(run.retry_count, 1);
        assert_eq!(
            run.stop_reason,
            GliomaMechanismAutopilotStopReason::BudgetExhausted
        );
        run.validate().unwrap();
    }

    #[test]
    fn autopilot_without_a_readout_adapter_does_not_dispatch_a_stale_followup_batch() {
        let mut request = request();
        let mut followup = request.candidates[0].clone();
        followup.action_id = "assay-followup".into();
        request.candidates.push(followup);
        let mut executor = MeasuredGliomaActionExecutor;

        let run = execute_glioma_mechanism_autopilot(&request, &mut executor).unwrap();

        assert_eq!(run.rounds.len(), 1);
        assert_eq!(run.completed_action_order.len(), 1);
        assert_eq!(
            run.stop_reason,
            GliomaMechanismAutopilotStopReason::FeedbackUnavailable
        );
        run.validate().unwrap();
    }

    #[test]
    fn autopilot_blocks_dispatch_when_ready_gate_is_required() {
        let mut request = request();
        request.require_ready_for_execution = true;
        request.campaign.pathway.require_cross_modal = true;
        let mut executor = DryRunGliomaActionExecutor;
        let run = execute_glioma_mechanism_autopilot(&request, &mut executor).unwrap();
        assert!(run.rounds[0].execution.is_none());
        assert_eq!(
            run.stop_reason,
            GliomaMechanismAutopilotStopReason::CampaignEvidenceBlocked
        );
        assert!(run
            .negative_evidence
            .iter()
            .any(|evidence| evidence.contains("evidence-gate")));
    }

    #[test]
    fn measured_artifact_updates_pathway_evidence_before_the_next_planning_round() {
        let mut request = request();
        let mut followup = request.candidates[0].clone();
        followup.action_id = "assay-followup".into();
        request.candidates.push(followup);
        let mut executor = MeasuredGliomaActionExecutor;
        let mut interpreter = PathwayReadoutInterpreter::default();

        let run = execute_glioma_mechanism_autopilot_with_feedback(
            &request,
            &mut executor,
            &mut interpreter,
        )
        .unwrap();

        assert_eq!(interpreter.calls, 2);
        assert_eq!(run.rounds.len(), 2);
        assert_eq!(run.rounds[0].assimilated_feedback.len(), 1);
        assert_eq!(run.rounds[1].assimilated_feedback.len(), 1);
        assert_ne!(
            run.rounds[0].campaign.pathway_analysis.digest,
            run.rounds[1].campaign.pathway_analysis.digest,
            "the second round must analyze the measured result, not replay the original evidence"
        );
        run.validate().unwrap();
    }

    #[test]
    fn dry_run_artifact_never_reaches_the_scientific_feedback_interpreter() {
        let mut executor = DryRunGliomaActionExecutor;
        let mut interpreter = PathwayReadoutInterpreter::default();

        let run = execute_glioma_mechanism_autopilot_with_feedback(
            &request(),
            &mut executor,
            &mut interpreter,
        )
        .unwrap();

        assert_eq!(interpreter.calls, 0);
        assert_eq!(run.rounds.len(), 1);
        assert!(run.rounds[0].assimilated_feedback.is_empty());
        assert_eq!(
            run.stop_reason,
            GliomaMechanismAutopilotStopReason::FeedbackUnavailable
        );
        run.validate().unwrap();
    }

    #[test]
    fn feedback_from_a_different_artifact_stops_before_another_action_is_dispatched() {
        let mut request = request();
        let mut followup = request.candidates[0].clone();
        followup.action_id = "assay-followup".into();
        request.candidates.push(followup);
        let mut executor = MeasuredGliomaActionExecutor;
        let mut interpreter = PathwayReadoutInterpreter {
            calls: 0,
            mismatch_source_artifact: true,
        };

        let run = execute_glioma_mechanism_autopilot_with_feedback(
            &request,
            &mut executor,
            &mut interpreter,
        )
        .unwrap();

        assert_eq!(run.rounds.len(), 1);
        assert!(run.rounds[0].assimilated_feedback.is_empty());
        assert_eq!(run.completed_action_order.len(), 1);
        assert_eq!(
            run.stop_reason,
            GliomaMechanismAutopilotStopReason::FeedbackRejected
        );
        run.validate().unwrap();
    }
}

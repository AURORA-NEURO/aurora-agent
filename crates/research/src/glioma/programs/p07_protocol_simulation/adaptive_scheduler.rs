//! Evidence-aware, dependency-closed scheduling for autonomous preclinical glioma programs.
//!
//! The director and engine can execute a bounded batch, but a long-running research program also
//! needs a scheduler that learns from returned outcomes without mistaking a negative result for a
//! failed task.  This module provides that product seam.  It estimates a conservative posterior
//! utility for each typed action, preserves negative and inconclusive observations, and uses a
//! deterministic beam search to choose a dependency-safe portfolio under cost, risk, authority,
//! and action-count budgets.  It never invents evidence, executes code, moves raw data, or makes a
//! clinical decision.

use bioprism_foundation::{AutonomyTier, Effect};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

use crate::glioma_engine::{
    GliomaActionCandidate, GliomaModality, GliomaModelSystem, GliomaSelectionWeights,
    GliomaStageKind,
};

pub const FEATURE_ID: &str = "GAF-GLIOMA-P07-F12";
pub const OUTPUT_SCHEMA: &str = "GliomaAdaptiveWorkflowScheduler1@1";
pub const MAX_CANDIDATES: usize = 256;
pub const MAX_OBSERVATIONS: usize = 512;
pub const MAX_ACTIONS: u16 = 64;
pub const MAX_BEAM_WIDTH: u16 = 128;

/// A typed outcome from a prior local run.  Negative and inconclusive outcomes remain informative
/// and are never silently turned into success or deleted from the scheduler state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SchedulerOutcome {
    Qualified,
    Negative,
    Inconclusive,
    Failed,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchedulerObservation {
    pub action_id: String,
    pub outcome: SchedulerOutcome,
    pub information_gain_milli: u16,
    pub uncertainty_milli: u16,
    pub round: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaAdaptiveWorkflowSchedulerRequest {
    pub mission_id: String,
    pub objective: String,
    pub candidates: Vec<GliomaActionCandidate>,
    pub completed_action_order: Vec<String>,
    pub observations: Vec<SchedulerObservation>,
    pub budget_units: u32,
    pub max_actions: u16,
    pub beam_width: u16,
    pub risk_budget_milli: u32,
    pub approval_granted: bool,
    pub allow_instrument_execution: bool,
    pub allow_federation: bool,
    pub selection_weights: GliomaSelectionWeights,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GliomaAdaptiveWorkflowSchedulerDisposition {
    Ready,
    Partial,
    Blocked,
    Exhausted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchedulerDecision {
    pub action_id: String,
    pub stage_kind: GliomaStageKind,
    pub modality: GliomaModality,
    pub model_system: GliomaModelSystem,
    pub expected_gain_milli: u16,
    pub risk_milli: u16,
    pub posterior_reliability_milli: u16,
    pub score_milli_per_cost: u64,
    pub selected: bool,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaAdaptiveWorkflowSchedulerPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub mission_id: String,
    pub objective: String,
    pub candidate_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub completed_action_order: Vec<String>,
    pub deferred_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub decisions: Vec<SchedulerDecision>,
    pub planned_cost_units: u32,
    pub remaining_budget_units: u32,
    pub risk_used_milli: u32,
    pub expected_information_gain_milli: u32,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: GliomaAdaptiveWorkflowSchedulerDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GliomaAdaptiveWorkflowSchedulerError {
    #[error("glioma adaptive workflow scheduler request is invalid: {0}")]
    InvalidRequest(String),
    #[error("glioma adaptive workflow scheduler output is invalid: {0}")]
    InvalidOutput(String),
    #[error("glioma adaptive workflow scheduler digest failed: {0}")]
    Digest(String),
}

#[derive(Debug, Clone)]
struct ScoredCandidate {
    candidate: GliomaActionCandidate,
    expected_gain_milli: u16,
    risk_milli: u16,
    posterior_reliability_milli: u16,
    score_milli_per_cost: u64,
    block_reason: Option<String>,
}

#[derive(Debug, Clone)]
struct BeamState {
    selected: Vec<String>,
    selected_set: BTreeSet<String>,
    cost_units: u32,
    risk_milli: u32,
    expected_gain_milli: u32,
    score: i64,
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn sum_weights(weights: GliomaSelectionWeights) -> u16 {
    weights.information_gain
        + weights.frontier_novelty
        + weights.workflow_leverage
        + weights.cross_stage_unlock
        + weights.reproducibility_safety
        + weights.federation_value
        + weights.feasibility
}

fn autonomy_risk(tier: AutonomyTier) -> u32 {
    match tier {
        AutonomyTier::A0 => 0,
        AutonomyTier::A1 => 100,
        AutonomyTier::A2 => 350,
        AutonomyTier::A3 => 700,
        AutonomyTier::A4 => 950,
    }
}

fn block_reason(
    candidate: &GliomaActionCandidate,
    request: &GliomaAdaptiveWorkflowSchedulerRequest,
) -> Option<String> {
    if !request.approval_granted && candidate.autonomy_tier.requires_approval() {
        return Some("approval-required".into());
    }
    if candidate.effects.contains(&Effect::InstrumentExecution)
        && !request.allow_instrument_execution
    {
        return Some("instrument-execution-disabled".into());
    }
    if candidate.effects.contains(&Effect::FederationExport) && !request.allow_federation {
        return Some("federation-export-disabled".into());
    }
    if candidate.effects.contains(&Effect::ExternalDataAccess) {
        return Some("external-data-access-disabled".into());
    }
    None
}

fn validate_candidate(
    candidate: &GliomaActionCandidate,
    known: &BTreeSet<String>,
) -> Result<(), GliomaAdaptiveWorkflowSchedulerError> {
    if candidate.action_id.trim().is_empty()
        || candidate.cost_units == 0
        || candidate.effects.is_empty()
        || candidate.depends_on.iter().any(|dependency| {
            dependency.trim().is_empty()
                || dependency == &candidate.action_id
                || !known.contains(dependency)
        })
        || !canonical(&candidate.depends_on)
        || [
            candidate.information_gain_milli,
            candidate.frontier_novelty_milli,
            candidate.workflow_leverage_milli,
            candidate.cross_stage_unlock_milli,
            candidate.reproducibility_safety_milli,
            candidate.federation_value_milli,
            candidate.feasibility_milli,
        ]
        .iter()
        .any(|score| *score > 1000)
    {
        return Err(GliomaAdaptiveWorkflowSchedulerError::InvalidRequest(
            format!(
                "candidate {} has an invalid identity, dependency, effect, cost, or score",
                candidate.action_id
            ),
        ));
    }
    Ok(())
}

fn detect_cycle(
    id: &str,
    candidates: &BTreeMap<String, GliomaActionCandidate>,
    marks: &mut BTreeMap<String, u8>,
) -> bool {
    match marks.get(id).copied() {
        Some(1) => return true,
        Some(2) => return false,
        _ => {}
    }
    marks.insert(id.to_string(), 1);
    let cycle = candidates
        .get(id)
        .map(|candidate| {
            candidate
                .depends_on
                .iter()
                .any(|dependency| detect_cycle(dependency, candidates, marks))
        })
        .unwrap_or(true);
    marks.insert(id.to_string(), 2);
    cycle
}

fn base_score(candidate: &GliomaActionCandidate, weights: GliomaSelectionWeights) -> u64 {
    candidate.information_gain_milli as u64 * weights.information_gain as u64
        + candidate.frontier_novelty_milli as u64 * weights.frontier_novelty as u64
        + candidate.workflow_leverage_milli as u64 * weights.workflow_leverage as u64
        + candidate.cross_stage_unlock_milli as u64 * weights.cross_stage_unlock as u64
        + candidate.reproducibility_safety_milli as u64 * weights.reproducibility_safety as u64
        + candidate.federation_value_milli as u64 * weights.federation_value as u64
        + candidate.feasibility_milli as u64 * weights.feasibility as u64
}

fn score_candidate(
    candidate: GliomaActionCandidate,
    observations: &[SchedulerObservation],
    request: &GliomaAdaptiveWorkflowSchedulerRequest,
) -> ScoredCandidate {
    let relevant = observations
        .iter()
        .filter(|observation| observation.action_id == candidate.action_id)
        .collect::<Vec<_>>();
    let mut reliability = 600_i32;
    let mut observed_gain = 0_u16;
    for observation in relevant {
        observed_gain = observed_gain.max(observation.information_gain_milli);
        reliability += match observation.outcome {
            SchedulerOutcome::Qualified => 130,
            SchedulerOutcome::Negative => 110,
            SchedulerOutcome::Inconclusive => 60,
            SchedulerOutcome::Failed => -170,
            SchedulerOutcome::Blocked => -250,
        };
    }
    let reliability = reliability.clamp(100, 1_000) as u16;
    let base = base_score(&candidate, request.selection_weights) / 100;
    let gap_bonus = u64::from(1_000_u16.saturating_sub(observed_gain))
        .saturating_mul(u64::from(candidate.information_gain_milli))
        / 5_000;
    let expected_gain = base
        .saturating_mul(u64::from(reliability))
        .saturating_div(1_000)
        .saturating_add(gap_bonus)
        .min(1_000) as u16;
    let mut risk = autonomy_risk(candidate.autonomy_tier).saturating_add(
        u32::from(1_000_u16.saturating_sub(candidate.reproducibility_safety_milli)) / 5,
    );
    if candidate.effects.contains(&Effect::InstrumentExecution) {
        risk = risk.saturating_add(200);
    }
    if candidate.effects.contains(&Effect::FederationExport) {
        risk = risk.saturating_add(150);
    }
    if candidate.effects.contains(&Effect::ExternalDataAccess) {
        risk = risk.saturating_add(100);
    }
    let risk = risk.min(1_000) as u16;
    let block_reason = block_reason(&candidate, request);
    ScoredCandidate {
        score_milli_per_cost: u64::from(expected_gain)
            .saturating_mul(1_000)
            .saturating_div(u64::from(candidate.cost_units)),
        candidate,
        expected_gain_milli: expected_gain,
        risk_milli: risk,
        posterior_reliability_milli: reliability,
        block_reason,
    }
}

fn state_score(
    state: &BeamState,
    candidates: &BTreeMap<String, ScoredCandidate>,
    completed: &BTreeSet<String>,
) -> i64 {
    let mut modalities = BTreeSet::new();
    let mut models = BTreeSet::new();
    let mut ready = 0_i64;
    for id in &state.selected {
        if let Some(candidate) = candidates.get(id) {
            modalities.insert(candidate.candidate.modality);
            models.insert(candidate.candidate.model_system);
        }
    }
    for (id, candidate) in candidates {
        if state.selected_set.contains(id)
            || completed.contains(id)
            || candidate.block_reason.is_some()
        {
            continue;
        }
        if candidate.candidate.depends_on.iter().all(|dependency| {
            completed.contains(dependency) || state.selected_set.contains(dependency)
        }) {
            ready += 1;
        }
    }
    i64::from(state.expected_gain_milli) * 1_000_000
        + i64::try_from(modalities.len()).unwrap_or(i64::MAX) * 10_000
        + i64::try_from(models.len()).unwrap_or(i64::MAX) * 10_000
        + ready * 1_000
        - i64::from(state.cost_units) * 100
        - i64::from(state.risk_milli) * 50
}

fn better_state(left: &BeamState, right: &BeamState) -> bool {
    left.score > right.score || (left.score == right.score && left.selected < right.selected)
}

fn digest_input(plan: &GliomaAdaptiveWorkflowSchedulerPlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": plan.feature_id,
        "output_schema": plan.output_schema,
        "mission_id": plan.mission_id,
        "objective": plan.objective,
        "candidate_order": plan.candidate_order,
        "selected_order": plan.selected_order,
        "completed_action_order": plan.completed_action_order,
        "deferred_order": plan.deferred_order,
        "blocked_order": plan.blocked_order,
        "decisions": plan.decisions,
        "planned_cost_units": plan.planned_cost_units,
        "remaining_budget_units": plan.remaining_budget_units,
        "risk_used_milli": plan.risk_used_milli,
        "expected_information_gain_milli": plan.expected_information_gain_milli,
        "negative_evidence": plan.negative_evidence,
        "uncertainty": plan.uncertainty,
        "disposition": plan.disposition,
    })
}

impl GliomaAdaptiveWorkflowSchedulerPlan {
    pub fn validate(&self) -> Result<(), GliomaAdaptiveWorkflowSchedulerError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.mission_id.trim().is_empty()
            || self.objective.trim().is_empty()
            || self.candidate_order.is_empty()
            || !canonical(&self.candidate_order)
            || !canonical(&self.completed_action_order)
            || !canonical(&self.deferred_order)
            || !canonical(&self.blocked_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || !self
                .selected_order
                .iter()
                .all(|id| self.candidate_order.binary_search(id).is_ok())
            || self.decisions.len() != self.candidate_order.len()
            || self
                .decisions
                .iter()
                .map(|decision| decision.action_id.clone())
                .collect::<Vec<_>>()
                != self.candidate_order
            || self.selected_order.iter().any(|id| {
                self.deferred_order.binary_search(id).is_ok()
                    || self.blocked_order.binary_search(id).is_ok()
            })
            || self
                .deferred_order
                .iter()
                .any(|id| self.blocked_order.binary_search(id).is_ok())
            || self
                .selected_order
                .iter()
                .chain(self.deferred_order.iter())
                .chain(self.blocked_order.iter())
                .collect::<BTreeSet<_>>()
                .len()
                != self.candidate_order.len()
            || self.risk_used_milli > 1_000_000
        {
            return Err(GliomaAdaptiveWorkflowSchedulerError::InvalidOutput(
                "identity, ordering, partition, decision, or risk invariants are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| GliomaAdaptiveWorkflowSchedulerError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(GliomaAdaptiveWorkflowSchedulerError::InvalidOutput(
                "scheduler digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Choose a dependency-safe portfolio using a conservative outcome-updated utility model and a
/// deterministic beam search.  The selected order is executable as-is: every prerequisite is
/// either already completed or appears earlier in the returned order.
pub fn plan_glioma_adaptive_workflow(
    request: &GliomaAdaptiveWorkflowSchedulerRequest,
) -> Result<GliomaAdaptiveWorkflowSchedulerPlan, GliomaAdaptiveWorkflowSchedulerError> {
    if request.mission_id.trim().is_empty()
        || request.objective.trim().is_empty()
        || request.candidates.is_empty()
        || request.candidates.len() > MAX_CANDIDATES
        || request.budget_units == 0
        || request.max_actions == 0
        || request.max_actions > MAX_ACTIONS
        || request.beam_width == 0
        || request.beam_width > MAX_BEAM_WIDTH
        || request.risk_budget_milli > 1_000_000
        || request.observations.len() > MAX_OBSERVATIONS
        || sum_weights(request.selection_weights) != 100
    {
        return Err(GliomaAdaptiveWorkflowSchedulerError::InvalidRequest(
            "mission/objective, bounded candidates, budget, action/beam/risk limits, observations, and weights are required".into(),
        ));
    }
    let mut candidate_map = BTreeMap::new();
    for candidate in &request.candidates {
        if candidate_map
            .insert(candidate.action_id.clone(), candidate.clone())
            .is_some()
        {
            return Err(GliomaAdaptiveWorkflowSchedulerError::InvalidRequest(
                "candidate action ids must be unique".into(),
            ));
        }
    }
    let known = candidate_map.keys().cloned().collect::<BTreeSet<_>>();
    for candidate in candidate_map.values() {
        validate_candidate(candidate, &known)?;
    }
    let mut marks = BTreeMap::new();
    if candidate_map
        .keys()
        .any(|id| detect_cycle(id, &candidate_map, &mut marks))
    {
        return Err(GliomaAdaptiveWorkflowSchedulerError::InvalidRequest(
            "candidate dependency graph contains a cycle".into(),
        ));
    }
    if !canonical(&request.completed_action_order)
        || request
            .completed_action_order
            .iter()
            .any(|id| !known.contains(id))
    {
        return Err(GliomaAdaptiveWorkflowSchedulerError::InvalidRequest(
            "completed actions must be canonical and reference known candidates".into(),
        ));
    }
    let mut seen_observations = BTreeSet::new();
    for observation in &request.observations {
        if observation.action_id.trim().is_empty()
            || !known.contains(&observation.action_id)
            || observation.information_gain_milli > 1_000
            || observation.uncertainty_milli > 1_000
            || observation.round == 0
            || !seen_observations.insert(observation.action_id.clone())
        {
            return Err(GliomaAdaptiveWorkflowSchedulerError::InvalidRequest(
                "observations must name each known action at most once with bounded values".into(),
            ));
        }
    }
    let mut scored = BTreeMap::new();
    for candidate in candidate_map.values() {
        scored.insert(
            candidate.action_id.clone(),
            score_candidate(candidate.clone(), &request.observations, request),
        );
    }
    let completed = request
        .completed_action_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let initial = BeamState {
        selected: Vec::new(),
        selected_set: BTreeSet::new(),
        cost_units: 0,
        risk_milli: 0,
        expected_gain_milli: 0,
        score: 0,
    };
    let mut beam = vec![initial.clone()];
    let mut best = initial;
    for _ in 0..request.max_actions {
        let mut next = beam.clone();
        for state in &beam {
            for (id, candidate) in &scored {
                if state.selected_set.contains(id)
                    || completed.contains(id)
                    || candidate.block_reason.is_some()
                    || candidate.candidate.depends_on.iter().any(|dependency| {
                        !completed.contains(dependency) && !state.selected_set.contains(dependency)
                    })
                {
                    continue;
                }
                let cost = state
                    .cost_units
                    .saturating_add(candidate.candidate.cost_units);
                let risk = state
                    .risk_milli
                    .saturating_add(u32::from(candidate.risk_milli));
                if cost > request.budget_units || risk > request.risk_budget_milli {
                    continue;
                }
                let mut expanded = state.clone();
                expanded.selected.push(id.clone());
                expanded.selected_set.insert(id.clone());
                expanded.cost_units = cost;
                expanded.risk_milli = risk;
                expanded.expected_gain_milli = expanded
                    .expected_gain_milli
                    .saturating_add(u32::from(candidate.expected_gain_milli));
                expanded.score = state_score(&expanded, &scored, &completed);
                next.push(expanded);
            }
        }
        next.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| left.selected.cmp(&right.selected))
        });
        let mut seen = BTreeSet::new();
        beam = next
            .into_iter()
            .filter(|state| seen.insert(state.selected.clone()))
            .take(request.beam_width as usize)
            .collect();
        if let Some(candidate) = beam.iter().find(|state| better_state(state, &best)) {
            best = candidate.clone();
        }
        if beam.is_empty() {
            break;
        }
    }
    let selected = best.selected.iter().cloned().collect::<BTreeSet<_>>();
    let mut deferred = Vec::new();
    let mut blocked = Vec::new();
    let mut decisions = Vec::new();
    let mut negative_evidence = BTreeSet::new();
    for observation in &request.observations {
        if observation.outcome == SchedulerOutcome::Negative {
            negative_evidence.insert(format!(
                "{}:negative-result-preserved",
                observation.action_id
            ));
        }
    }
    let mut uncertainty = BTreeSet::new();
    if request.observations.is_empty() {
        uncertainty.insert("prior-only-no-observations".into());
    }
    for (id, candidate) in &scored {
        let is_selected = selected.contains(id);
        let reason = if is_selected {
            "selected-by-outcome-aware-dependency-beam".into()
        } else if let Some(reason) = &candidate.block_reason {
            blocked.push(id.clone());
            format!("blocked:{reason}")
        } else if completed.contains(id) {
            deferred.push(id.clone());
            "already-completed".into()
        } else {
            deferred.push(id.clone());
            uncertainty.insert(format!("{id}:not-selected-under-current-budget-or-risk"));
            "deferred:budget-risk-action-limit-or-portfolio-tradeoff".into()
        };
        decisions.push(SchedulerDecision {
            action_id: id.clone(),
            stage_kind: candidate.candidate.stage_kind,
            modality: candidate.candidate.modality,
            model_system: candidate.candidate.model_system,
            expected_gain_milli: candidate.expected_gain_milli,
            risk_milli: candidate.risk_milli,
            posterior_reliability_milli: candidate.posterior_reliability_milli,
            score_milli_per_cost: candidate.score_milli_per_cost,
            selected: is_selected,
            reason,
        });
    }
    let candidate_order = scored.keys().cloned().collect::<Vec<_>>();
    let disposition = if best.selected.is_empty() {
        if blocked.is_empty() {
            GliomaAdaptiveWorkflowSchedulerDisposition::Exhausted
        } else {
            GliomaAdaptiveWorkflowSchedulerDisposition::Blocked
        }
    } else if deferred.is_empty() && blocked.is_empty() {
        GliomaAdaptiveWorkflowSchedulerDisposition::Ready
    } else {
        GliomaAdaptiveWorkflowSchedulerDisposition::Partial
    };
    let mut plan = GliomaAdaptiveWorkflowSchedulerPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        mission_id: request.mission_id.clone(),
        objective: request.objective.clone(),
        candidate_order,
        selected_order: best.selected,
        completed_action_order: request.completed_action_order.clone(),
        deferred_order: deferred,
        blocked_order: blocked,
        decisions,
        planned_cost_units: best.cost_units,
        remaining_budget_units: request.budget_units.saturating_sub(best.cost_units),
        risk_used_milli: best.risk_milli,
        expected_information_gain_milli: best.expected_gain_milli,
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-adaptive-workflow-scheduler"),
    };
    plan.digest = ContentHash::of_value(&digest_input(&plan))
        .map_err(|error| GliomaAdaptiveWorkflowSchedulerError::Digest(error.to_string()))?;
    plan.validate()?;
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(
        id: &str,
        stage_kind: GliomaStageKind,
        depends_on: Vec<&str>,
        cost_units: u32,
        information_gain_milli: u16,
    ) -> GliomaActionCandidate {
        GliomaActionCandidate {
            action_id: id.into(),
            stage_kind,
            modality: GliomaModality::Computational,
            model_system: GliomaModelSystem::Organoid,
            depends_on: depends_on.into_iter().map(str::to_string).collect(),
            cost_units,
            information_gain_milli,
            frontier_novelty_milli: information_gain_milli,
            workflow_leverage_milli: information_gain_milli,
            cross_stage_unlock_milli: information_gain_milli,
            reproducibility_safety_milli: 900,
            federation_value_milli: 300,
            feasibility_milli: 800,
            autonomy_tier: AutonomyTier::A1,
            effects: BTreeSet::from([Effect::ReadLocalData, Effect::ExecuteLocalComputation]),
        }
    }

    fn request(candidates: Vec<GliomaActionCandidate>) -> GliomaAdaptiveWorkflowSchedulerRequest {
        GliomaAdaptiveWorkflowSchedulerRequest {
            mission_id: "scheduler-mission".into(),
            objective: "identify a reproducible glioma mechanism".into(),
            candidates,
            completed_action_order: Vec::new(),
            observations: Vec::new(),
            budget_units: 6,
            max_actions: 3,
            beam_width: 32,
            risk_budget_milli: 2_000,
            approval_granted: false,
            allow_instrument_execution: false,
            allow_federation: false,
            selection_weights: GliomaSelectionWeights::default(),
        }
    }

    #[test]
    fn dependency_aware_beam_prefers_prerequisite_plus_downstream_gain() {
        let output = plan_glioma_adaptive_workflow(&request(vec![
            candidate(
                "isolated",
                GliomaStageKind::MechanismExploration,
                vec![],
                6,
                700,
            ),
            candidate(
                "prereq",
                GliomaStageKind::MultimodalIngestionQc,
                vec![],
                3,
                300,
            ),
            candidate(
                "downstream",
                GliomaStageKind::MechanismExploration,
                vec!["prereq"],
                3,
                950,
            ),
        ]))
        .unwrap();
        assert_eq!(output.selected_order, vec!["prereq", "downstream"]);
        assert_eq!(output.planned_cost_units, 6);
        output.validate().unwrap();
    }

    #[test]
    fn negative_observation_is_preserved_without_suppressing_future_information() {
        let mut request = request(vec![candidate(
            "negative-assay",
            GliomaStageKind::ExperimentDesign,
            vec![],
            2,
            850,
        )]);
        request.observations = vec![SchedulerObservation {
            action_id: "negative-assay".into(),
            outcome: SchedulerOutcome::Negative,
            information_gain_milli: 900,
            uncertainty_milli: 300,
            round: 1,
        }];
        let output = plan_glioma_adaptive_workflow(&request).unwrap();
        assert!(output
            .negative_evidence
            .contains(&"negative-assay:negative-result-preserved".to_string()));
        assert!(output
            .selected_order
            .contains(&"negative-assay".to_string()));
    }

    #[test]
    fn authority_and_effect_gates_block_unsafe_actions() {
        let mut instrument = candidate(
            "instrument",
            GliomaStageKind::InstrumentPreflight,
            vec![],
            1,
            900,
        );
        instrument.autonomy_tier = AutonomyTier::A3;
        instrument.effects.insert(Effect::InstrumentExecution);
        let output = plan_glioma_adaptive_workflow(&request(vec![instrument])).unwrap();
        assert_eq!(
            output.disposition,
            GliomaAdaptiveWorkflowSchedulerDisposition::Blocked
        );
        assert_eq!(output.blocked_order, vec!["instrument"]);
        assert!(output.selected_order.is_empty());
    }

    #[test]
    fn scheduler_replays_byte_stably() {
        let request = request(vec![
            candidate("a", GliomaStageKind::EvidenceCompilation, vec![], 2, 500),
            candidate(
                "b",
                GliomaStageKind::MechanismExploration,
                vec!["a"],
                2,
                500,
            ),
        ]);
        let first = plan_glioma_adaptive_workflow(&request).unwrap();
        let second = plan_glioma_adaptive_workflow(&request).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.digest, second.digest);
    }
}

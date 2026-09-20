//! Replayable computation to interpretation/replication frontier.
//!
//! P09 computation campaigns currently end with task-level replay and failure partitions. This
//! module makes those outcomes useful to the autonomous research engine: completed or cached
//! computation becomes an interpretation candidate, negative computation becomes replication
//! work, and partial/failed/skipped computation becomes a bounded recovery candidate. The bridge
//! carries the replay identity and evaluation obligations forward; it never calls a result a
//! biological conclusion.

use super::campaign::GliomaComputationCampaign;
use crate::glioma::programs::p07_protocol_simulation::{
    execute_glioma_autonomous_research_mission, GliomaActionExecutor,
    GliomaAutonomousResearchMission, GliomaMissionDisposition, GliomaMissionError,
    GliomaMissionGates, GliomaMissionRequest,
};
use crate::glioma_engine::{
    GliomaActionCandidate, GliomaModality, GliomaSelectionConfig, GliomaStageKind,
};
use bioprism_foundation::{AutonomyTier, Effect};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P09-F26";
pub const OUTPUT_SCHEMA: &str = "GliomaComputationInterpretationFrontier1@1";
pub const MAX_ACTIONS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationInterpretationFrontierRequest {
    pub mission_id: String,
    pub objective: String,
    pub campaign: GliomaComputationCampaign,
    pub modality: GliomaModality,
    pub max_actions: usize,
    pub default_cost_units: u32,
    #[serde(default)]
    pub completed_action_order: Vec<String>,
    pub selection: GliomaSelectionConfig,
    pub gates: GliomaMissionGates,
    pub max_rounds: u16,
    pub max_retries: u8,
    pub require_artifacts: bool,
    pub stop_on_negative: bool,
    pub allow_empty_frontier: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationInterpretationFrontier {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub source_campaign_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub action_order: Vec<String>,
    pub candidates: Vec<GliomaActionCandidate>,
    pub completed_source_order: Vec<String>,
    pub negative_source_order: Vec<String>,
    pub unresolved_source_order: Vec<String>,
    pub evaluation_requirement_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub digest: ContentHash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputationInterpretationFrontierDisposition {
    Executed,
    Held,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationInterpretationFrontierRun {
    pub feature_id: String,
    pub output_schema: String,
    pub mission_id: String,
    pub objective: String,
    pub frontier: ComputationInterpretationFrontier,
    pub mission: Option<GliomaAutonomousResearchMission>,
    pub disposition: ComputationInterpretationFrontierDisposition,
    pub next_step: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ComputationInterpretationFrontierError {
    #[error("computation interpretation frontier request is invalid: {0}")]
    InvalidRequest(String),
    #[error("computation interpretation frontier input is invalid: {0}")]
    InvalidInput(String),
    #[error("computation interpretation frontier mission failed: {0}")]
    Mission(#[from] GliomaMissionError),
    #[error("computation interpretation frontier output is invalid: {0}")]
    InvalidOutput(String),
    #[error("computation interpretation frontier digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(frontier: &ComputationInterpretationFrontier) -> serde_json::Value {
    serde_json::json!({
        "feature_id": frontier.feature_id,
        "output_schema": frontier.output_schema,
        "objective": frontier.objective,
        "source_campaign_digest": frontier.source_campaign_digest,
        "replay_identity": frontier.replay_identity,
        "action_order": frontier.action_order,
        "candidates": frontier.candidates,
        "completed_source_order": frontier.completed_source_order,
        "negative_source_order": frontier.negative_source_order,
        "unresolved_source_order": frontier.unresolved_source_order,
        "evaluation_requirement_order": frontier.evaluation_requirement_order,
        "negative_evidence": frontier.negative_evidence,
        "uncertainty": frontier.uncertainty,
    })
}

fn run_digest_input(run: &ComputationInterpretationFrontierRun) -> serde_json::Value {
    serde_json::json!({
        "feature_id": run.feature_id,
        "output_schema": run.output_schema,
        "mission_id": run.mission_id,
        "objective": run.objective,
        "frontier": run.frontier,
        "mission": run.mission,
        "disposition": run.disposition,
        "next_step": run.next_step,
    })
}

fn candidate_id(task_id: &str, stage: GliomaStageKind) -> String {
    format!("computation-frontier:{task_id}:{}", stage.stage_id())
}

fn action_for_status(
    task_id: &str,
    status: &str,
    modality: GliomaModality,
    cost_units: u32,
) -> GliomaActionCandidate {
    let (stage_kind, information, novelty, leverage, safety) = match status {
        "completed" | "cached" => (
            GliomaStageKind::StatisticalInterpretation,
            850,
            700,
            900,
            900,
        ),
        "negative" => (GliomaStageKind::ReplicationRobustness, 900, 900, 850, 950),
        _ => (GliomaStageKind::ComputationalExecution, 950, 800, 900, 850),
    };
    GliomaActionCandidate {
        action_id: candidate_id(task_id, stage_kind),
        stage_kind,
        modality,
        model_system: crate::glioma_engine::GliomaModelSystem::InSilico,
        depends_on: Vec::new(),
        cost_units: cost_units.max(1),
        information_gain_milli: information,
        frontier_novelty_milli: novelty,
        workflow_leverage_milli: leverage,
        cross_stage_unlock_milli: if stage_kind == GliomaStageKind::StatisticalInterpretation {
            950
        } else {
            850
        },
        reproducibility_safety_milli: safety,
        federation_value_milli: safety,
        feasibility_milli: 900,
        autonomy_tier: AutonomyTier::A1,
        effects: BTreeSet::from([
            Effect::ReadLocalData,
            Effect::ExecuteLocalComputation,
            Effect::WriteLocalArtifact,
        ]),
    }
}

fn validate_request(
    request: &ComputationInterpretationFrontierRequest,
) -> Result<(), ComputationInterpretationFrontierError> {
    if request.mission_id.trim().is_empty()
        || request.objective.trim().is_empty()
        || request.max_actions == 0
        || request.max_actions > MAX_ACTIONS
        || request.default_cost_units == 0
        || request.selection.budget_units == 0
        || request.selection.max_actions == 0
        || request.max_rounds == 0
        || !canonical(&request.completed_action_order)
        || request
            .completed_action_order
            .iter()
            .any(|id| id.trim().is_empty())
    {
        return Err(ComputationInterpretationFrontierError::InvalidRequest(
            "mission identity, objective, bounded actions, positive costs/budget, rounds, and canonical completed actions are required".into(),
        ));
    }
    request
        .campaign
        .validate()
        .map_err(|error| ComputationInterpretationFrontierError::InvalidInput(error.to_string()))?;
    if request.objective != request.campaign.objective {
        return Err(ComputationInterpretationFrontierError::InvalidRequest(
            "frontier objective must match the computation campaign objective".into(),
        ));
    }
    Ok(())
}

impl ComputationInterpretationFrontier {
    pub fn validate(&self) -> Result<(), ComputationInterpretationFrontierError> {
        let ids = self
            .candidates
            .iter()
            .map(|candidate| candidate.action_id.clone())
            .collect::<Vec<_>>();
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.source_campaign_digest.as_str().len() != 64
            || self.replay_identity.as_str().len() != 64
            || self.action_order != ids
            || !canonical(&self.action_order)
            || self.candidates.len() > MAX_ACTIONS
            || !canonical(&self.completed_source_order)
            || !canonical(&self.negative_source_order)
            || !canonical(&self.unresolved_source_order)
            || !canonical(&self.evaluation_requirement_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.candidates.iter().any(|candidate| {
                candidate.autonomy_tier != AutonomyTier::A1
                    || candidate.effects
                        != BTreeSet::from([
                            Effect::ReadLocalData,
                            Effect::ExecuteLocalComputation,
                            Effect::WriteLocalArtifact,
                        ])
            })
        {
            return Err(ComputationInterpretationFrontierError::InvalidOutput(
                "frontier identity, ordering, bounds, local-effect, or replay invariants are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ComputationInterpretationFrontierError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ComputationInterpretationFrontierError::InvalidOutput(
                "frontier digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

impl ComputationInterpretationFrontierRun {
    pub fn validate(&self) -> Result<(), ComputationInterpretationFrontierError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.mission_id.trim().is_empty()
            || self.objective.trim().is_empty()
            || self.frontier.objective != self.objective
            || matches!(
                self.disposition,
                ComputationInterpretationFrontierDisposition::Held
            ) == self.mission.is_some()
            || self.next_step.trim().is_empty()
        {
            return Err(ComputationInterpretationFrontierError::InvalidOutput(
                "run identity, frontier binding, mission presence, or next-step invariants are invalid".into(),
            ));
        }
        self.frontier.validate()?;
        if let Some(mission) = &self.mission {
            mission.validate()?;
            if mission.mission_id != self.mission_id || mission.objective != self.objective {
                return Err(ComputationInterpretationFrontierError::InvalidOutput(
                    "nested mission identity does not match the computation frontier".into(),
                ));
            }
        }
        let expected = ContentHash::of_value(&run_digest_input(self))
            .map_err(|error| ComputationInterpretationFrontierError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ComputationInterpretationFrontierError::InvalidOutput(
                "frontier run digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Compile computation outcomes into interpretation, replication, or recovery candidates.
pub fn compile_glioma_computation_interpretation_frontier(
    request: &ComputationInterpretationFrontierRequest,
) -> Result<ComputationInterpretationFrontier, ComputationInterpretationFrontierError> {
    validate_request(request)?;
    let mut candidates = BTreeMap::<String, GliomaActionCandidate>::new();
    let mut completed_source = BTreeSet::new();
    let mut negative_source = BTreeSet::new();
    let mut unresolved_source = BTreeSet::new();
    let mut statuses = BTreeMap::<String, &'static str>::new();
    for id in request
        .campaign
        .completed_order
        .iter()
        .chain(request.campaign.cached_order.iter())
    {
        completed_source.insert(id.clone());
        statuses.insert(
            id.clone(),
            if request.campaign.cached_order.binary_search(id).is_ok() {
                "cached"
            } else {
                "completed"
            },
        );
    }
    for id in &request.campaign.negative_order {
        negative_source.insert(id.clone());
        statuses.insert(id.clone(), "negative");
    }
    for (ids, status) in [
        (&request.campaign.partial_order, "partial"),
        (&request.campaign.failed_order, "failed"),
        (&request.campaign.skipped_order, "skipped"),
    ] {
        for id in ids {
            unresolved_source.insert(id.clone());
            statuses.insert(id.clone(), status);
        }
    }
    for (task_id, status) in statuses {
        let candidate = action_for_status(
            &task_id,
            status,
            request.modality,
            request.default_cost_units,
        );
        candidates
            .entry(candidate.action_id.clone())
            .or_insert(candidate);
    }
    let mut candidates = candidates.into_values().collect::<Vec<_>>();
    candidates.sort_by(|left, right| left.action_id.cmp(&right.action_id));
    candidates.truncate(request.max_actions);
    if candidates.is_empty() && !request.allow_empty_frontier {
        return Err(ComputationInterpretationFrontierError::InvalidInput(
            "computation campaign produced no interpretation, replication, or recovery action"
                .into(),
        ));
    }
    let mut requirements = BTreeSet::from([
        "replay-identity-check".to_string(),
        "baseline-comparison".to_string(),
        "uncertainty-and-negative-result-review".to_string(),
    ]);
    if !negative_source.is_empty() {
        requirements.insert("negative-result-replication-or-falsification".into());
    }
    if !unresolved_source.is_empty() {
        requirements.insert("failed-partial-task-recovery-before-interpretation".into());
    }
    let mut frontier = ComputationInterpretationFrontier {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        source_campaign_digest: request.campaign.digest.clone(),
        replay_identity: request.campaign.replay_identity.clone(),
        action_order: candidates
            .iter()
            .map(|candidate| candidate.action_id.clone())
            .collect(),
        candidates,
        completed_source_order: completed_source.into_iter().collect(),
        negative_source_order: negative_source.into_iter().collect(),
        unresolved_source_order: unresolved_source.into_iter().collect(),
        evaluation_requirement_order: requirements.into_iter().collect(),
        negative_evidence: request.campaign.negative_evidence.clone(),
        uncertainty: request.campaign.uncertainty.clone(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-computation-interpretation-frontier"),
    };
    frontier.digest = ContentHash::of_value(&digest_input(&frontier))
        .map_err(|error| ComputationInterpretationFrontierError::Digest(error.to_string()))?;
    frontier.validate()?;
    Ok(frontier)
}

/// Compile and execute the computation-derived frontier through the P07 mission controller.
pub fn execute_glioma_computation_interpretation_frontier<E: GliomaActionExecutor>(
    request: &ComputationInterpretationFrontierRequest,
    executor: &mut E,
) -> Result<ComputationInterpretationFrontierRun, ComputationInterpretationFrontierError> {
    let frontier = compile_glioma_computation_interpretation_frontier(request)?;
    if request
        .completed_action_order
        .iter()
        .any(|id| frontier.action_order.binary_search(id).is_err())
    {
        return Err(ComputationInterpretationFrontierError::InvalidRequest(
            "completed action is not present in the compiled computation frontier".into(),
        ));
    }
    if frontier.candidates.is_empty() {
        let mut output = ComputationInterpretationFrontierRun {
            feature_id: FEATURE_ID.into(),
            output_schema: OUTPUT_SCHEMA.into(),
            mission_id: request.mission_id.clone(),
            objective: request.objective.clone(),
            frontier,
            mission: None,
            disposition: ComputationInterpretationFrontierDisposition::Held,
            next_step: "hold until the computation campaign exposes a routable interpretation, replication, or recovery action".into(),
            digest: ContentHash::of_bytes(b"unsealed-glioma-computation-interpretation-frontier-run"),
        };
        output.digest = ContentHash::of_value(&run_digest_input(&output))
            .map_err(|error| ComputationInterpretationFrontierError::Digest(error.to_string()))?;
        output.validate()?;
        return Ok(output);
    }
    let mission = execute_glioma_autonomous_research_mission(
        &GliomaMissionRequest {
            mission_id: request.mission_id.clone(),
            objective: request.objective.clone(),
            candidates: frontier.candidates.clone(),
            completed_action_order: request.completed_action_order.clone(),
            selection: request.selection.clone(),
            gates: request.gates.clone(),
            max_rounds: request.max_rounds,
            max_retries: request.max_retries,
            require_artifacts: request.require_artifacts,
            stop_on_negative: request.stop_on_negative,
        },
        executor,
    )?;
    let disposition = if mission.disposition == GliomaMissionDisposition::Blocked {
        ComputationInterpretationFrontierDisposition::Blocked
    } else {
        ComputationInterpretationFrontierDisposition::Executed
    };
    let next_step = if matches!(
        disposition,
        ComputationInterpretationFrontierDisposition::Blocked
    ) {
        "resolve failed, unsafe, or budget-blocked computation interpretation work before continuing"
    } else {
        "review baseline, uncertainty, replay, and negative-result gates before promoting outputs to interpretation"
    };
    let mut output = ComputationInterpretationFrontierRun {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        mission_id: request.mission_id.clone(),
        objective: request.objective.clone(),
        frontier,
        mission: Some(mission),
        disposition,
        next_step: next_step.into(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-computation-interpretation-frontier-run"),
    };
    output.digest = ContentHash::of_value(&run_digest_input(&output))
        .map_err(|error| ComputationInterpretationFrontierError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computation_statuses_route_to_distinct_scientific_stages() {
        let completed = action_for_status(
            "task-complete",
            "completed",
            GliomaModality::Transcriptomics,
            3,
        );
        let negative = action_for_status(
            "task-negative",
            "negative",
            GliomaModality::Transcriptomics,
            3,
        );
        let failed = action_for_status("task-failed", "failed", GliomaModality::Transcriptomics, 3);
        assert_eq!(
            completed.stage_kind,
            GliomaStageKind::StatisticalInterpretation
        );
        assert_eq!(negative.stage_kind, GliomaStageKind::ReplicationRobustness);
        assert_eq!(failed.stage_kind, GliomaStageKind::ComputationalExecution);
        assert!(completed.effects.contains(&Effect::ReadLocalData));
        assert_eq!(completed.autonomy_tier, AutonomyTier::A1);
    }
}

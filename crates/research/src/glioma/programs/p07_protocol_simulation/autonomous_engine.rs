//! End-to-end autonomous research engine for preclinical glioma programs.
//!
//! The director already knows how to compile one bounded stage portfolio. This module closes the
//! product loop around it: it repeatedly compiles the current intent, executes one dependency-safe
//! local batch, promotes only returned typed artifacts to stage checkpoints, and replans the next
//! frontier. It is deliberately an engine rather than a receipt collector: the useful output is a
//! reproducible sequence of scientific workflow decisions, execution outcomes, and honest stops.
//! No dry-run result is treated as biological evidence and no clinical decision is produced.

use super::action_execution::{
    ActionExecutionDisposition, ActionPortfolioExecutionDisposition, GliomaActionExecutor,
};
use super::director::{
    execute_glioma_research_director, GliomaDirectorCheckpoint, GliomaDirectorFocus,
    GliomaResearchDirectorError, GliomaResearchDirectorRequest, GliomaResearchDirectorRun,
};
use crate::glioma_engine::{GliomaResearchIntent, GliomaSelectionWeights, GliomaStageKind};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P07-F11";
pub const OUTPUT_SCHEMA: &str = "GliomaAutonomousResearchEngine1@1";
pub const MAX_CYCLES: u16 = 32;

/// High-level request for a complete local research program. The intent remains the scientific
/// source of truth; all other fields bound how far this invocation may proceed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GliomaAutonomousResearchEngineRequest {
    pub mission_id: String,
    pub intent: GliomaResearchIntent,
    pub focus: GliomaDirectorFocus,
    pub completed_checkpoints: Vec<GliomaDirectorCheckpoint>,
    pub budget_units: u32,
    pub max_actions: u16,
    pub max_cycles: u16,
    pub approval_granted: bool,
    pub allow_instrument_execution: bool,
    pub allow_federation: bool,
    pub selection_weights: GliomaSelectionWeights,
    pub max_retries: u8,
    pub require_artifacts: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GliomaAutonomousResearchEngineDisposition {
    Completed,
    Partial,
    Blocked,
    NoRunnableActions,
    BudgetExhausted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GliomaAutonomousResearchEngineStopReason {
    Qualified,
    MaxCycles,
    BudgetExhausted,
    NoRunnableActions,
    Blocked,
    ExecutorFailed,
    NoProgress,
}

/// One closed-loop engine cycle. The embedded director run is the complete planning and
/// execution evidence for that cycle, so a caller can inspect why each stage was selected.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GliomaAutonomousResearchEngineCycle {
    pub cycle: u16,
    pub budget_before_units: u32,
    pub budget_after_units: u32,
    pub director: GliomaResearchDirectorRun,
    pub admitted_checkpoint_order: Vec<String>,
    pub negative_action_order: Vec<String>,
    pub uncertainty: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GliomaAutonomousResearchEngineRun {
    pub feature_id: String,
    pub output_schema: String,
    pub mission_id: String,
    pub objective: String,
    pub cycles: Vec<GliomaAutonomousResearchEngineCycle>,
    pub completed_checkpoints: Vec<GliomaDirectorCheckpoint>,
    pub completed_stage_order: Vec<String>,
    pub pending_stage_order: Vec<String>,
    pub hold_order: Vec<String>,
    pub approval_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub budget_spent_units: u32,
    pub remaining_budget_units: u32,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: GliomaAutonomousResearchEngineDisposition,
    pub stop_reason: GliomaAutonomousResearchEngineStopReason,
    pub next_step: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GliomaAutonomousResearchEngineError {
    #[error("glioma autonomous research engine request is invalid: {0}")]
    InvalidRequest(String),
    #[error("glioma autonomous research engine director failed: {0}")]
    Director(String),
    #[error("glioma autonomous research engine output is invalid: {0}")]
    InvalidOutput(String),
    #[error("glioma autonomous research engine digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn checkpoint_order(checkpoints: &[GliomaDirectorCheckpoint]) -> bool {
    checkpoints
        .windows(2)
        .all(|pair| pair[0].stage_kind < pair[1].stage_kind)
}

fn digest_input(run: &GliomaAutonomousResearchEngineRun) -> serde_json::Value {
    serde_json::json!({
        "feature_id": run.feature_id,
        "output_schema": run.output_schema,
        "mission_id": run.mission_id,
        "objective": run.objective,
        "cycles": run.cycles,
        "completed_checkpoints": run.completed_checkpoints,
        "completed_stage_order": run.completed_stage_order,
        "pending_stage_order": run.pending_stage_order,
        "hold_order": run.hold_order,
        "approval_order": run.approval_order,
        "blocked_order": run.blocked_order,
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
    request: &GliomaAutonomousResearchEngineRequest,
) -> Result<(), GliomaAutonomousResearchEngineError> {
    if request.mission_id.trim().is_empty()
        || request.budget_units == 0
        || request.max_actions == 0
        || request.max_cycles == 0
        || request.max_cycles > MAX_CYCLES
        || request.max_retries > super::action_execution::MAX_RETRIES
        || !checkpoint_order(&request.completed_checkpoints)
        || request
            .completed_checkpoints
            .iter()
            .any(|checkpoint| checkpoint.artifact_id.trim().is_empty())
    {
        return Err(GliomaAutonomousResearchEngineError::InvalidRequest(
            "mission identity, positive budget/actions/cycles, bounded retries, and canonical checkpoints are required".into(),
        ));
    }
    if request
        .completed_checkpoints
        .iter()
        .any(|checkpoint| checkpoint.artifact.validate().is_err())
    {
        return Err(GliomaAutonomousResearchEngineError::InvalidRequest(
            "every starting checkpoint must reference a valid local preclinical artifact".into(),
        ));
    }
    Ok(())
}

fn collect_checkpoint_candidates(
    director: &GliomaResearchDirectorRun,
) -> HashMap<String, (GliomaStageKind, ActionExecutionDisposition)> {
    let stage_by_action = director
        .actions
        .iter()
        .map(|action| {
            (
                action.candidate.action_id.clone(),
                action.candidate.stage_kind,
            )
        })
        .collect::<HashMap<_, _>>();
    director
        .execution
        .as_ref()
        .map(|execution| {
            execution
                .results
                .iter()
                .filter_map(|result| {
                    stage_by_action
                        .get(&result.action_id)
                        .copied()
                        .map(|stage| (result.action_id.clone(), (stage, result.disposition)))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn add_checkpoints(
    checkpoints: &mut Vec<GliomaDirectorCheckpoint>,
    director: &GliomaResearchDirectorRun,
) -> (Vec<String>, Vec<String>) {
    let stage_by_action = collect_checkpoint_candidates(director);
    let mut existing = checkpoints
        .iter()
        .map(|checkpoint| checkpoint.stage_kind)
        .collect::<BTreeSet<_>>();
    let mut admitted = BTreeSet::new();
    let mut negative = BTreeSet::new();
    if let Some(execution) = &director.execution {
        for result in &execution.results {
            let Some((stage_kind, disposition)) = stage_by_action.get(&result.action_id) else {
                continue;
            };
            if matches!(disposition, ActionExecutionDisposition::Negative) {
                negative.insert(result.action_id.clone());
            }
            let Some(artifact) = &result.artifact else {
                continue;
            };
            if matches!(
                disposition,
                ActionExecutionDisposition::Skipped | ActionExecutionDisposition::Failed
            ) {
                continue;
            }
            if existing.insert(*stage_kind) {
                checkpoints.push(GliomaDirectorCheckpoint {
                    stage_kind: *stage_kind,
                    artifact_id: artifact.artifact_id.clone(),
                    artifact: artifact.clone(),
                });
                admitted.insert(result.action_id.clone());
            }
        }
    }
    checkpoints.sort_by_key(|checkpoint| checkpoint.stage_kind);
    (
        admitted.into_iter().collect(),
        negative.into_iter().collect(),
    )
}

impl GliomaAutonomousResearchEngineRun {
    pub fn validate(&self) -> Result<(), GliomaAutonomousResearchEngineError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.mission_id.trim().is_empty()
            || self.objective.trim().is_empty()
            || !canonical(&self.completed_stage_order)
            || !canonical(&self.pending_stage_order)
            || !canonical(&self.hold_order)
            || !canonical(&self.approval_order)
            || !canonical(&self.blocked_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || !checkpoint_order(&self.completed_checkpoints)
            || self
                .remaining_budget_units
                .saturating_add(self.budget_spent_units)
                == 0
            || self
                .completed_checkpoints
                .iter()
                .any(|checkpoint| checkpoint.artifact.validate().is_err())
            || self.next_step.trim().is_empty()
        {
            return Err(GliomaAutonomousResearchEngineError::InvalidOutput(
                "identity, ordering, checkpoint, budget, artifact, or next-step invariants are invalid".into(),
            ));
        }
        for pair in &self.cycles {
            pair.director.validate().map_err(|error| {
                GliomaAutonomousResearchEngineError::InvalidOutput(error.to_string())
            })?;
        }
        if self
            .cycles
            .windows(2)
            .any(|pair| pair[0].cycle >= pair[1].cycle)
            || self
                .cycles
                .iter()
                .any(|cycle| cycle.budget_before_units < cycle.budget_after_units)
        {
            return Err(GliomaAutonomousResearchEngineError::InvalidOutput(
                "cycle order or budget monotonicity is invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| GliomaAutonomousResearchEngineError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(GliomaAutonomousResearchEngineError::InvalidOutput(
                "engine digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Compile and execute a complete bounded glioma program. Each cycle is recompiled from the
/// artifacts returned by the previous cycle, so stale plans cannot silently dispatch downstream
/// work. A production caller supplies its institution-local executor; MCP uses the dry-run seam.
pub fn execute_glioma_autonomous_research_engine<E: GliomaActionExecutor>(
    request: &GliomaAutonomousResearchEngineRequest,
    executor: &mut E,
) -> Result<GliomaAutonomousResearchEngineRun, GliomaAutonomousResearchEngineError> {
    validate_request(request)?;
    let mut checkpoints = request.completed_checkpoints.clone();
    let mut cycles = Vec::new();
    let mut spent = 0_u32;
    let mut negative_evidence = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut pending = BTreeSet::new();
    let mut hold = BTreeSet::new();
    let mut approval = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut disposition = GliomaAutonomousResearchEngineDisposition::Partial;
    let mut stop_reason = GliomaAutonomousResearchEngineStopReason::MaxCycles;

    for cycle_number in 1..=request.max_cycles {
        let budget_before = request.budget_units.saturating_sub(spent);
        if budget_before == 0 {
            disposition = GliomaAutonomousResearchEngineDisposition::BudgetExhausted;
            stop_reason = GliomaAutonomousResearchEngineStopReason::BudgetExhausted;
            break;
        }
        let director_request = GliomaResearchDirectorRequest {
            intent: request.intent.clone(),
            focus: request.focus,
            completed_checkpoints: checkpoints.clone(),
            budget_units: budget_before,
            max_actions: request.max_actions,
            approval_granted: request.approval_granted,
            allow_instrument_execution: request.allow_instrument_execution,
            allow_federation: request.allow_federation,
            selection_weights: request.selection_weights,
            max_retries: request.max_retries,
            require_artifacts: request.require_artifacts,
        };
        let director = execute_glioma_research_director(&director_request, executor).map_err(
            |error: GliomaResearchDirectorError| {
                GliomaAutonomousResearchEngineError::Director(error.to_string())
            },
        )?;
        let cost_by_action = director
            .actions
            .iter()
            .map(|action| {
                (
                    action.candidate.action_id.clone(),
                    action.candidate.cost_units,
                )
            })
            .collect::<HashMap<_, _>>();
        let round_cost = director
            .next_stage_order
            .iter()
            .filter_map(|action| cost_by_action.get(action))
            .copied()
            .sum::<u32>()
            .min(budget_before);
        spent = spent.saturating_add(round_cost);
        for item in &director.negative_evidence {
            negative_evidence.insert(item.clone());
        }
        for item in &director.uncertainty {
            uncertainty.insert(item.clone());
        }
        pending.extend(director.next_stage_order.iter().cloned());
        hold.extend(director.hold_order.iter().cloned());
        approval.extend(director.approval_order.iter().cloned());
        blocked.extend(director.blocked_order.iter().cloned());
        let (admitted, negative) = add_checkpoints(&mut checkpoints, &director);
        let cycle_uncertainty = director.uncertainty.clone();
        cycles.push(GliomaAutonomousResearchEngineCycle {
            cycle: cycle_number,
            budget_before_units: budget_before,
            budget_after_units: budget_before.saturating_sub(round_cost),
            director: director.clone(),
            admitted_checkpoint_order: admitted,
            negative_action_order: negative,
            uncertainty: cycle_uncertainty,
        });

        let qualified = director.workflow_plan.disposition
            == crate::glioma_engine::GliomaPlanDisposition::Admitted
            && director.next_stage_order.is_empty()
            && director.hold_order.is_empty()
            && director.approval_order.is_empty()
            && director.blocked_order.is_empty();
        if qualified {
            disposition = GliomaAutonomousResearchEngineDisposition::Completed;
            stop_reason = GliomaAutonomousResearchEngineStopReason::Qualified;
            break;
        }
        if matches!(
            director
                .execution
                .as_ref()
                .map(|execution| execution.disposition),
            Some(ActionPortfolioExecutionDisposition::Failed)
                | Some(ActionPortfolioExecutionDisposition::Blocked)
        ) {
            disposition = GliomaAutonomousResearchEngineDisposition::Blocked;
            stop_reason = GliomaAutonomousResearchEngineStopReason::ExecutorFailed;
            break;
        }
        if director.next_stage_order.is_empty() {
            disposition = if director.workflow_plan.disposition
                == crate::glioma_engine::GliomaPlanDisposition::Blocked
                || !director.blocked_order.is_empty()
            {
                GliomaAutonomousResearchEngineDisposition::Blocked
            } else {
                GliomaAutonomousResearchEngineDisposition::NoRunnableActions
            };
            stop_reason = if matches!(
                disposition,
                GliomaAutonomousResearchEngineDisposition::Blocked
            ) {
                GliomaAutonomousResearchEngineStopReason::Blocked
            } else {
                GliomaAutonomousResearchEngineStopReason::NoRunnableActions
            };
            break;
        }
        if round_cost == 0 {
            disposition = GliomaAutonomousResearchEngineDisposition::NoRunnableActions;
            stop_reason = GliomaAutonomousResearchEngineStopReason::NoProgress;
            break;
        }
        if spent >= request.budget_units {
            disposition = GliomaAutonomousResearchEngineDisposition::BudgetExhausted;
            stop_reason = GliomaAutonomousResearchEngineStopReason::BudgetExhausted;
            break;
        }
    }

    let mut completed_stage_order = checkpoints
        .iter()
        .map(|checkpoint| checkpoint.stage_kind.stage_id().to_string())
        .collect::<Vec<_>>();
    let completed_stage_set = completed_stage_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    pending.retain(|stage| !completed_stage_set.contains(stage));
    completed_stage_order.sort();
    let next_step = match stop_reason {
        GliomaAutonomousResearchEngineStopReason::Qualified => {
            "review the complete staged program and release only independently validated artifacts"
        }
        GliomaAutonomousResearchEngineStopReason::BudgetExhausted => {
            "increase the bounded research budget or resume from the returned checkpoints"
        }
        GliomaAutonomousResearchEngineStopReason::Blocked
        | GliomaAutonomousResearchEngineStopReason::ExecutorFailed => {
            "inspect the local failure or policy hold before authorizing another cycle"
        }
        GliomaAutonomousResearchEngineStopReason::NoRunnableActions
        | GliomaAutonomousResearchEngineStopReason::NoProgress => {
            "supply the missing typed artifacts or revise the bounded research intent"
        }
        GliomaAutonomousResearchEngineStopReason::MaxCycles => {
            "resume from the returned checkpoints with an explicit continuation budget"
        }
    }
    .to_string();
    let mut output = GliomaAutonomousResearchEngineRun {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        mission_id: request.mission_id.clone(),
        objective: request.intent.objective.clone(),
        cycles,
        completed_checkpoints: checkpoints,
        completed_stage_order,
        pending_stage_order: pending.into_iter().collect(),
        hold_order: hold.into_iter().collect(),
        approval_order: approval.into_iter().collect(),
        blocked_order: blocked.into_iter().collect(),
        budget_spent_units: spent,
        remaining_budget_units: request.budget_units.saturating_sub(spent),
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        stop_reason,
        next_step,
        digest: ContentHash::of_bytes(b"unsealed-glioma-autonomous-research-engine"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| GliomaAutonomousResearchEngineError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma_engine::{GliomaModality, GliomaModelSystem, LocalArtifactRef};
    use bioprism_foundation::{AutonomyTier, PRECLINICAL_BOUNDARY};
    use bioprism_onco::OutputUse;
    use std::collections::BTreeSet;

    fn request() -> GliomaAutonomousResearchEngineRequest {
        let hash = ContentHash::of_bytes(b"engine-input");
        GliomaAutonomousResearchEngineRequest {
            mission_id: "engine-test".into(),
            intent: GliomaResearchIntent {
                research_id: "engine-research".into(),
                study_id: "engine-study".into(),
                objective: "identify reproducible invasion mechanisms in organoids".into(),
                output_uses: BTreeSet::from([OutputUse::CohortAnalysis]),
                model_systems: BTreeSet::from([GliomaModelSystem::Organoid]),
                modalities: BTreeSet::from([
                    GliomaModality::Transcriptomics,
                    GliomaModality::Imaging,
                    GliomaModality::Spatial,
                ]),
                input_artifacts: vec![LocalArtifactRef {
                    artifact_id: "input".into(),
                    content_hash: hash.clone(),
                    content_type: "application/json".into(),
                    local_only: true,
                    contains_human_data: false,
                    contains_direct_identifiers: false,
                }],
                requested_autonomy: AutonomyTier::A1,
                approval_reference: None,
                budget_units: 160,
                max_retries: 1,
                allow_instrument_execution: false,
                allow_federation: false,
                raw_data_local: true,
                aggregate_only: true,
                replay_identity: hash,
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            focus: GliomaDirectorFocus::MechanismFirst,
            completed_checkpoints: Vec::new(),
            budget_units: 160,
            max_actions: 2,
            max_cycles: 8,
            approval_granted: false,
            allow_instrument_execution: false,
            allow_federation: false,
            selection_weights: GliomaSelectionWeights::default(),
            max_retries: 1,
            require_artifacts: true,
        }
    }

    #[test]
    fn engine_replans_from_local_artifacts_across_multiple_cycles() {
        let mut executor = super::super::action_execution::DryRunGliomaActionExecutor;
        let run = execute_glioma_autonomous_research_engine(&request(), &mut executor).unwrap();
        assert!(run.cycles.len() > 1);
        assert!(!run.completed_checkpoints.is_empty());
        assert!(run
            .cycles
            .windows(2)
            .all(|pair| pair[0].budget_after_units >= pair[1].budget_after_units));
        assert!(run
            .negative_evidence
            .iter()
            .any(|item| item.contains("synthetic-dry-run")));
        run.validate().unwrap();
    }

    #[test]
    fn invalid_checkpoint_cannot_enter_the_engine() {
        let mut request = request();
        request
            .completed_checkpoints
            .push(GliomaDirectorCheckpoint {
                stage_kind: GliomaStageKind::EvidenceSurveillance,
                artifact_id: "human".into(),
                artifact: LocalArtifactRef {
                    artifact_id: "human".into(),
                    content_hash: ContentHash::of_bytes(b"human"),
                    content_type: "application/json".into(),
                    local_only: false,
                    contains_human_data: true,
                    contains_direct_identifiers: false,
                },
            });
        let mut executor = super::super::action_execution::DryRunGliomaActionExecutor;
        assert!(execute_glioma_autonomous_research_engine(&request, &mut executor).is_err());
    }
}

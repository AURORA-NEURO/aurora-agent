//! Replanned, evidence-driven execution for preclinical glioma research missions.
//!
//! Unlike a static stage checklist, each round is admitted against the current P02 claim
//! frontier and P03 multimodal readiness. After local execution, an institution-owned state
//! builder recompiles those scientific inputs from the newly produced artifacts before another
//! batch can be dispatched. The engine only carries artifact handles; it never interprets raw
//! assay data itself or turns an execution status into biological evidence.

use super::action_execution::{
    ActionExecutionDisposition, GliomaActionArtifactInput, GliomaActionWorkflowScope, MAX_RETRIES,
};
use super::frontier_execution::{
    execute_glioma_scientific_frontier, ScientificFrontierExecution,
    ScientificFrontierExecutionDisposition, ScientificFrontierExecutionError,
    ScientificFrontierExecutionRequest,
};
use super::scientific_frontier::{
    plan_glioma_scientific_frontier, ScientificFrontierError, ScientificFrontierPlan,
    ScientificFrontierRequest,
};
use crate::glioma_engine::LocalArtifactRef;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = super::mission::FEATURE_ID;
pub const OUTPUT_SCHEMA: &str = "GliomaAdaptiveScientificMission1@1";
pub const MAX_CYCLES: u16 = 64;

/// Runtime and safety envelope for a multi-round glioma research mission.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdaptiveScientificMissionRequest {
    pub objective: String,
    pub initial_frontier: ScientificFrontierRequest,
    pub budget_units: u32,
    pub max_cycles: u16,
    pub max_retries: u8,
    pub require_artifacts: bool,
    pub source_artifacts: Vec<LocalArtifactRef>,
    pub completed_artifacts: Vec<GliomaActionArtifactInput>,
    pub workflow_scope: Option<GliomaActionWorkflowScope>,
}

/// Inputs supplied to a local state builder after each execution batch. The builder may read
/// local artifact bytes from its institution-owned store, then rerun evidence, knowledge, and QC
/// compilation. Raw measurements never cross this interface.
pub struct AdaptiveScientificMissionUpdateContext<'a> {
    pub objective: &'a str,
    pub completed_action_order: &'a [String],
    pub completed_artifacts: &'a [GliomaActionArtifactInput],
    pub previous_frontier: &'a ScientificFrontierRequest,
    pub previous_rounds: &'a [AdaptiveScientificMissionRound],
    /// True when the most recent worker run was a dry-run/simulation. A state builder must never
    /// convert its outputs into biological evidence or experimental observations.
    pub latest_execution_simulation_only: bool,
    pub remaining_budget_units: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScientificStateBuildFailure {
    pub reason: String,
}

/// Rebuild the current P02/P03 research state from the latest local study artifacts. A production
/// implementation should run the institution's evidence, knowledge-frontier, and multimodal-QC
/// compilers here; this seam deliberately does not guess what an assay artifact means.
pub trait GliomaScientificStateBuilder {
    fn recompile_frontier(
        &mut self,
        context: &AdaptiveScientificMissionUpdateContext<'_>,
    ) -> Result<ScientificFrontierRequest, ScientificStateBuildFailure>;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdaptiveScientificMissionRound {
    pub round: u16,
    pub budget_before_units: u32,
    pub budget_after_units: u32,
    pub plan: ScientificFrontierPlan,
    pub execution: ScientificFrontierExecution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdaptiveScientificMissionDisposition {
    FrontierExhausted,
    Partial,
    Blocked,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdaptiveScientificMissionStopReason {
    NoRunnableActions,
    FrontierGated,
    BudgetExhausted,
    MaxCycles,
    SimulationOnly,
    SafetyBlocked,
    ExecutorFailed,
    NoProgress,
    StateUpdateFailed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdaptiveScientificMissionRun {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub initial_budget_units: u32,
    pub budget_spent_units: u32,
    pub remaining_budget_units: u32,
    pub completed_action_order: Vec<String>,
    pub completed_artifacts: Vec<GliomaActionArtifactInput>,
    pub rounds: Vec<AdaptiveScientificMissionRound>,
    pub final_knowledge_digest: ContentHash,
    pub final_frontier_digest: ContentHash,
    pub final_readiness_digest: ContentHash,
    pub disposition: AdaptiveScientificMissionDisposition,
    pub stop_reason: AdaptiveScientificMissionStopReason,
    pub stop_detail: String,
    pub next_step: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AdaptiveScientificMissionError {
    #[error("adaptive scientific mission request is invalid: {0}")]
    InvalidRequest(String),
    #[error("scientific frontier planning failed: {0}")]
    Planning(String),
    #[error("scientific frontier execution failed: {0}")]
    Execution(String),
    #[error("adaptive scientific mission output is invalid: {0}")]
    InvalidOutput(String),
    #[error("adaptive scientific mission digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(run: &AdaptiveScientificMissionRun) -> serde_json::Value {
    serde_json::json!({
        "feature_id": run.feature_id,
        "output_schema": run.output_schema,
        "objective": run.objective,
        "initial_budget_units": run.initial_budget_units,
        "budget_spent_units": run.budget_spent_units,
        "remaining_budget_units": run.remaining_budget_units,
        "completed_action_order": run.completed_action_order,
        "completed_artifacts": run.completed_artifacts,
        "rounds": run.rounds,
        "final_knowledge_digest": run.final_knowledge_digest,
        "final_frontier_digest": run.final_frontier_digest,
        "final_readiness_digest": run.final_readiness_digest,
        "disposition": run.disposition,
        "stop_reason": run.stop_reason,
        "stop_detail": run.stop_detail,
        "next_step": run.next_step,
    })
}

impl AdaptiveScientificMissionRun {
    pub fn validate(&self) -> Result<(), AdaptiveScientificMissionError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.initial_budget_units == 0
            || self
                .budget_spent_units
                .saturating_add(self.remaining_budget_units)
                != self.initial_budget_units
            || !canonical(&self.completed_action_order)
            || self.stop_detail.trim().is_empty()
            || self.next_step.trim().is_empty()
            || self.rounds.len() > MAX_CYCLES as usize
            || self.final_knowledge_digest.as_str().len() != 64
            || self.final_frontier_digest.as_str().len() != 64
            || self.final_readiness_digest.as_str().len() != 64
        {
            return Err(AdaptiveScientificMissionError::InvalidOutput(
                "identity, budget reconciliation, action ordering, final science state, or cycle bound is invalid".into(),
            ));
        }
        for (index, round) in self.rounds.iter().enumerate() {
            if round.round != index as u16 + 1
                || round.budget_after_units > round.budget_before_units
                || round.plan.objective != self.objective
                || round.execution.objective != self.objective
                || round.execution.plan_digest != round.plan.digest
            {
                return Err(AdaptiveScientificMissionError::InvalidOutput(
                    "round order, objective, plan binding, or budget progression is invalid".into(),
                ));
            }
            round.plan.validate().map_err(|error| {
                AdaptiveScientificMissionError::InvalidOutput(error.to_string())
            })?;
            round.execution.validate().map_err(|error| {
                AdaptiveScientificMissionError::InvalidOutput(error.to_string())
            })?;
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| AdaptiveScientificMissionError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(AdaptiveScientificMissionError::InvalidOutput(
                "adaptive scientific mission digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &AdaptiveScientificMissionRequest,
) -> Result<(), AdaptiveScientificMissionError> {
    if request.objective.trim().is_empty()
        || request.initial_frontier.objective != request.objective
        || request.budget_units == 0
        || request.max_cycles == 0
        || request.max_cycles > MAX_CYCLES
        || request.max_retries > MAX_RETRIES
        || request.initial_frontier.selection.max_actions == 0
        || !request.require_artifacts
    {
        return Err(AdaptiveScientificMissionError::InvalidRequest(
            "objective must bind the initial scientific frontier; budget, cycles, action limit, and retries must be positive and bounded, and scientific outputs must require local artifacts".into(),
        ));
    }
    for artifact in request.source_artifacts.iter().chain(
        request
            .completed_artifacts
            .iter()
            .map(|item| &item.artifact),
    ) {
        artifact
            .validate()
            .map_err(|error| AdaptiveScientificMissionError::InvalidRequest(error.to_string()))?;
    }
    let artifact_actions = request
        .completed_artifacts
        .iter()
        .map(|item| item.action_id.as_str())
        .collect::<BTreeSet<_>>();
    if artifact_actions.len() != request.completed_artifacts.len()
        || request
            .completed_artifacts
            .iter()
            .any(|item| item.action_id.trim().is_empty())
    {
        return Err(AdaptiveScientificMissionError::InvalidRequest(
            "starting prerequisite artifact actions must be named and unique".into(),
        ));
    }
    Ok(())
}

fn merge_execution_feedback(
    execution: &ScientificFrontierExecution,
    completed: &mut BTreeSet<String>,
    artifacts: &mut Vec<GliomaActionArtifactInput>,
) {
    // Simulated workers are useful for exercising plans and adapters, but their artifacts are
    // not observations. Keep them in the round's execution record and never feed them into the
    // scientific state builder or mark their actions complete.
    if execution.simulation_only {
        return;
    }
    let Some(portfolio) = &execution.execution else {
        return;
    };
    merge_action_results(
        &portfolio.results,
        execution.simulation_only,
        completed,
        artifacts,
    );
}

fn merge_action_results(
    results: &[super::action_execution::ActionExecutionResult],
    simulation_only: bool,
    completed: &mut BTreeSet<String>,
    artifacts: &mut Vec<GliomaActionArtifactInput>,
) {
    if simulation_only {
        return;
    }
    for result in results {
        match result.disposition {
            ActionExecutionDisposition::Completed | ActionExecutionDisposition::Negative => {
                completed.insert(result.action_id.clone());
                if let Some(artifact) = &result.artifact {
                    artifacts.retain(|item| item.action_id != result.action_id);
                    artifacts.push(GliomaActionArtifactInput {
                        action_id: result.action_id.clone(),
                        artifact: artifact.clone(),
                    });
                }
            }
            ActionExecutionDisposition::Partial
            | ActionExecutionDisposition::Failed
            | ActionExecutionDisposition::Skipped => {}
        }
    }
    artifacts.sort_by(|left, right| left.action_id.cmp(&right.action_id));
}

fn estimated_round_cost(
    plan: &ScientificFrontierPlan,
    state: &ScientificFrontierRequest,
) -> Result<u32, AdaptiveScientificMissionError> {
    let selected = plan.next_action_order.iter().collect::<BTreeSet<_>>();
    let mut total = 0_u32;
    for candidate in &state.candidates {
        if selected.contains(&candidate.action_id) {
            total = total.checked_add(candidate.cost_units).ok_or_else(|| {
                AdaptiveScientificMissionError::InvalidOutput(
                    "selected action costs exceed the bounded mission budget representation".into(),
                )
            })?;
        }
    }
    if total == 0 && !plan.next_action_order.is_empty() {
        return Err(AdaptiveScientificMissionError::InvalidOutput(
            "selected frontier actions do not reconcile with declared action costs".into(),
        ));
    }
    Ok(total)
}

/// Run an autonomous, budget-bounded research loop. Each planning round is regenerated from the
/// current knowledge/readiness state; negative outcomes are terminal for that exact action but
/// remain available to the state builder so it can prioritize replication or a competing model.
pub fn execute_glioma_adaptive_scientific_mission<
    B: GliomaScientificStateBuilder,
    E: super::action_execution::GliomaActionExecutor,
>(
    request: &AdaptiveScientificMissionRequest,
    state_builder: &mut B,
    executor: &mut E,
) -> Result<AdaptiveScientificMissionRun, AdaptiveScientificMissionError> {
    validate_request(request)?;

    let mut state = request.initial_frontier.clone();
    let mut completed = state
        .completed_action_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut artifacts = request.completed_artifacts.clone();
    artifacts.sort_by(|left, right| left.action_id.cmp(&right.action_id));
    let mut remaining = request.budget_units;
    let mut rounds = Vec::new();
    let mut stop_reason = AdaptiveScientificMissionStopReason::MaxCycles;
    let mut stop_detail = "bounded scientific replanning limit reached".to_owned();
    let mut next_step = "review the latest evidence/readiness state and start a new bounded mission if further work is warranted".to_owned();

    for cycle in 1..=request.max_cycles {
        if remaining == 0 {
            stop_reason = AdaptiveScientificMissionStopReason::BudgetExhausted;
            stop_detail = "the declared research budget is exhausted; exhaustion is not a scientific stopping result".into();
            next_step = "review the completed research artifacts and explicitly authorize any follow-on budget".into();
            break;
        }

        state.objective = request.objective.clone();
        state.completed_action_order = completed.iter().cloned().collect();
        state.selection.budget_units = remaining;
        let plan =
            plan_glioma_scientific_frontier(&state).map_err(|error: ScientificFrontierError| {
                AdaptiveScientificMissionError::Planning(error.to_string())
            })?;
        if plan.next_action_order.is_empty() {
            let frontier_gated = !plan.held_order.is_empty() || !plan.blocked_order.is_empty();
            stop_reason = if frontier_gated {
                AdaptiveScientificMissionStopReason::FrontierGated
            } else {
                AdaptiveScientificMissionStopReason::NoRunnableActions
            };
            stop_detail = if !frontier_gated {
                "the current scientific frontier has no remaining candidate actions".into()
            } else {
                "all remaining actions are held or blocked by evidence, modality readiness, dependency, or safety gates".into()
            };
            next_step = "inspect the held/blocked frontier and acquire the missing evidence or QC-qualified study inputs before resuming".into();
            break;
        }
        let cost = estimated_round_cost(&plan, &state)?;
        if cost > remaining {
            return Err(AdaptiveScientificMissionError::InvalidOutput(
                "scientific planner selected a batch above the remaining mission budget".into(),
            ));
        }
        let before = remaining;
        let execution_request = ScientificFrontierExecutionRequest {
            objective: request.objective.clone(),
            plan_digest: plan.digest.clone(),
            candidates: state.candidates.clone(),
            completed_action_order: state.completed_action_order.clone(),
            source_artifacts: request.source_artifacts.clone(),
            completed_artifacts: artifacts.clone(),
            workflow_scope: request.workflow_scope.clone(),
            selection: state.selection.clone(),
            max_retries: request.max_retries,
            require_artifacts: request.require_artifacts,
        };
        let execution = execute_glioma_scientific_frontier(&execution_request, &plan, executor)
            .map_err(|error: ScientificFrontierExecutionError| {
                AdaptiveScientificMissionError::Execution(error.to_string())
            })?;
        remaining -= cost;
        merge_execution_feedback(&execution, &mut completed, &mut artifacts);
        rounds.push(AdaptiveScientificMissionRound {
            round: cycle,
            budget_before_units: before,
            budget_after_units: remaining,
            plan,
            execution: execution.clone(),
        });

        if execution.simulation_only {
            stop_reason = AdaptiveScientificMissionStopReason::SimulationOnly;
            stop_detail = "the selected workflow ran in simulation-only mode; its outputs are not biological observations and cannot advance the research frontier".into();
            next_step = "connect a qualified institution-local data or assay executor, then start a new bounded mission from the last evidence-backed frontier".into();
            break;
        }

        match execution.disposition {
            ScientificFrontierExecutionDisposition::Failed => {
                stop_reason = AdaptiveScientificMissionStopReason::ExecutorFailed;
                stop_detail = "a local workflow executor failed; the engine preserved all prior outputs and stopped".into();
                next_step = "inspect the local executor failure and artifact state; resume only after reconciling any partial side effects".into();
                break;
            }
            ScientificFrontierExecutionDisposition::Partial
            | ScientificFrontierExecutionDisposition::Blocked => {
                stop_reason = AdaptiveScientificMissionStopReason::SafetyBlocked;
                stop_detail = "a selected action was partial or blocked, so no further research action was dispatched".into();
                next_step = "resolve the preflight, dependency, artifact, or policy block and recompile the research frontier".into();
                break;
            }
            ScientificFrontierExecutionDisposition::NoRunnableActions => {
                stop_reason = AdaptiveScientificMissionStopReason::NoRunnableActions;
                stop_detail = "the admitted frontier produced no executable actions".into();
                next_step = "review the current evidence and modality-readiness gates".into();
                break;
            }
            ScientificFrontierExecutionDisposition::Completed => {}
        }

        if remaining == 0 {
            stop_reason = AdaptiveScientificMissionStopReason::BudgetExhausted;
            stop_detail = "the declared research budget was consumed by the completed batch; budget exhaustion is not evidence of convergence".into();
            next_step =
                "review the newly compiled research state before authorizing more work".into();
            break;
        }
        if cycle == request.max_cycles {
            stop_reason = AdaptiveScientificMissionStopReason::MaxCycles;
            break;
        }
        if execution.executed_order.is_empty() {
            stop_reason = AdaptiveScientificMissionStopReason::NoProgress;
            stop_detail = "the selected batch produced no terminal action result; automatic re-dispatch is suppressed".into();
            next_step = "reconcile local execution status before retrying to avoid duplicating scientific or instrument effects".into();
            break;
        }

        state.completed_action_order = completed.iter().cloned().collect();
        state.selection.budget_units = remaining;
        let completed_action_order = completed.iter().cloned().collect::<Vec<_>>();
        let context = AdaptiveScientificMissionUpdateContext {
            objective: &request.objective,
            completed_action_order: &completed_action_order,
            completed_artifacts: &artifacts,
            previous_frontier: &state,
            previous_rounds: &rounds,
            latest_execution_simulation_only: execution.simulation_only,
            remaining_budget_units: remaining,
        };
        match state_builder.recompile_frontier(&context) {
            Ok(next_state) if next_state.objective == request.objective => state = next_state,
            Ok(_) => {
                stop_reason = AdaptiveScientificMissionStopReason::StateUpdateFailed;
                stop_detail =
                    "the local state builder changed the mission objective; re-planning stopped"
                        .into();
                next_step = "correct the state-builder objective binding before resuming".into();
                break;
            }
            Err(error) => {
                stop_reason = AdaptiveScientificMissionStopReason::StateUpdateFailed;
                stop_detail = error.reason;
                next_step = "inspect local evidence compilation and multimodal QC; continue only after a valid current frontier is available".into();
                break;
            }
        }
    }

    let disposition = match stop_reason {
        AdaptiveScientificMissionStopReason::NoRunnableActions => {
            AdaptiveScientificMissionDisposition::FrontierExhausted
        }
        AdaptiveScientificMissionStopReason::FrontierGated
        | AdaptiveScientificMissionStopReason::SafetyBlocked => {
            AdaptiveScientificMissionDisposition::Blocked
        }
        AdaptiveScientificMissionStopReason::BudgetExhausted
        | AdaptiveScientificMissionStopReason::MaxCycles
        | AdaptiveScientificMissionStopReason::SimulationOnly
        | AdaptiveScientificMissionStopReason::NoProgress
        | AdaptiveScientificMissionStopReason::StateUpdateFailed => {
            AdaptiveScientificMissionDisposition::Partial
        }
        AdaptiveScientificMissionStopReason::ExecutorFailed => {
            AdaptiveScientificMissionDisposition::Failed
        }
    };
    let completed_action_order = completed.into_iter().collect::<Vec<_>>();
    let mut output = AdaptiveScientificMissionRun {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        initial_budget_units: request.budget_units,
        budget_spent_units: request.budget_units.saturating_sub(remaining),
        remaining_budget_units: remaining,
        completed_action_order,
        completed_artifacts: artifacts,
        rounds,
        final_knowledge_digest: state.knowledge.digest.clone(),
        final_frontier_digest: state.frontier.digest.clone(),
        final_readiness_digest: state.readiness.digest.clone(),
        disposition,
        stop_reason,
        stop_detail,
        next_step,
        digest: ContentHash::of_bytes(b"unsealed-glioma-adaptive-scientific-mission"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| AdaptiveScientificMissionError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_assay_results_are_terminal_actions_but_keep_local_artifacts() {
        let mut completed = BTreeSet::new();
        let artifact = LocalArtifactRef {
            artifact_id: "negative-result".into(),
            content_hash: ContentHash::of_bytes(b"negative-result"),
            content_type: "application/json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        };
        let mut artifacts = Vec::new();
        merge_action_results(
            &[super::super::action_execution::ActionExecutionResult {
                action_id: "assay-1".into(),
                disposition: ActionExecutionDisposition::Negative,
                attempt_count: 1,
                artifact: Some(artifact.clone()),
                note: "measured result did not support the predicted transition".into(),
                uncertainty: Vec::new(),
                negative_evidence: vec!["transition-not-observed".into()],
            }],
            false,
            &mut completed,
            &mut artifacts,
        );

        assert!(completed.contains("assay-1"));
        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].artifact, artifact);
    }

    #[test]
    fn simulation_artifacts_never_advance_the_scientific_frontier() {
        let simulation_artifact = LocalArtifactRef {
            artifact_id: "simulated-assay".into(),
            content_hash: ContentHash::of_bytes(b"simulated-assay"),
            content_type: "application/json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        };
        let simulation_results = vec![super::super::action_execution::ActionExecutionResult {
            action_id: "simulated-assay-action".into(),
            disposition: ActionExecutionDisposition::Completed,
            attempt_count: 1,
            artifact: Some(simulation_artifact),
            note: "dry run".into(),
            uncertainty: vec!["simulation-only-result".into()],
            negative_evidence: vec!["synthetic-dry-run-not-biological-evidence".into()],
        }];
        let mut completed = BTreeSet::new();
        let mut artifacts = Vec::new();

        merge_action_results(&simulation_results, true, &mut completed, &mut artifacts);

        assert!(completed.is_empty());
        assert!(artifacts.is_empty());
    }
}

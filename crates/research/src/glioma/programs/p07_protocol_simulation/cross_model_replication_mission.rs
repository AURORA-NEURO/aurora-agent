//! P10-to-P07 autonomous cross-model replication mission.
//!
//! The P10 replication frontier is deliberately scientific and bounded: it chooses which
//! model-system follow-ups would reduce uncertainty, but it does not decide whether the local
//! institution is allowed to run them.  This composition compiles that frontier into the existing
//! P07 adaptive scheduler so budget, dependency, autonomy, instrument, and federation gates are
//! applied in one deterministic plan. A caller may then submit the selected actions to an
//! institution-local executor; the built-in MCP path is explicitly synthetic dry-run only.

use super::action_execution::{
    execute_glioma_action_portfolio_with_context, ActionPortfolioExecution,
    ActionPortfolioExecutionError, ActionPortfolioExecutionRequest, DryRunGliomaActionExecutor,
    GliomaActionArtifactInput, GliomaActionExecutor, GliomaActionWorkflowScope,
};
use super::adaptive_scheduler::{
    plan_glioma_adaptive_workflow, GliomaAdaptiveWorkflowSchedulerError,
    GliomaAdaptiveWorkflowSchedulerPlan, GliomaAdaptiveWorkflowSchedulerRequest,
    SchedulerObservation,
};
use crate::glioma::programs::p10_interpretation_replication::{
    materialize_glioma_cross_model_replication_actions,
    plan_glioma_cross_model_replication_frontier, CrossModelReplicationFrontierDisposition,
    CrossModelReplicationFrontierError, CrossModelReplicationFrontierRequest,
};
use crate::glioma_engine::{GliomaSelectionConfig, GliomaSelectionWeights, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const COMPOSITION_ID: &str = "glioma-cross-model-replication-mission";
pub const OUTPUT_SCHEMA: &str = "GliomaCrossModelReplicationMission1@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossModelReplicationMissionRequest {
    pub mission_id: String,
    pub objective: String,
    pub frontier_request: CrossModelReplicationFrontierRequest,
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
pub enum CrossModelReplicationMissionDisposition {
    Ready,
    Partial,
    FrontierHold,
    RouteHold,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossModelReplicationMissionPlan {
    pub composition_id: String,
    pub output_schema: String,
    pub mission_id: String,
    pub objective: String,
    pub frontier_digest: ContentHash,
    pub selected_frontier_order: Vec<String>,
    pub deferred_frontier_order: Vec<String>,
    pub blocked_frontier_order: Vec<String>,
    pub selected_action_order: Vec<String>,
    pub deferred_action_order: Vec<String>,
    pub scheduler: Option<GliomaAdaptiveWorkflowSchedulerPlan>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub next_operator_action: String,
    pub disposition: CrossModelReplicationMissionDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossModelReplicationMissionExecutionRequest {
    pub mission: CrossModelReplicationMissionRequest,
    pub source_artifacts: Vec<LocalArtifactRef>,
    pub completed_artifacts: Vec<GliomaActionArtifactInput>,
    pub scope: Option<GliomaActionWorkflowScope>,
    pub max_retries: u8,
    pub require_artifacts: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossModelReplicationMissionExecution {
    pub mission: CrossModelReplicationMissionPlan,
    pub execution: Option<ActionPortfolioExecution>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CrossModelReplicationMissionError {
    #[error("cross-model replication mission request is invalid: {0}")]
    InvalidRequest(String),
    #[error("cross-model replication mission frontier failed: {0}")]
    Frontier(String),
    #[error("cross-model replication mission scheduler failed: {0}")]
    Scheduler(String),
    #[error("cross-model replication mission execution failed: {0}")]
    Execution(String),
    #[error("cross-model replication mission output is invalid: {0}")]
    InvalidOutput(String),
    #[error("cross-model replication mission digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn sorted_unique(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
}

fn digest_input(plan: &CrossModelReplicationMissionPlan) -> serde_json::Value {
    serde_json::json!({
        "composition_id": plan.composition_id,
        "output_schema": plan.output_schema,
        "mission_id": plan.mission_id,
        "objective": plan.objective,
        "frontier_digest": plan.frontier_digest,
        "selected_frontier_order": plan.selected_frontier_order,
        "deferred_frontier_order": plan.deferred_frontier_order,
        "blocked_frontier_order": plan.blocked_frontier_order,
        "selected_action_order": plan.selected_action_order,
        "deferred_action_order": plan.deferred_action_order,
        "scheduler": plan.scheduler,
        "negative_evidence": plan.negative_evidence,
        "uncertainty": plan.uncertainty,
        "next_operator_action": plan.next_operator_action,
        "disposition": plan.disposition,
    })
}

fn validate_request(
    request: &CrossModelReplicationMissionRequest,
) -> Result<(), CrossModelReplicationMissionError> {
    if request.mission_id.trim().is_empty()
        || request.objective.trim().is_empty()
        || request.budget_units == 0
        || request.max_actions == 0
        || request.beam_width == 0
        || request.risk_budget_milli > 1_000_000
    {
        return Err(CrossModelReplicationMissionError::InvalidRequest(
            "mission/objective and positive scheduler bounds with bounded risk are required".into(),
        ));
    }
    if !canonical(&request.completed_action_order) {
        return Err(CrossModelReplicationMissionError::InvalidRequest(
            "completed actions must be strictly canonical".into(),
        ));
    }
    if request
        .observations
        .iter()
        .any(|observation| observation.action_id.trim().is_empty())
    {
        return Err(CrossModelReplicationMissionError::InvalidRequest(
            "scheduler observations require non-empty action ids".into(),
        ));
    }
    Ok(())
}

impl CrossModelReplicationMissionPlan {
    pub fn validate(&self) -> Result<(), CrossModelReplicationMissionError> {
        if self.composition_id != COMPOSITION_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.mission_id.trim().is_empty()
            || self.objective.trim().is_empty()
            || !canonical(&self.selected_frontier_order)
            || !canonical(&self.deferred_frontier_order)
            || !canonical(&self.blocked_frontier_order)
            || !canonical(&self.selected_action_order)
            || !canonical(&self.deferred_action_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.selected_frontier_order.iter().any(|id| {
                self.deferred_frontier_order.binary_search(id).is_ok()
                    || self.blocked_frontier_order.binary_search(id).is_ok()
            })
            || self
                .deferred_frontier_order
                .iter()
                .any(|id| self.blocked_frontier_order.binary_search(id).is_ok())
            || self
                .selected_action_order
                .iter()
                .any(|id| self.deferred_action_order.binary_search(id).is_ok())
            || matches!(
                self.disposition,
                CrossModelReplicationMissionDisposition::Ready
                    | CrossModelReplicationMissionDisposition::Partial
            ) && self.scheduler.is_none()
            || matches!(
                self.disposition,
                CrossModelReplicationMissionDisposition::FrontierHold
            ) && self.scheduler.is_some()
        {
            return Err(CrossModelReplicationMissionError::InvalidOutput(
                "identity, partition ordering, disposition, or scheduler binding is invalid".into(),
            ));
        }
        if let Some(scheduler) = &self.scheduler {
            scheduler.validate().map_err(|error| {
                CrossModelReplicationMissionError::InvalidOutput(error.to_string())
            })?;
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| CrossModelReplicationMissionError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(CrossModelReplicationMissionError::InvalidOutput(
                "mission plan digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Compile the P10 scientific frontier into the P07 adaptive workflow scheduler. This function
/// never executes a study, exports data, or grants authority.
pub fn plan_glioma_cross_model_replication_mission(
    request: &CrossModelReplicationMissionRequest,
) -> Result<CrossModelReplicationMissionPlan, CrossModelReplicationMissionError> {
    validate_request(request)?;
    let frontier = plan_glioma_cross_model_replication_frontier(&request.frontier_request)
        .map_err(|error: CrossModelReplicationFrontierError| {
            CrossModelReplicationMissionError::Frontier(error.to_string())
        })?;

    let selected_frontier_order = frontier.selected_order.clone();
    let deferred_frontier_order = frontier.deferred_order.clone();
    let blocked_frontier_order = frontier.blocked_order.clone();
    let mut negative_evidence = frontier.negative_evidence.clone();
    let mut uncertainty = frontier.uncertainty.clone();
    let mut selected_action_order = Vec::new();
    let mut deferred_action_order = Vec::new();
    let mut scheduler = None;

    let frontier_hold = matches!(
        frontier.disposition,
        CrossModelReplicationFrontierDisposition::NegativeHold
            | CrossModelReplicationFrontierDisposition::NoRunnableActions
            | CrossModelReplicationFrontierDisposition::Blocked
    ) || selected_frontier_order.is_empty();

    let disposition = if frontier_hold {
        if matches!(
            frontier.disposition,
            CrossModelReplicationFrontierDisposition::Blocked
        ) {
            CrossModelReplicationMissionDisposition::Blocked
        } else {
            CrossModelReplicationMissionDisposition::FrontierHold
        }
    } else {
        let candidates = materialize_glioma_cross_model_replication_actions(
            &request.frontier_request,
            &frontier,
        )
        .map_err(|error| CrossModelReplicationMissionError::Frontier(error.to_string()))?;
        if candidates.is_empty() {
            uncertainty.push("frontier selected no materializable P07 actions".into());
            CrossModelReplicationMissionDisposition::RouteHold
        } else {
            let scheduler_request = GliomaAdaptiveWorkflowSchedulerRequest {
                mission_id: request.mission_id.clone(),
                objective: request.objective.clone(),
                candidates,
                completed_action_order: request.completed_action_order.clone(),
                observations: request.observations.clone(),
                budget_units: request.budget_units,
                max_actions: request.max_actions,
                beam_width: request.beam_width,
                risk_budget_milli: request.risk_budget_milli,
                approval_granted: request.approval_granted,
                allow_instrument_execution: request.allow_instrument_execution,
                allow_federation: request.allow_federation,
                selection_weights: request.selection_weights,
            };
            let plan = plan_glioma_adaptive_workflow(&scheduler_request).map_err(
                |error: GliomaAdaptiveWorkflowSchedulerError| {
                    CrossModelReplicationMissionError::Scheduler(error.to_string())
                },
            )?;
            selected_action_order = sorted_unique(plan.selected_order.clone());
            deferred_action_order = sorted_unique(
                plan.deferred_order
                    .iter()
                    .chain(plan.blocked_order.iter())
                    .cloned()
                    .collect(),
            );
            negative_evidence.extend(plan.negative_evidence.clone());
            uncertainty.extend(plan.uncertainty.clone());
            let route_blocked = plan.selected_order.is_empty() && !plan.blocked_order.is_empty();
            scheduler = Some(plan);
            if route_blocked {
                CrossModelReplicationMissionDisposition::RouteHold
            } else if selected_action_order.is_empty() {
                CrossModelReplicationMissionDisposition::Blocked
            } else if !deferred_action_order.is_empty() || !deferred_frontier_order.is_empty() {
                CrossModelReplicationMissionDisposition::Partial
            } else {
                CrossModelReplicationMissionDisposition::Ready
            }
        }
    };

    let mut output = CrossModelReplicationMissionPlan {
        composition_id: COMPOSITION_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        mission_id: request.mission_id.clone(),
        objective: request.objective.clone(),
        frontier_digest: frontier.digest.clone(),
        selected_frontier_order,
        deferred_frontier_order,
        blocked_frontier_order,
        selected_action_order,
        deferred_action_order,
        scheduler,
        negative_evidence: sorted_unique(negative_evidence),
        uncertainty: sorted_unique(uncertainty),
        next_operator_action: frontier.next_operator_action.clone(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-cross-model-replication-mission"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| CrossModelReplicationMissionError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

/// Execute the selected cross-model replication actions through an institution-local executor.
/// The caller supplies the executor and local artifact references; this function never opens an
/// instrument or transports raw data itself. The dry-run wrapper below is the MCP-safe default.
pub fn execute_glioma_cross_model_replication_mission_with_executor<
    E: GliomaActionExecutor + ?Sized,
>(
    request: &CrossModelReplicationMissionExecutionRequest,
    executor: &mut E,
) -> Result<CrossModelReplicationMissionExecution, CrossModelReplicationMissionError> {
    let mission = plan_glioma_cross_model_replication_mission(&request.mission)?;
    let Some(scheduler) = &mission.scheduler else {
        return Ok(CrossModelReplicationMissionExecution {
            mission,
            execution: None,
        });
    };
    if mission.selected_action_order.is_empty() {
        return Ok(CrossModelReplicationMissionExecution {
            mission,
            execution: None,
        });
    }
    let frontier = plan_glioma_cross_model_replication_frontier(&request.mission.frontier_request)
        .map_err(|error| CrossModelReplicationMissionError::Frontier(error.to_string()))?;
    let selected = mission
        .selected_action_order
        .iter()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    let candidates = materialize_glioma_cross_model_replication_actions(
        &request.mission.frontier_request,
        &frontier,
    )
    .map_err(|error| CrossModelReplicationMissionError::Frontier(error.to_string()))?
    .into_iter()
    .filter(|candidate| selected.contains(&candidate.action_id))
    .collect::<Vec<_>>();
    if candidates.is_empty() {
        return Err(CrossModelReplicationMissionError::Execution(
            "scheduler selected actions that could not be rematerialized".into(),
        ));
    }
    let execution_request = ActionPortfolioExecutionRequest {
        candidates,
        completed_actions: request
            .mission
            .completed_action_order
            .iter()
            .cloned()
            .collect(),
        selection: GliomaSelectionConfig {
            budget_units: scheduler.planned_cost_units,
            max_actions: request.mission.max_actions,
            approval_granted: request.mission.approval_granted,
            allow_instrument_execution: request.mission.allow_instrument_execution,
            allow_federation: request.mission.allow_federation,
            weights: request.mission.selection_weights,
        },
        max_retries: request.max_retries,
        require_artifacts: request.require_artifacts,
    };
    let execution = execute_glioma_action_portfolio_with_context(
        &execution_request,
        &request.source_artifacts,
        &request.completed_artifacts,
        request.scope.as_ref(),
        executor,
    )
    .map_err(|error: ActionPortfolioExecutionError| {
        CrossModelReplicationMissionError::Execution(error.to_string())
    })?;
    Ok(CrossModelReplicationMissionExecution {
        mission,
        execution: Some(execution),
    })
}

/// Run a synthetic local-only execution for workbench previews and protocol tests. It produces
/// content-addressed synthetic artifacts and explicitly does not represent biological evidence.
pub fn execute_glioma_cross_model_replication_mission_dry_run(
    request: &CrossModelReplicationMissionExecutionRequest,
) -> Result<CrossModelReplicationMissionExecution, CrossModelReplicationMissionError> {
    let mut executor = DryRunGliomaActionExecutor;
    execute_glioma_cross_model_replication_mission_with_executor(request, &mut executor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p10_interpretation_replication::cross_model_claim_envelope::{
        analyze_glioma_cross_model_claim_envelope, CrossModelClaimEnvelopeRequest,
        CrossModelStudyEstimate,
    };
    use crate::glioma::programs::p10_interpretation_replication::cross_model_replication_frontier::{
        CrossModelFollowUpKind, CrossModelReplicationCandidate,
    };
    use crate::glioma_engine::{GliomaModelSystem, GliomaStageKind, LocalArtifactRef};

    fn source() -> crate::glioma::programs::p10_interpretation_replication::CrossModelClaimEnvelope
    {
        let artifact = |id: &str| LocalArtifactRef {
            artifact_id: format!("artifact-{id}"),
            content_hash: ContentHash::of_bytes(id.as_bytes()),
            content_type: "application/json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        };
        let estimate = |id: &str, study: &str, model_system, low, high| CrossModelStudyEstimate {
            estimate_id: id.into(),
            study_id: study.into(),
            model_system,
            independent_group: format!("group-{study}"),
            interval_low_milli: low,
            interval_high_milli: high,
            quality_milli: 900,
            uncertainty_milli: 100,
            artifact: artifact(id),
        };
        analyze_glioma_cross_model_claim_envelope(&CrossModelClaimEnvelopeRequest {
            objective: "compile executable cross-model replication".into(),
            claim_id: "claim-mission".into(),
            min_model_systems: 2,
            min_studies_per_system: 1,
            practical_effect_milli: 50,
            hidden_bias_budget_milli: 100,
            max_between_system_range_milli: 500,
            estimates: vec![
                estimate(
                    "e1",
                    "study-organoid-a",
                    GliomaModelSystem::Organoid,
                    400,
                    700,
                ),
                estimate(
                    "e2",
                    "study-organoid-b",
                    GliomaModelSystem::Organoid,
                    450,
                    750,
                ),
                estimate(
                    "e3",
                    "study-insilico-a",
                    GliomaModelSystem::InSilico,
                    500,
                    800,
                ),
                estimate(
                    "e4",
                    "study-insilico-b",
                    GliomaModelSystem::InSilico,
                    480,
                    780,
                ),
            ],
        })
        .unwrap()
    }

    fn request() -> CrossModelReplicationMissionRequest {
        CrossModelReplicationMissionRequest {
            mission_id: "mission-cross-model".into(),
            objective: "run bounded cross-model replication planning".into(),
            frontier_request: CrossModelReplicationFrontierRequest {
                source: source(),
                candidates: vec![
                    CrossModelReplicationCandidate {
                        action_id: "replicate-organoid".into(),
                        kind: CrossModelFollowUpKind::IndependentReplication,
                        model_system: GliomaModelSystem::Organoid,
                        independent_group: "site-b".into(),
                        rationale: "independent organoid replication".into(),
                        expected_information_milli: 700,
                        expected_range_reduction_milli: 120,
                        reproducibility_milli: 900,
                        feasibility_milli: 900,
                        risk_milli: 100,
                        cost_units: 2,
                        depends_on: Vec::new(),
                    },
                    CrossModelReplicationCandidate {
                        action_id: "replicate-insilico".into(),
                        kind: CrossModelFollowUpKind::IndependentReplication,
                        model_system: GliomaModelSystem::InSilico,
                        independent_group: "compute-b".into(),
                        rationale: "independent computational replication".into(),
                        expected_information_milli: 650,
                        expected_range_reduction_milli: 100,
                        reproducibility_milli: 950,
                        feasibility_milli: 950,
                        risk_milli: 50,
                        cost_units: 1,
                        depends_on: Vec::new(),
                    },
                ],
                budget_units: 3,
                max_actions: 2,
                max_risk_milli: 1_000,
                min_information_milli: 100,
                target_between_system_range_milli: 100,
            },
            completed_action_order: Vec::new(),
            observations: Vec::new(),
            budget_units: 3,
            max_actions: 2,
            beam_width: 16,
            risk_budget_milli: 10_000,
            approval_granted: false,
            allow_instrument_execution: false,
            allow_federation: false,
            selection_weights: GliomaSelectionWeights::default(),
        }
    }

    #[test]
    fn compiles_frontier_into_dependency_and_budget_checked_scheduler() {
        let output = plan_glioma_cross_model_replication_mission(&request()).unwrap();
        assert!(matches!(
            output.disposition,
            CrossModelReplicationMissionDisposition::Ready
                | CrossModelReplicationMissionDisposition::Partial
        ));
        assert!(!output.selected_action_order.is_empty());
        assert!(output.selected_action_order.iter().all(|id| output
            .scheduler
            .as_ref()
            .unwrap()
            .selected_order
            .contains(id)));
        output.validate().unwrap();
    }

    #[test]
    fn preserves_frontier_hold_without_creating_scheduler() {
        let mut request = request();
        request.frontier_request.max_risk_milli = 1;
        let output = plan_glioma_cross_model_replication_mission(&request).unwrap();
        assert_eq!(
            output.disposition,
            CrossModelReplicationMissionDisposition::FrontierHold
        );
        assert!(output.scheduler.is_none());
        output.validate().unwrap();
    }

    #[test]
    fn materialized_actions_are_non_physical_and_local_by_default() {
        let output = plan_glioma_cross_model_replication_mission(&request()).unwrap();
        let scheduler = output.scheduler.unwrap();
        assert!(scheduler
            .decisions
            .iter()
            .all(|decision| { decision.stage_kind == GliomaStageKind::ReplicationRobustness }));
    }
}

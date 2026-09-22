//! Bounded autonomous control for one preclinical glioma protocol mission.
//!
//! This controller closes the loop between branch optimization, caller-owned protocol execution,
//! and contract-preserving compensation. It evaluates a branch, executes only the selected local
//! protocol, records negative or failed outcomes, and moves to an untried branch when the worker
//! cannot complete the selected plan. The loop is bounded by rounds, branch search, retries, and
//! budget; it never performs an instrument effect from this crate and never emits a clinical claim.

use super::branch_optimizer::{
    materialize_glioma_protocol_branch, optimize_glioma_protocol_branches, ProtocolBranchCandidate,
    ProtocolBranchEvaluation, ProtocolBranchOptimizationRequest, ProtocolBranchWeights,
};
use super::compensation::{
    plan_glioma_protocol_compensation, ProtocolCompensationCandidate, ProtocolCompensationPlan,
    ProtocolCompensationRequest,
};
use super::execution::{
    execute_glioma_protocol, GliomaProtocolExecutor, ProtocolExecution,
    ProtocolExecutionDisposition, ProtocolExecutionRequest,
};
use super::simulator::ProtocolSimulationRequest;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P07-F09";
pub const OUTPUT_SCHEMA: &str = "GliomaAutonomousProtocolController1@1";
pub const MAX_ROUNDS: u8 = 32;
pub const MAX_BRANCHES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutonomousProtocolControllerRequest {
    pub mission_id: String,
    pub objective: String,
    pub base_protocol: ProtocolSimulationRequest,
    pub branch_candidates: Vec<ProtocolBranchCandidate>,
    pub compensation_candidates: Vec<ProtocolCompensationCandidate>,
    pub budget_units: u64,
    pub max_rounds: u8,
    pub max_branches: usize,
    pub beam_width: usize,
    pub max_retries: u8,
    pub require_artifacts: bool,
    pub branch_weights: ProtocolBranchWeights,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutonomousProtocolRound {
    pub round: u16,
    pub budget_before_units: u64,
    pub budget_after_units: u64,
    pub branch_id: String,
    pub branch_plan_digest: ContentHash,
    pub selected_candidate_order: Vec<String>,
    pub execution: Option<ProtocolExecution>,
    pub execution_error: Option<String>,
    pub compensation: Option<ProtocolCompensationPlan>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutonomousProtocolControllerDisposition {
    Completed,
    Partial,
    Exhausted,
    NoRunnableBranches,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutonomousProtocolControllerStopReason {
    Qualified,
    MaxRounds,
    BudgetExhausted,
    NoUntestedBranches,
    ExecutionFailed,
    CompensationUnresolved,
    NoRunnableBranches,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutonomousProtocolControllerRun {
    pub feature_id: String,
    pub output_schema: String,
    pub mission_id: String,
    pub objective: String,
    pub rounds: Vec<AutonomousProtocolRound>,
    pub tried_branch_order: Vec<String>,
    pub completed_branch_order: Vec<String>,
    pub failed_branch_order: Vec<String>,
    pub budget_spent_units: u64,
    pub remaining_budget_units: u64,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: AutonomousProtocolControllerDisposition,
    pub stop_reason: AutonomousProtocolControllerStopReason,
    pub next_step: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AutonomousProtocolControllerError {
    #[error("autonomous protocol controller request is invalid: {0}")]
    InvalidRequest(String),
    #[error("autonomous protocol branch planning failed: {0}")]
    BranchPlanning(String),
    #[error("autonomous protocol compensation planning failed: {0}")]
    Compensation(String),
    #[error("autonomous protocol output is invalid: {0}")]
    InvalidOutput(String),
    #[error("autonomous protocol digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(run: &AutonomousProtocolControllerRun) -> serde_json::Value {
    serde_json::json!({
        "feature_id": run.feature_id,
        "output_schema": run.output_schema,
        "mission_id": run.mission_id,
        "objective": run.objective,
        "rounds": run.rounds,
        "tried_branch_order": run.tried_branch_order,
        "completed_branch_order": run.completed_branch_order,
        "failed_branch_order": run.failed_branch_order,
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
    request: &AutonomousProtocolControllerRequest,
) -> Result<(), AutonomousProtocolControllerError> {
    if request.mission_id.trim().is_empty()
        || request.objective.trim().is_empty()
        || request.base_protocol.objective != request.objective
        || request.branch_candidates.is_empty()
        || request.compensation_candidates.is_empty()
        || request.budget_units == 0
        || request.max_rounds == 0
        || request.max_rounds > MAX_ROUNDS
        || request.max_branches == 0
        || request.max_branches > MAX_BRANCHES
    {
        return Err(AutonomousProtocolControllerError::InvalidRequest(
            "mission/objective binding, non-empty branch and compensation catalogs, bounded rounds/branches, and positive budget are required".into(),
        ));
    }
    if request.max_retries > super::execution::MAX_RETRIES {
        return Err(AutonomousProtocolControllerError::InvalidRequest(
            "retry bound exceeds the protocol execution policy".into(),
        ));
    }
    Ok(())
}

fn validate_run(
    run: &AutonomousProtocolControllerRun,
) -> Result<(), AutonomousProtocolControllerError> {
    if run.feature_id != FEATURE_ID
        || run.output_schema != OUTPUT_SCHEMA
        || run.mission_id.trim().is_empty()
        || run.objective.trim().is_empty()
        || !canonical(&run.tried_branch_order)
        || !canonical(&run.completed_branch_order)
        || !canonical(&run.failed_branch_order)
        || !canonical(&run.negative_evidence)
        || !canonical(&run.uncertainty)
        || run
            .rounds
            .windows(2)
            .any(|pair| pair[0].round >= pair[1].round)
        || run.rounds.iter().any(|round| {
            round.branch_id.trim().is_empty()
                || !canonical(&round.selected_candidate_order)
                || round.execution.is_some() == round.execution_error.is_some()
                || round
                    .execution_error
                    .as_ref()
                    .is_some_and(|error| error.trim().is_empty())
                || !canonical(&round.negative_evidence)
                || !canonical(&round.uncertainty)
        })
    {
        return Err(AutonomousProtocolControllerError::InvalidOutput(
            "identity, ordering, round, execution, or uncertainty invariants are invalid".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(run))
        .map_err(|error| AutonomousProtocolControllerError::Digest(error.to_string()))?;
    if expected != run.digest {
        return Err(AutonomousProtocolControllerError::InvalidOutput(
            "digest is not bound to the autonomous protocol run".into(),
        ));
    }
    Ok(())
}

impl AutonomousProtocolControllerRun {
    pub fn validate(&self) -> Result<(), AutonomousProtocolControllerError> {
        validate_run(self)
    }
}

fn ranked_untried<'a>(
    evaluations: &'a [ProtocolBranchEvaluation],
    tried: &BTreeSet<String>,
    remaining_budget: u64,
) -> Option<&'a ProtocolBranchEvaluation> {
    evaluations
        .iter()
        .filter(|evaluation| {
            !tried.contains(&evaluation.branch_id)
                && evaluation.projected_cost_units <= remaining_budget
        })
        .max_by(|left, right| {
            left.score_milli
                .cmp(&right.score_milli)
                .then_with(|| right.branch_id.cmp(&left.branch_id))
        })
}

/// Execute a bounded autonomous protocol mission through a caller-owned local executor.
pub fn execute_glioma_autonomous_protocol<E: GliomaProtocolExecutor>(
    request: &AutonomousProtocolControllerRequest,
    executor: &mut E,
) -> Result<AutonomousProtocolControllerRun, AutonomousProtocolControllerError> {
    validate_request(request)?;
    let mut remaining_budget = request.budget_units;
    let mut tried = BTreeSet::new();
    let mut completed = BTreeSet::new();
    let mut failed = BTreeSet::new();
    let mut rounds = Vec::new();
    let mut negative_evidence = Vec::new();
    let mut uncertainty = Vec::new();
    let mut stop_reason = AutonomousProtocolControllerStopReason::MaxRounds;
    for round_index in 0..request.max_rounds {
        if remaining_budget == 0 {
            stop_reason = AutonomousProtocolControllerStopReason::BudgetExhausted;
            break;
        }
        let branch_plan = optimize_glioma_protocol_branches(&ProtocolBranchOptimizationRequest {
            objective: request.objective.clone(),
            base_protocol: request.base_protocol.clone(),
            candidates: request.branch_candidates.clone(),
            budget_units: remaining_budget,
            max_branches: request.max_branches,
            beam_width: request.beam_width,
            weights: request.branch_weights,
        })
        .map_err(|error| AutonomousProtocolControllerError::BranchPlanning(error.to_string()))?;
        let Some(selected) = ranked_untried(&branch_plan.evaluations, &tried, remaining_budget)
        else {
            stop_reason = if tried.is_empty() {
                AutonomousProtocolControllerStopReason::NoRunnableBranches
            } else {
                AutonomousProtocolControllerStopReason::NoUntestedBranches
            };
            uncertainty.extend(branch_plan.uncertainty.clone());
            break;
        };
        let branch_id = selected.branch_id.clone();
        let selected_candidate_order = selected.selected_candidate_order.clone();
        let branch_protocol = materialize_glioma_protocol_branch(
            &request.base_protocol,
            &request.branch_candidates,
            &selected_candidate_order,
        )
        .map_err(|error| AutonomousProtocolControllerError::BranchPlanning(error.to_string()))?;
        let budget_before = remaining_budget;
        remaining_budget = remaining_budget.saturating_sub(selected.projected_cost_units);
        tried.insert(branch_id.clone());
        let execution_result = execute_glioma_protocol(
            &ProtocolExecutionRequest {
                protocol: branch_protocol.clone(),
                max_retries: request.max_retries,
                require_artifacts: request.require_artifacts,
            },
            executor,
        );
        let (execution, execution_error) = match execution_result {
            Ok(execution) => (Some(execution), None),
            Err(error) => (None, Some(error.to_string())),
        };
        let mut compensation = None;
        let mut round_negative = Vec::new();
        let mut round_uncertainty = branch_plan.uncertainty.clone();
        if let Some(execution) = &execution {
            round_negative.extend(execution.negative_evidence.clone());
            round_uncertainty.extend(execution.uncertainty.clone());
            if execution.disposition == ProtocolExecutionDisposition::Completed {
                completed.insert(branch_id.clone());
                stop_reason = AutonomousProtocolControllerStopReason::Qualified;
            } else {
                failed.insert(branch_id.clone());
                round_negative.push(format!(
                    "{branch_id}:execution-disposition:{:?}",
                    execution.disposition
                ));
                if remaining_budget > 0 {
                    compensation = Some(
                        plan_glioma_protocol_compensation(&ProtocolCompensationRequest {
                            objective: request.objective.clone(),
                            protocol: branch_protocol,
                            execution: execution.clone(),
                            candidates: request.compensation_candidates.clone(),
                            budget_units: remaining_budget,
                            max_selected: 1,
                        })
                        .map_err(|error| {
                            AutonomousProtocolControllerError::Compensation(error.to_string())
                        })?,
                    );
                }
            }
        } else if let Some(error) = &execution_error {
            failed.insert(branch_id.clone());
            round_negative.push(format!("{branch_id}:execution-refused"));
            round_uncertainty.push(error.clone());
        }
        round_negative.sort();
        round_negative.dedup();
        round_uncertainty.sort();
        round_uncertainty.dedup();
        negative_evidence.extend(round_negative.clone());
        uncertainty.extend(round_uncertainty.clone());
        rounds.push(AutonomousProtocolRound {
            round: u16::from(round_index) + 1,
            budget_before_units: budget_before,
            budget_after_units: remaining_budget,
            branch_id,
            branch_plan_digest: branch_plan.digest,
            selected_candidate_order,
            execution,
            execution_error,
            compensation,
            negative_evidence: round_negative,
            uncertainty: round_uncertainty,
        });
        if stop_reason == AutonomousProtocolControllerStopReason::Qualified {
            break;
        }
    }
    let disposition = if !completed.is_empty() {
        AutonomousProtocolControllerDisposition::Completed
    } else if rounds.is_empty()
        && stop_reason == AutonomousProtocolControllerStopReason::NoRunnableBranches
    {
        AutonomousProtocolControllerDisposition::NoRunnableBranches
    } else if remaining_budget == 0 {
        AutonomousProtocolControllerDisposition::Exhausted
    } else if matches!(
        stop_reason,
        AutonomousProtocolControllerStopReason::NoRunnableBranches
    ) {
        AutonomousProtocolControllerDisposition::Blocked
    } else {
        AutonomousProtocolControllerDisposition::Partial
    };
    if rounds.len() >= usize::from(request.max_rounds)
        && completed.is_empty()
        && stop_reason == AutonomousProtocolControllerStopReason::MaxRounds
    {
        uncertainty.push(
            "maximum autonomous protocol rounds reached without a qualified execution".into(),
        );
    }
    negative_evidence.sort();
    negative_evidence.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    let mut run = AutonomousProtocolControllerRun {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        mission_id: request.mission_id.clone(),
        objective: request.objective.clone(),
        rounds,
        tried_branch_order: tried.into_iter().collect(),
        completed_branch_order: completed.into_iter().collect(),
        failed_branch_order: failed.into_iter().collect(),
        budget_spent_units: request.budget_units.saturating_sub(remaining_budget),
        remaining_budget_units: remaining_budget,
        negative_evidence,
        uncertainty,
        disposition,
        stop_reason,
        next_step: if disposition == AutonomousProtocolControllerDisposition::Completed {
            "promote only the selected local artifacts into the next typed research stage".into()
        } else {
            "inspect explicit execution/compensation limits and submit an institution-local follow-up plan".into()
        },
        digest: ContentHash::of_bytes(b"unsealed-glioma-autonomous-protocol"),
    };
    run.digest = ContentHash::of_value(&digest_input(&run))
        .map_err(|error| AutonomousProtocolControllerError::Digest(error.to_string()))?;
    validate_run(&run)?;
    Ok(run)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p07_protocol_simulation::branch_optimizer::ProtocolBranchCandidate;
    use crate::glioma::programs::p07_protocol_simulation::compensation::ProtocolCompensationCandidate;
    use crate::glioma::programs::p07_protocol_simulation::execution::{
        DryRunGliomaProtocolExecutor, GliomaProtocolExecutor, ProtocolExecutionFailure,
        ProtocolTaskResult,
    };
    use crate::glioma::programs::p07_protocol_simulation::simulator::{
        ProtocolResource, ProtocolResourceKind, ProtocolTask,
    };
    use crate::glioma_engine::GliomaModelSystem;

    fn protocol() -> ProtocolSimulationRequest {
        ProtocolSimulationRequest {
            objective: "autonomously run an organoid invasion protocol".into(),
            model_system: GliomaModelSystem::Organoid,
            tasks: vec![
                ProtocolTask {
                    task_id: "prepare".into(),
                    label: "prepare organoids".into(),
                    resource_kind: ProtocolResourceKind::Culture,
                    resource_units: 1,
                    duration_ticks: 1,
                    depends_on: Vec::new(),
                    model_system: GliomaModelSystem::Organoid,
                    output_schema: "Setup1@1".into(),
                    risk_milli: 100,
                    requires_instrument: false,
                },
                ProtocolTask {
                    task_id: "assay".into(),
                    label: "run invasion assay".into(),
                    resource_kind: ProtocolResourceKind::Imaging,
                    resource_units: 1,
                    duration_ticks: 2,
                    depends_on: vec!["prepare".into()],
                    model_system: GliomaModelSystem::Organoid,
                    output_schema: "Assay1@1".into(),
                    risk_milli: 200,
                    requires_instrument: false,
                },
            ],
            resources: vec![
                ProtocolResource {
                    resource_id: "culture".into(),
                    kind: ProtocolResourceKind::Culture,
                    capacity_units: 1,
                },
                ProtocolResource {
                    resource_id: "imaging".into(),
                    kind: ProtocolResourceKind::Imaging,
                    capacity_units: 1,
                },
            ],
            max_ticks: 10,
            max_risk_milli: 500,
            allow_instrument_execution: false,
            approval_reference: None,
            randomization_seed: bioprism_ids::ContentHash::of_bytes(b"autonomous-protocol"),
        }
    }

    fn request() -> AutonomousProtocolControllerRequest {
        AutonomousProtocolControllerRequest {
            mission_id: "mission-autonomous-protocol".into(),
            objective: "autonomously run an organoid invasion protocol".into(),
            base_protocol: protocol(),
            branch_candidates: vec![ProtocolBranchCandidate {
                candidate_id: "fast-assay".into(),
                task: ProtocolTask {
                    task_id: "assay".into(),
                    label: "fast invasion assay".into(),
                    resource_kind: ProtocolResourceKind::Imaging,
                    resource_units: 1,
                    duration_ticks: 1,
                    depends_on: vec!["prepare".into()],
                    model_system: GliomaModelSystem::Organoid,
                    output_schema: "Assay1@1".into(),
                    risk_milli: 200,
                    requires_instrument: false,
                },
                expected_information_milli: 850,
                evidence_prior_milli: 800,
                cost_units: 2,
            }],
            compensation_candidates: vec![ProtocolCompensationCandidate {
                candidate_id: "assay-compensation".into(),
                replaces_task_id: "assay".into(),
                output_schema: "Assay1@1".into(),
                model_system: GliomaModelSystem::Organoid,
                resource_kind: ProtocolResourceKind::Imaging,
                resource_units: 1,
                duration_ticks: 1,
                cost_units: 1,
                risk_milli: 200,
                expected_information_milli: 800,
                depends_on: vec!["prepare".into()],
                unlocks_task_order: vec!["assay".into()],
            }],
            budget_units: 5,
            max_rounds: 3,
            max_branches: 8,
            beam_width: 8,
            max_retries: 0,
            require_artifacts: true,
            branch_weights: ProtocolBranchWeights {
                information_milli: 400,
                feasibility_milli: 300,
                time_milli: 100,
                risk_milli: 100,
                cost_milli: 100,
            },
        }
    }

    #[test]
    fn controller_executes_the_selected_branch_and_promotes_completion() {
        let mut executor = DryRunGliomaProtocolExecutor;
        let run = execute_glioma_autonomous_protocol(&request(), &mut executor).unwrap();
        assert_eq!(
            run.disposition,
            AutonomousProtocolControllerDisposition::Completed
        );
        assert_eq!(run.rounds.len(), 1);
        assert!(run.rounds[0].execution.is_some());
        assert!(run.rounds[0].execution_error.is_none());
        run.validate().unwrap();
    }

    #[test]
    fn controller_digest_is_stable_under_catalogue_permutation() {
        let mut first = request();
        first.branch_candidates.reverse();
        first.compensation_candidates.reverse();
        let second = request();
        let mut executor = DryRunGliomaProtocolExecutor;
        let left = execute_glioma_autonomous_protocol(&first, &mut executor).unwrap();
        let mut executor = DryRunGliomaProtocolExecutor;
        let right = execute_glioma_autonomous_protocol(&second, &mut executor).unwrap();
        assert_eq!(left.digest, right.digest);
    }

    struct FailAssayExecutor;

    impl GliomaProtocolExecutor for FailAssayExecutor {
        fn execute_task(
            &mut self,
            task: &ProtocolTask,
            schedule: &super::super::simulator::ScheduleEntry,
            attempt: u8,
        ) -> Result<ProtocolTaskResult, ProtocolExecutionFailure> {
            if task.task_id == "assay" {
                return Err(ProtocolExecutionFailure {
                    reason: "synthetic imaging gateway failure".into(),
                    retryable: false,
                });
            }
            let mut dry_run = DryRunGliomaProtocolExecutor;
            dry_run.execute_task(task, schedule, attempt)
        }
    }

    #[test]
    fn controller_preserves_failure_and_compensation_before_exhaustion() {
        let mut executor = FailAssayExecutor;
        let run = execute_glioma_autonomous_protocol(&request(), &mut executor).unwrap();
        assert_eq!(
            run.disposition,
            AutonomousProtocolControllerDisposition::Partial
        );
        assert!(!run.failed_branch_order.is_empty());
        assert!(run.rounds.iter().any(|round| round.compensation.is_some()));
        assert!(run
            .negative_evidence
            .iter()
            .any(|evidence| evidence.contains("execution-refused")
                || evidence.contains("execution-disposition")));
        run.validate().unwrap();
    }
}

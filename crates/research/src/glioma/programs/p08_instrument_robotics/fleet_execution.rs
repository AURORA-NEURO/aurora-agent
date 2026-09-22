//! Schedule-aware execution of a preclinical glioma instrument fleet.
//!
//! Fleet scheduling is useful only when its assignments survive the transition into guarded
//! execution. This controller binds every scheduled task to one admitted instrument plan,
//! executes dependency-safe tasks through the existing institution-owned gateway, and keeps
//! schedule-blocked, dependency-blocked, negative, partial, unresolved, and failed physical
//! outcomes separate. It never opens a device connection itself and never treats hardware
//! completion as biological evidence.

use super::execution::{
    execute_glioma_instrument_plan, InstrumentExecutionDisposition, InstrumentExecutionRequest,
    InstrumentExecutionRun, InstrumentExecutor,
};
use super::fleet_scheduler::InstrumentFleetSchedule;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P08-F21";
pub const OUTPUT_SCHEMA: &str = "GliomaInstrumentFleetExecution1@1";
pub const MAX_TASKS: usize = 1_024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentFleetExecutionRunRequest {
    pub task_id: String,
    pub depends_on: Vec<String>,
    pub execution: InstrumentExecutionRequest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentFleetExecutionRequest {
    pub objective: String,
    pub schedule: InstrumentFleetSchedule,
    pub runs: Vec<InstrumentFleetExecutionRunRequest>,
    pub stop_on_negative: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentFleetExecutionResult {
    pub task_id: String,
    pub instrument_id: String,
    pub scheduled_start_tick: u64,
    pub scheduled_end_tick: u64,
    pub execution: InstrumentExecutionRun,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentFleetExecutionFailure {
    pub task_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstrumentFleetExecutionDisposition {
    Completed,
    Negative,
    Partial,
    Failed,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstrumentFleetExecutionStopReason {
    Completed,
    NegativeResult,
    PartialEffect,
    UnresolvedResult,
    ExecutionFailure,
    DependencyBlocked,
    ScheduleBlocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentFleetExecution {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub schedule_digest: ContentHash,
    pub task_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub execution_order: Vec<String>,
    pub results: Vec<InstrumentFleetExecutionResult>,
    pub failures: Vec<InstrumentFleetExecutionFailure>,
    pub completed_order: Vec<String>,
    pub negative_order: Vec<String>,
    pub partial_order: Vec<String>,
    pub failed_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub retry_count: u32,
    pub emergency_stop_requested: bool,
    pub emergency_stop_succeeded: bool,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub simulation_only: bool,
    pub disposition: InstrumentFleetExecutionDisposition,
    pub stop_reason: InstrumentFleetExecutionStopReason,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum InstrumentFleetExecutionError {
    #[error("instrument fleet execution request is invalid: {0}")]
    InvalidRequest(String),
    #[error("instrument fleet execution schedule is invalid: {0}")]
    InvalidSchedule(String),
    #[error("instrument fleet execution output is invalid: {0}")]
    InvalidOutput(String),
    #[error("instrument fleet execution digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique_any_order(values: &[String]) -> bool {
    let mut seen = BTreeSet::new();
    values
        .iter()
        .all(|value| !value.trim().is_empty() && seen.insert(value))
}

fn digest_input(output: &InstrumentFleetExecution) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "schedule_digest": output.schedule_digest,
        "task_order": output.task_order,
        "admitted_order": output.admitted_order,
        "execution_order": output.execution_order,
        "results": output.results,
        "failures": output.failures,
        "completed_order": output.completed_order,
        "negative_order": output.negative_order,
        "partial_order": output.partial_order,
        "failed_order": output.failed_order,
        "blocked_order": output.blocked_order,
        "unresolved_order": output.unresolved_order,
        "retry_count": output.retry_count,
        "emergency_stop_requested": output.emergency_stop_requested,
        "emergency_stop_succeeded": output.emergency_stop_succeeded,
        "uncertainty": output.uncertainty,
        "negative_evidence": output.negative_evidence,
        "simulation_only": output.simulation_only,
        "disposition": output.disposition,
        "stop_reason": output.stop_reason,
    })
}

fn assignment_map(schedule: &InstrumentFleetSchedule) -> BTreeMap<String, (String, u64, u64)> {
    schedule
        .assignments
        .iter()
        .map(|assignment| {
            (
                assignment.task_id.clone(),
                (
                    assignment.instrument_id.clone(),
                    assignment.scheduled_start_tick,
                    assignment.scheduled_end_tick,
                ),
            )
        })
        .collect()
}

fn normalize_request(input: &InstrumentFleetExecutionRequest) -> InstrumentFleetExecutionRequest {
    let mut request = input.clone();
    request
        .runs
        .sort_by(|left, right| left.task_id.cmp(&right.task_id));
    for run in &mut request.runs {
        run.depends_on.sort();
    }
    request
}

fn validate_request(
    request: &InstrumentFleetExecutionRequest,
) -> Result<(), InstrumentFleetExecutionError> {
    if request.objective.trim().is_empty()
        || request.schedule.objective.trim().is_empty()
        || request.objective.trim() != request.schedule.objective.trim()
        || request.schedule.task_order.len() > MAX_TASKS
        || request.runs.len() > MAX_TASKS
    {
        return Err(InstrumentFleetExecutionError::InvalidRequest(
            "objective, schedule identity, and bounded task/run counts are required".into(),
        ));
    }
    request
        .schedule
        .validate()
        .map_err(|error| InstrumentFleetExecutionError::InvalidSchedule(error.to_string()))?;
    let task_ids = request
        .schedule
        .task_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let admitted = request
        .schedule
        .admitted_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let assignments = assignment_map(&request.schedule);
    let mut run_ids = BTreeSet::new();
    for run in &request.runs {
        if run.task_id.trim().is_empty()
            || !run_ids.insert(run.task_id.clone())
            || !admitted.contains(&run.task_id)
            || !canonical(&run.depends_on)
            || run.depends_on.iter().any(|dependency| {
                dependency == &run.task_id
                    || !task_ids.contains(dependency)
                    || !admitted.contains(dependency)
            })
        {
            return Err(InstrumentFleetExecutionError::InvalidRequest(
                "run ids, admitted-task membership, and canonical dependency bindings are required"
                    .into(),
            ));
        }
        let Some((instrument_id, _, _)) = assignments.get(&run.task_id) else {
            return Err(InstrumentFleetExecutionError::InvalidRequest(
                "every execution run must resolve to a scheduled assignment".into(),
            ));
        };
        if run.execution.objective.trim() != run.execution.plan.objective.trim()
            || run.execution.plan.instrument_id != *instrument_id
            || run.execution.plan.model_system != request.schedule.model_system
        {
            return Err(InstrumentFleetExecutionError::InvalidRequest(
                "execution objective, plan instrument, and model scope must match the schedule"
                    .into(),
            ));
        }
        run.execution
            .plan
            .validate()
            .map_err(|error| InstrumentFleetExecutionError::InvalidRequest(error.to_string()))?;
    }
    if run_ids != admitted {
        return Err(InstrumentFleetExecutionError::InvalidRequest(
            "execution runs must cover every admitted schedule task exactly once".into(),
        ));
    }
    Ok(())
}

impl InstrumentFleetExecution {
    pub fn validate(&self) -> Result<(), InstrumentFleetExecutionError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.schedule_digest.as_str().len() != 64
            || self.digest.as_str().len() != 64
            || !canonical(&self.task_order)
            || !canonical(&self.admitted_order)
            || !unique_any_order(&self.execution_order)
            || !canonical(&self.completed_order)
            || !canonical(&self.negative_order)
            || !canonical(&self.partial_order)
            || !canonical(&self.failed_order)
            || !canonical(&self.blocked_order)
            || !canonical(&self.unresolved_order)
            || !canonical(&self.uncertainty)
            || !canonical(&self.negative_evidence)
            || self.emergency_stop_succeeded && !self.emergency_stop_requested
            || self.results.len() != self.execution_order.len()
            || self.results.iter().any(|result| {
                result.task_id.trim().is_empty()
                    || result.instrument_id.trim().is_empty()
                    || result.execution.validate().is_err()
            })
        {
            return Err(InstrumentFleetExecutionError::InvalidOutput(
                "identity, partition, ordering, execution, emergency-stop, or digest fields are invalid"
                    .into(),
            ));
        }
        let tasks = self.task_order.iter().cloned().collect::<BTreeSet<_>>();
        let admitted = self.admitted_order.iter().cloned().collect::<BTreeSet<_>>();
        let executed = self
            .execution_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let completed = self
            .completed_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let negative = self.negative_order.iter().cloned().collect::<BTreeSet<_>>();
        let partial = self.partial_order.iter().cloned().collect::<BTreeSet<_>>();
        let failed = self.failed_order.iter().cloned().collect::<BTreeSet<_>>();
        let blocked = self.blocked_order.iter().cloned().collect::<BTreeSet<_>>();
        let unresolved = self
            .unresolved_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        if tasks.len() != self.task_order.len()
            || admitted.len() != self.admitted_order.len()
            || executed.len() != self.execution_order.len()
            || !admitted.is_subset(&tasks)
            || !executed.is_subset(&admitted)
            || ![
                &completed,
                &negative,
                &partial,
                &failed,
                &blocked,
                &unresolved,
            ]
            .iter()
            .all(|partition| partition.is_subset(&tasks))
        {
            return Err(InstrumentFleetExecutionError::InvalidOutput(
                "task identities or schedule partitions do not reconcile".into(),
            ));
        }
        let partitions = [
            &completed,
            &negative,
            &partial,
            &failed,
            &blocked,
            &unresolved,
        ];
        let mut union = BTreeSet::new();
        for partition in partitions {
            if partition.iter().any(|task| !union.insert(task.clone())) {
                return Err(InstrumentFleetExecutionError::InvalidOutput(
                    "execution disposition partitions overlap".into(),
                ));
            }
        }
        if union != tasks {
            return Err(InstrumentFleetExecutionError::InvalidOutput(
                "execution disposition partitions do not cover the schedule".into(),
            ));
        }
        let assignment_ids = self
            .results
            .iter()
            .map(|result| result.task_id.clone())
            .collect::<BTreeSet<_>>();
        if assignment_ids != executed {
            return Err(InstrumentFleetExecutionError::InvalidOutput(
                "execution result identities do not reconcile".into(),
            ));
        }
        let mut failure_ids = BTreeSet::new();
        for failure in &self.failures {
            if failure.task_id.trim().is_empty()
                || !failure_ids.insert(failure.task_id.clone())
                || !failed.contains(&failure.task_id)
                || failure.reason.trim().is_empty()
            {
                return Err(InstrumentFleetExecutionError::InvalidOutput(
                    "execution failure identities or reasons are invalid".into(),
                ));
            }
        }
        let disposition_for = |run: &InstrumentFleetExecutionResult| match run.execution.disposition
        {
            InstrumentExecutionDisposition::Completed => completed.contains(&run.task_id),
            InstrumentExecutionDisposition::Negative => negative.contains(&run.task_id),
            InstrumentExecutionDisposition::Partial => partial.contains(&run.task_id),
            InstrumentExecutionDisposition::Failed => failed.contains(&run.task_id),
            InstrumentExecutionDisposition::Blocked => blocked.contains(&run.task_id),
            InstrumentExecutionDisposition::Unresolved => unresolved.contains(&run.task_id),
        };
        if self.results.iter().any(|result| !disposition_for(result)) {
            return Err(InstrumentFleetExecutionError::InvalidOutput(
                "execution result dispositions do not match outer partitions".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| InstrumentFleetExecutionError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(InstrumentFleetExecutionError::InvalidOutput(
                "fleet execution digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn blocked_reason(
    task_id: &str,
    dependencies: &BTreeMap<String, Vec<String>>,
    completed: &BTreeSet<String>,
    blocked: &BTreeSet<String>,
) -> String {
    let reasons = dependencies
        .get(task_id)
        .into_iter()
        .flatten()
        .filter(|dependency| !completed.contains(*dependency))
        .map(|dependency| {
            if blocked.contains(dependency) {
                format!("dependency-blocked:{dependency}")
            } else {
                format!("dependency-not-completed:{dependency}")
            }
        })
        .collect::<Vec<_>>();
    if reasons.is_empty() {
        "dependency-blocked".into()
    } else {
        reasons.join(",")
    }
}

/// Execute the admitted portion of a fleet schedule through one caller-owned guarded gateway.
/// Assignment timing and instrument identity are checked before each plan crosses the gateway;
/// a non-completed physical outcome blocks dependent work and safety-halts the remaining queue.
pub fn execute_glioma_instrument_fleet<E: InstrumentExecutor>(
    input: &InstrumentFleetExecutionRequest,
    executor: &mut E,
) -> Result<InstrumentFleetExecution, InstrumentFleetExecutionError> {
    let request = normalize_request(input);
    validate_request(&request)?;
    let assignment_map = assignment_map(&request.schedule);
    let mut run_map = request
        .runs
        .iter()
        .map(|run| (run.task_id.clone(), run))
        .collect::<BTreeMap<_, _>>();
    let dependencies = request
        .runs
        .iter()
        .map(|run| (run.task_id.clone(), run.depends_on.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut execution_order = request
        .schedule
        .assignments
        .iter()
        .map(|assignment| {
            (
                assignment.scheduled_start_tick,
                assignment.scheduled_end_tick,
                assignment.task_id.clone(),
            )
        })
        .collect::<Vec<_>>();
    execution_order.sort();
    let execution_order = execution_order
        .into_iter()
        .map(|(_, _, task_id)| task_id)
        .collect::<Vec<_>>();
    let mut remaining = request
        .schedule
        .admitted_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut completed = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut partial = BTreeSet::new();
    let mut failed = BTreeSet::new();
    let mut blocked = request
        .schedule
        .blocked_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut unresolved = BTreeSet::new();
    let mut results = Vec::new();
    let mut failures = Vec::new();
    let mut retry_count = 0_u32;
    let mut emergency_stop_requested = false;
    let mut emergency_stop_succeeded = false;
    let mut uncertainty = request
        .schedule
        .uncertainty
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut negative_evidence = request
        .schedule
        .negative_evidence
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut stop_reason = if blocked.is_empty() {
        InstrumentFleetExecutionStopReason::Completed
    } else {
        InstrumentFleetExecutionStopReason::ScheduleBlocked
    };
    let mut halt_reason: Option<String> = None;

    while !remaining.is_empty() {
        if let Some(reason) = halt_reason.clone() {
            for task_id in remaining.iter() {
                blocked.insert(task_id.clone());
                uncertainty.insert(format!("{task_id}:{reason}"));
            }
            remaining.clear();
            break;
        }
        let next_task = execution_order.iter().find(|task_id| {
            remaining.contains(*task_id)
                && dependencies
                    .get(*task_id)
                    .into_iter()
                    .flatten()
                    .all(|dependency| completed.contains(dependency))
        });
        let Some(task_id) = next_task.cloned() else {
            for task_id in remaining.iter() {
                blocked.insert(task_id.clone());
                uncertainty.insert(format!(
                    "{task_id}:{}",
                    blocked_reason(task_id, &dependencies, &completed, &blocked)
                ));
            }
            remaining.clear();
            stop_reason = InstrumentFleetExecutionStopReason::DependencyBlocked;
            break;
        };
        remaining.remove(&task_id);
        let run = run_map
            .remove(&task_id)
            .expect("validated execution run exists for every admitted task");
        let (instrument_id, scheduled_start_tick, scheduled_end_tick) = assignment_map
            .get(&task_id)
            .cloned()
            .expect("validated fleet assignment exists for every admitted task");
        if run.execution.plan.instrument_id != instrument_id {
            failed.insert(task_id.clone());
            failures.push(InstrumentFleetExecutionFailure {
                task_id: task_id.clone(),
                reason: "execution plan instrument changed after schedule validation".into(),
            });
            uncertainty.insert(format!("{task_id}:instrument-binding-changed"));
            halt_reason = Some("instrument-binding-changed".into());
            stop_reason = InstrumentFleetExecutionStopReason::ExecutionFailure;
            continue;
        }
        let execution = match execute_glioma_instrument_plan(&run.execution, executor) {
            Ok(execution) => execution,
            Err(error) => {
                failed.insert(task_id.clone());
                failures.push(InstrumentFleetExecutionFailure {
                    task_id: task_id.clone(),
                    reason: error.to_string(),
                });
                uncertainty.insert(format!("{task_id}:execution-refused"));
                halt_reason = Some("execution-refused".into());
                stop_reason = InstrumentFleetExecutionStopReason::ExecutionFailure;
                continue;
            }
        };
        retry_count = retry_count.saturating_add(execution.retry_count);
        let prior_emergency_stop_requested = emergency_stop_requested;
        emergency_stop_requested |= execution.emergency_stop_requested;
        if execution.emergency_stop_requested {
            emergency_stop_succeeded = if prior_emergency_stop_requested {
                emergency_stop_succeeded && execution.emergency_stop_succeeded
            } else {
                execution.emergency_stop_succeeded
            };
        }
        uncertainty.extend(execution.uncertainty.iter().cloned());
        negative_evidence.extend(execution.negative_evidence.iter().cloned());
        let disposition = execution.disposition;
        results.push(InstrumentFleetExecutionResult {
            task_id: task_id.clone(),
            instrument_id,
            scheduled_start_tick,
            scheduled_end_tick,
            execution,
        });
        match disposition {
            InstrumentExecutionDisposition::Completed => completed.insert(task_id),
            InstrumentExecutionDisposition::Negative => {
                negative.insert(task_id.clone());
                if request.stop_on_negative {
                    halt_reason = Some("negative-result".into());
                    stop_reason = InstrumentFleetExecutionStopReason::NegativeResult;
                }
                false
            }
            InstrumentExecutionDisposition::Partial => {
                partial.insert(task_id);
                halt_reason = Some("partial-effect".into());
                stop_reason = InstrumentFleetExecutionStopReason::PartialEffect;
                false
            }
            InstrumentExecutionDisposition::Failed => {
                failed.insert(task_id);
                halt_reason = Some("failed-result".into());
                stop_reason = InstrumentFleetExecutionStopReason::ExecutionFailure;
                false
            }
            InstrumentExecutionDisposition::Blocked => {
                blocked.insert(task_id);
                halt_reason = Some("blocked-result".into());
                stop_reason = InstrumentFleetExecutionStopReason::ExecutionFailure;
                false
            }
            InstrumentExecutionDisposition::Unresolved => {
                unresolved.insert(task_id);
                halt_reason = Some("unresolved-result".into());
                stop_reason = InstrumentFleetExecutionStopReason::UnresolvedResult;
                false
            }
        };
    }

    if stop_reason == InstrumentFleetExecutionStopReason::Completed && !blocked.is_empty() {
        stop_reason = InstrumentFleetExecutionStopReason::ScheduleBlocked;
    }
    let disposition = if !failed.is_empty() {
        InstrumentFleetExecutionDisposition::Failed
    } else if !partial.is_empty() {
        InstrumentFleetExecutionDisposition::Partial
    } else if !unresolved.is_empty() {
        InstrumentFleetExecutionDisposition::Unresolved
    } else if !blocked.is_empty() {
        InstrumentFleetExecutionDisposition::Blocked
    } else if !negative.is_empty() {
        InstrumentFleetExecutionDisposition::Negative
    } else {
        InstrumentFleetExecutionDisposition::Completed
    };
    let executed_order = results
        .iter()
        .map(|result| result.task_id.clone())
        .collect::<Vec<_>>();
    let mut output = InstrumentFleetExecution {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective,
        schedule_digest: request.schedule.digest,
        task_order: request.schedule.task_order,
        admitted_order: request.schedule.admitted_order,
        execution_order: executed_order,
        results,
        failures,
        completed_order: completed.into_iter().collect(),
        negative_order: negative.into_iter().collect(),
        partial_order: partial.into_iter().collect(),
        failed_order: failed.into_iter().collect(),
        blocked_order: blocked.into_iter().collect(),
        unresolved_order: unresolved.into_iter().collect(),
        retry_count,
        emergency_stop_requested,
        emergency_stop_succeeded,
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative_evidence.into_iter().collect(),
        simulation_only: executor.simulation_only(),
        disposition,
        stop_reason,
        digest: ContentHash::of_bytes(b"unsealed-glioma-instrument-fleet-execution"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| InstrumentFleetExecutionError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p08_instrument_robotics::calibration::{
        analyze_instrument_calibration, CalibrationRequest, CalibrationRun,
    };
    use crate::glioma::programs::p08_instrument_robotics::execution::DryRunInstrumentExecutor;
    use crate::glioma::programs::p08_instrument_robotics::fleet_scheduler::{
        schedule_glioma_instrument_fleet, InstrumentFleetResource, InstrumentFleetScheduleRequest,
        InstrumentFleetTask,
    };
    use crate::glioma::programs::p08_instrument_robotics::preflight::{
        preflight_glioma_instrument, InstrumentAction, InstrumentAuthorization,
        InstrumentInterlockSnapshot, InstrumentOperation, InstrumentPreflightRequest,
    };
    use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};

    fn hash() -> ContentHash {
        ContentHash::of_bytes(b"fleet-execution-test")
    }

    fn interlocks() -> InstrumentInterlockSnapshot {
        InstrumentInterlockSnapshot {
            observed_tick: 1,
            emergency_stop_clear: true,
            guard_closed: true,
            deck_clear: true,
            consumables_available: true,
            waste_capacity_milli: 100_000,
            temperature_milli: Some(37_000),
            minimum_temperature_milli: Some(36_000),
            maximum_temperature_milli: Some(38_000),
            calibration_valid_until_tick: 100,
            calibration_sequence_index: 3,
        }
    }

    fn calibration(
    ) -> crate::glioma::programs::p08_instrument_robotics::calibration::InstrumentCalibration {
        let artifact = hash();
        let runs = (1..=3)
            .map(|index| CalibrationRun {
                run_id: format!("control-{index}"),
                sequence_index: index,
                batch_id: format!("batch-{index}"),
                instrument_id: "scope-a".into(),
                metric_name: "control_intensity".into(),
                model_system: GliomaModelSystem::Organoid,
                observed_milli: 500 + i64::from(index),
                expected_milli: 500,
                artifact: LocalArtifactRef {
                    artifact_id: format!("control-artifact-{index}"),
                    content_hash: artifact.clone(),
                    content_type: "application/json".into(),
                    local_only: true,
                    contains_human_data: false,
                    contains_direct_identifiers: false,
                },
            })
            .collect::<Vec<_>>();
        analyze_instrument_calibration(
            &CalibrationRequest {
                objective: "qualify scope controls".into(),
                instrument_id: "scope-a".into(),
                model_system: GliomaModelSystem::Organoid,
                metric_name: "control_intensity".into(),
                minimum_runs: 3,
                reference_run_count: 2,
                max_reference_mad_milli: 10,
                max_drift_milli: 20,
                max_slope_milli_per_tick: 10,
            },
            &runs,
        )
        .unwrap()
    }

    fn execution(objective: &str, action_id: &str) -> InstrumentExecutionRequest {
        let artifact = hash();
        let actions = vec![InstrumentAction {
            action_id: action_id.into(),
            instrument_id: "scope-a".into(),
            operation: InstrumentOperation::AcquireImage,
            model_system: GliomaModelSystem::Organoid,
            requested_start_tick: 1,
            duration_ticks: 2,
            risk_milli: 100,
            requires_operator: false,
            output_schema: format!("{action_id}1@1"),
            parameters: Vec::new(),
        }];
        let preflight = preflight_glioma_instrument(&InstrumentPreflightRequest {
            objective: objective.into(),
            instrument_id: "scope-a".into(),
            model_system: GliomaModelSystem::Organoid,
            actions: actions.clone(),
            calibration: calibration(),
            interlocks: interlocks(),
            authorization: InstrumentAuthorization {
                authorization_id: "approval-fleet".into(),
                operator_id: "operator-fleet".into(),
                instrument_scope: "scope-a".into(),
                approval_digest: artifact.clone(),
                issued_tick: 0,
                expires_tick: 100,
                revoked: false,
            },
            current_tick: 1,
            maximum_total_risk_milli: 500,
            maximum_duration_ticks: 20,
            minimum_waste_capacity_milli: 100,
        })
        .unwrap();
        InstrumentExecutionRequest {
            objective: objective.into(),
            plan: preflight,
            actions,
            authorization: InstrumentAuthorization {
                authorization_id: "approval-fleet".into(),
                operator_id: "operator-fleet".into(),
                instrument_scope: "scope-a".into(),
                approval_digest: artifact,
                issued_tick: 0,
                expires_tick: 100,
                revoked: false,
            },
            live_interlocks: interlocks(),
            current_tick: 1,
            minimum_waste_capacity_milli: 100,
            max_retries: 1,
            require_artifacts: true,
        }
    }

    fn schedule() -> InstrumentFleetSchedule {
        schedule_glioma_instrument_fleet(&InstrumentFleetScheduleRequest {
            objective: "execute organoid imaging fleet".into(),
            model_system: GliomaModelSystem::Organoid,
            current_tick: 0,
            max_end_tick: 30,
            maximum_total_risk_milli: 1_000,
            operator_capacity: 1,
            tasks: vec![
                InstrumentFleetTask {
                    task_id: "image-a".into(),
                    label: "image a".into(),
                    operation: InstrumentOperation::AcquireImage,
                    model_system: GliomaModelSystem::Organoid,
                    candidate_instrument_order: vec!["scope-a".into()],
                    depends_on: Vec::new(),
                    release_tick: 0,
                    deadline_tick: Some(30),
                    duration_ticks: 5,
                    risk_milli: 100,
                    information_milli: 900,
                    requires_operator: false,
                    output_schema: "ImageA1@1".into(),
                },
                InstrumentFleetTask {
                    task_id: "image-b".into(),
                    label: "image b".into(),
                    operation: InstrumentOperation::AcquireImage,
                    model_system: GliomaModelSystem::Organoid,
                    candidate_instrument_order: vec!["scope-a".into()],
                    depends_on: vec!["image-a".into()],
                    release_tick: 0,
                    deadline_tick: Some(30),
                    duration_ticks: 5,
                    risk_milli: 100,
                    information_milli: 800,
                    requires_operator: false,
                    output_schema: "ImageB1@1".into(),
                },
            ],
            resources: vec![InstrumentFleetResource {
                instrument_id: "scope-a".into(),
                model_system_order: vec![GliomaModelSystem::Organoid],
                operation_order: vec![InstrumentOperation::AcquireImage],
                available_from_tick: 0,
                available_until_tick: 30,
                calibration_valid_until_tick: 30,
                enabled: true,
            }],
        })
        .unwrap()
    }

    fn request() -> InstrumentFleetExecutionRequest {
        InstrumentFleetExecutionRequest {
            objective: "execute organoid imaging fleet".into(),
            schedule: schedule(),
            runs: vec![
                InstrumentFleetExecutionRunRequest {
                    task_id: "image-a".into(),
                    depends_on: Vec::new(),
                    execution: execution("run image a", "acquire-a"),
                },
                InstrumentFleetExecutionRunRequest {
                    task_id: "image-b".into(),
                    depends_on: vec!["image-a".into()],
                    execution: execution("run image b", "acquire-b"),
                },
            ],
            stop_on_negative: true,
        }
    }

    #[test]
    fn fleet_execution_replays_schedule_and_executes_dependency_order() {
        let request = request();
        let mut executor = DryRunInstrumentExecutor {
            interlocks: interlocks(),
            emergency_stop_called: false,
        };
        let first = execute_glioma_instrument_fleet(&request, &mut executor).unwrap();
        let mut reversed = request;
        reversed.runs.reverse();
        let mut executor = DryRunInstrumentExecutor {
            interlocks: interlocks(),
            emergency_stop_called: false,
        };
        let second = execute_glioma_instrument_fleet(&reversed, &mut executor).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            first.disposition,
            InstrumentFleetExecutionDisposition::Completed
        );
        assert_eq!(first.completed_order, vec!["image-a", "image-b"]);
        assert_eq!(first.execution_order, vec!["image-a", "image-b"]);
        assert!(first
            .results
            .iter()
            .all(|result| result.instrument_id == "scope-a"));
        first.validate().unwrap();
    }

    #[test]
    fn fleet_execution_blocks_schedule_rejections_without_dispatch() {
        let schedule = schedule_glioma_instrument_fleet(&InstrumentFleetScheduleRequest {
            objective: "execute blocked imaging fleet".into(),
            model_system: GliomaModelSystem::Organoid,
            current_tick: 0,
            max_end_tick: 30,
            maximum_total_risk_milli: 1_000,
            operator_capacity: 1,
            tasks: vec![InstrumentFleetTask {
                task_id: "image-blocked".into(),
                label: "image blocked".into(),
                operation: InstrumentOperation::AcquireImage,
                model_system: GliomaModelSystem::Organoid,
                candidate_instrument_order: vec!["scope-a".into()],
                depends_on: Vec::new(),
                release_tick: 0,
                deadline_tick: Some(30),
                duration_ticks: 5,
                risk_milli: 100,
                information_milli: 500,
                requires_operator: false,
                output_schema: "ImageBlocked1@1".into(),
            }],
            resources: vec![InstrumentFleetResource {
                instrument_id: "scope-a".into(),
                model_system_order: vec![GliomaModelSystem::Organoid],
                operation_order: vec![InstrumentOperation::Sequence],
                available_from_tick: 0,
                available_until_tick: 30,
                calibration_valid_until_tick: 30,
                enabled: true,
            }],
        })
        .unwrap();
        let output = execute_glioma_instrument_fleet(
            &InstrumentFleetExecutionRequest {
                objective: "execute blocked imaging fleet".into(),
                schedule,
                runs: Vec::new(),
                stop_on_negative: true,
            },
            &mut DryRunInstrumentExecutor {
                interlocks: interlocks(),
                emergency_stop_called: false,
            },
        )
        .unwrap();
        assert_eq!(
            output.disposition,
            InstrumentFleetExecutionDisposition::Blocked
        );
        assert_eq!(output.blocked_order, vec!["image-blocked"]);
        assert!(output.results.is_empty());
        output.validate().unwrap();
    }
}

//! P07-simulation-gated execution of one instrument-backed glioma protocol slice
//! (`GAF-GLIOMA-P08-F23`), not an end-to-end executor for every P07 task.
//!
//! The bridge keeps scientific scheduling and device control in their owning programs: P07
//! simulates the declared task graph, then P08 requires an exact task/action mapping, completed
//! artifacts for non-instrument prerequisites, operator authorization, qualified calibration,
//! clear interlocks, and a live protocol-manifest match before dispatch. A failed live execution
//! is marked unreconciled rather than retried at this layer because a device may already have acted.

use super::execution::{
    execute_glioma_instrument_plan, InstrumentExecutionDisposition, InstrumentExecutionRequest,
    InstrumentExecutionRun,
};
use super::preflight::{
    preflight_glioma_instrument, InstrumentPreflightDisposition, InstrumentPreflightPlan,
    InstrumentPreflightRequest,
};
use super::protocol_binding::{
    compile_glioma_instrument_protocol_binding, InstrumentProtocolBindingPlan,
    InstrumentProtocolGateway, ProtocolBoundInstrumentExecutor,
};
use crate::glioma::programs::p07_protocol_simulation::{
    simulate_glioma_protocol, ProtocolDisposition, ProtocolSimulation, ProtocolSimulationRequest,
    ProtocolTask,
};
use crate::glioma_engine::LocalArtifactRef;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P08-F23";
pub const OUTPUT_SCHEMA: &str = "GliomaSimulationGatedInstrumentWorkflow1@1";
pub const MAX_RETRIES: u8 = 8;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletedProtocolPrerequisite {
    pub task_id: String,
    pub artifact: LocalArtifactRef,
}

/// The P07 schedule is relative; `simulation_epoch_tick` anchors it to the instrument clock.
/// `instrument_preflight` contains the explicit physical actions and operator authorization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimulationGatedInstrumentWorkflowRequest {
    pub protocol: ProtocolSimulationRequest,
    pub simulation_epoch_tick: u64,
    pub instrument_preflight: InstrumentPreflightRequest,
    /// Exact content-addressed outputs for non-instrument task ancestors already completed locally.
    pub completed_prerequisite_artifacts: Vec<CompletedProtocolPrerequisite>,
    /// Snapshot used to compile bindings; P08 re-reads and compares it at the local gateway seam.
    pub protocol_manifest: super::protocol_binding::InstrumentProtocolManifest,
    pub max_retries: u8,
    pub require_artifacts: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SimulationGatedInstrumentWorkflowDisposition {
    Completed,
    Negative,
    Partial,
    Blocked,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SimulationGatedInstrumentWorkflowStopReason {
    Completed,
    NegativeEvidence,
    PartialExecution,
    ProtocolNotFeasible,
    PrerequisitesMissing,
    ApprovalMismatch,
    PreflightBlocked,
    ScheduleChanged,
    ProtocolBindingBlocked,
    InstrumentExecutionBlocked,
    ExecutionUnreconciled,
}

/// Linked evidence for one P07-simulated and P08-gated physical workflow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimulationGatedInstrumentWorkflowRun {
    pub feature_id: String,
    pub output_schema: String,
    pub simulation: ProtocolSimulation,
    /// P07 topological order filtered to instrument-backed tasks.
    pub instrument_task_order: Vec<String>,
    /// P07 tasks outside this instrument slice that still require a local P07 executor.
    pub deferred_task_order: Vec<String>,
    /// Canonical set of non-instrument dependency tasks already satisfied by local artifacts.
    pub completed_prerequisite_order: Vec<String>,
    /// Canonical set of required non-instrument dependencies that are not yet satisfied.
    pub missing_prerequisite_order: Vec<String>,
    pub preflight: Option<InstrumentPreflightPlan>,
    pub protocol_binding: Option<InstrumentProtocolBindingPlan>,
    pub execution: Option<InstrumentExecutionRun>,
    pub disposition: SimulationGatedInstrumentWorkflowDisposition,
    pub stop_reason: SimulationGatedInstrumentWorkflowStopReason,
    pub stop_detail: Option<String>,
    pub uncertainty: Vec<String>,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SimulationGatedInstrumentWorkflowError {
    #[error("simulation-gated instrument workflow request is invalid: {0}")]
    InvalidRequest(String),
    #[error("P07 protocol simulation failed: {0}")]
    Simulation(String),
    #[error("P08 instrument preflight failed: {0}")]
    Preflight(String),
    #[error("P08 protocol binding failed: {0}")]
    Binding(String),
    #[error("P08 instrument execution failed and may require gateway reconciliation: {0}")]
    ExecutionUnreconciled(String),
    #[error("simulation-gated instrument workflow output is invalid: {0}")]
    InvalidOutput(String),
    #[error("simulation-gated instrument workflow digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique(values: &[String]) -> bool {
    values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

fn digest_input(run: &SimulationGatedInstrumentWorkflowRun) -> serde_json::Value {
    serde_json::json!({
        "feature_id": run.feature_id,
        "output_schema": run.output_schema,
        "simulation": run.simulation,
        "instrument_task_order": run.instrument_task_order,
        "deferred_task_order": run.deferred_task_order,
        "completed_prerequisite_order": run.completed_prerequisite_order,
        "missing_prerequisite_order": run.missing_prerequisite_order,
        "preflight": run.preflight,
        "protocol_binding": run.protocol_binding,
        "execution": run.execution,
        "disposition": run.disposition,
        "stop_reason": run.stop_reason,
        "stop_detail": run.stop_detail,
        "uncertainty": run.uncertainty,
    })
}

impl SimulationGatedInstrumentWorkflowRun {
    pub fn validate(&self) -> Result<(), SimulationGatedInstrumentWorkflowError> {
        self.simulation.validate().map_err(|error| {
            SimulationGatedInstrumentWorkflowError::InvalidOutput(error.to_string())
        })?;
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.instrument_task_order.is_empty()
            || !unique(&self.instrument_task_order)
            || !unique(&self.deferred_task_order)
            || !canonical(&self.completed_prerequisite_order)
            || !canonical(&self.missing_prerequisite_order)
            || !canonical(&self.uncertainty)
            || self
                .completed_prerequisite_order
                .iter()
                .any(|id| self.missing_prerequisite_order.binary_search(id).is_ok())
            || self
                .stop_detail
                .as_ref()
                .is_some_and(|detail| detail.trim().is_empty())
            || self.deferred_task_order.iter().any(|task_id| {
                self.instrument_task_order.contains(task_id)
                    || self.completed_prerequisite_order.contains(task_id)
                    || self.missing_prerequisite_order.contains(task_id)
            })
        {
            return Err(SimulationGatedInstrumentWorkflowError::InvalidOutput(
                "identity, task partitions, diagnostics, or uncertainty ordering is invalid".into(),
            ));
        }
        if let Some(preflight) = &self.preflight {
            preflight.validate().map_err(|error| {
                SimulationGatedInstrumentWorkflowError::InvalidOutput(error.to_string())
            })?;
            if preflight.objective != self.simulation.objective
                || preflight.model_system != self.simulation.model_system
            {
                return Err(SimulationGatedInstrumentWorkflowError::InvalidOutput(
                    "P08 preflight does not match the P07 scientific scope".into(),
                ));
            }
        }
        if let Some(binding) = &self.protocol_binding {
            binding.validate().map_err(|error| {
                SimulationGatedInstrumentWorkflowError::InvalidOutput(error.to_string())
            })?;
            if self
                .preflight
                .as_ref()
                .is_none_or(|preflight| binding.preflight_digest != preflight.digest)
            {
                return Err(SimulationGatedInstrumentWorkflowError::InvalidOutput(
                    "protocol binding is not linked to this run's P08 preflight".into(),
                ));
            }
        }
        if let Some(execution) = &self.execution {
            execution.validate().map_err(|error| {
                SimulationGatedInstrumentWorkflowError::InvalidOutput(error.to_string())
            })?;
            if self.preflight.as_ref().is_none_or(|preflight| {
                execution.plan_digest != preflight.digest
                    || execution.action_order != self.instrument_task_order
            }) || self.protocol_binding.is_none()
            {
                return Err(SimulationGatedInstrumentWorkflowError::InvalidOutput(
                    "instrument execution does not match the simulated task and preflight chain"
                        .into(),
                ));
            }
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| SimulationGatedInstrumentWorkflowError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(SimulationGatedInstrumentWorkflowError::InvalidOutput(
                "workflow digest does not cover the complete simulation-to-execution chain".into(),
            ));
        }
        Ok(())
    }
}

fn seal_run(
    simulation: ProtocolSimulation,
    instrument_task_order: Vec<String>,
    deferred_task_order: Vec<String>,
    completed_prerequisite_order: Vec<String>,
    missing_prerequisite_order: Vec<String>,
    preflight: Option<InstrumentPreflightPlan>,
    protocol_binding: Option<InstrumentProtocolBindingPlan>,
    execution: Option<InstrumentExecutionRun>,
    disposition: SimulationGatedInstrumentWorkflowDisposition,
    stop_reason: SimulationGatedInstrumentWorkflowStopReason,
    stop_detail: Option<String>,
    uncertainty: Vec<String>,
) -> Result<SimulationGatedInstrumentWorkflowRun, SimulationGatedInstrumentWorkflowError> {
    let mut run = SimulationGatedInstrumentWorkflowRun {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        simulation,
        instrument_task_order,
        deferred_task_order,
        completed_prerequisite_order,
        missing_prerequisite_order,
        preflight,
        protocol_binding,
        execution,
        disposition,
        stop_reason,
        stop_detail,
        uncertainty,
        digest: ContentHash::of_bytes(b"unsealed-simulation-gated-instrument-workflow"),
    };
    run.digest = ContentHash::of_value(&digest_input(&run))
        .map_err(|error| SimulationGatedInstrumentWorkflowError::Digest(error.to_string()))?;
    run.validate()?;
    Ok(run)
}

fn task_map(tasks: &[ProtocolTask]) -> BTreeMap<&str, &ProtocolTask> {
    tasks
        .iter()
        .map(|task| (task.task_id.as_str(), task))
        .collect()
}

fn instrument_order(simulation: &ProtocolSimulation, tasks: &[ProtocolTask]) -> Vec<String> {
    let by_id = task_map(tasks);
    simulation
        .topological_order
        .iter()
        .filter(|task_id| {
            by_id
                .get(task_id.as_str())
                .is_some_and(|task| task.requires_instrument)
        })
        .cloned()
        .collect()
}

fn deferred_task_order(
    simulation: &ProtocolSimulation,
    tasks: &[ProtocolTask],
    completed_prerequisite_order: &[String],
    missing_prerequisite_order: &[String],
) -> Vec<String> {
    let by_id = task_map(tasks);
    let completed = completed_prerequisite_order.iter().collect::<BTreeSet<_>>();
    let missing = missing_prerequisite_order.iter().collect::<BTreeSet<_>>();
    simulation
        .topological_order
        .iter()
        .filter(|task_id| {
            by_id.get(task_id.as_str()).is_some_and(|task| {
                !task.requires_instrument
                    && !completed.contains(task_id)
                    && !missing.contains(task_id)
            })
        })
        .cloned()
        .collect()
}

fn required_non_instrument_ancestors(
    protocol: &ProtocolSimulationRequest,
    instrument_order: &[String],
) -> Result<BTreeSet<String>, SimulationGatedInstrumentWorkflowError> {
    let tasks = task_map(&protocol.tasks);
    let mut required = BTreeSet::new();
    let mut visited = BTreeSet::new();
    let mut pending = instrument_order
        .iter()
        .filter_map(|task_id| tasks.get(task_id.as_str()))
        .flat_map(|task| task.depends_on.iter().cloned())
        .collect::<Vec<_>>();
    while let Some(task_id) = pending.pop() {
        if !visited.insert(task_id.clone()) {
            continue;
        }
        let task = tasks.get(task_id.as_str()).ok_or_else(|| {
            SimulationGatedInstrumentWorkflowError::InvalidRequest(format!(
                "protocol dependency {task_id} is not declared"
            ))
        })?;
        if task.requires_instrument {
            pending.extend(task.depends_on.iter().cloned());
        } else {
            // A completed non-instrument task is a checkpoint for its entire own dependency cone.
            required.insert(task_id);
        }
    }
    Ok(required)
}

fn validate_completed_prerequisites(
    protocol: &ProtocolSimulationRequest,
    instrument_order: &[String],
    artifacts: &[CompletedProtocolPrerequisite],
) -> Result<(Vec<String>, Vec<String>), SimulationGatedInstrumentWorkflowError> {
    let tasks = task_map(&protocol.tasks);
    let required = required_non_instrument_ancestors(protocol, instrument_order)?;
    let mut supplied = BTreeMap::new();
    for entry in artifacts {
        let task = tasks.get(entry.task_id.as_str()).ok_or_else(|| {
            SimulationGatedInstrumentWorkflowError::InvalidRequest(format!(
                "completed prerequisite {} is not in the simulated protocol",
                entry.task_id
            ))
        })?;
        entry.artifact.validate().map_err(|error| {
            SimulationGatedInstrumentWorkflowError::InvalidRequest(error.to_string())
        })?;
        if task.requires_instrument
            || task.output_schema != entry.artifact.content_type
            || supplied
                .insert(entry.task_id.as_str(), &entry.artifact)
                .is_some()
        {
            return Err(SimulationGatedInstrumentWorkflowError::InvalidRequest(
                format!(
                    "completed prerequisite {} is duplicated, instrument-backed, or has the wrong typed output",
                    entry.task_id
                ),
            ));
        }
    }
    let supplied_ids = supplied
        .keys()
        .map(|id| (*id).to_owned())
        .collect::<BTreeSet<_>>();
    let extraneous = supplied_ids
        .difference(&required)
        .cloned()
        .collect::<Vec<_>>();
    if !extraneous.is_empty() {
        return Err(SimulationGatedInstrumentWorkflowError::InvalidRequest(
            format!(
                "completed artifacts must match the exact non-instrument dependency closure; unexpected task {}",
                extraneous[0]
            ),
        ));
    }
    let missing = required
        .difference(&supplied_ids)
        .cloned()
        .collect::<Vec<_>>();
    Ok((supplied_ids.into_iter().collect(), missing))
}

fn validate_task_action_mapping(
    request: &SimulationGatedInstrumentWorkflowRequest,
    simulation: &ProtocolSimulation,
    instrument_order: &[String],
) -> Result<(), SimulationGatedInstrumentWorkflowError> {
    let preflight = &request.instrument_preflight;
    let actions = preflight
        .actions
        .iter()
        .map(|action| (action.action_id.as_str(), action))
        .collect::<BTreeMap<_, _>>();
    if actions.len() != preflight.actions.len()
        || actions.len() != instrument_order.len()
        || instrument_order
            .iter()
            .any(|task_id| !actions.contains_key(task_id.as_str()))
    {
        return Err(SimulationGatedInstrumentWorkflowError::InvalidRequest(
            "P08 actions must map one-to-one to every P07 instrument task using its exact task id"
                .into(),
        ));
    }
    if preflight.objective != request.protocol.objective
        || preflight.model_system != request.protocol.model_system
        || preflight.instrument_id.trim().is_empty()
        || preflight.actions.iter().any(|action| {
            action.instrument_id != preflight.instrument_id
                || action.model_system != request.protocol.model_system
        })
    {
        return Err(SimulationGatedInstrumentWorkflowError::InvalidRequest(
            "P07 simulation and P08 actions must share one declared objective, preclinical model, and instrument scope".into(),
        ));
    }
    let tasks = task_map(&request.protocol.tasks);
    let schedule = simulation
        .schedule
        .iter()
        .map(|entry| (entry.task_id.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    for task_id in instrument_order {
        let task = tasks[task_id.as_str()];
        let action = actions[task_id.as_str()];
        let entry = schedule.get(task_id.as_str()).ok_or_else(|| {
            SimulationGatedInstrumentWorkflowError::InvalidRequest(format!(
                "P07 did not schedule instrument task {task_id}"
            ))
        })?;
        let requested_start = request
            .simulation_epoch_tick
            .checked_add(u64::from(entry.start_tick))
            .filter(|tick| *tick <= super::preflight::MAX_TICK)
            .ok_or_else(|| {
                SimulationGatedInstrumentWorkflowError::InvalidRequest(format!(
                    "instrument task {task_id} exceeds the bounded instrument time horizon"
                ))
            })?;
        let duration = u64::from(entry.finish_tick.saturating_sub(entry.start_tick));
        if action.output_schema != task.output_schema
            || action.risk_milli != u64::from(task.risk_milli)
            || action.requested_start_tick != requested_start
            || action.duration_ticks != duration
            || duration != u64::from(task.duration_ticks)
        {
            return Err(SimulationGatedInstrumentWorkflowError::InvalidRequest(
                format!(
                    "P08 action {task_id} does not preserve its simulated start, duration, risk, and output contract"
                ),
            ));
        }
    }
    if preflight.current_tick
        > request
            .simulation_epoch_tick
            .saturating_add(u64::from(simulation.makespan_ticks))
        || preflight.current_tick > super::preflight::MAX_TICK
    {
        return Err(SimulationGatedInstrumentWorkflowError::InvalidRequest(
            "P08 current tick is beyond the simulated protocol horizon".into(),
        ));
    }
    Ok(())
}

fn preflight_still_matches_schedule(
    plan: &InstrumentPreflightPlan,
    request: &SimulationGatedInstrumentWorkflowRequest,
    simulation: &ProtocolSimulation,
    instrument_order: &[String],
) -> bool {
    if plan.action_order != instrument_order || plan.admitted_order != instrument_order {
        return false;
    }
    let p07_schedule = simulation
        .schedule
        .iter()
        .map(|entry| (entry.task_id.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    plan.decisions.iter().all(|decision| {
        let Some(entry) = p07_schedule.get(decision.action_id.as_str()) else {
            return false;
        };
        let Some(expected_start) = request
            .simulation_epoch_tick
            .checked_add(u64::from(entry.start_tick))
        else {
            return false;
        };
        let expected_end = request
            .simulation_epoch_tick
            .saturating_add(u64::from(entry.finish_tick));
        decision.scheduled_start_tick == Some(expected_start)
            && decision.scheduled_end_tick == Some(expected_end)
    })
}

fn blocked_run(
    simulation: ProtocolSimulation,
    instrument_task_order: Vec<String>,
    deferred_task_order: Vec<String>,
    completed_prerequisite_order: Vec<String>,
    missing_prerequisite_order: Vec<String>,
    preflight: Option<InstrumentPreflightPlan>,
    stop_reason: SimulationGatedInstrumentWorkflowStopReason,
    detail: impl Into<String>,
    uncertainty: Vec<String>,
) -> Result<SimulationGatedInstrumentWorkflowRun, SimulationGatedInstrumentWorkflowError> {
    seal_run(
        simulation,
        instrument_task_order,
        deferred_task_order,
        completed_prerequisite_order,
        missing_prerequisite_order,
        preflight,
        None,
        None,
        SimulationGatedInstrumentWorkflowDisposition::Blocked,
        stop_reason,
        Some(detail.into()),
        uncertainty,
    )
}

/// Simulate, qualify, bind, and execute the instrument-backed portion of a glioma protocol.
///
/// The function dispatches only after P07 feasibility, exact P07/P08 task parity, precompleted
/// non-instrument prerequisites, current operator authorization, P08 calibration/interlock
/// preflight, unchanged P07 timing, and a live P08 protocol-manifest recheck all pass. The local
/// gateway remains the only hardware effect boundary. Errors from dispatch are explicitly
/// unreconciled because a controller may have performed a physical action before returning them.
pub fn execute_glioma_simulation_gated_instrument_workflow<G: InstrumentProtocolGateway>(
    request: &SimulationGatedInstrumentWorkflowRequest,
    gateway: G,
) -> Result<SimulationGatedInstrumentWorkflowRun, SimulationGatedInstrumentWorkflowError> {
    if request.max_retries > MAX_RETRIES
        || request.simulation_epoch_tick > super::preflight::MAX_TICK
        || request.protocol_manifest.revision == 0
        || request.protocol_manifest.instrument_id != request.instrument_preflight.instrument_id
    {
        return Err(SimulationGatedInstrumentWorkflowError::InvalidRequest(
            "bounded retries, instrument time, and a versioned manifest are required".into(),
        ));
    }
    request.protocol_manifest.validate().map_err(|error| {
        SimulationGatedInstrumentWorkflowError::InvalidRequest(error.to_string())
    })?;
    let simulation = simulate_glioma_protocol(&request.protocol)
        .map_err(|error| SimulationGatedInstrumentWorkflowError::Simulation(error.to_string()))?;
    let instrument_task_order = instrument_order(&simulation, &request.protocol.tasks);
    if instrument_task_order.is_empty() {
        return Err(SimulationGatedInstrumentWorkflowError::InvalidRequest(
            "P07 protocol has no instrument-backed tasks for this execution path".into(),
        ));
    }
    let (completed_prerequisite_order, missing_prerequisite_order) =
        validate_completed_prerequisites(
            &request.protocol,
            &instrument_task_order,
            &request.completed_prerequisite_artifacts,
        )?;
    let deferred_task_order = deferred_task_order(
        &simulation,
        &request.protocol.tasks,
        &completed_prerequisite_order,
        &missing_prerequisite_order,
    );

    if simulation.disposition != ProtocolDisposition::Feasible {
        return blocked_run(
            simulation,
            instrument_task_order,
            deferred_task_order,
            completed_prerequisite_order,
            missing_prerequisite_order,
            None,
            SimulationGatedInstrumentWorkflowStopReason::ProtocolNotFeasible,
            "P07 did not admit the declared protocol; no instrument preflight or gateway call was made",
            vec!["the instrument task plan remains unexecuted".into()],
        );
    }
    validate_task_action_mapping(request, &simulation, &instrument_task_order)?;
    if !missing_prerequisite_order.is_empty() {
        let missing = missing_prerequisite_order.join(", ");
        return blocked_run(
            simulation,
            instrument_task_order,
            deferred_task_order,
            completed_prerequisite_order,
            missing_prerequisite_order,
            None,
            SimulationGatedInstrumentWorkflowStopReason::PrerequisitesMissing,
            format!(
                "non-instrument dependency artifacts are missing: {missing}; no gateway call was made"
            ),
            vec!["P07 simulation does not mean prerequisite tasks were executed".into()],
        );
    }

    let approval_matches = request.protocol.allow_instrument_execution
        && request.protocol.approval_reference.as_deref()
            == Some(
                request
                    .instrument_preflight
                    .authorization
                    .authorization_id
                    .as_str(),
            );
    if !approval_matches {
        return blocked_run(
            simulation,
            instrument_task_order,
            deferred_task_order,
            completed_prerequisite_order,
            missing_prerequisite_order,
            None,
            SimulationGatedInstrumentWorkflowStopReason::ApprovalMismatch,
            "P07 approval reference and P08 instrument authorization must be the same explicit grant; no gateway call was made",
            vec!["operator authorization is required for instrument-backed execution".into()],
        );
    }

    let preflight = preflight_glioma_instrument(&request.instrument_preflight)
        .map_err(|error| SimulationGatedInstrumentWorkflowError::Preflight(error.to_string()))?;
    if preflight.disposition != InstrumentPreflightDisposition::Admitted
        || !preflight.dispatch_permitted
    {
        let detail = format!(
            "P08 preflight is {:?}; no instrument command was dispatched",
            preflight.disposition
        );
        return blocked_run(
            simulation,
            instrument_task_order,
            deferred_task_order,
            completed_prerequisite_order,
            missing_prerequisite_order,
            Some(preflight),
            SimulationGatedInstrumentWorkflowStopReason::PreflightBlocked,
            detail,
            vec!["instrument calibration, authorization, or interlock conditions are unresolved or blocked".into()],
        );
    }
    if !preflight_still_matches_schedule(&preflight, request, &simulation, &instrument_task_order) {
        return blocked_run(
            simulation,
            instrument_task_order,
            deferred_task_order,
            completed_prerequisite_order,
            missing_prerequisite_order,
            Some(preflight),
            SimulationGatedInstrumentWorkflowStopReason::ScheduleChanged,
            "P08 preflight changed the P07 scheduled order or timing; re-simulate before dispatch",
            vec!["a shifted instrument schedule can invalidate downstream task timing".into()],
        );
    }

    let binding = match compile_glioma_instrument_protocol_binding(
        &preflight,
        &request.instrument_preflight.actions,
        &request.protocol_manifest,
    ) {
        Ok(binding) => binding,
        Err(error) => {
            return blocked_run(
                simulation,
                instrument_task_order,
                deferred_task_order,
                completed_prerequisite_order,
                missing_prerequisite_order,
                Some(preflight),
                SimulationGatedInstrumentWorkflowStopReason::ProtocolBindingBlocked,
                format!("no unambiguous live protocol command binding exists: {error}"),
                vec!["device capability mapping is unresolved; no command was dispatched".into()],
            );
        }
    };
    let mut bound_executor = match ProtocolBoundInstrumentExecutor::new(
        binding.clone(),
        &preflight,
        &request.instrument_preflight.actions,
        gateway,
    ) {
        Ok(executor) => executor,
        Err(error) => {
            return blocked_run(
                simulation,
                instrument_task_order,
                deferred_task_order,
                completed_prerequisite_order,
                missing_prerequisite_order,
                Some(preflight),
                SimulationGatedInstrumentWorkflowStopReason::ProtocolBindingBlocked,
                format!("live instrument manifest recheck failed before dispatch: {error}"),
                vec!["the expected device capability revision is not currently available".into()],
            );
        }
    };
    let execution = match execute_glioma_instrument_plan(
        &InstrumentExecutionRequest {
            objective: request.protocol.objective.clone(),
            plan: preflight.clone(),
            actions: request.instrument_preflight.actions.clone(),
            authorization: request.instrument_preflight.authorization.clone(),
            live_interlocks: request.instrument_preflight.interlocks.clone(),
            current_tick: request.instrument_preflight.current_tick,
            minimum_waste_capacity_milli: request.instrument_preflight.minimum_waste_capacity_milli,
            max_retries: request.max_retries,
            require_artifacts: request.require_artifacts,
        },
        &mut bound_executor,
    ) {
        Ok(execution) => execution,
        Err(error) => {
            return Err(
                SimulationGatedInstrumentWorkflowError::ExecutionUnreconciled(error.to_string()),
            );
        }
    };
    let (disposition, stop_reason, stop_detail) = match execution.disposition {
        InstrumentExecutionDisposition::Completed if deferred_task_order.is_empty() => (
            SimulationGatedInstrumentWorkflowDisposition::Completed,
            SimulationGatedInstrumentWorkflowStopReason::Completed,
            None,
        ),
        InstrumentExecutionDisposition::Completed => (
            SimulationGatedInstrumentWorkflowDisposition::Partial,
            SimulationGatedInstrumentWorkflowStopReason::PartialExecution,
            Some("the instrument slice completed, but P07 non-instrument tasks remain for a local task executor".into()),
        ),
        InstrumentExecutionDisposition::Negative if deferred_task_order.is_empty() => (
            SimulationGatedInstrumentWorkflowDisposition::Negative,
            SimulationGatedInstrumentWorkflowStopReason::NegativeEvidence,
            None,
        ),
        InstrumentExecutionDisposition::Negative => (
            SimulationGatedInstrumentWorkflowDisposition::Partial,
            SimulationGatedInstrumentWorkflowStopReason::PartialExecution,
            Some("the instrument slice returned negative evidence; downstream P07 tasks remain unexecuted".into()),
        ),
        InstrumentExecutionDisposition::Partial => (
            SimulationGatedInstrumentWorkflowDisposition::Partial,
            SimulationGatedInstrumentWorkflowStopReason::PartialExecution,
            None,
        ),
        InstrumentExecutionDisposition::Failed => (
            SimulationGatedInstrumentWorkflowDisposition::Failed,
            SimulationGatedInstrumentWorkflowStopReason::InstrumentExecutionBlocked,
            Some(
                "P08 reports a failed instrument task; inspect its failure and recovery record"
                    .into(),
            ),
        ),
        InstrumentExecutionDisposition::Blocked | InstrumentExecutionDisposition::Unresolved => (
            SimulationGatedInstrumentWorkflowDisposition::Blocked,
            SimulationGatedInstrumentWorkflowStopReason::InstrumentExecutionBlocked,
            Some(
                "P08 stopped on an authorization, interlock, or unresolved instrument result"
                    .into(),
            ),
        ),
    };
    let mut uncertainty = execution.uncertainty.clone();
    uncertainty.sort();
    uncertainty.dedup();
    seal_run(
        simulation,
        instrument_task_order,
        deferred_task_order,
        completed_prerequisite_order,
        missing_prerequisite_order,
        Some(preflight),
        Some(binding),
        Some(execution),
        disposition,
        stop_reason,
        stop_detail,
        uncertainty,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p07_protocol_simulation::{
        simulate_glioma_protocol, ProtocolResource, ProtocolResourceKind, ProtocolTask,
    };
    use crate::glioma::programs::p08_instrument_robotics::calibration::{
        analyze_instrument_calibration, CalibrationRequest, CalibrationRun,
    };
    use crate::glioma::programs::p08_instrument_robotics::execution::{
        InstrumentExecutionFailure, InstrumentExecutionResult,
    };
    use crate::glioma::programs::p08_instrument_robotics::preflight::{
        InstrumentAction, InstrumentAuthorization, InstrumentInterlockSnapshot, InstrumentOperation,
    };
    use crate::glioma::programs::p08_instrument_robotics::protocol_binding::{
        InstrumentCommandCapability, InstrumentControlProtocol, InstrumentProtocolManifest,
    };
    use crate::glioma_engine::GliomaModelSystem;
    use bioprism_ids::ContentHash;
    use std::sync::{Arc, Mutex};

    const EPOCH: u64 = 100;

    fn protocol() -> ProtocolSimulationRequest {
        ProtocolSimulationRequest {
            objective: "measure invasive-edge response in glioma organoids".into(),
            model_system: GliomaModelSystem::Organoid,
            tasks: vec![
                ProtocolTask {
                    task_id: "prepare-model".into(),
                    label: "prepare organoid and matched controls".into(),
                    resource_kind: ProtocolResourceKind::Culture,
                    resource_units: 1,
                    duration_ticks: 2,
                    depends_on: Vec::new(),
                    model_system: GliomaModelSystem::Organoid,
                    output_schema: "GliomaModelPreparation1@1".into(),
                    risk_milli: 10,
                    requires_instrument: false,
                },
                ProtocolTask {
                    task_id: "capture-edge".into(),
                    label: "capture a declared invasive-edge image".into(),
                    resource_kind: ProtocolResourceKind::Imaging,
                    resource_units: 1,
                    duration_ticks: 3,
                    depends_on: vec!["prepare-model".into()],
                    model_system: GliomaModelSystem::Organoid,
                    output_schema: "GliomaInvasiveEdgeImage1@1".into(),
                    risk_milli: 40,
                    requires_instrument: true,
                },
            ],
            resources: vec![
                ProtocolResource {
                    resource_id: "organoid-culture-slot".into(),
                    kind: ProtocolResourceKind::Culture,
                    capacity_units: 1,
                },
                ProtocolResource {
                    resource_id: "organoid-imager-slot".into(),
                    kind: ProtocolResourceKind::Imaging,
                    capacity_units: 1,
                },
            ],
            max_ticks: 20,
            max_risk_milli: 100,
            allow_instrument_execution: true,
            approval_reference: Some("operator-approval-1".into()),
            randomization_seed: ContentHash::of_bytes(b"glioma-protocol-seed"),
        }
    }

    fn calibration(instrument_id: &str) -> super::super::calibration::InstrumentCalibration {
        let runs = (1..=3)
            .map(|sequence_index| CalibrationRun {
                run_id: format!("calibration-{sequence_index}"),
                sequence_index,
                batch_id: format!("reference-batch-{sequence_index}"),
                instrument_id: instrument_id.into(),
                metric_name: "image-reference".into(),
                model_system: GliomaModelSystem::Organoid,
                observed_milli: 1_000,
                expected_milli: 1_000,
                artifact: LocalArtifactRef {
                    artifact_id: format!("calibration-artifact-{sequence_index}"),
                    content_hash: ContentHash::of_bytes(
                        format!("calibration-artifact-{sequence_index}").as_bytes(),
                    ),
                    content_type: "application/vnd.aurora.glioma.calibration+json".into(),
                    local_only: true,
                    contains_human_data: false,
                    contains_direct_identifiers: false,
                },
            })
            .collect::<Vec<_>>();
        analyze_instrument_calibration(
            &CalibrationRequest {
                objective: "qualify organoid imager against local reference".into(),
                instrument_id: instrument_id.into(),
                model_system: GliomaModelSystem::Organoid,
                metric_name: "image-reference".into(),
                minimum_runs: 3,
                reference_run_count: 2,
                max_reference_mad_milli: 100,
                max_drift_milli: 100,
                max_slope_milli_per_tick: 100,
            },
            &runs,
        )
        .unwrap()
    }

    fn instrument_action(protocol: &ProtocolSimulationRequest) -> InstrumentAction {
        let simulation = simulate_glioma_protocol(protocol).unwrap();
        let scheduled = simulation
            .schedule
            .iter()
            .find(|entry| entry.task_id == "capture-edge")
            .unwrap();
        InstrumentAction {
            action_id: "capture-edge".into(),
            instrument_id: "organoid-imager-1".into(),
            operation: InstrumentOperation::AcquireImage,
            model_system: GliomaModelSystem::Organoid,
            requested_start_tick: EPOCH + u64::from(scheduled.start_tick),
            duration_ticks: u64::from(scheduled.finish_tick - scheduled.start_tick),
            risk_milli: 40,
            requires_operator: false,
            output_schema: "GliomaInvasiveEdgeImage1@1".into(),
            parameters: Vec::new(),
        }
    }

    fn manifest() -> InstrumentProtocolManifest {
        InstrumentProtocolManifest {
            instrument_id: "organoid-imager-1".into(),
            protocol: InstrumentControlProtocol::Sila2,
            protocol_version: "1.1".into(),
            manifest_id: "organoid-imager-capabilities".into(),
            revision: 7,
            commands: vec![InstrumentCommandCapability {
                operation: InstrumentOperation::AcquireImage,
                feature_path: "org.aurora.GliomaImaging".into(),
                command_id: "AcquireFrame".into(),
                output_schema: "GliomaInvasiveEdgeImage1@1".into(),
                retry_idempotent: false,
                parameters: Vec::new(),
            }],
        }
    }

    fn workflow_request() -> SimulationGatedInstrumentWorkflowRequest {
        let protocol = protocol();
        SimulationGatedInstrumentWorkflowRequest {
            instrument_preflight: InstrumentPreflightRequest {
                objective: protocol.objective.clone(),
                instrument_id: "organoid-imager-1".into(),
                model_system: GliomaModelSystem::Organoid,
                actions: vec![instrument_action(&protocol)],
                calibration: calibration("organoid-imager-1"),
                interlocks: InstrumentInterlockSnapshot {
                    observed_tick: EPOCH,
                    emergency_stop_clear: true,
                    guard_closed: true,
                    deck_clear: true,
                    consumables_available: true,
                    waste_capacity_milli: 1_000,
                    temperature_milli: Some(37_000),
                    minimum_temperature_milli: Some(36_000),
                    maximum_temperature_milli: Some(38_000),
                    calibration_valid_until_tick: 1_000,
                    calibration_sequence_index: 3,
                },
                authorization: InstrumentAuthorization {
                    authorization_id: "operator-approval-1".into(),
                    operator_id: "operator-1".into(),
                    instrument_scope: "organoid-imager-1".into(),
                    approval_digest: ContentHash::of_bytes(b"signed-glioma-assay-approval"),
                    issued_tick: EPOCH - 1,
                    expires_tick: 500,
                    revoked: false,
                },
                current_tick: EPOCH,
                maximum_total_risk_milli: 100,
                maximum_duration_ticks: 50,
                minimum_waste_capacity_milli: 100,
            },
            protocol,
            simulation_epoch_tick: EPOCH,
            completed_prerequisite_artifacts: vec![CompletedProtocolPrerequisite {
                task_id: "prepare-model".into(),
                artifact: LocalArtifactRef {
                    artifact_id: "prepared-organoid-study-1".into(),
                    content_hash: ContentHash::of_bytes(b"prepared-organoid-study-1"),
                    content_type: "GliomaModelPreparation1@1".into(),
                    local_only: true,
                    contains_human_data: false,
                    contains_direct_identifiers: false,
                },
            }],
            protocol_manifest: manifest(),
            max_retries: 0,
            require_artifacts: true,
        }
    }

    #[derive(Debug)]
    struct Gateway {
        manifest: InstrumentProtocolManifest,
        interlocks: InstrumentInterlockSnapshot,
        outcome: InstrumentExecutionDisposition,
        stats: Arc<Mutex<GatewayStats>>,
    }

    #[derive(Debug, Default)]
    struct GatewayStats {
        manifest_reads: usize,
        authorization_checks: usize,
        dispatches: Vec<String>,
        emergency_stops: usize,
    }

    impl Gateway {
        fn from_request(
            request: &SimulationGatedInstrumentWorkflowRequest,
            outcome: InstrumentExecutionDisposition,
        ) -> (Self, Arc<Mutex<GatewayStats>>) {
            let stats = Arc::new(Mutex::new(GatewayStats::default()));
            (
                Self {
                    manifest: request.protocol_manifest.clone(),
                    interlocks: request.instrument_preflight.interlocks.clone(),
                    outcome,
                    stats: stats.clone(),
                },
                stats,
            )
        }
    }

    impl InstrumentProtocolGateway for Gateway {
        fn observe_interlocks(
            &mut self,
        ) -> Result<InstrumentInterlockSnapshot, InstrumentExecutionFailure> {
            Ok(self.interlocks.clone())
        }

        fn verify_authorization(
            &mut self,
            _authorization: &InstrumentAuthorization,
        ) -> Result<(), InstrumentExecutionFailure> {
            self.stats.lock().unwrap().authorization_checks += 1;
            Ok(())
        }

        fn current_manifest(
            &mut self,
            instrument_id: &str,
        ) -> Result<InstrumentProtocolManifest, InstrumentExecutionFailure> {
            self.stats.lock().unwrap().manifest_reads += 1;
            if instrument_id != self.manifest.instrument_id {
                return Err(InstrumentExecutionFailure {
                    reason: "instrument id mismatch".into(),
                    retryable: false,
                });
            }
            Ok(self.manifest.clone())
        }

        fn execute_bound_command(
            &mut self,
            command: &super::super::protocol_binding::BoundInstrumentCommand,
            attempt: u8,
        ) -> Result<InstrumentExecutionResult, InstrumentExecutionFailure> {
            self.stats
                .lock()
                .unwrap()
                .dispatches
                .push(command.action_id.clone());
            Ok(InstrumentExecutionResult {
                action_id: command.action_id.clone(),
                disposition: self.outcome,
                attempt_count: attempt,
                started_tick: Some(EPOCH + 2),
                completed_tick: Some(EPOCH + 5),
                artifact: Some(LocalArtifactRef {
                    artifact_id: format!("instrument-output:{}", command.action_id),
                    content_hash: ContentHash::of_bytes(command.action_id.as_bytes()),
                    content_type: command.output_schema.clone(),
                    local_only: true,
                    contains_human_data: false,
                    contains_direct_identifiers: false,
                }),
                note: "local gateway returned a task-bound instrument artifact".into(),
                uncertainty: Vec::new(),
                negative_evidence: if self.outcome == InstrumentExecutionDisposition::Negative {
                    vec!["declared assay endpoint was not detected".into()]
                } else {
                    Vec::new()
                },
            })
        }

        fn emergency_stop(&mut self) -> Result<(), InstrumentExecutionFailure> {
            self.stats.lock().unwrap().emergency_stops += 1;
            Ok(())
        }
    }

    #[test]
    fn feasible_simulation_runs_only_after_exact_prerequisite_schedule_and_live_binding_checks() {
        let request = workflow_request();
        let (gateway, stats) =
            Gateway::from_request(&request, InstrumentExecutionDisposition::Completed);
        let run = execute_glioma_simulation_gated_instrument_workflow(&request, gateway).unwrap();

        assert_eq!(
            run.disposition,
            SimulationGatedInstrumentWorkflowDisposition::Completed
        );
        assert_eq!(run.instrument_task_order, vec!["capture-edge"]);
        assert_eq!(run.completed_prerequisite_order, vec!["prepare-model"]);
        assert!(run.preflight.as_ref().unwrap().dispatch_permitted);
        assert_eq!(run.protocol_binding.as_ref().unwrap().manifest_revision, 7);
        assert_eq!(
            run.execution.as_ref().unwrap().completed_order,
            vec!["capture-edge"]
        );
        assert_eq!(
            run.execution.as_ref().unwrap().action_order,
            run.instrument_task_order
        );
        let stats = stats.lock().unwrap();
        assert_eq!(stats.manifest_reads, 2);
        assert_eq!(stats.authorization_checks, 1);
        assert_eq!(stats.dispatches, vec!["capture-edge"]);
        assert_eq!(stats.emergency_stops, 0);
        run.validate().unwrap();
    }

    #[test]
    fn missing_local_task_artifacts_block_before_the_gateway_is_read_or_dispatched() {
        let mut request = workflow_request();
        request.completed_prerequisite_artifacts.clear();
        let (gateway, stats) =
            Gateway::from_request(&request, InstrumentExecutionDisposition::Completed);
        let run = execute_glioma_simulation_gated_instrument_workflow(&request, gateway).unwrap();

        assert_eq!(
            run.disposition,
            SimulationGatedInstrumentWorkflowDisposition::Blocked
        );
        assert_eq!(
            run.stop_reason,
            SimulationGatedInstrumentWorkflowStopReason::PrerequisitesMissing
        );
        assert_eq!(run.missing_prerequisite_order, vec!["prepare-model"]);
        assert!(run.preflight.is_none());
        let stats = stats.lock().unwrap();
        assert_eq!(stats.manifest_reads, 0);
        assert!(stats.dispatches.is_empty());
        run.validate().unwrap();
    }

    #[test]
    fn an_unapproved_p07_simulation_cannot_reach_instrument_preflight() {
        let mut request = workflow_request();
        request.protocol.allow_instrument_execution = false;
        let (gateway, stats) =
            Gateway::from_request(&request, InstrumentExecutionDisposition::Completed);
        let run = execute_glioma_simulation_gated_instrument_workflow(&request, gateway).unwrap();

        assert_eq!(
            run.simulation.disposition,
            ProtocolDisposition::ApprovalRequired
        );
        assert_eq!(
            run.stop_reason,
            SimulationGatedInstrumentWorkflowStopReason::ProtocolNotFeasible
        );
        assert!(run.preflight.is_none());
        let stats = stats.lock().unwrap();
        assert_eq!(stats.manifest_reads, 0);
        assert!(stats.dispatches.is_empty());
        run.validate().unwrap();
    }

    #[test]
    fn a_p08_action_cannot_silently_shift_from_its_p07_simulated_slot() {
        let mut request = workflow_request();
        request.instrument_preflight.actions[0].requested_start_tick += 1;
        let (gateway, stats) =
            Gateway::from_request(&request, InstrumentExecutionDisposition::Completed);

        let error =
            execute_glioma_simulation_gated_instrument_workflow(&request, gateway).unwrap_err();
        assert!(matches!(
            error,
            SimulationGatedInstrumentWorkflowError::InvalidRequest(_)
        ));
        let stats = stats.lock().unwrap();
        assert_eq!(stats.manifest_reads, 0);
        assert!(stats.dispatches.is_empty());
    }

    #[test]
    fn null_or_negative_instrument_results_remain_scientific_negative_evidence() {
        let request = workflow_request();
        let (gateway, _) =
            Gateway::from_request(&request, InstrumentExecutionDisposition::Negative);
        let run = execute_glioma_simulation_gated_instrument_workflow(&request, gateway).unwrap();

        assert_eq!(
            run.disposition,
            SimulationGatedInstrumentWorkflowDisposition::Negative
        );
        assert_eq!(
            run.stop_reason,
            SimulationGatedInstrumentWorkflowStopReason::NegativeEvidence
        );
        assert_eq!(
            run.execution.as_ref().unwrap().negative_evidence,
            vec!["declared assay endpoint was not detected"]
        );
        run.validate().unwrap();
    }

    #[test]
    fn completion_of_the_instrument_slice_does_not_claim_downstream_p07_work_is_done() {
        let mut request = workflow_request();
        request.protocol.resources.push(ProtocolResource {
            resource_id: "local-analysis-slot".into(),
            kind: ProtocolResourceKind::Compute,
            capacity_units: 1,
        });
        request.protocol.tasks.push(ProtocolTask {
            task_id: "summarize-edge".into(),
            label: "summarize the acquired invasive-edge measurements".into(),
            resource_kind: ProtocolResourceKind::Compute,
            resource_units: 1,
            duration_ticks: 2,
            depends_on: vec!["capture-edge".into()],
            model_system: GliomaModelSystem::Organoid,
            output_schema: "GliomaInvasiveEdgeSummary1@1".into(),
            risk_milli: 5,
            requires_instrument: false,
        });
        let (gateway, _) =
            Gateway::from_request(&request, InstrumentExecutionDisposition::Completed);
        let run = execute_glioma_simulation_gated_instrument_workflow(&request, gateway).unwrap();

        assert_eq!(run.deferred_task_order, vec!["summarize-edge"]);
        assert_eq!(
            run.disposition,
            SimulationGatedInstrumentWorkflowDisposition::Partial
        );
        assert_eq!(
            run.stop_reason,
            SimulationGatedInstrumentWorkflowStopReason::PartialExecution
        );
        run.validate().unwrap();
    }
}

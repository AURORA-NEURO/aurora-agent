//! Deterministic worker placement for reproducible preclinical glioma computation.
//!
//! P09 already compiles and executes typed computation DAGs. This feature adds the missing
//! production scheduling layer: it chooses institution-local workers, accounts for data locality
//! and transfer budgets, reuses only replay-valid cache artifacts, and exposes critical-path and
//! worker-utilization results. It never runs code, moves payloads, or dispatches a worker; the
//! schedule is the bounded handoff consumed by the existing computation executor.

use super::execution::{
    ComputationCacheEntry, ComputationOperation, ComputationTask, MAX_COST_UNITS,
    MAX_DURATION_TICKS,
};
use crate::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P09-F20";
pub const OUTPUT_SCHEMA: &str = "GliomaComputationPlacementSchedule1@1";
pub const MAX_TASKS: usize = 2_048;
pub const MAX_WORKERS: usize = 256;
pub const MAX_TICK: u64 = 10_000_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationWorkerProfile {
    pub worker_id: String,
    pub model_system_order: Vec<GliomaModelSystem>,
    pub operation_order: Vec<ComputationOperation>,
    pub local_artifact_order: Vec<String>,
    pub available_from_tick: u64,
    pub available_until_tick: u64,
    pub max_task_cost_units: u64,
    pub transfer_ticks_per_artifact: u64,
    pub transfer_cost_units_per_artifact: u64,
    pub speed_milli: u16,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationPlacementRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub replay_identity: ContentHash,
    pub current_tick: u64,
    pub max_end_tick: u64,
    pub max_budget_units: u64,
    pub max_transfer_cost_units: u64,
    pub tasks: Vec<ComputationTask>,
    pub workers: Vec<ComputationWorkerProfile>,
    pub completed_task_order: Vec<String>,
    pub cache: Vec<ComputationCacheEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationPlacementAssignment {
    pub task_id: String,
    pub worker_id: String,
    pub scheduled_start_tick: u64,
    pub scheduled_end_tick: u64,
    pub dependency_end_tick: u64,
    pub transfer_artifact_count: u32,
    pub transfer_ticks: u64,
    pub transfer_cost_units: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationPlacementBlockedTask {
    pub task_id: String,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationWorkerUtilization {
    pub worker_id: String,
    pub busy_ticks: u64,
    pub transfer_ticks: u64,
    pub first_start_tick: Option<u64>,
    pub last_end_tick: Option<u64>,
    pub utilization_milli: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputationPlacementDisposition {
    Ready,
    Partial,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationPlacementSchedule {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub replay_identity: ContentHash,
    pub worker_order: Vec<String>,
    pub task_order: Vec<String>,
    pub assigned_order: Vec<String>,
    pub completed_order: Vec<String>,
    pub cached_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub assignments: Vec<ComputationPlacementAssignment>,
    pub blocked_tasks: Vec<ComputationPlacementBlockedTask>,
    pub critical_path_order: Vec<String>,
    pub worker_utilization: Vec<ComputationWorkerUtilization>,
    pub total_compute_cost_units: u64,
    pub total_transfer_cost_units: u64,
    pub makespan_ticks: u64,
    pub dispatch_permitted: bool,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: ComputationPlacementDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ComputationPlacementError {
    #[error("computation placement request is invalid: {0}")]
    InvalidRequest(String),
    #[error("computation placement graph is invalid: {0}")]
    InvalidGraph(String),
    #[error("computation placement output is invalid: {0}")]
    InvalidOutput(String),
    #[error("computation placement digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique_nonempty(values: &[String]) -> bool {
    values.iter().all(|value| !value.trim().is_empty()) && canonical(values)
}

fn unique_nonempty_any_order(values: &[String]) -> bool {
    values.iter().all(|value| !value.trim().is_empty())
        && values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

fn digest_input(schedule: &ComputationPlacementSchedule) -> serde_json::Value {
    serde_json::json!({
        "feature_id": schedule.feature_id,
        "output_schema": schedule.output_schema,
        "objective": schedule.objective,
        "model_system": schedule.model_system,
        "replay_identity": schedule.replay_identity,
        "worker_order": schedule.worker_order,
        "task_order": schedule.task_order,
        "assigned_order": schedule.assigned_order,
        "completed_order": schedule.completed_order,
        "cached_order": schedule.cached_order,
        "blocked_order": schedule.blocked_order,
        "assignments": schedule.assignments,
        "blocked_tasks": schedule.blocked_tasks,
        "critical_path_order": schedule.critical_path_order,
        "worker_utilization": schedule.worker_utilization,
        "total_compute_cost_units": schedule.total_compute_cost_units,
        "total_transfer_cost_units": schedule.total_transfer_cost_units,
        "makespan_ticks": schedule.makespan_ticks,
        "dispatch_permitted": schedule.dispatch_permitted,
        "negative_evidence": schedule.negative_evidence,
        "uncertainty": schedule.uncertainty,
        "disposition": schedule.disposition,
    })
}

impl ComputationPlacementSchedule {
    pub fn validate(&self) -> Result<(), ComputationPlacementError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.replay_identity.as_str().len() != 64
            || !unique_nonempty(&self.worker_order)
            || !unique_nonempty(&self.task_order)
            || !unique_nonempty(&self.assigned_order)
            || !unique_nonempty(&self.completed_order)
            || !unique_nonempty(&self.cached_order)
            || !unique_nonempty(&self.blocked_order)
            || !unique_nonempty_any_order(&self.critical_path_order)
            || !unique_nonempty(&self.negative_evidence)
            || !unique_nonempty(&self.uncertainty)
            || self.assignments.len() != self.assigned_order.len()
            || self
                .assignments
                .windows(2)
                .any(|pair| pair[0].task_id >= pair[1].task_id)
            || self.blocked_tasks.len() != self.blocked_order.len()
            || self
                .blocked_tasks
                .windows(2)
                .any(|pair| pair[0].task_id >= pair[1].task_id)
            || self.blocked_tasks.iter().any(|blocked| {
                blocked.task_id.trim().is_empty() || !unique_nonempty(&blocked.reasons)
            })
            || self.assignments.iter().any(|assignment| {
                assignment.task_id.trim().is_empty()
                    || assignment.worker_id.trim().is_empty()
                    || assignment.scheduled_start_tick >= assignment.scheduled_end_tick
                    || assignment.dependency_end_tick > assignment.scheduled_start_tick
            })
            || self
                .worker_utilization
                .windows(2)
                .any(|pair| pair[0].worker_id >= pair[1].worker_id)
            || self
                .worker_utilization
                .iter()
                .any(|utilization| utilization.utilization_milli > 1_000)
            || self.dispatch_permitted
        {
            return Err(ComputationPlacementError::InvalidOutput(
                "identity, canonical partitions, assignment timing, utilization, cost, or dispatch-boundary invariants are invalid".into(),
            ));
        }
        let tasks = self.task_order.iter().cloned().collect::<BTreeSet<_>>();
        let assigned = self.assigned_order.iter().cloned().collect::<BTreeSet<_>>();
        let completed = self
            .completed_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let cached = self.cached_order.iter().cloned().collect::<BTreeSet<_>>();
        let blocked = self.blocked_order.iter().cloned().collect::<BTreeSet<_>>();
        if tasks.len() != self.task_order.len()
            || assigned.len() != self.assigned_order.len()
            || completed.len() != self.completed_order.len()
            || cached.len() != self.cached_order.len()
            || blocked.len() != self.blocked_order.len()
            || assigned.intersection(&cached).next().is_some()
            || assigned.intersection(&completed).next().is_some()
            || completed.intersection(&cached).next().is_some()
            || assigned.intersection(&blocked).next().is_some()
            || completed.intersection(&blocked).next().is_some()
            || cached.intersection(&blocked).next().is_some()
            || assigned
                .union(&completed)
                .cloned()
                .collect::<BTreeSet<_>>()
                .union(&cached)
                .cloned()
                .collect::<BTreeSet<_>>()
                .union(&blocked)
                .cloned()
                .collect::<BTreeSet<_>>()
                != tasks
            || self
                .assignments
                .iter()
                .map(|assignment| assignment.task_id.clone())
                .collect::<BTreeSet<_>>()
                != assigned
            || self
                .blocked_tasks
                .iter()
                .map(|blocked| blocked.task_id.clone())
                .collect::<BTreeSet<_>>()
                != blocked
            || self
                .critical_path_order
                .iter()
                .any(|task| !assigned.contains(task))
        {
            return Err(ComputationPlacementError::InvalidOutput(
                "task partitions, assignment identities, or critical path do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ComputationPlacementError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ComputationPlacementError::InvalidOutput(
                "computation placement digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn normalize_request(input: &ComputationPlacementRequest) -> ComputationPlacementRequest {
    let mut request = input.clone();
    request
        .tasks
        .sort_by(|left, right| left.task_id.cmp(&right.task_id));
    for task in &mut request.tasks {
        task.depends_on.sort();
        task.input_artifact_ids.sort();
    }
    request
        .workers
        .sort_by(|left, right| left.worker_id.cmp(&right.worker_id));
    for worker in &mut request.workers {
        worker.model_system_order.sort();
        worker.operation_order.sort();
        worker.local_artifact_order.sort();
    }
    request.completed_task_order.sort();
    request
        .cache
        .sort_by(|left, right| left.task_id.cmp(&right.task_id));
    request
}

fn validate_request(
    request: &ComputationPlacementRequest,
) -> Result<(), ComputationPlacementError> {
    if request.objective.trim().is_empty()
        || request.replay_identity.as_str().len() != 64
        || request.current_tick > request.max_end_tick
        || request.max_end_tick > MAX_TICK
        || request.max_budget_units == 0
        || request.max_budget_units > MAX_COST_UNITS
        || request.max_transfer_cost_units > MAX_COST_UNITS
        || request.tasks.is_empty()
        || request.tasks.len() > MAX_TASKS
        || request.workers.is_empty()
        || request.workers.len() > MAX_WORKERS
    {
        return Err(ComputationPlacementError::InvalidRequest(
            "objective, replay identity, bounded time/cost windows, tasks, and workers are required".into(),
        ));
    }
    if request
        .completed_task_order
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
        || request
            .completed_task_order
            .iter()
            .any(|task| task.trim().is_empty())
    {
        return Err(ComputationPlacementError::InvalidRequest(
            "completed task ids must be canonical and non-empty".into(),
        ));
    }
    Ok(())
}

type PlacementGraph = (
    BTreeMap<String, ComputationTask>,
    BTreeMap<String, ComputationWorkerProfile>,
    BTreeSet<String>,
    BTreeMap<String, ComputationCacheEntry>,
);

fn validate_graph(
    request: &ComputationPlacementRequest,
) -> Result<PlacementGraph, ComputationPlacementError> {
    let completed = request
        .completed_task_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut tasks = BTreeMap::new();
    for task in &request.tasks {
        if task.task_id.trim().is_empty()
            || task.output_schema.trim().is_empty()
            || task.model_system != request.model_system
            || task.estimated_cost_units == 0
            || task.estimated_cost_units > MAX_COST_UNITS
            || task.estimated_duration_ticks == 0
            || task.estimated_duration_ticks > MAX_DURATION_TICKS
            || task
                .depends_on
                .iter()
                .any(|dependency| dependency.trim().is_empty() || dependency == &task.task_id)
            || task.depends_on.windows(2).any(|pair| pair[0] >= pair[1])
            || task
                .input_artifact_ids
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || tasks.insert(task.task_id.clone(), task.clone()).is_some()
        {
            return Err(ComputationPlacementError::InvalidGraph(
                "task identity, model scope, output schema, bounded cost/duration, canonical dependencies, and input artifacts are required".into(),
            ));
        }
    }
    if completed.iter().any(|task| !tasks.contains_key(task)) {
        return Err(ComputationPlacementError::InvalidGraph(
            "completed tasks must resolve to declared task identities".into(),
        ));
    }
    for task in tasks.values() {
        if task
            .depends_on
            .iter()
            .any(|dependency| !completed.contains(dependency) && !tasks.contains_key(dependency))
        {
            return Err(ComputationPlacementError::InvalidGraph(
                "task dependency references an unknown task".into(),
            ));
        }
    }
    let mut workers = BTreeMap::new();
    for worker in &request.workers {
        if worker.worker_id.trim().is_empty()
            || worker.model_system_order.is_empty()
            || worker.operation_order.is_empty()
            || worker.available_from_tick > worker.available_until_tick
            || worker.available_from_tick < request.current_tick
            || worker.available_until_tick > request.max_end_tick
            || worker.max_task_cost_units == 0
            || worker.transfer_ticks_per_artifact > MAX_DURATION_TICKS
            || worker.transfer_cost_units_per_artifact > MAX_COST_UNITS
            || worker.speed_milli == 0
            || worker.speed_milli > 10_000
            || !canonical(&worker.model_system_order)
            || !canonical(&worker.operation_order)
            || !unique_nonempty(&worker.local_artifact_order)
            || workers
                .insert(worker.worker_id.clone(), worker.clone())
                .is_some()
        {
            return Err(ComputationPlacementError::InvalidGraph(
                "worker identity, capabilities, availability, speed, capacity, and canonical locality are required".into(),
            ));
        }
    }
    let mut cache = BTreeMap::new();
    for entry in &request.cache {
        if entry.task_id.trim().is_empty()
            || completed.contains(&entry.task_id)
            || entry.replay_identity != request.replay_identity
            || entry.output_schema
                != tasks
                    .get(&entry.task_id)
                    .map(|task| task.output_schema.as_str())
                    .unwrap_or("")
            || entry.artifact.content_type
                != tasks
                    .get(&entry.task_id)
                    .map(|task| task.output_schema.as_str())
                    .unwrap_or("")
            || entry.artifact.validate().is_err()
            || cache.insert(entry.task_id.clone(), entry.clone()).is_some()
        {
            return Err(ComputationPlacementError::InvalidGraph(
                "cache entries must be unique, task-bound, replay-bound, local, and schema-compatible".into(),
            ));
        }
    }
    let mut indegree = tasks
        .iter()
        .map(|(id, task)| {
            (
                id.clone(),
                task.depends_on
                    .iter()
                    .filter(|dependency| !completed.contains(*dependency))
                    .count(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut outgoing = BTreeMap::<String, Vec<String>>::new();
    for task in tasks.values() {
        for dependency in &task.depends_on {
            if !completed.contains(dependency) {
                outgoing
                    .entry(dependency.clone())
                    .or_default()
                    .push(task.task_id.clone());
            }
        }
    }
    let mut queue = VecDeque::from(
        indegree
            .iter()
            .filter(|(_, degree)| **degree == 0)
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>(),
    );
    let mut visited = 0_usize;
    while let Some(id) = queue.pop_front() {
        visited += 1;
        for child in outgoing.get(&id).into_iter().flatten() {
            let degree = indegree.get_mut(child).expect("graph child exists");
            *degree = degree.saturating_sub(1);
            if *degree == 0 {
                queue.push_back(child.clone());
            }
        }
    }
    if visited != tasks.len() {
        return Err(ComputationPlacementError::InvalidGraph(
            "computation dependency graph contains a cycle".into(),
        ));
    }
    Ok((tasks, workers, completed, cache))
}

#[derive(Debug, Clone)]
struct CandidateSlot {
    worker_id: String,
    start: u64,
    end: u64,
    transfer_ticks: u64,
    transfer_cost_units: u64,
    transfer_artifact_count: u32,
}

fn task_priority(task: &ComputationTask, child_count: usize) -> (usize, u8, u64, String) {
    (
        child_count,
        operation_rank(task.operation),
        u64::MAX.saturating_sub(task.estimated_duration_ticks),
        task.task_id.clone(),
    )
}

fn operation_rank(operation: ComputationOperation) -> u8 {
    match operation {
        ComputationOperation::Ingest => 1,
        ComputationOperation::Normalize => 2,
        ComputationOperation::Register => 3,
        ComputationOperation::Segment => 4,
        ComputationOperation::Quantify => 5,
        ComputationOperation::Integrate => 6,
        ComputationOperation::ModelFit => 7,
        ComputationOperation::Validate => 8,
        ComputationOperation::Export => 9,
    }
}

fn choose_slot(
    task: &ComputationTask,
    request: &ComputationPlacementRequest,
    workers: &BTreeMap<String, ComputationWorkerProfile>,
    worker_cursor: &BTreeMap<String, u64>,
    dependency_end: u64,
) -> (Option<CandidateSlot>, Vec<String>) {
    let mut best = None;
    let mut reasons = BTreeSet::new();
    for (worker_id, worker) in workers {
        if !worker.enabled {
            reasons.insert(format!("{worker_id}:disabled"));
            continue;
        }
        if !worker.model_system_order.contains(&task.model_system) {
            reasons.insert(format!("{worker_id}:model-system-incompatible"));
            continue;
        }
        if !worker.operation_order.contains(&task.operation) {
            reasons.insert(format!("{worker_id}:operation-incompatible"));
            continue;
        }
        if task.estimated_cost_units > worker.max_task_cost_units {
            reasons.insert(format!("{worker_id}:worker-capacity"));
            continue;
        }
        let missing = task
            .input_artifact_ids
            .iter()
            .filter(|artifact| !worker.local_artifact_order.binary_search(artifact).is_ok())
            .count() as u32;
        let transfer_ticks = u64::from(missing).saturating_mul(worker.transfer_ticks_per_artifact);
        let transfer_cost_units =
            u64::from(missing).saturating_mul(worker.transfer_cost_units_per_artifact);
        let scaled_duration = task
            .estimated_duration_ticks
            .saturating_mul(1_000)
            .saturating_add(u64::from(worker.speed_milli).saturating_sub(1))
            / u64::from(worker.speed_milli);
        let duration = transfer_ticks.saturating_add(scaled_duration);
        let start = dependency_end
            .max(
                *worker_cursor
                    .get(worker_id)
                    .unwrap_or(&worker.available_from_tick),
            )
            .max(worker.available_from_tick);
        let Some(end) = start.checked_add(duration) else {
            reasons.insert(format!("{worker_id}:time-overflow"));
            continue;
        };
        if end > worker.available_until_tick {
            reasons.insert(format!("{worker_id}:availability-window"));
            continue;
        }
        if transfer_cost_units > request.max_transfer_cost_units {
            reasons.insert(format!("{worker_id}:transfer-budget"));
            continue;
        }
        if end > request.max_end_tick {
            reasons.insert(format!("{worker_id}:mission-window"));
            continue;
        }
        let candidate = CandidateSlot {
            worker_id: worker_id.clone(),
            start,
            end,
            transfer_ticks,
            transfer_cost_units,
            transfer_artifact_count: missing,
        };
        let replace = best.as_ref().is_none_or(|current: &CandidateSlot| {
            (
                candidate.end,
                candidate.transfer_cost_units,
                candidate.worker_id.clone(),
            ) < (
                current.end,
                current.transfer_cost_units,
                current.worker_id.clone(),
            )
        });
        if replace {
            best = Some(candidate);
        }
    }
    (best, reasons.into_iter().collect())
}

fn critical_path(
    assignments: &BTreeMap<String, ComputationPlacementAssignment>,
    tasks: &BTreeMap<String, ComputationTask>,
) -> Vec<String> {
    let Some((last_id, _)) = assignments
        .iter()
        .max_by_key(|(_, assignment)| (assignment.scheduled_end_tick, assignment.task_id.clone()))
    else {
        return Vec::new();
    };
    let mut result = Vec::new();
    let mut cursor = last_id.clone();
    loop {
        result.push(cursor.clone());
        let Some(task) = tasks.get(&cursor) else {
            break;
        };
        let assignment = &assignments[&cursor];
        let dependency_predecessor = task
            .depends_on
            .iter()
            .filter_map(|dependency| assignments.get(dependency))
            .filter(|candidate| candidate.scheduled_end_tick <= assignment.scheduled_start_tick)
            .max_by_key(|candidate| (candidate.scheduled_end_tick, candidate.task_id.clone()));
        let worker_predecessor = assignments
            .values()
            .filter(|candidate| {
                candidate.task_id != cursor
                    && candidate.worker_id == assignment.worker_id
                    && candidate.scheduled_end_tick <= assignment.scheduled_start_tick
            })
            .max_by_key(|candidate| (candidate.scheduled_end_tick, candidate.task_id.clone()));
        let Some(previous) = [dependency_predecessor, worker_predecessor]
            .into_iter()
            .flatten()
            .max_by_key(|candidate| (candidate.scheduled_end_tick, candidate.task_id.clone()))
        else {
            break;
        };
        cursor = previous.task_id.clone();
    }
    result.reverse();
    result
}

/// Build a deterministic, locality-aware computation placement schedule. The result is a
/// pre-dispatch handoff for `execute_glioma_computation`; it never invokes workers or moves data.
pub fn schedule_glioma_computation_placement(
    input: &ComputationPlacementRequest,
) -> Result<ComputationPlacementSchedule, ComputationPlacementError> {
    let request = normalize_request(input);
    validate_request(&request)?;
    let (tasks, workers, completed, cache) = validate_graph(&request)?;
    let mut remaining = tasks
        .keys()
        .filter(|id| !completed.contains(*id))
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut assigned = BTreeSet::new();
    let cached = cache.keys().cloned().collect::<BTreeSet<_>>();
    let mut blocked = BTreeMap::<String, Vec<String>>::new();
    let mut assignments = BTreeMap::<String, ComputationPlacementAssignment>::new();
    let mut worker_cursor = workers
        .iter()
        .map(|(id, worker)| {
            (
                id.clone(),
                worker.available_from_tick.max(request.current_tick),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut total_compute_cost = 0_u64;
    let mut total_transfer_cost = 0_u64;
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    while !remaining.is_empty() {
        let ready = remaining
            .iter()
            .filter(|id| {
                tasks[*id].depends_on.iter().all(|dependency| {
                    assigned.contains(dependency)
                        || completed.contains(dependency)
                        || cached.contains(dependency)
                        || blocked.contains_key(dependency)
                })
            })
            .cloned()
            .collect::<Vec<_>>();
        if ready.is_empty() {
            return Err(ComputationPlacementError::InvalidGraph(
                "scheduler could not progress through the validated computation graph".into(),
            ));
        }
        let child_counts =
            tasks
                .values()
                .fold(BTreeMap::<String, usize>::new(), |mut counts, task| {
                    for dependency in &task.depends_on {
                        *counts.entry(dependency.clone()).or_default() += 1;
                    }
                    counts
                });
        let mut ready = ready;
        ready.sort_by(|left, right| {
            let left_priority = task_priority(&tasks[left], *child_counts.get(left).unwrap_or(&0));
            let right_priority =
                task_priority(&tasks[right], *child_counts.get(right).unwrap_or(&0));
            right_priority
                .0
                .cmp(&left_priority.0)
                .then_with(|| right_priority.1.cmp(&left_priority.1))
                .then_with(|| right_priority.2.cmp(&left_priority.2))
                .then_with(|| left.cmp(right))
        });
        for task_id in ready {
            remaining.remove(&task_id);
            let task = &tasks[&task_id];
            if cached.contains(&task_id) {
                continue;
            }
            if task
                .depends_on
                .iter()
                .any(|dependency| blocked.contains_key(dependency))
            {
                blocked.insert(task_id.clone(), vec!["dependency-blocked".into()]);
                uncertainty.insert(format!("{task_id}:dependency-blocked"));
                continue;
            }
            let dependency_end = task
                .depends_on
                .iter()
                .filter_map(|dependency| assignments.get(dependency))
                .map(|assignment| assignment.scheduled_end_tick)
                .max()
                .unwrap_or(request.current_tick);
            let (slot, reasons) =
                choose_slot(task, &request, &workers, &worker_cursor, dependency_end);
            let Some(slot) = slot else {
                let reasons = if reasons.is_empty() {
                    vec!["no-compatible-worker".into()]
                } else {
                    reasons
                };
                for reason in &reasons {
                    negative.insert(format!("{task_id}:{reason}"));
                }
                blocked.insert(task_id.clone(), reasons);
                continue;
            };
            let next_compute_cost = total_compute_cost.saturating_add(task.estimated_cost_units);
            let next_transfer_cost = total_transfer_cost.saturating_add(slot.transfer_cost_units);
            if next_compute_cost > request.max_budget_units {
                blocked.insert(task_id.clone(), vec!["compute-budget".into()]);
                negative.insert(format!("{task_id}:compute-budget"));
                continue;
            }
            if next_transfer_cost > request.max_transfer_cost_units {
                blocked.insert(task_id.clone(), vec!["transfer-budget".into()]);
                negative.insert(format!("{task_id}:transfer-budget"));
                continue;
            }
            worker_cursor.insert(slot.worker_id.clone(), slot.end);
            total_compute_cost = next_compute_cost;
            total_transfer_cost = next_transfer_cost;
            assigned.insert(task_id.clone());
            assignments.insert(
                task_id.clone(),
                ComputationPlacementAssignment {
                    task_id,
                    worker_id: slot.worker_id,
                    scheduled_start_tick: slot.start,
                    scheduled_end_tick: slot.end,
                    dependency_end_tick: dependency_end,
                    transfer_artifact_count: slot.transfer_artifact_count,
                    transfer_ticks: slot.transfer_ticks,
                    transfer_cost_units: slot.transfer_cost_units,
                },
            );
        }
    }
    let mut assignment_values = assignments.values().cloned().collect::<Vec<_>>();
    assignment_values.sort_by(|left, right| left.task_id.cmp(&right.task_id));
    let assigned_order = assignment_values
        .iter()
        .map(|assignment| assignment.task_id.clone())
        .collect::<Vec<_>>();
    let completed_order = completed.iter().cloned().collect::<Vec<_>>();
    let cached_order = cached
        .iter()
        .filter(|id| tasks.contains_key(*id))
        .cloned()
        .collect::<Vec<_>>();
    let blocked_tasks = blocked
        .into_iter()
        .map(|(task_id, reasons)| ComputationPlacementBlockedTask { task_id, reasons })
        .collect::<Vec<_>>();
    let blocked_order = blocked_tasks
        .iter()
        .map(|blocked| blocked.task_id.clone())
        .collect::<Vec<_>>();
    let makespan = assignment_values
        .iter()
        .map(|assignment| assignment.scheduled_end_tick)
        .max()
        .unwrap_or(request.current_tick);
    let mut utilization = workers
        .values()
        .map(|worker| {
            let rows = assignment_values
                .iter()
                .filter(|assignment| assignment.worker_id == worker.worker_id)
                .collect::<Vec<_>>();
            let busy_ticks = rows
                .iter()
                .map(|assignment| {
                    assignment
                        .scheduled_end_tick
                        .saturating_sub(assignment.scheduled_start_tick)
                })
                .sum::<u64>();
            let transfer_ticks = rows
                .iter()
                .map(|assignment| assignment.transfer_ticks)
                .sum();
            let window = worker
                .available_until_tick
                .saturating_sub(worker.available_from_tick)
                .max(1);
            ComputationWorkerUtilization {
                worker_id: worker.worker_id.clone(),
                busy_ticks,
                transfer_ticks,
                first_start_tick: rows
                    .iter()
                    .map(|assignment| assignment.scheduled_start_tick)
                    .min(),
                last_end_tick: rows
                    .iter()
                    .map(|assignment| assignment.scheduled_end_tick)
                    .max(),
                utilization_milli: (busy_ticks.saturating_mul(1_000) / window).min(1_000) as u16,
            }
        })
        .collect::<Vec<_>>();
    utilization.sort_by(|left, right| left.worker_id.cmp(&right.worker_id));
    let critical_path_order = critical_path(&assignments, &tasks);
    let disposition = if blocked_order.is_empty() {
        ComputationPlacementDisposition::Ready
    } else if assigned_order.is_empty() && cached_order.is_empty() && completed_order.is_empty() {
        ComputationPlacementDisposition::Blocked
    } else {
        ComputationPlacementDisposition::Partial
    };
    if assigned_order.is_empty() && cached_order.is_empty() && completed_order.is_empty() {
        uncertainty.insert("no-task-admitted".into());
    }
    let mut output = ComputationPlacementSchedule {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        replay_identity: request.replay_identity.clone(),
        worker_order: workers.keys().cloned().collect(),
        task_order: tasks.keys().cloned().collect(),
        assigned_order,
        completed_order,
        cached_order,
        blocked_order,
        assignments: assignment_values,
        blocked_tasks,
        critical_path_order,
        worker_utilization: utilization,
        total_compute_cost_units: total_compute_cost,
        total_transfer_cost_units: total_transfer_cost,
        makespan_ticks: makespan,
        dispatch_permitted: false,
        negative_evidence: negative.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-computation-placement"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ComputationPlacementError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma_engine::LocalArtifactRef;

    fn worker(id: &str, until: u64, local: Vec<&str>) -> ComputationWorkerProfile {
        ComputationWorkerProfile {
            worker_id: id.into(),
            model_system_order: vec![GliomaModelSystem::Organoid],
            operation_order: vec![
                ComputationOperation::Normalize,
                ComputationOperation::Integrate,
            ],
            local_artifact_order: local.into_iter().map(str::to_string).collect(),
            available_from_tick: 0,
            available_until_tick: until,
            max_task_cost_units: 1_000,
            transfer_ticks_per_artifact: 2,
            transfer_cost_units_per_artifact: 3,
            speed_milli: 1_000,
            enabled: true,
        }
    }

    fn task(id: &str, operation: ComputationOperation, depends_on: Vec<String>) -> ComputationTask {
        ComputationTask {
            task_id: id.into(),
            operation,
            model_system: GliomaModelSystem::Organoid,
            depends_on,
            input_artifact_ids: vec!["matrix".into()],
            output_schema: format!("{id}1@1"),
            estimated_cost_units: 10,
            estimated_duration_ticks: 5,
            deterministic: true,
        }
    }

    fn request() -> ComputationPlacementRequest {
        ComputationPlacementRequest {
            objective: "integrate organoid imaging and transcriptomics".into(),
            model_system: GliomaModelSystem::Organoid,
            replay_identity: ContentHash::of_bytes(b"placement-replay"),
            current_tick: 0,
            max_end_tick: 40,
            max_budget_units: 100,
            max_transfer_cost_units: 100,
            tasks: vec![
                task("normalize", ComputationOperation::Normalize, Vec::new()),
                task(
                    "integrate",
                    ComputationOperation::Integrate,
                    vec!["normalize".into()],
                ),
            ],
            workers: vec![
                worker("gpu-a", 40, vec!["matrix"]),
                worker("cpu-b", 40, Vec::new()),
            ],
            completed_task_order: Vec::new(),
            cache: Vec::new(),
        }
    }

    #[test]
    fn placement_parallelism_locality_and_replay_are_explicit() {
        let first = schedule_glioma_computation_placement(&request()).unwrap();
        let mut reversed = request();
        reversed.tasks.reverse();
        reversed.workers.reverse();
        let second = schedule_glioma_computation_placement(&reversed).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.disposition, ComputationPlacementDisposition::Ready);
        assert_eq!(first.assignments.len(), 2);
        assert_eq!(first.total_transfer_cost_units, 0);
        assert!(first.critical_path_order.contains(&"integrate".into()));
        assert!(!first.dispatch_permitted);
        first.validate().unwrap();
    }

    #[test]
    fn placement_reuses_replay_valid_cache_and_rejects_stale_cache() {
        let mut request = request();
        let task = request.tasks[0].clone();
        request.cache = vec![ComputationCacheEntry {
            task_id: task.task_id.clone(),
            replay_identity: request.replay_identity.clone(),
            output_schema: task.output_schema.clone(),
            artifact: LocalArtifactRef {
                artifact_id: "cached-normalize".into(),
                content_hash: ContentHash::of_bytes(b"cached-normalize"),
                content_type: task.output_schema,
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
        }];
        let output = schedule_glioma_computation_placement(&request).unwrap();
        assert_eq!(output.cached_order, vec!["normalize"]);
        assert_eq!(output.assignments.len(), 1);
        let mut stale = request;
        stale.cache[0].replay_identity = ContentHash::of_bytes(b"stale");
        assert!(matches!(
            schedule_glioma_computation_placement(&stale),
            Err(ComputationPlacementError::InvalidGraph(_))
        ));
    }

    #[test]
    fn placement_propagates_worker_and_budget_blocks() {
        let mut request = request();
        request.workers[0].operation_order = vec![ComputationOperation::Normalize];
        request.workers[1].operation_order = vec![ComputationOperation::Normalize];
        request.max_budget_units = 5;
        let output = schedule_glioma_computation_placement(&request).unwrap();
        assert_eq!(output.disposition, ComputationPlacementDisposition::Blocked);
        assert!(!output.blocked_order.is_empty());
        assert!(output
            .negative_evidence
            .iter()
            .any(|item| item.contains("budget") || item.contains("operation")));
        output.validate().unwrap();
    }
}

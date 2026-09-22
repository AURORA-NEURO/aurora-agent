//! Deterministic fleet scheduling for preclinical glioma instrument workflows.
//!
//! P08 already admits and executes one instrument plan. This feature coordinates a bounded fleet
//! before any plan is admitted: it closes task dependencies, assigns compatible instruments,
//! respects calibration and availability windows, reserves operator slots, and enforces a shared
//! risk budget. The result is an executable *schedule*, not a hardware effect; every assignment
//! still has to pass the normal instrument preflight and institution-owned gateway.

use super::preflight::InstrumentOperation;
use crate::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P08-F18";
pub const OUTPUT_SCHEMA: &str = "GliomaInstrumentFleetSchedule1@1";
pub const MAX_TASKS: usize = 1_024;
pub const MAX_RESOURCES: usize = 128;
pub const MAX_OPERATORS: usize = 64;
pub const MAX_TICK: u64 = 10_000_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentFleetTask {
    pub task_id: String,
    pub label: String,
    pub operation: InstrumentOperation,
    pub model_system: GliomaModelSystem,
    pub candidate_instrument_order: Vec<String>,
    pub depends_on: Vec<String>,
    pub release_tick: u64,
    pub deadline_tick: Option<u64>,
    pub duration_ticks: u64,
    pub risk_milli: u16,
    pub information_milli: u32,
    pub requires_operator: bool,
    pub output_schema: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentFleetResource {
    pub instrument_id: String,
    pub model_system_order: Vec<GliomaModelSystem>,
    pub operation_order: Vec<InstrumentOperation>,
    pub available_from_tick: u64,
    pub available_until_tick: u64,
    pub calibration_valid_until_tick: u64,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentFleetScheduleRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub current_tick: u64,
    pub max_end_tick: u64,
    pub maximum_total_risk_milli: u32,
    pub operator_capacity: usize,
    pub tasks: Vec<InstrumentFleetTask>,
    pub resources: Vec<InstrumentFleetResource>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentFleetAssignment {
    pub task_id: String,
    pub instrument_id: String,
    pub operator_slot: Option<usize>,
    pub scheduled_start_tick: u64,
    pub scheduled_end_tick: u64,
    pub dependency_end_tick: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentFleetBlockedTask {
    pub task_id: String,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentFleetUtilization {
    pub instrument_id: String,
    pub busy_ticks: u64,
    pub first_start_tick: Option<u64>,
    pub last_end_tick: Option<u64>,
    pub utilization_milli: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstrumentFleetDisposition {
    Ready,
    Partial,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentFleetSchedule {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub resource_order: Vec<String>,
    pub task_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub assignments: Vec<InstrumentFleetAssignment>,
    pub blocked_tasks: Vec<InstrumentFleetBlockedTask>,
    pub critical_path_order: Vec<String>,
    pub utilization: Vec<InstrumentFleetUtilization>,
    pub total_risk_milli: u32,
    pub makespan_ticks: u64,
    pub dispatch_permitted: bool,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: InstrumentFleetDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum InstrumentFleetSchedulerError {
    #[error("instrument fleet request is invalid: {0}")]
    InvalidRequest(String),
    #[error("instrument fleet graph is invalid: {0}")]
    InvalidGraph(String),
    #[error("instrument fleet output is invalid: {0}")]
    InvalidOutput(String),
    #[error("instrument fleet digest failed: {0}")]
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

fn digest_input(schedule: &InstrumentFleetSchedule) -> serde_json::Value {
    serde_json::json!({
        "feature_id": schedule.feature_id,
        "output_schema": schedule.output_schema,
        "objective": schedule.objective,
        "model_system": schedule.model_system,
        "resource_order": schedule.resource_order,
        "task_order": schedule.task_order,
        "admitted_order": schedule.admitted_order,
        "blocked_order": schedule.blocked_order,
        "assignments": schedule.assignments,
        "blocked_tasks": schedule.blocked_tasks,
        "critical_path_order": schedule.critical_path_order,
        "utilization": schedule.utilization,
        "total_risk_milli": schedule.total_risk_milli,
        "makespan_ticks": schedule.makespan_ticks,
        "dispatch_permitted": schedule.dispatch_permitted,
        "negative_evidence": schedule.negative_evidence,
        "uncertainty": schedule.uncertainty,
        "disposition": schedule.disposition,
    })
}

impl InstrumentFleetSchedule {
    pub fn validate(&self) -> Result<(), InstrumentFleetSchedulerError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || !canonical(&self.resource_order)
            || !canonical(&self.task_order)
            || !canonical(&self.admitted_order)
            || !canonical(&self.blocked_order)
            || !unique_any_order(&self.critical_path_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || !unique_any_order(&self.resource_order)
            || !unique_any_order(&self.task_order)
            || self.assignments.len() != self.admitted_order.len()
            || self
                .assignments
                .windows(2)
                .any(|pair| pair[0].task_id >= pair[1].task_id)
            || self.blocked_tasks.len() != self.blocked_order.len()
            || self
                .blocked_tasks
                .windows(2)
                .any(|pair| pair[0].task_id >= pair[1].task_id)
            || self.assignments.iter().any(|assignment| {
                assignment.task_id.trim().is_empty()
                    || assignment.instrument_id.trim().is_empty()
                    || assignment.scheduled_start_tick >= assignment.scheduled_end_tick
                    || assignment.dependency_end_tick > assignment.scheduled_start_tick
            })
            || self
                .blocked_tasks
                .iter()
                .any(|blocked| blocked.task_id.trim().is_empty() || !canonical(&blocked.reasons))
            || self
                .utilization
                .windows(2)
                .any(|pair| pair[0].instrument_id >= pair[1].instrument_id)
            || self
                .utilization
                .iter()
                .any(|item| item.utilization_milli > 1_000)
            || self.total_risk_milli > 1_000_000
            || self.dispatch_permitted
        {
            return Err(InstrumentFleetSchedulerError::InvalidOutput(
                "identity, canonical partitions, assignment timing, utilization, risk, or dispatch-boundary invariants are invalid".into(),
            ));
        }
        let task_ids = self.task_order.iter().cloned().collect::<BTreeSet<_>>();
        let admitted = self.admitted_order.iter().cloned().collect::<BTreeSet<_>>();
        let blocked = self.blocked_order.iter().cloned().collect::<BTreeSet<_>>();
        if task_ids.len() != self.task_order.len()
            || admitted.len() != self.admitted_order.len()
            || blocked.len() != self.blocked_order.len()
            || admitted.intersection(&blocked).next().is_some()
            || admitted.union(&blocked).cloned().collect::<BTreeSet<_>>() != task_ids
            || self
                .assignments
                .iter()
                .map(|assignment| assignment.task_id.clone())
                .collect::<BTreeSet<_>>()
                != admitted
            || self
                .blocked_tasks
                .iter()
                .map(|blocked| blocked.task_id.clone())
                .collect::<BTreeSet<_>>()
                != blocked
            || self
                .critical_path_order
                .iter()
                .any(|task| !admitted.contains(task))
        {
            return Err(InstrumentFleetSchedulerError::InvalidOutput(
                "task partitions, assignment identities, or critical path do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| InstrumentFleetSchedulerError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(InstrumentFleetSchedulerError::InvalidOutput(
                "instrument fleet schedule digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &InstrumentFleetScheduleRequest,
) -> Result<(), InstrumentFleetSchedulerError> {
    if request.objective.trim().is_empty()
        || request.current_tick > request.max_end_tick
        || request.max_end_tick > MAX_TICK
        || request.maximum_total_risk_milli == 0
        || request.maximum_total_risk_milli > 1_000_000
        || request.operator_capacity > MAX_OPERATORS
        || request.tasks.is_empty()
        || request.tasks.len() > MAX_TASKS
        || request.resources.is_empty()
        || request.resources.len() > MAX_RESOURCES
    {
        return Err(InstrumentFleetSchedulerError::InvalidRequest(
            "objective, bounded execution window, positive risk budget, bounded operator capacity, tasks, and resources are required".into(),
        ));
    }
    Ok(())
}

type FleetGraph = (
    BTreeMap<String, InstrumentFleetTask>,
    BTreeMap<String, InstrumentFleetResource>,
);

fn validate_graph(
    request: &InstrumentFleetScheduleRequest,
) -> Result<FleetGraph, InstrumentFleetSchedulerError> {
    let mut tasks = BTreeMap::new();
    for task in &request.tasks {
        if task.task_id.trim().is_empty()
            || task.label.trim().is_empty()
            || task.output_schema.trim().is_empty()
            || task.model_system != request.model_system
            || task.candidate_instrument_order.is_empty()
            || task.duration_ticks == 0
            || task.release_tick < request.current_tick
            || task.release_tick > request.max_end_tick
            || task
                .deadline_tick
                .is_some_and(|deadline| deadline > request.max_end_tick)
            || task.risk_milli > 1_000
            || task.information_milli == 0
            || tasks.insert(task.task_id.clone(), task.clone()).is_some()
        {
            return Err(InstrumentFleetSchedulerError::InvalidGraph(
                "task identity, output schema, candidates, timing, duration, information, risk, and uniqueness are required".into(),
            ));
        }
        if task
            .depends_on
            .iter()
            .any(|dependency| dependency == &task.task_id)
            || !unique_any_order(&task.depends_on)
            || !unique_any_order(&task.candidate_instrument_order)
        {
            return Err(InstrumentFleetSchedulerError::InvalidGraph(
                "task dependencies and candidate instruments must be non-self, non-duplicate identities".into(),
            ));
        }
    }
    let mut resources = BTreeMap::new();
    for resource in &request.resources {
        if resource.instrument_id.trim().is_empty()
            || resource.model_system_order.is_empty()
            || resource.operation_order.is_empty()
            || resource.available_from_tick > resource.available_until_tick
            || resource.available_from_tick < request.current_tick
            || resource.available_until_tick > request.max_end_tick
            || resource.calibration_valid_until_tick < resource.available_from_tick
            || !unique_any_order(
                &resource
                    .model_system_order
                    .iter()
                    .map(|model| format!("{model:?}"))
                    .collect::<Vec<_>>(),
            )
            || !unique_any_order(
                &resource
                    .operation_order
                    .iter()
                    .map(|operation| format!("{operation:?}"))
                    .collect::<Vec<_>>(),
            )
            || resources
                .insert(resource.instrument_id.clone(), resource.clone())
                .is_some()
        {
            return Err(InstrumentFleetSchedulerError::InvalidGraph(
                "resource identity, capabilities, availability, calibration, and uniqueness are required".into(),
            ));
        }
    }
    for task in tasks.values() {
        if task
            .depends_on
            .iter()
            .any(|dependency| !tasks.contains_key(dependency))
            || task
                .candidate_instrument_order
                .iter()
                .any(|instrument| !resources.contains_key(instrument))
        {
            return Err(InstrumentFleetSchedulerError::InvalidGraph(
                "task dependencies and candidate instruments must resolve to declared graph nodes"
                    .into(),
            ));
        }
    }
    let mut indegree = tasks
        .iter()
        .map(|(id, task)| (id.clone(), task.depends_on.len()))
        .collect::<BTreeMap<_, _>>();
    let mut children = BTreeMap::<String, Vec<String>>::new();
    for task in tasks.values() {
        for dependency in &task.depends_on {
            children
                .entry(dependency.clone())
                .or_default()
                .push(task.task_id.clone());
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
        visited = visited.saturating_add(1);
        for child in children.get(&id).into_iter().flatten() {
            let degree = indegree.get_mut(child).expect("child is in graph");
            *degree = degree.saturating_sub(1);
            if *degree == 0 {
                queue.push_back(child.clone());
            }
        }
    }
    if visited != tasks.len() {
        return Err(InstrumentFleetSchedulerError::InvalidGraph(
            "task dependency graph contains a cycle".into(),
        ));
    }
    Ok((tasks, resources))
}

#[derive(Debug, Clone)]
struct CandidateSlot {
    instrument_id: String,
    start: u64,
    end: u64,
    operator_slot: Option<usize>,
}

fn task_priority(task: &InstrumentFleetTask) -> (u128, u16, String) {
    let utility = u128::from(task.information_milli)
        .saturating_mul(1_000)
        .checked_div(u128::from(task.duration_ticks.max(1)))
        .unwrap_or(0);
    (utility, task.risk_milli, task.task_id.clone())
}

fn choose_slot(
    task: &InstrumentFleetTask,
    request: &InstrumentFleetScheduleRequest,
    resources: &BTreeMap<String, InstrumentFleetResource>,
    resource_cursor: &BTreeMap<String, u64>,
    operator_cursor: &[u64],
    dependency_end: u64,
) -> (Option<CandidateSlot>, Vec<String>) {
    let mut best = None;
    let mut reasons = BTreeSet::new();
    for instrument_id in &task.candidate_instrument_order {
        let Some(resource) = resources.get(instrument_id) else {
            reasons.insert(format!("{instrument_id}:unknown-instrument"));
            continue;
        };
        if !resource.enabled {
            reasons.insert(format!("{instrument_id}:disabled"));
            continue;
        }
        if !resource.model_system_order.contains(&task.model_system) {
            reasons.insert(format!("{instrument_id}:model-system-incompatible"));
            continue;
        }
        if !resource.operation_order.contains(&task.operation) {
            reasons.insert(format!("{instrument_id}:operation-incompatible"));
            continue;
        }
        let mut start = dependency_end
            .max(task.release_tick)
            .max(
                *resource_cursor
                    .get(instrument_id)
                    .unwrap_or(&resource.available_from_tick),
            )
            .max(resource.available_from_tick);
        let mut operator_slot = None;
        if task.requires_operator {
            if operator_cursor.is_empty() {
                reasons.insert(format!("{instrument_id}:operator-capacity-zero"));
                continue;
            }
            if let Some((slot, available)) = operator_cursor
                .iter()
                .enumerate()
                .min_by_key(|(_, tick)| **tick)
            {
                start = start.max(*available);
                operator_slot = Some(slot);
            }
        }
        let Some(end) = start.checked_add(task.duration_ticks) else {
            reasons.insert(format!("{instrument_id}:time-overflow"));
            continue;
        };
        if end > resource.available_until_tick {
            reasons.insert(format!("{instrument_id}:availability-window"));
            continue;
        }
        if end > resource.calibration_valid_until_tick {
            reasons.insert(format!("{instrument_id}:calibration-expired"));
            continue;
        }
        if task.deadline_tick.is_some_and(|deadline| end > deadline) {
            reasons.insert(format!("{instrument_id}:task-deadline"));
            continue;
        }
        if end > request.max_end_tick {
            reasons.insert(format!("{instrument_id}:mission-window"));
            continue;
        }
        let candidate = CandidateSlot {
            instrument_id: instrument_id.clone(),
            start,
            end,
            operator_slot,
        };
        let replace = best.as_ref().is_none_or(|current: &CandidateSlot| {
            (
                candidate.end,
                candidate.start,
                candidate.instrument_id.clone(),
            ) < (current.end, current.start, current.instrument_id.clone())
        });
        if replace {
            best = Some(candidate);
        }
    }
    (best, reasons.into_iter().collect())
}

fn critical_path(
    assignments: &BTreeMap<String, InstrumentFleetAssignment>,
    tasks: &BTreeMap<String, InstrumentFleetTask>,
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
            .max_by_key(|candidate| (candidate.scheduled_end_tick, candidate.task_id.clone()));
        let resource_predecessor = assignments
            .values()
            .filter(|candidate| {
                candidate.task_id != cursor
                    && candidate.instrument_id == assignment.instrument_id
                    && candidate.scheduled_end_tick <= assignment.scheduled_start_tick
            })
            .max_by_key(|candidate| (candidate.scheduled_end_tick, candidate.task_id.clone()));
        let operator_predecessor = assignment.operator_slot.and_then(|slot| {
            assignments
                .values()
                .filter(|candidate| {
                    candidate.task_id != cursor
                        && candidate.operator_slot == Some(slot)
                        && candidate.scheduled_end_tick <= assignment.scheduled_start_tick
                })
                .max_by_key(|candidate| (candidate.scheduled_end_tick, candidate.task_id.clone()))
        });
        let Some(previous) = [
            dependency_predecessor,
            resource_predecessor,
            operator_predecessor,
        ]
        .into_iter()
        .flatten()
        .max_by_key(|candidate| (candidate.scheduled_end_tick, candidate.task_id.clone())) else {
            break;
        };
        if previous.scheduled_end_tick > assignment.scheduled_start_tick {
            break;
        }
        cursor = previous.task_id.clone();
    }
    result.reverse();
    result
}

/// Schedule a bounded multi-instrument preclinical workflow. The returned schedule is a
/// deterministic handoff to per-instrument preflight; no hardware or material is touched here.
pub fn schedule_glioma_instrument_fleet(
    request: &InstrumentFleetScheduleRequest,
) -> Result<InstrumentFleetSchedule, InstrumentFleetSchedulerError> {
    validate_request(request)?;
    let (tasks, resources) = validate_graph(request)?;
    let mut remaining = tasks.keys().cloned().collect::<BTreeSet<_>>();
    let mut admitted = BTreeSet::new();
    let mut blocked = BTreeMap::<String, Vec<String>>::new();
    let mut assignments = BTreeMap::<String, InstrumentFleetAssignment>::new();
    let mut resource_cursor = resources
        .iter()
        .map(|(id, resource)| {
            (
                id.clone(),
                resource.available_from_tick.max(request.current_tick),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut operator_cursor = vec![request.current_tick; request.operator_capacity];
    let mut total_risk = 0_u32;
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    while !remaining.is_empty() {
        let ready = remaining
            .iter()
            .filter(|id| {
                let task = &tasks[*id];
                task.depends_on.iter().all(|dependency| {
                    admitted.contains(dependency) || blocked.contains_key(dependency)
                })
            })
            .cloned()
            .collect::<Vec<_>>();
        if ready.is_empty() {
            return Err(InstrumentFleetSchedulerError::InvalidGraph(
                "scheduler could not make progress through a validated dependency graph".into(),
            ));
        }
        let mut ready = ready;
        ready.sort_by(|left, right| {
            let left_priority = task_priority(&tasks[left]);
            let right_priority = task_priority(&tasks[right]);
            right_priority
                .0
                .cmp(&left_priority.0)
                .then_with(|| left_priority.1.cmp(&right_priority.1))
                .then_with(|| left.cmp(right))
        });
        for task_id in ready {
            remaining.remove(&task_id);
            let task = &tasks[&task_id];
            if task
                .depends_on
                .iter()
                .any(|dependency| blocked.contains_key(dependency))
            {
                blocked.insert(task_id.clone(), vec!["dependency-blocked".into()]);
                uncertainty.insert(format!("{task_id}:dependency-blocked"));
                continue;
            }
            if total_risk.saturating_add(u32::from(task.risk_milli))
                > request.maximum_total_risk_milli
            {
                blocked.insert(task_id.clone(), vec!["shared-risk-budget".into()]);
                negative.insert(format!("{task_id}:risk-budget"));
                continue;
            }
            let dependency_end = task
                .depends_on
                .iter()
                .filter_map(|dependency| assignments.get(dependency))
                .map(|assignment| assignment.scheduled_end_tick)
                .max()
                .unwrap_or(request.current_tick);
            let (slot, reasons) = choose_slot(
                task,
                request,
                &resources,
                &resource_cursor,
                &operator_cursor,
                dependency_end,
            );
            let Some(slot) = slot else {
                let reasons = if reasons.is_empty() {
                    vec!["no-compatible-resource".into()]
                } else {
                    reasons
                };
                for reason in &reasons {
                    negative.insert(format!("{task_id}:{reason}"));
                }
                blocked.insert(task_id.clone(), reasons);
                continue;
            };
            resource_cursor.insert(slot.instrument_id.clone(), slot.end);
            if let Some(operator_slot) = slot.operator_slot {
                operator_cursor[operator_slot] = slot.end;
            }
            total_risk = total_risk.saturating_add(u32::from(task.risk_milli));
            admitted.insert(task_id.clone());
            assignments.insert(
                task_id.clone(),
                InstrumentFleetAssignment {
                    task_id,
                    instrument_id: slot.instrument_id,
                    operator_slot: slot.operator_slot,
                    scheduled_start_tick: slot.start,
                    scheduled_end_tick: slot.end,
                    dependency_end_tick: dependency_end,
                },
            );
        }
    }
    let mut assignment_values = assignments.values().cloned().collect::<Vec<_>>();
    assignment_values.sort_by(|left, right| left.task_id.cmp(&right.task_id));
    let admitted_order = assignment_values
        .iter()
        .map(|assignment| assignment.task_id.clone())
        .collect::<Vec<_>>();
    let blocked_tasks = blocked
        .into_iter()
        .map(|(task_id, reasons)| InstrumentFleetBlockedTask { task_id, reasons })
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
    let mut utilization = resources
        .values()
        .map(|resource| {
            let resource_assignments = assignment_values
                .iter()
                .filter(|assignment| assignment.instrument_id == resource.instrument_id)
                .collect::<Vec<_>>();
            let busy_ticks = resource_assignments
                .iter()
                .map(|assignment| {
                    assignment
                        .scheduled_end_tick
                        .saturating_sub(assignment.scheduled_start_tick)
                })
                .sum::<u64>();
            let window = resource
                .available_until_tick
                .saturating_sub(resource.available_from_tick)
                .max(1);
            InstrumentFleetUtilization {
                instrument_id: resource.instrument_id.clone(),
                busy_ticks,
                first_start_tick: resource_assignments
                    .iter()
                    .map(|assignment| assignment.scheduled_start_tick)
                    .min(),
                last_end_tick: resource_assignments
                    .iter()
                    .map(|assignment| assignment.scheduled_end_tick)
                    .max(),
                utilization_milli: (busy_ticks.saturating_mul(1_000) / window).min(1_000) as u16,
            }
        })
        .collect::<Vec<_>>();
    utilization.sort_by(|left, right| left.instrument_id.cmp(&right.instrument_id));
    let critical_path_order = critical_path(&assignments, &tasks);
    let disposition = if blocked_order.is_empty() {
        InstrumentFleetDisposition::Ready
    } else if admitted_order.is_empty() {
        InstrumentFleetDisposition::Blocked
    } else {
        InstrumentFleetDisposition::Partial
    };
    if admitted_order.is_empty() {
        uncertainty.insert("no-task-admitted".into());
    }
    let mut output = InstrumentFleetSchedule {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        resource_order: resources.keys().cloned().collect(),
        task_order: tasks.keys().cloned().collect(),
        admitted_order,
        blocked_order,
        assignments: assignment_values,
        blocked_tasks,
        critical_path_order,
        utilization,
        total_risk_milli: total_risk,
        makespan_ticks: makespan,
        dispatch_permitted: false,
        negative_evidence: negative.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-instrument-fleet"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| InstrumentFleetSchedulerError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resource(id: &str, until: u64, calibration: u64) -> InstrumentFleetResource {
        InstrumentFleetResource {
            instrument_id: id.into(),
            model_system_order: vec![GliomaModelSystem::Organoid],
            operation_order: vec![
                InstrumentOperation::AcquireImage,
                InstrumentOperation::Sequence,
            ],
            available_from_tick: 0,
            available_until_tick: until,
            calibration_valid_until_tick: calibration,
            enabled: true,
        }
    }

    fn task(
        id: &str,
        operation: InstrumentOperation,
        depends_on: Vec<String>,
    ) -> InstrumentFleetTask {
        InstrumentFleetTask {
            task_id: id.into(),
            label: id.into(),
            operation,
            model_system: GliomaModelSystem::Organoid,
            candidate_instrument_order: vec!["scope-a".into(), "scope-b".into()],
            depends_on,
            release_tick: 0,
            deadline_tick: None,
            duration_ticks: 5,
            risk_milli: 100,
            information_milli: 500,
            requires_operator: false,
            output_schema: format!("{id}1@1"),
        }
    }

    fn request() -> InstrumentFleetScheduleRequest {
        InstrumentFleetScheduleRequest {
            objective: "coordinate organoid imaging and sequencing".into(),
            model_system: GliomaModelSystem::Organoid,
            current_tick: 0,
            max_end_tick: 30,
            maximum_total_risk_milli: 1_000,
            operator_capacity: 1,
            tasks: vec![
                task("image", InstrumentOperation::AcquireImage, Vec::new()),
                task("sequence", InstrumentOperation::Sequence, Vec::new()),
                task(
                    "integrate",
                    InstrumentOperation::AcquireImage,
                    vec!["image".into(), "sequence".into()],
                ),
            ],
            resources: vec![resource("scope-a", 30, 30), resource("scope-b", 30, 30)],
        }
    }

    #[test]
    fn fleet_scheduler_parallelizes_independent_work_and_replays() {
        let first = schedule_glioma_instrument_fleet(&request()).unwrap();
        let mut reversed = request();
        reversed.tasks.reverse();
        reversed.resources.reverse();
        let second = schedule_glioma_instrument_fleet(&reversed).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.disposition, InstrumentFleetDisposition::Ready);
        assert_eq!(first.assignments.len(), 3);
        assert!(first.critical_path_order.contains(&"integrate".into()));
        assert!(!first.dispatch_permitted);
        first.validate().unwrap();
    }

    #[test]
    fn fleet_scheduler_exposes_calibration_and_risk_blocks() {
        let mut request = request();
        request.maximum_total_risk_milli = 150;
        request.resources = vec![resource("scope-a", 30, 3), resource("scope-b", 30, 3)];
        let output = schedule_glioma_instrument_fleet(&request).unwrap();
        assert_eq!(output.disposition, InstrumentFleetDisposition::Blocked);
        assert!(
            output.blocked_order.contains(&"image".into())
                || output.blocked_order.contains(&"sequence".into())
        );
        assert!(output
            .negative_evidence
            .iter()
            .any(|item| item.contains("calibration") || item.contains("risk")));
    }

    #[test]
    fn dependency_failure_propagates_without_fake_execution() {
        let mut request = request();
        request.resources = vec![resource("scope-a", 4, 4), resource("scope-b", 4, 4)];
        let output = schedule_glioma_instrument_fleet(&request).unwrap();
        assert!(output.blocked_order.contains(&"integrate".into()));
        assert!(output
            .blocked_tasks
            .iter()
            .any(|blocked| blocked.task_id == "integrate"
                && blocked.reasons.contains(&"dependency-blocked".into())));
    }

    #[test]
    fn critical_path_includes_serialized_instrument_work() {
        let mut request = request();
        for task in &mut request.tasks {
            task.candidate_instrument_order = vec!["scope-a".into()];
        }
        request.resources = vec![resource("scope-a", 30, 30)];
        let output = schedule_glioma_instrument_fleet(&request).unwrap();
        assert_eq!(output.disposition, InstrumentFleetDisposition::Ready);
        assert_eq!(output.critical_path_order.len(), 3);
        assert_eq!(output.makespan_ticks, 15);
    }

    #[test]
    fn scope_and_capability_duplicates_are_refused() {
        let mut scoped_request = request();
        scoped_request.tasks[0].model_system = GliomaModelSystem::CellLine;
        assert!(matches!(
            schedule_glioma_instrument_fleet(&scoped_request),
            Err(InstrumentFleetSchedulerError::InvalidGraph(_))
        ));

        let mut invalid_request = request();
        invalid_request.resources[0]
            .operation_order
            .push(InstrumentOperation::Sequence);
        assert!(matches!(
            schedule_glioma_instrument_fleet(&invalid_request),
            Err(InstrumentFleetSchedulerError::InvalidGraph(_))
        ));
    }
}

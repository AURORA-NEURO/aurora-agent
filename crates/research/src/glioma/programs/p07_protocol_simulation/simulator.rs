//! Deterministic resource-constrained protocol simulation for preclinical glioma campaigns.
//!
//! This is a scheduling product, not a biological simulator.  It compiles a declared protocol
//! into a reproducible discrete-event schedule, exposes the critical path and resource pressure,
//! and fails closed on capacity, risk, and instrument-approval gates.  A local host may use the
//! result to prepare work for its own executor; this module never touches a specimen or dispatches
//! an instrument.

use crate::glioma::experiment::{ExperimentDesign, ExperimentDisposition};
use crate::glioma_engine::GliomaModelSystem;
use bioprism_foundation::PRECLINICAL_BOUNDARY;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P07-F02";
pub const OUTPUT_SCHEMA: &str = "GliomaProtocolSimulation1@1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolResourceKind {
    Culture,
    AnimalFacility,
    Imaging,
    Sequencing,
    Compute,
    Robotics,
    Storage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolResource {
    pub resource_id: String,
    pub kind: ProtocolResourceKind,
    pub capacity_units: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolTask {
    pub task_id: String,
    pub label: String,
    pub resource_kind: ProtocolResourceKind,
    pub resource_units: u16,
    pub duration_ticks: u32,
    pub depends_on: Vec<String>,
    pub model_system: GliomaModelSystem,
    pub output_schema: String,
    pub risk_milli: u16,
    pub requires_instrument: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolSimulationRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub tasks: Vec<ProtocolTask>,
    pub resources: Vec<ProtocolResource>,
    pub max_ticks: u32,
    pub max_risk_milli: u16,
    pub allow_instrument_execution: bool,
    pub approval_reference: Option<String>,
    pub randomization_seed: ContentHash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolDisposition {
    Feasible,
    CapacityBlocked,
    RiskBlocked,
    ApprovalRequired,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduleEntry {
    pub task_id: String,
    pub resource_id: String,
    pub start_tick: u32,
    pub finish_tick: u32,
    pub dependency_finish_tick: u32,
    pub risk_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceUtilization {
    pub resource_id: String,
    pub busy_unit_ticks: u64,
    pub utilization_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolSimulation {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub topological_order: Vec<String>,
    pub schedule: Vec<ScheduleEntry>,
    pub critical_path_order: Vec<String>,
    pub unscheduled_order: Vec<String>,
    pub makespan_ticks: u32,
    pub risk_total_milli: u32,
    pub resource_utilization: Vec<ResourceUtilization>,
    pub disposition: ProtocolDisposition,
    pub acceptance_gate: String,
    pub stop_conditions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub boundary: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProtocolSimulationError {
    #[error("protocol simulation request is invalid: {0}")]
    InvalidRequest(String),
    #[error("protocol task is invalid: {0}")]
    InvalidTask(String),
    #[error("protocol resource is invalid: {0}")]
    InvalidResource(String),
    #[error("protocol simulation output is invalid: {0}")]
    InvalidOutput(String),
    #[error("protocol simulation digest failed: {0}")]
    Digest(String),
}

fn digest_input(output: &ProtocolSimulation) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "model_system": output.model_system,
        "topological_order": output.topological_order,
        "schedule": output.schedule,
        "critical_path_order": output.critical_path_order,
        "unscheduled_order": output.unscheduled_order,
        "makespan_ticks": output.makespan_ticks,
        "risk_total_milli": output.risk_total_milli,
        "resource_utilization": output.resource_utilization,
        "disposition": output.disposition,
        "acceptance_gate": output.acceptance_gate,
        "stop_conditions": output.stop_conditions,
        "uncertainty": output.uncertainty,
        "negative_evidence": output.negative_evidence,
        "boundary": output.boundary,
    })
}

impl ProtocolSimulation {
    pub fn validate(&self) -> Result<(), ProtocolSimulationError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.topological_order.is_empty()
            || self.acceptance_gate.trim().is_empty()
            || self.stop_conditions.is_empty()
            || self
                .stop_conditions
                .iter()
                .any(|item| item.trim().is_empty())
            || self.uncertainty.iter().any(|item| item.trim().is_empty())
            || self
                .negative_evidence
                .iter()
                .any(|item| item.trim().is_empty())
            || self
                .topological_order
                .windows(2)
                .any(|pair| pair[0] == pair[1])
            || self.schedule.windows(2).any(|pair| {
                (pair[0].start_tick, &pair[0].task_id) > (pair[1].start_tick, &pair[1].task_id)
            })
            || self
                .unscheduled_order
                .windows(2)
                .any(|pair| pair[0] > pair[1])
            || self
                .resource_utilization
                .windows(2)
                .any(|pair| pair[0].resource_id > pair[1].resource_id)
        {
            return Err(ProtocolSimulationError::InvalidOutput(
                "identity, ordering, schedule, or limitation fields are invalid".into(),
            ));
        }
        if self.schedule.iter().any(|entry| {
            entry.task_id.trim().is_empty()
                || entry.resource_id.trim().is_empty()
                || entry.start_tick >= entry.finish_tick
                || entry.dependency_finish_tick > entry.start_tick
                || entry.risk_milli > 1_000
        }) {
            return Err(ProtocolSimulationError::InvalidOutput(
                "schedule entries contain invalid bounds or identifiers".into(),
            ));
        }
        if self
            .resource_utilization
            .iter()
            .any(|item| item.resource_id.trim().is_empty() || item.utilization_milli > 1_000)
        {
            return Err(ProtocolSimulationError::InvalidOutput(
                "resource utilization contains invalid bounds".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ProtocolSimulationError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ProtocolSimulationError::InvalidOutput(
                "digest is not bound to the simulation output".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(request: &ProtocolSimulationRequest) -> Result<(), ProtocolSimulationError> {
    if request.objective.trim().is_empty()
        || request.tasks.is_empty()
        || request.resources.is_empty()
        || request.max_ticks == 0
        || request.max_risk_milli > 1_000
        || request.randomization_seed.as_str().len() != 64
        || request
            .approval_reference
            .as_ref()
            .is_some_and(|value| value.trim().is_empty())
    {
        return Err(ProtocolSimulationError::InvalidRequest(
            "objective, tasks, resources, horizon, risk bound, seed, or approval is invalid".into(),
        ));
    }
    let mut task_ids = BTreeSet::new();
    for task in &request.tasks {
        if task.task_id.trim().is_empty()
            || task.label.trim().is_empty()
            || task.output_schema.trim().is_empty()
            || task.resource_units == 0
            || task.duration_ticks == 0
            || task.risk_milli > 1_000
            || task.model_system != request.model_system
            || !task_ids.insert(task.task_id.clone())
            || task.depends_on.windows(2).any(|pair| pair[0] > pair[1])
            || task
                .depends_on
                .iter()
                .any(|dependency| dependency == &task.task_id)
        {
            return Err(ProtocolSimulationError::InvalidTask(
                "task identity, model binding, dimensions, risk, ordering, or uniqueness is invalid".into(),
            ));
        }
    }
    let mut resource_ids = BTreeSet::new();
    for resource in &request.resources {
        if resource.resource_id.trim().is_empty()
            || resource.capacity_units == 0
            || !resource_ids.insert(resource.resource_id.clone())
        {
            return Err(ProtocolSimulationError::InvalidResource(
                "resource identity, capacity, or uniqueness is invalid".into(),
            ));
        }
    }
    if request.tasks.iter().any(|task| {
        !request.resources.iter().any(|resource| {
            resource.kind == task.resource_kind && resource.capacity_units >= task.resource_units
        })
    }) {
        return Err(ProtocolSimulationError::InvalidResource(
            "every task must have a resource with sufficient capacity".into(),
        ));
    }
    Ok(())
}

fn topological_order(
    tasks: &BTreeMap<String, ProtocolTask>,
) -> Result<Vec<String>, ProtocolSimulationError> {
    let mut indegree = tasks
        .iter()
        .map(|(id, task)| (id.clone(), task.depends_on.len()))
        .collect::<BTreeMap<_, _>>();
    let mut successors = BTreeMap::<String, Vec<String>>::new();
    for (id, task) in tasks {
        for dependency in &task.depends_on {
            if !tasks.contains_key(dependency) {
                return Err(ProtocolSimulationError::InvalidTask(format!(
                    "task {id} depends on unknown task {dependency}"
                )));
            }
            successors
                .entry(dependency.clone())
                .or_default()
                .push(id.clone());
        }
    }
    for children in successors.values_mut() {
        children.sort();
    }
    let mut ready = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(id, _)| id.clone())
        .collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(tasks.len());
    while let Some(id) = ready.pop_first() {
        order.push(id.clone());
        for child in successors.get(&id).into_iter().flatten() {
            let degree = indegree
                .get_mut(child)
                .expect("successor was built from the task map");
            *degree -= 1;
            if *degree == 0 {
                ready.insert(child.clone());
            }
        }
    }
    if order.len() != tasks.len() {
        return Err(ProtocolSimulationError::InvalidTask(
            "protocol dependency graph contains a cycle".into(),
        ));
    }
    Ok(order)
}

fn earliest_slot(
    intervals: &[(u32, u32, u16)],
    capacity: u16,
    units: u16,
    dependency_finish: u32,
    duration: u32,
    max_ticks: u32,
) -> Option<u32> {
    if duration > max_ticks || units > capacity {
        return None;
    }
    let latest = max_ticks - duration;
    for start in dependency_finish..=latest {
        let finish = start + duration;
        let overlap_units = intervals
            .iter()
            .filter(|(other_start, other_finish, _)| *other_start < finish && start < *other_finish)
            .map(|(_, _, occupied)| *occupied as u32)
            .sum::<u32>();
        if overlap_units + units as u32 <= capacity as u32 {
            return Some(start);
        }
    }
    None
}

fn downstream_weights(
    order: &[String],
    tasks: &BTreeMap<String, ProtocolTask>,
) -> BTreeMap<String, u32> {
    let mut children = BTreeMap::<String, Vec<String>>::new();
    for (id, task) in tasks {
        for dependency in &task.depends_on {
            children
                .entry(dependency.clone())
                .or_default()
                .push(id.clone());
        }
    }
    let mut weights = BTreeMap::new();
    for id in order.iter().rev() {
        let child_weight = children
            .get(id)
            .into_iter()
            .flatten()
            .filter_map(|child| weights.get(child))
            .copied()
            .max()
            .unwrap_or(0);
        weights.insert(id.clone(), tasks[id].duration_ticks + child_weight);
    }
    weights
}

fn critical_path(
    schedule: &BTreeMap<String, ScheduleEntry>,
    tasks: &BTreeMap<String, ProtocolTask>,
) -> Vec<String> {
    let Some((last_id, _)) = schedule
        .iter()
        .max_by(|(left_id, left), (right_id, right)| {
            left.finish_tick
                .cmp(&right.finish_tick)
                .then_with(|| right_id.cmp(left_id))
        })
    else {
        return Vec::new();
    };
    let mut path = vec![last_id.clone()];
    let mut cursor = last_id.clone();
    while let Some(parent) = tasks[&cursor]
        .depends_on
        .iter()
        .filter(|dependency| schedule.contains_key(*dependency))
        .max_by(|left, right| {
            schedule[*left]
                .finish_tick
                .cmp(&schedule[*right].finish_tick)
                .then_with(|| right.cmp(left))
        })
    {
        cursor = parent.clone();
        path.push(cursor.clone());
    }
    path.reverse();
    path
}

/// Simulate a declared protocol with deterministic critical-path-first list scheduling.
pub fn simulate_glioma_protocol(
    request: &ProtocolSimulationRequest,
) -> Result<ProtocolSimulation, ProtocolSimulationError> {
    validate_request(request)?;
    let tasks = request
        .tasks
        .iter()
        .cloned()
        .map(|task| (task.task_id.clone(), task))
        .collect::<BTreeMap<_, _>>();
    let order = topological_order(&tasks)?;
    let weights = downstream_weights(&order, &tasks);
    let resources = request
        .resources
        .iter()
        .cloned()
        .map(|resource| {
            (
                resource.resource_id.clone(),
                (resource, Vec::<(u32, u32, u16)>::new()),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut resources = resources;
    let mut scheduled = BTreeMap::<String, ScheduleEntry>::new();
    let mut remaining = tasks.keys().cloned().collect::<BTreeSet<_>>();
    let mut disposition = ProtocolDisposition::Feasible;
    let mut unscheduled = BTreeSet::new();
    let mut risk_total = 0_u32;
    let mut stop_conditions = BTreeSet::from([
        "stop before any effect when a declared dependency, resource, risk, or approval gate is unresolved".to_string(),
        "a schedule is not evidence of biological efficacy or assay validity".to_string(),
    ]);
    while !remaining.is_empty() {
        let mut ready = remaining
            .iter()
            .filter(|id| {
                tasks[*id]
                    .depends_on
                    .iter()
                    .all(|dependency| scheduled.contains_key(dependency))
            })
            .cloned()
            .collect::<Vec<_>>();
        if ready.is_empty() {
            disposition = ProtocolDisposition::Unresolved;
            unscheduled.extend(remaining.iter().cloned());
            stop_conditions.insert("dependency frontier became unresolved".into());
            break;
        }
        ready.sort_by(|left, right| {
            weights[right]
                .cmp(&weights[left])
                .then_with(|| left.cmp(right))
        });
        let task_id = ready[0].clone();
        let task = &tasks[&task_id];
        if task.requires_instrument
            && (!request.allow_instrument_execution || request.approval_reference.is_none())
        {
            disposition = ProtocolDisposition::ApprovalRequired;
            unscheduled.extend(remaining.iter().cloned());
            stop_conditions.insert(format!("{task_id}:instrument-approval-required"));
            break;
        }
        if risk_total + task.risk_milli as u32 > request.max_risk_milli as u32 {
            disposition = ProtocolDisposition::RiskBlocked;
            unscheduled.extend(remaining.iter().cloned());
            stop_conditions.insert(format!("{task_id}:risk-budget-exhausted"));
            break;
        }
        let dependency_finish = task
            .depends_on
            .iter()
            .filter_map(|dependency| scheduled.get(dependency))
            .map(|entry| entry.finish_tick)
            .max()
            .unwrap_or(0);
        let candidate = resources
            .iter()
            .filter(|(_, (resource, _))| resource.kind == task.resource_kind)
            .filter_map(|(resource_id, (resource, intervals))| {
                earliest_slot(
                    intervals,
                    resource.capacity_units,
                    task.resource_units,
                    dependency_finish,
                    task.duration_ticks,
                    request.max_ticks,
                )
                .map(|start| (start, resource_id.clone()))
            })
            .min_by(|left, right| left.cmp(right));
        let Some((start, resource_id)) = candidate else {
            disposition = ProtocolDisposition::CapacityBlocked;
            unscheduled.extend(remaining.iter().cloned());
            stop_conditions.insert(format!("{task_id}:capacity-or-horizon-exhausted"));
            break;
        };
        let finish = start + task.duration_ticks;
        resources
            .get_mut(&resource_id)
            .expect("candidate was selected from the resource map")
            .1
            .push((start, finish, task.resource_units));
        scheduled.insert(
            task_id.clone(),
            ScheduleEntry {
                task_id: task_id.clone(),
                resource_id,
                start_tick: start,
                finish_tick: finish,
                dependency_finish_tick: dependency_finish,
                risk_milli: task.risk_milli,
            },
        );
        risk_total += task.risk_milli as u32;
        remaining.remove(&task_id);
    }
    if disposition == ProtocolDisposition::Feasible && scheduled.len() != tasks.len() {
        disposition = ProtocolDisposition::Unresolved;
        unscheduled.extend(
            tasks
                .keys()
                .filter(|id| !scheduled.contains_key(*id))
                .cloned(),
        );
    }
    if !unscheduled.is_empty() {
        stop_conditions.insert("unscheduled work is not silently treated as completed".into());
    }
    let mut schedule = scheduled.values().cloned().collect::<Vec<_>>();
    schedule.sort_by(|left, right| {
        (left.start_tick, &left.task_id).cmp(&(right.start_tick, &right.task_id))
    });
    let makespan = schedule
        .iter()
        .map(|entry| entry.finish_tick)
        .max()
        .unwrap_or(0);
    let critical_path_order = critical_path(&scheduled, &tasks);
    let resource_utilization = resources
        .iter()
        .map(|(resource_id, (resource, intervals))| {
            let busy_unit_ticks = intervals
                .iter()
                .map(|(start, finish, units)| (*finish as u64 - *start as u64) * *units as u64)
                .sum::<u64>();
            let utilization_milli = if makespan == 0 {
                0
            } else {
                ((busy_unit_ticks * 1_000) / (makespan as u64 * resource.capacity_units as u64))
                    .min(1_000) as u16
            };
            ResourceUtilization {
                resource_id: resource_id.clone(),
                busy_unit_ticks,
                utilization_milli,
            }
        })
        .collect::<Vec<_>>();
    let disposition_note = match disposition {
        ProtocolDisposition::Feasible => {
            "all declared work is scheduled within the horizon and risk budget"
        }
        ProtocolDisposition::CapacityBlocked => {
            "capacity or horizon is insufficient; request a larger local resource envelope"
        }
        ProtocolDisposition::RiskBlocked => {
            "risk budget is exhausted; do not continue without an explicit bounded review"
        }
        ProtocolDisposition::ApprovalRequired => {
            "instrument work is withheld until local authorization is present"
        }
        ProtocolDisposition::Unresolved => {
            "dependency closure is incomplete; no scientific conclusion is emitted"
        }
    };
    let mut output = ProtocolSimulation {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        topological_order: order,
        schedule,
        critical_path_order,
        unscheduled_order: unscheduled.into_iter().collect(),
        makespan_ticks: makespan,
        risk_total_milli: risk_total,
        resource_utilization,
        disposition,
        acceptance_gate: disposition_note.into(),
        stop_conditions: stop_conditions.into_iter().collect(),
        uncertainty: vec![
            "simulation-only; no biological observation or assay validity claim is produced".into(),
            "durations, capacity, and risk are caller-declared planning assumptions".into(),
        ],
        negative_evidence: Vec::new(),
        boundary: PRECLINICAL_BOUNDARY.into(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-protocol-simulation"),
    };
    if !output.unscheduled_order.is_empty() {
        output.negative_evidence = output
            .unscheduled_order
            .iter()
            .map(|task_id| format!("{task_id}:not-scheduled"))
            .collect();
    }
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ProtocolSimulationError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

/// Expand a power-aware experiment design into a useful default protocol campaign.  The caller
/// can edit the returned typed request before simulation to reflect local instruments and timing.
pub fn protocol_request_from_experiment_design(
    design: &ExperimentDesign,
    randomization_seed: ContentHash,
    max_ticks: u32,
    max_risk_milli: u16,
) -> Result<ProtocolSimulationRequest, ProtocolSimulationError> {
    design
        .validate()
        .map_err(|error| ProtocolSimulationError::InvalidRequest(error.to_string()))?;
    if max_ticks == 0 || max_risk_milli > 1_000 || randomization_seed.as_str().len() != 64 {
        return Err(ProtocolSimulationError::InvalidRequest(
            "protocol horizon, risk bound, or seed is invalid".into(),
        ));
    }
    let primary_kind = match design.model_system {
        GliomaModelSystem::InSilico => ProtocolResourceKind::Compute,
        GliomaModelSystem::MouseModel
        | GliomaModelSystem::ZebrafishModel
        | GliomaModelSystem::PatientDerivedXenograft => ProtocolResourceKind::AnimalFacility,
        GliomaModelSystem::CellLine | GliomaModelSystem::Organoid => ProtocolResourceKind::Culture,
    };
    let mut tasks = vec![ProtocolTask {
        task_id: "setup".into(),
        label: "prepare declared preclinical model and controls".into(),
        resource_kind: primary_kind,
        resource_units: 1,
        duration_ticks: 2,
        depends_on: Vec::new(),
        model_system: design.model_system,
        output_schema: "GliomaProtocolSetup1@1".into(),
        risk_milli: 20,
        requires_instrument: false,
    }];
    let mut qc_ids = Vec::new();
    for allocation in &design.allocations {
        let assay_id = format!("assay:{}", allocation.arm_id);
        tasks.push(ProtocolTask {
            task_id: assay_id.clone(),
            label: format!("execute declared assay arm {}", allocation.arm_id),
            resource_kind: primary_kind,
            resource_units: 1,
            duration_ticks: allocation.planned_replicates as u32,
            depends_on: vec!["setup".into()],
            model_system: design.model_system,
            output_schema: "GliomaProtocolAssay1@1".into(),
            risk_milli: 25_u16.saturating_add(allocation.planned_replicates.min(475) * 2),
            requires_instrument: false,
        });
        let qc_id = format!("qc:{}", allocation.arm_id);
        qc_ids.push(qc_id.clone());
        tasks.push(ProtocolTask {
            task_id: qc_id,
            label: format!("quality-control arm {}", allocation.arm_id),
            resource_kind: ProtocolResourceKind::Imaging,
            resource_units: 1,
            duration_ticks: 2,
            depends_on: vec![assay_id],
            model_system: design.model_system,
            output_schema: "GliomaProtocolQc1@1".into(),
            risk_milli: 15,
            requires_instrument: false,
        });
    }
    qc_ids.sort();
    tasks.push(ProtocolTask {
        task_id: "analysis".into(),
        label: "prepare analysis-ready protocol output".into(),
        resource_kind: ProtocolResourceKind::Compute,
        resource_units: 1,
        duration_ticks: 4,
        depends_on: qc_ids,
        model_system: design.model_system,
        output_schema: "GliomaProtocolAnalysisInput1@1".into(),
        risk_milli: 10,
        requires_instrument: false,
    });
    let resources = vec![
        ProtocolResource {
            resource_id: "primary-model-facility".into(),
            kind: primary_kind,
            capacity_units: if matches!(primary_kind, ProtocolResourceKind::AnimalFacility) {
                1
            } else {
                2
            },
        },
        ProtocolResource {
            resource_id: "imaging-core".into(),
            kind: ProtocolResourceKind::Imaging,
            capacity_units: 1,
        },
        ProtocolResource {
            resource_id: "analysis-cluster".into(),
            kind: ProtocolResourceKind::Compute,
            capacity_units: 4,
        },
    ];
    let note = if design.disposition == ExperimentDisposition::Ready {
        "source design is power-ready; simulation still does not validate assay biology"
    } else {
        "source design is underpowered or blocked; simulation is for repair planning only"
    };
    Ok(ProtocolSimulationRequest {
        objective: format!("{} ({note})", design.objective),
        model_system: design.model_system,
        tasks,
        resources,
        max_ticks,
        max_risk_milli,
        allow_instrument_execution: false,
        approval_reference: None,
        randomization_seed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::experiment::{
        design_preclinical_experiment, ExperimentArm, ExperimentRequest, OutcomeKind,
    };

    fn request() -> ProtocolSimulationRequest {
        ProtocolSimulationRequest {
            objective: "schedule organoid invasion campaign".into(),
            model_system: GliomaModelSystem::Organoid,
            tasks: vec![
                ProtocolTask {
                    task_id: "prepare".into(),
                    label: "prepare".into(),
                    resource_kind: ProtocolResourceKind::Culture,
                    resource_units: 1,
                    duration_ticks: 2,
                    depends_on: Vec::new(),
                    model_system: GliomaModelSystem::Organoid,
                    output_schema: "Setup1@1".into(),
                    risk_milli: 10,
                    requires_instrument: false,
                },
                ProtocolTask {
                    task_id: "assay-a".into(),
                    label: "assay a".into(),
                    resource_kind: ProtocolResourceKind::Culture,
                    resource_units: 1,
                    duration_ticks: 4,
                    depends_on: vec!["prepare".into()],
                    model_system: GliomaModelSystem::Organoid,
                    output_schema: "Assay1@1".into(),
                    risk_milli: 20,
                    requires_instrument: false,
                },
                ProtocolTask {
                    task_id: "assay-b".into(),
                    label: "assay b".into(),
                    resource_kind: ProtocolResourceKind::Culture,
                    resource_units: 1,
                    duration_ticks: 4,
                    depends_on: vec!["prepare".into()],
                    model_system: GliomaModelSystem::Organoid,
                    output_schema: "Assay1@1".into(),
                    risk_milli: 20,
                    requires_instrument: false,
                },
            ],
            resources: vec![ProtocolResource {
                resource_id: "culture".into(),
                kind: ProtocolResourceKind::Culture,
                capacity_units: 2,
            }],
            max_ticks: 20,
            max_risk_milli: 100,
            allow_instrument_execution: false,
            approval_reference: None,
            randomization_seed: ContentHash::of_bytes(b"protocol-seed"),
        }
    }

    #[test]
    fn scheduler_parallelizes_independent_assays_and_is_replay_stable() {
        let first = simulate_glioma_protocol(&request()).unwrap();
        let second = simulate_glioma_protocol(&request()).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.disposition, ProtocolDisposition::Feasible);
        assert_eq!(first.schedule.len(), 3);
        assert_eq!(first.schedule[1].start_tick, first.schedule[2].start_tick);
        assert_eq!(first.critical_path_order, vec!["prepare", "assay-a"]);
        first.validate().unwrap();
    }

    #[test]
    fn risk_and_capacity_stops_are_honest_and_never_claim_completion() {
        let mut risk = request();
        risk.max_risk_milli = 20;
        let output = simulate_glioma_protocol(&risk).unwrap();
        assert_eq!(output.disposition, ProtocolDisposition::RiskBlocked);
        assert!(!output.unscheduled_order.is_empty());
        assert!(!output.negative_evidence.is_empty());

        let mut capacity = request();
        capacity.resources[0].capacity_units = 1;
        capacity.max_ticks = 7;
        let output = simulate_glioma_protocol(&capacity).unwrap();
        assert_eq!(output.disposition, ProtocolDisposition::CapacityBlocked);
        assert!(!output.unscheduled_order.is_empty());
    }

    #[test]
    fn design_compiler_emits_typed_setup_assay_qc_and_analysis_tasks() {
        let design = design_preclinical_experiment(
            &ExperimentRequest {
                objective: "organoid invasion".into(),
                model_system: GliomaModelSystem::Organoid,
                outcome: OutcomeKind::Continuous,
                alpha_milli: 50,
                target_power_milli: 800,
                standardized_effect_milli: 500,
                variance_milli: 1_000,
                dropout_milli: 100,
                max_replicates_per_arm: 20,
                blocking_factors: Vec::new(),
                randomization_seed: ContentHash::of_bytes(b"design-seed"),
                release_null_result: true,
            },
            &[
                ExperimentArm {
                    arm_id: "control".into(),
                    model_system: GliomaModelSystem::Organoid,
                    condition: "vehicle".into(),
                },
                ExperimentArm {
                    arm_id: "perturbed".into(),
                    model_system: GliomaModelSystem::Organoid,
                    condition: "knockdown".into(),
                },
            ],
        )
        .unwrap();
        let request = protocol_request_from_experiment_design(
            &design,
            ContentHash::of_bytes(b"protocol-seed"),
            100,
            1_000,
        )
        .unwrap();
        assert!(request.tasks.iter().any(|task| task.task_id == "analysis"));
        let output = simulate_glioma_protocol(&request).unwrap();
        assert_eq!(output.model_system, GliomaModelSystem::Organoid);
    }

    #[test]
    fn instrument_task_requires_approval_before_scheduling() {
        let mut request = request();
        request.tasks[0].requires_instrument = true;
        let output = simulate_glioma_protocol(&request).unwrap();
        assert_eq!(output.disposition, ProtocolDisposition::ApprovalRequired);
        assert!(output
            .stop_conditions
            .iter()
            .any(|item| item.contains("instrument-approval-required")));
    }
}

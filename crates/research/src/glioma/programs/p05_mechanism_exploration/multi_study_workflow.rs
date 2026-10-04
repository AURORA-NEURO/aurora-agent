//! Multimodal, multi-study mechanism workflow compilation for preclinical glioma research.
//!
//! A local mechanism workflow is not enough when a result must survive model, modality, and
//! site variation. This feature joins institution-local P05 workflow plans into an aggregate
//! portfolio, computes study/site/modality coverage and transportability pressure, and emits a
//! deterministic set of local tasks. Raw data never leaves a lane; only typed workflow metadata
//! and aggregate coverage are represented here.

use super::mechanism_workflow::{MechanismWorkflowNodeStatus, MechanismWorkflowPlan};
use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P05-F14";
pub const OUTPUT_SCHEMA: &str = "GliomaMechanismMultiStudyWorkflow1@1";
pub const MAX_LANES: usize = 256;
pub const MAX_TASKS: usize = 4_096;
pub const MAX_WAVES: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanismFederationMode {
    LocalOnly,
    AggregateMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismStudyLane {
    pub lane_id: String,
    pub study_id: String,
    pub site_id: String,
    pub model_system: GliomaModelSystem,
    pub modalities: Vec<GliomaModality>,
    pub workflow: MechanismWorkflowPlan,
    pub transport_penalty_milli: u16,
    pub replication_weight_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismMultiStudyWorkflowRequest {
    pub objective: String,
    pub lanes: Vec<MechanismStudyLane>,
    pub required_modalities: Vec<GliomaModality>,
    pub minimum_studies_per_action: usize,
    pub minimum_sites_per_action: usize,
    pub minimum_transport_score_milli: u16,
    pub budget_units: u64,
    pub max_tasks: usize,
    pub max_waves: usize,
    pub federation_mode: MechanismFederationMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismMultiStudyTask {
    pub task_id: String,
    pub lane_id: String,
    pub study_id: String,
    pub site_id: String,
    pub action_id: String,
    pub mechanism_id: String,
    pub model_system: GliomaModelSystem,
    pub modality: GliomaModality,
    pub dependency_order: Vec<String>,
    pub cost_units: u64,
    pub priority_milli: u16,
    pub transport_score_milli: u16,
    pub status: MechanismMultiStudyTaskStatus,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanismMultiStudyTaskStatus {
    Scheduled,
    Deferred,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismMultiStudyActionGroup {
    pub action_id: String,
    pub mechanism_id: String,
    pub task_order: Vec<String>,
    pub study_order: Vec<String>,
    pub site_order: Vec<String>,
    pub modality_order: Vec<GliomaModality>,
    pub supporting_study_count: usize,
    pub independent_site_count: usize,
    pub transport_score_milli: u16,
    pub priority_milli: u16,
    pub required_modality_complete: bool,
    pub replication_complete: bool,
    pub selected: bool,
    pub block_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanismMultiStudyWorkflowDisposition {
    Ready,
    Partial,
    CoverageBlocked,
    BudgetBlocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismMultiStudyWorkflowPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub lane_order: Vec<String>,
    pub group_order: Vec<String>,
    pub task_order: Vec<String>,
    pub topological_task_order: Vec<String>,
    pub execution_waves: Vec<Vec<String>>,
    pub tasks: Vec<MechanismMultiStudyTask>,
    pub groups: Vec<MechanismMultiStudyActionGroup>,
    pub scheduled_task_order: Vec<String>,
    pub deferred_task_order: Vec<String>,
    pub blocked_task_order: Vec<String>,
    pub federation_mode: MechanismFederationMode,
    pub total_cost_units: u64,
    pub budget_units: u64,
    pub budget_remaining_units: u64,
    pub critical_path_units: u64,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: MechanismMultiStudyWorkflowDisposition,
    pub next_route: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MechanismMultiStudyWorkflowError {
    #[error("multi-study mechanism workflow request is invalid: {0}")]
    InvalidRequest(String),
    #[error("multi-study mechanism workflow input is invalid: {0}")]
    InvalidInput(String),
    #[error("multi-study mechanism workflow output is invalid: {0}")]
    InvalidOutput(String),
    #[error("multi-study mechanism workflow digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique_nonempty(values: &[String]) -> bool {
    values.iter().all(|value| !value.trim().is_empty())
        && values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

pub(crate) fn digest_input(output: &MechanismMultiStudyWorkflowPlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "lane_order": output.lane_order,
        "group_order": output.group_order,
        "task_order": output.task_order,
        "topological_task_order": output.topological_task_order,
        "execution_waves": output.execution_waves,
        "tasks": output.tasks,
        "groups": output.groups,
        "scheduled_task_order": output.scheduled_task_order,
        "deferred_task_order": output.deferred_task_order,
        "blocked_task_order": output.blocked_task_order,
        "federation_mode": output.federation_mode,
        "total_cost_units": output.total_cost_units,
        "budget_units": output.budget_units,
        "budget_remaining_units": output.budget_remaining_units,
        "critical_path_units": output.critical_path_units,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
        "next_route": output.next_route,
    })
}

fn validate_lane(
    lane: &MechanismStudyLane,
    objective: &str,
) -> Result<(), MechanismMultiStudyWorkflowError> {
    if lane.lane_id.trim().is_empty()
        || lane.study_id.trim().is_empty()
        || lane.site_id.trim().is_empty()
        || lane.lane_id.contains("::")
        || lane.modalities.is_empty()
        || !canonical(&lane.modalities)
        || lane.modalities.windows(2).any(|pair| pair[0] == pair[1])
        || lane.transport_penalty_milli > 1_000
        || lane.replication_weight_milli > 1_000
        || lane.workflow.objective != objective
    {
        return Err(MechanismMultiStudyWorkflowError::InvalidInput(format!(
            "lane {} has invalid identity, modality, transport, replication, or objective binding",
            lane.lane_id
        )));
    }
    lane.workflow
        .validate()
        .map_err(|error| MechanismMultiStudyWorkflowError::InvalidInput(error.to_string()))?;
    if lane.workflow.nodes.iter().any(|node| {
        node.status == MechanismWorkflowNodeStatus::Scheduled
            && (node.action.model_system != lane.model_system
                || !lane.modalities.contains(&node.action.modality))
    }) {
        return Err(MechanismMultiStudyWorkflowError::InvalidInput(format!(
            "lane {} schedules an action outside its declared model-system or modality capability",
            lane.lane_id
        )));
    }
    Ok(())
}

fn validate_request(
    request: &MechanismMultiStudyWorkflowRequest,
) -> Result<(), MechanismMultiStudyWorkflowError> {
    if request.objective.trim().is_empty()
        || request.lanes.is_empty()
        || request.lanes.len() > MAX_LANES
        || request
            .required_modalities
            .windows(2)
            .any(|pair| pair[0] == pair[1])
        || !canonical(&request.required_modalities)
        || request.minimum_studies_per_action == 0
        || request.minimum_studies_per_action > MAX_LANES
        || request.minimum_sites_per_action == 0
        || request.minimum_sites_per_action > MAX_LANES
        || request.minimum_transport_score_milli > 1_000
        || request.budget_units == 0
        || request.max_tasks == 0
        || request.max_tasks > MAX_TASKS
        || request.max_waves == 0
        || request.max_waves > MAX_WAVES
    {
        return Err(MechanismMultiStudyWorkflowError::InvalidRequest(
            "objective, bounded lanes/tasks/waves, canonical modality requirements, coverage floors, and positive budget are required".into(),
        ));
    }
    let mut lane_ids = BTreeSet::new();
    for lane in &request.lanes {
        validate_lane(lane, &request.objective)?;
        if !lane_ids.insert(lane.lane_id.clone()) {
            return Err(MechanismMultiStudyWorkflowError::InvalidInput(
                "lane ids must be unique".into(),
            ));
        }
    }
    Ok(())
}

fn task_key(lane_id: &str, action_id: &str) -> String {
    format!("{lane_id}::{action_id}")
}

fn transport_score(lane: &MechanismStudyLane) -> u16 {
    let base = u32::from(1_000_u16.saturating_sub(lane.transport_penalty_milli));
    ((base + u32::from(lane.replication_weight_milli)) / 2).min(1_000) as u16
}

fn topological_tasks(
    task_ids: &BTreeSet<String>,
    dependencies: &BTreeMap<String, Vec<String>>,
) -> Result<Vec<String>, MechanismMultiStudyWorkflowError> {
    let mut indegree = task_ids
        .iter()
        .map(|task_id| {
            (
                task_id.clone(),
                dependencies
                    .get(task_id)
                    .into_iter()
                    .flatten()
                    .filter(|dependency| task_ids.contains(*dependency))
                    .count(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut outgoing = BTreeMap::<String, Vec<String>>::new();
    for (task_id, task_dependencies) in dependencies {
        for dependency in task_dependencies {
            outgoing
                .entry(dependency.clone())
                .or_default()
                .push(task_id.clone());
        }
    }
    for successors in outgoing.values_mut() {
        successors.sort();
    }
    let mut ready = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(task_id, _)| task_id.clone())
        .collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(task_ids.len());
    while let Some(task_id) = ready.pop_first() {
        order.push(task_id.clone());
        for successor in outgoing.get(&task_id).into_iter().flatten() {
            let degree = indegree
                .get_mut(successor)
                .expect("validated task successor");
            *degree = degree.saturating_sub(1);
            if *degree == 0 {
                ready.insert(successor.clone());
            }
        }
    }
    if order.len() != task_ids.len() {
        return Err(MechanismMultiStudyWorkflowError::InvalidInput(
            "multi-study task dependencies contain a cycle".into(),
        ));
    }
    Ok(order)
}

/// Compile institution-local mechanism workflows into a multimodal, multi-study portfolio while
/// retaining transport, replication, and federation boundaries.
pub fn compile_glioma_multi_study_mechanism_workflow(
    request: &MechanismMultiStudyWorkflowRequest,
) -> Result<MechanismMultiStudyWorkflowPlan, MechanismMultiStudyWorkflowError> {
    validate_request(request)?;
    let mut lane_order = request
        .lanes
        .iter()
        .map(|lane| lane.lane_id.clone())
        .collect::<Vec<_>>();
    lane_order.sort();
    let mut tasks_by_id = BTreeMap::<String, MechanismMultiStudyTask>::new();
    let mut dependencies = BTreeMap::<String, Vec<String>>::new();
    for lane in &request.lanes {
        let nodes = lane
            .workflow
            .nodes
            .iter()
            .map(|node| (node.action.action_id.clone(), node))
            .collect::<BTreeMap<_, _>>();
        let scheduled_ids = lane
            .workflow
            .scheduled_action_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        for action_id in &lane.workflow.topological_order {
            if !scheduled_ids.contains(action_id) {
                continue;
            }
            let node = nodes.get(action_id).expect("validated workflow node");
            if node.status != MechanismWorkflowNodeStatus::Scheduled {
                return Err(MechanismMultiStudyWorkflowError::InvalidInput(format!(
                    "workflow scheduled order disagrees with node status for {}",
                    action_id
                )));
            }
            let id = task_key(&lane.lane_id, action_id);
            let task_dependencies = node
                .action
                .dependency_order
                .iter()
                .filter(|dependency| scheduled_ids.contains(*dependency))
                .map(|dependency| task_key(&lane.lane_id, dependency))
                .collect::<Vec<_>>();
            let score = transport_score(lane);
            let task = MechanismMultiStudyTask {
                task_id: id.clone(),
                lane_id: lane.lane_id.clone(),
                study_id: lane.study_id.clone(),
                site_id: lane.site_id.clone(),
                action_id: action_id.clone(),
                mechanism_id: node.action.mechanism_id.clone(),
                model_system: node.action.model_system,
                modality: node.action.modality,
                dependency_order: task_dependencies.clone(),
                cost_units: node.action.cost_units,
                priority_milli: 0,
                transport_score_milli: score,
                status: MechanismMultiStudyTaskStatus::Deferred,
                reason: None,
            };
            if tasks_by_id.insert(id.clone(), task).is_some() {
                return Err(MechanismMultiStudyWorkflowError::InvalidInput(
                    "lane/action task ids must be unique".into(),
                ));
            }
            dependencies.insert(id, task_dependencies);
        }
    }
    if tasks_by_id.is_empty() {
        return Err(MechanismMultiStudyWorkflowError::InvalidInput(
            "at least one scheduled local workflow task is required".into(),
        ));
    }
    let task_ids = tasks_by_id.keys().cloned().collect::<BTreeSet<_>>();
    let topological = topological_tasks(&task_ids, &dependencies)?;
    let mut groups_by_action = BTreeMap::<String, Vec<String>>::new();
    for task in tasks_by_id.values() {
        groups_by_action
            .entry(task.action_id.clone())
            .or_default()
            .push(task.task_id.clone());
    }
    for task_ids in groups_by_action.values_mut() {
        task_ids.sort();
    }
    let mut groups = Vec::new();
    let mut group_eligibility = BTreeMap::<String, (bool, Option<String>)>::new();
    for (action_id, group_task_ids) in &groups_by_action {
        let mut mechanism_id = None;
        let mut studies = BTreeSet::new();
        let mut sites = BTreeSet::new();
        let mut modalities = BTreeSet::new();
        let mut scores = Vec::new();
        for task_id in group_task_ids {
            let task = tasks_by_id.get(task_id).expect("validated task");
            if let Some(existing) = &mechanism_id {
                if existing != &task.mechanism_id {
                    return Err(MechanismMultiStudyWorkflowError::InvalidInput(format!(
                        "action {action_id} maps to multiple mechanism ids"
                    )));
                }
            } else {
                mechanism_id = Some(task.mechanism_id.clone());
            }
            studies.insert(task.study_id.clone());
            sites.insert(task.site_id.clone());
            modalities.insert(task.modality);
            scores.push(task.transport_score_milli);
        }
        let supporting_study_count = studies.len();
        let independent_site_count = sites.len();
        let transport_score_milli = (scores.iter().map(|score| u32::from(*score)).sum::<u32>()
            / scores.len() as u32) as u16;
        let required_modality_complete = request
            .required_modalities
            .iter()
            .all(|modality| modalities.contains(modality));
        let replication_complete = supporting_study_count >= request.minimum_studies_per_action
            && independent_site_count >= request.minimum_sites_per_action;
        let priority_milli = (u32::from(transport_score_milli) / 2
            + (supporting_study_count.min(4) as u32 * 100)
            + (modalities.len().min(4) as u32 * 75))
            .min(1_000) as u16;
        let block_reason = if !replication_complete {
            Some("insufficient-study-or-site-replication".into())
        } else if !required_modality_complete {
            Some("required-modality-coverage-missing".into())
        } else if transport_score_milli < request.minimum_transport_score_milli {
            Some("transport-score-below-threshold".into())
        } else {
            None
        };
        let eligible = block_reason.is_none();
        group_eligibility.insert(action_id.clone(), (eligible, block_reason.clone()));
        groups.push(MechanismMultiStudyActionGroup {
            action_id: action_id.clone(),
            mechanism_id: mechanism_id.expect("non-empty group"),
            task_order: group_task_ids.clone(),
            study_order: studies.iter().cloned().collect(),
            site_order: sites.iter().cloned().collect(),
            modality_order: modalities.iter().copied().collect(),
            supporting_study_count,
            independent_site_count,
            transport_score_milli,
            priority_milli,
            required_modality_complete,
            replication_complete,
            selected: false,
            block_reason,
        });
    }
    groups.sort_by(|left, right| left.action_id.cmp(&right.action_id));
    let mut wave_by_task = BTreeMap::<String, usize>::new();
    let mut cumulative_by_task = BTreeMap::<String, u64>::new();
    for task_id in &topological {
        let dependency_wave = dependencies
            .get(task_id)
            .into_iter()
            .flatten()
            .filter_map(|dependency| wave_by_task.get(dependency))
            .copied()
            .max();
        let wave = dependency_wave.map_or(0, |value| value.saturating_add(1));
        let dependency_cost = dependencies
            .get(task_id)
            .into_iter()
            .flatten()
            .filter_map(|dependency| cumulative_by_task.get(dependency))
            .copied()
            .max()
            .unwrap_or(0);
        let cost = tasks_by_id.get(task_id).expect("validated task").cost_units;
        wave_by_task.insert(task_id.clone(), wave);
        cumulative_by_task.insert(task_id.clone(), dependency_cost.saturating_add(cost));
    }
    let mut scheduled = Vec::new();
    let mut deferred = BTreeMap::<String, String>::new();
    let mut blocked = BTreeMap::<String, String>::new();
    let mut selected_groups = BTreeSet::new();
    let mut spent = 0_u64;
    for task_id in &topological {
        let action_id = tasks_by_id
            .get(task_id)
            .expect("validated task")
            .action_id
            .clone();
        let (eligible, reason) = group_eligibility
            .get(&action_id)
            .cloned()
            .expect("group coverage");
        if !eligible {
            blocked.insert(
                task_id.clone(),
                reason.unwrap_or_else(|| "group-coverage-policy".into()),
            );
            continue;
        }
        if dependencies
            .get(task_id)
            .into_iter()
            .flatten()
            .any(|dependency| !scheduled.iter().any(|id| id == dependency))
        {
            deferred.insert(task_id.clone(), "dependency-not-scheduled".into());
        } else if wave_by_task[task_id] >= request.max_waves {
            deferred.insert(task_id.clone(), "wave-limit".into());
        } else if scheduled.len() >= request.max_tasks {
            deferred.insert(task_id.clone(), "task-limit".into());
        } else {
            let cost = tasks_by_id.get(task_id).expect("validated task").cost_units;
            if spent.saturating_add(cost) > request.budget_units {
                deferred.insert(task_id.clone(), "budget-limit".into());
            } else {
                spent = spent.saturating_add(cost);
                scheduled.push(task_id.clone());
                selected_groups.insert(action_id);
            }
        }
    }
    let scheduled_set = scheduled.iter().cloned().collect::<BTreeSet<_>>();
    let mut execution_waves = Vec::<Vec<String>>::new();
    for task_id in &scheduled {
        let wave = wave_by_task[task_id];
        while execution_waves.len() <= wave {
            execution_waves.push(Vec::new());
        }
        execution_waves[wave].push(task_id.clone());
    }
    execution_waves.retain(|wave| !wave.is_empty());
    for wave in &mut execution_waves {
        wave.sort();
    }
    let mut negative_evidence = Vec::new();
    let mut uncertainty = Vec::new();
    for lane in &request.lanes {
        negative_evidence.extend(
            lane.workflow
                .negative_evidence
                .iter()
                .map(|item| format!("{}:{item}", lane.lane_id)),
        );
        uncertainty.extend(
            lane.workflow
                .uncertainty
                .iter()
                .map(|item| format!("{}:{item}", lane.lane_id)),
        );
    }
    for (task_id, reason) in &blocked {
        negative_evidence.push(format!("multi-study:{task_id}:blocked:{reason}"));
    }
    for (task_id, reason) in &deferred {
        uncertainty.push(format!("multi-study:{task_id}:deferred:{reason}"));
    }
    negative_evidence.sort();
    negative_evidence.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    for task in tasks_by_id.values_mut() {
        let action_id = task.action_id.clone();
        let group = groups
            .iter()
            .find(|group| group.action_id == action_id)
            .expect("group exists");
        task.priority_milli = group.priority_milli;
        if scheduled_set.contains(&task.task_id) {
            task.status = MechanismMultiStudyTaskStatus::Scheduled;
        } else if let Some(reason) = blocked.get(&task.task_id) {
            task.status = MechanismMultiStudyTaskStatus::Blocked;
            task.reason = Some(reason.clone());
        } else if let Some(reason) = deferred.get(&task.task_id) {
            task.status = MechanismMultiStudyTaskStatus::Deferred;
            task.reason = Some(reason.clone());
        } else {
            return Err(MechanismMultiStudyWorkflowError::InvalidOutput(
                "every task must receive a status partition".into(),
            ));
        }
    }
    for group in &mut groups {
        group.selected = selected_groups.contains(&group.action_id);
    }
    let blocked_order = blocked.keys().cloned().collect::<Vec<_>>();
    let deferred_order = deferred.keys().cloned().collect::<Vec<_>>();
    let disposition = if scheduled.len() == tasks_by_id.len() {
        MechanismMultiStudyWorkflowDisposition::Ready
    } else if scheduled.is_empty()
        && blocked.values().any(|reason| {
            reason.contains("study") || reason.contains("site") || reason.contains("modality")
        })
    {
        MechanismMultiStudyWorkflowDisposition::CoverageBlocked
    } else if deferred.values().any(|reason| reason.contains("budget")) {
        MechanismMultiStudyWorkflowDisposition::BudgetBlocked
    } else {
        MechanismMultiStudyWorkflowDisposition::Partial
    };
    let next_route = if scheduled.is_empty() {
        "glioma_mechanism_workflow_compile"
    } else {
        "glioma_mechanism_operating_cycle"
    };
    let group_order = groups
        .iter()
        .map(|group| group.action_id.clone())
        .collect::<Vec<_>>();
    let task_order = tasks_by_id.keys().cloned().collect::<Vec<_>>();
    let mut output = MechanismMultiStudyWorkflowPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        lane_order,
        group_order,
        task_order,
        topological_task_order: topological,
        execution_waves,
        tasks: tasks_by_id.into_values().collect(),
        groups,
        scheduled_task_order: scheduled,
        deferred_task_order: deferred_order,
        blocked_task_order: blocked_order,
        federation_mode: request.federation_mode,
        total_cost_units: spent,
        budget_units: request.budget_units,
        budget_remaining_units: request.budget_units.saturating_sub(spent),
        critical_path_units: cumulative_by_task.values().copied().max().unwrap_or(0),
        negative_evidence,
        uncertainty,
        disposition,
        next_route: next_route.into(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-multi-study-workflow"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| MechanismMultiStudyWorkflowError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

impl MechanismMultiStudyWorkflowPlan {
    pub fn validate(&self) -> Result<(), MechanismMultiStudyWorkflowError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.lane_order.is_empty()
            || !canonical(&self.lane_order)
            || !unique_nonempty(&self.lane_order)
            || !canonical(&self.group_order)
            || !unique_nonempty(&self.group_order)
            || !canonical(&self.task_order)
            || !unique_nonempty(&self.task_order)
            || self.tasks.len() != self.task_order.len()
            || self.groups.len() != self.group_order.len()
            || self.topological_task_order.len() != self.task_order.len()
            || self
                .execution_waves
                .iter()
                .any(|wave| wave.is_empty() || !canonical(wave) || !unique_nonempty(wave))
            || !canonical(&self.deferred_task_order)
            || !canonical(&self.blocked_task_order)
            || !unique_nonempty(&self.deferred_task_order)
            || !unique_nonempty(&self.blocked_task_order)
            || !unique_nonempty(&self.scheduled_task_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.next_route.trim().is_empty()
        {
            return Err(MechanismMultiStudyWorkflowError::InvalidOutput(
                "identity, lane/group/task ordering, wave, or digest shape is invalid".into(),
            ));
        }
        let task_ids = self.task_order.iter().cloned().collect::<BTreeSet<_>>();
        let task_action_order = self
            .tasks
            .iter()
            .map(|task| task.task_id.clone())
            .collect::<Vec<_>>();
        let topological_ids = self
            .topological_task_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let scheduled_ids = self
            .scheduled_task_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let deferred_ids = self
            .deferred_task_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let blocked_ids = self
            .blocked_task_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let partition = scheduled_ids
            .union(&deferred_ids)
            .cloned()
            .collect::<BTreeSet<_>>()
            .union(&blocked_ids)
            .cloned()
            .collect::<BTreeSet<_>>();
        let status_scheduled = self
            .tasks
            .iter()
            .filter(|task| task.status == MechanismMultiStudyTaskStatus::Scheduled)
            .map(|task| task.task_id.clone())
            .collect::<BTreeSet<_>>();
        let status_deferred = self
            .tasks
            .iter()
            .filter(|task| task.status == MechanismMultiStudyTaskStatus::Deferred)
            .map(|task| task.task_id.clone())
            .collect::<BTreeSet<_>>();
        let status_blocked = self
            .tasks
            .iter()
            .filter(|task| task.status == MechanismMultiStudyTaskStatus::Blocked)
            .map(|task| task.task_id.clone())
            .collect::<BTreeSet<_>>();
        if task_action_order != self.task_order
            || topological_ids != task_ids
            || partition != task_ids
            || scheduled_ids.len() + deferred_ids.len() + blocked_ids.len() != partition.len()
            || status_scheduled != scheduled_ids
            || status_deferred != deferred_ids
            || status_blocked != blocked_ids
            || self
                .execution_waves
                .iter()
                .flatten()
                .cloned()
                .collect::<BTreeSet<_>>()
                != scheduled_ids
            || self.tasks.iter().any(|task| {
                task.task_id.trim().is_empty()
                    || task.action_id.trim().is_empty()
                    || task.lane_id.trim().is_empty()
                    || !canonical(&task.dependency_order)
                    || task.dependency_order.iter().any(|dependency| {
                        !task_ids.contains(dependency)
                            || self
                                .topological_task_order
                                .iter()
                                .position(|id| id == dependency)
                                >= self
                                    .topological_task_order
                                    .iter()
                                    .position(|id| id == &task.task_id)
                    })
            })
        {
            return Err(MechanismMultiStudyWorkflowError::InvalidOutput(
                "task topology, status partition, wave coverage, or dependency order is invalid"
                    .into(),
            ));
        }
        if self.groups.iter().any(|group| {
            group.action_id.trim().is_empty()
                || group.task_order.is_empty()
                || !canonical(&group.task_order)
                || !unique_nonempty(&group.task_order)
                || !canonical(&group.study_order)
                || !canonical(&group.site_order)
                || !canonical(&group.modality_order)
                || group
                    .task_order
                    .iter()
                    .any(|task_id| !task_ids.contains(task_id))
                || group.supporting_study_count != group.study_order.len()
                || group.independent_site_count != group.site_order.len()
        }) {
            return Err(MechanismMultiStudyWorkflowError::InvalidOutput(
                "action-group coverage or task references are invalid".into(),
            ));
        }
        let scheduled_cost = self
            .tasks
            .iter()
            .filter(|task| task.status == MechanismMultiStudyTaskStatus::Scheduled)
            .map(|task| task.cost_units)
            .sum::<u64>();
        if scheduled_cost != self.total_cost_units
            || self.total_cost_units > self.budget_units
            || self
                .total_cost_units
                .saturating_add(self.budget_remaining_units)
                != self.budget_units
        {
            return Err(MechanismMultiStudyWorkflowError::InvalidOutput(
                "scheduled cost and budget accounting do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| MechanismMultiStudyWorkflowError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(MechanismMultiStudyWorkflowError::Digest(
                "multi-study workflow digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::mechanism_workflow::{
        MechanismWorkflowAction, MechanismWorkflowNode, MechanismWorkflowNodeStatus,
    };
    use super::*;

    fn hash(seed: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"seed": seed})).unwrap()
    }

    fn local_workflow(
        lane: &str,
        action_id: &str,
        modality: GliomaModality,
    ) -> MechanismWorkflowPlan {
        let action = MechanismWorkflowAction {
            action_id: action_id.into(),
            mechanism_id: "m1".into(),
            model_system: GliomaModelSystem::Organoid,
            modality,
            dependency_order: vec![],
            cost_units: 1,
            risk_milli: 100,
            requires_approval: false,
            input_artifacts: vec![],
            execution_route: format!("local://{lane}/{action_id}"),
            compensation_route: None,
        };
        let mut output = MechanismWorkflowPlan {
            feature_id: super::super::mechanism_workflow::FEATURE_ID.into(),
            output_schema: super::super::mechanism_workflow::OUTPUT_SCHEMA.into(),
            objective: "multi-study glioma mechanism".into(),
            replan_digest: hash("replan"),
            node_order: vec![action_id.into()],
            topological_order: vec![action_id.into()],
            execution_waves: vec![vec![action_id.into()]],
            nodes: vec![MechanismWorkflowNode {
                action,
                status: MechanismWorkflowNodeStatus::Scheduled,
                wave: Some(0),
                cumulative_cost_units: 1,
                reason: None,
            }],
            scheduled_action_order: vec![action_id.into()],
            deferred_action_order: vec![],
            blocked_action_order: vec![],
            total_cost_units: 1,
            budget_units: 1,
            budget_remaining_units: 0,
            critical_path_units: 1,
            negative_evidence: vec![],
            uncertainty: vec![],
            disposition: super::super::mechanism_workflow::MechanismWorkflowDisposition::Ready,
            next_route: "glioma_mechanism_operating_cycle".into(),
            digest: hash("unsealed"),
        };
        output.digest =
            ContentHash::of_value(&super::super::mechanism_workflow::digest_input(&output))
                .unwrap();
        output.validate().unwrap();
        output
    }

    fn lane(id: &str, study: &str, site: &str, modality: GliomaModality) -> MechanismStudyLane {
        MechanismStudyLane {
            lane_id: id.into(),
            study_id: study.into(),
            site_id: site.into(),
            model_system: GliomaModelSystem::Organoid,
            modalities: vec![modality],
            workflow: local_workflow(id, "measure", modality),
            transport_penalty_milli: 100,
            replication_weight_milli: 900,
        }
    }

    #[test]
    fn joins_sites_and_modalities_into_parallel_research_tasks() {
        let plan =
            compile_glioma_multi_study_mechanism_workflow(&MechanismMultiStudyWorkflowRequest {
                objective: "multi-study glioma mechanism".into(),
                lanes: vec![
                    lane("lane-a", "study-a", "site-a", GliomaModality::Imaging),
                    lane(
                        "lane-b",
                        "study-b",
                        "site-b",
                        GliomaModality::Transcriptomics,
                    ),
                ],
                required_modalities: vec![GliomaModality::Transcriptomics, GliomaModality::Imaging],
                minimum_studies_per_action: 2,
                minimum_sites_per_action: 2,
                minimum_transport_score_milli: 700,
                budget_units: 2,
                max_tasks: 4,
                max_waves: 4,
                federation_mode: MechanismFederationMode::AggregateMetadata,
            })
            .unwrap();
        assert_eq!(plan.scheduled_task_order.len(), 2);
        assert_eq!(plan.execution_waves.len(), 1);
        assert_eq!(plan.groups[0].supporting_study_count, 2);
        assert!(plan.groups[0].required_modality_complete);
        assert!(plan.groups[0].replication_complete);
        plan.validate().unwrap();
    }

    #[test]
    fn preserves_missing_replication_as_a_blocked_frontier() {
        let plan =
            compile_glioma_multi_study_mechanism_workflow(&MechanismMultiStudyWorkflowRequest {
                objective: "multi-study glioma mechanism".into(),
                lanes: vec![lane("lane-a", "study-a", "site-a", GliomaModality::Imaging)],
                required_modalities: vec![GliomaModality::Imaging],
                minimum_studies_per_action: 2,
                minimum_sites_per_action: 2,
                minimum_transport_score_milli: 0,
                budget_units: 2,
                max_tasks: 4,
                max_waves: 4,
                federation_mode: MechanismFederationMode::LocalOnly,
            })
            .unwrap();
        assert!(plan.scheduled_task_order.is_empty());
        assert_eq!(
            plan.disposition,
            MechanismMultiStudyWorkflowDisposition::CoverageBlocked
        );
        assert!(plan.blocked_task_order[0].contains("lane-a"));
        plan.validate().unwrap();
    }
}

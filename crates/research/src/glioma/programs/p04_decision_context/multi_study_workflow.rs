//! Dependency-safe multi-study workflow planning for preclinical glioma research.
//!
//! P04-F06 aligns independent study contexts into a typed frontier. This feature turns that
//! frontier into an executable *plan* for the institution-local research engine: it closes action
//! dependencies, allocates qualified actions across independent study groups, reserves bounded
//! compute/material/instrument budgets, assigns deterministic waves, and routes higher-risk work
//! through approval or signed-preflight gates. It never executes an assay or instrument and it
//! cannot convert a planning candidate into a clinical decision.

use super::multi_study_context_artifact::{
    MultiStudyActionDisposition, MultiStudyDecisionContextArtifact,
};
use bioprism_foundation::{AutonomyTier, Effect, PRECLINICAL_BOUNDARY};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P04-F07";
pub const OUTPUT_SCHEMA: &str = "GliomaMultiStudyWorkflowPlan1@1";
pub const MAX_STUDY_BUDGETS: usize = 256;
pub const MAX_WAVES: usize = 256;
pub const MAX_TASKS: usize = 4_096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultiStudyWorkflowDisposition {
    Ready,
    Partial,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultiStudyTaskDisposition {
    Scheduled,
    ApprovalRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiStudyStudyBudget {
    pub study_id: String,
    pub independent_group: String,
    pub policy_allowed: bool,
    pub local_only: bool,
    pub compute_units: u64,
    pub material_units: u64,
    pub instrument_units: u64,
    pub autonomy_ceiling: AutonomyTier,
    pub approved_autonomy: AutonomyTier,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiStudyWorkflowRequest {
    pub objective: String,
    pub epoch: u32,
    pub maximum_waves: usize,
    pub maximum_tasks: usize,
    pub minimum_independent_groups: usize,
    pub require_replication: bool,
    pub budget_units: u64,
    pub artifact: MultiStudyDecisionContextArtifact,
    pub study_budgets: Vec<MultiStudyStudyBudget>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiStudyWorkflowTask {
    pub task_id: String,
    pub action_id: String,
    pub study_id: String,
    pub independent_group: String,
    pub wave: usize,
    pub cost_units: u32,
    pub autonomy_tier: AutonomyTier,
    pub effects: BTreeSet<Effect>,
    pub depends_on: Vec<String>,
    pub route: String,
    pub disposition: MultiStudyTaskDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiStudyWorkflowPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub epoch: u32,
    pub boundary: String,
    pub source_artifact_digest: ContentHash,
    pub study_order: Vec<String>,
    pub eligible_study_order: Vec<String>,
    pub task_order: Vec<String>,
    pub wave_order: Vec<Vec<String>>,
    pub scheduled_action_order: Vec<String>,
    pub approval_action_order: Vec<String>,
    pub deferred_action_order: Vec<String>,
    pub blocked_action_order: Vec<String>,
    pub tasks: Vec<MultiStudyWorkflowTask>,
    pub omissions: BTreeMap<String, String>,
    pub negative_evidence_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub disposition: MultiStudyWorkflowDisposition,
    pub next_route: String,
    pub budget_units: u64,
    pub budget_reserved_units: u64,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MultiStudyWorkflowError {
    #[error("multi-study workflow request is invalid: {0}")]
    InvalidRequest(String),
    #[error("multi-study workflow source artifact is invalid: {0}")]
    InvalidArtifact(String),
    #[error("multi-study workflow plan is invalid: {0}")]
    InvalidOutput(String),
    #[error("multi-study workflow digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn ranked_unique(values: &[String]) -> bool {
    let mut seen = BTreeSet::new();
    values
        .iter()
        .all(|value| !value.trim().is_empty() && seen.insert(value))
}

fn digest_input(output: &MultiStudyWorkflowPlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "epoch": output.epoch,
        "boundary": output.boundary,
        "source_artifact_digest": output.source_artifact_digest,
        "study_order": output.study_order,
        "eligible_study_order": output.eligible_study_order,
        "task_order": output.task_order,
        "wave_order": output.wave_order,
        "scheduled_action_order": output.scheduled_action_order,
        "approval_action_order": output.approval_action_order,
        "deferred_action_order": output.deferred_action_order,
        "blocked_action_order": output.blocked_action_order,
        "tasks": output.tasks,
        "omissions": output.omissions,
        "negative_evidence_order": output.negative_evidence_order,
        "uncertainty_order": output.uncertainty_order,
        "disposition": output.disposition,
        "next_route": output.next_route,
        "budget_units": output.budget_units,
        "budget_reserved_units": output.budget_reserved_units,
    })
}

impl MultiStudyWorkflowPlan {
    pub fn validate(&self) -> Result<(), MultiStudyWorkflowError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.epoch == 0
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.source_artifact_digest.as_str().len() != 64
            || !canonical(&self.study_order)
            || !canonical(&self.eligible_study_order)
            || !ranked_unique(&self.task_order)
            || self.wave_order.len() > MAX_WAVES
            || self.tasks.len() > MAX_TASKS
            || self.task_order.len() != self.tasks.len()
            || !ranked_unique(&self.scheduled_action_order)
            || !ranked_unique(&self.approval_action_order)
            || !ranked_unique(&self.deferred_action_order)
            || !ranked_unique(&self.blocked_action_order)
            || !canonical(&self.negative_evidence_order)
            || !canonical(&self.uncertainty_order)
            || self.budget_reserved_units > self.budget_units
            || self.digest.as_str().len() != 64
        {
            return Err(MultiStudyWorkflowError::InvalidOutput(
                "identity, boundary, ordering, bounds, budget, or digest invariants are invalid"
                    .into(),
            ));
        }
        let study_set = self.study_order.iter().cloned().collect::<BTreeSet<_>>();
        let eligible_set = self
            .eligible_study_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        if !eligible_set.is_subset(&study_set) {
            return Err(MultiStudyWorkflowError::InvalidOutput(
                "eligible studies must be declared in study_order".into(),
            ));
        }
        let task_ids = self
            .tasks
            .iter()
            .map(|task| task.task_id.clone())
            .collect::<BTreeSet<_>>();
        if self.task_order.iter().cloned().collect::<BTreeSet<_>>() != task_ids
            || self
                .wave_order
                .iter()
                .flatten()
                .cloned()
                .collect::<BTreeSet<_>>()
                != task_ids
            || self.wave_order.iter().flatten().count() != task_ids.len()
            || self.tasks.iter().any(|task| {
                task.task_id.trim().is_empty()
                    || task.action_id.trim().is_empty()
                    || !study_set.contains(&task.study_id)
                    || task.independent_group.trim().is_empty()
                    || task.wave == 0
                    || task.wave > self.wave_order.len().max(1)
                    || task.cost_units == 0
                    || task.depends_on.windows(2).any(|pair| pair[0] >= pair[1])
                    || task.depends_on.iter().any(|dependency| {
                        !task_ids.contains(dependency) || dependency == &task.task_id
                    })
            })
        {
            return Err(MultiStudyWorkflowError::InvalidOutput(
                "task identity, wave, dependency, or disposition invariants are invalid".into(),
            ));
        }
        let waves = self
            .wave_order
            .iter()
            .enumerate()
            .flat_map(|(index, wave)| wave.iter().map(move |task_id| (task_id, index + 1)))
            .collect::<BTreeMap<_, _>>();
        if self.tasks.iter().any(|task| {
            task.depends_on
                .iter()
                .any(|dependency| waves.get(dependency).is_some_and(|wave| *wave >= task.wave))
        }) {
            return Err(MultiStudyWorkflowError::InvalidOutput(
                "dependencies must be scheduled in earlier waves".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| MultiStudyWorkflowError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(MultiStudyWorkflowError::Digest(
                "multi-study workflow digest does not match canonical content".into(),
            ));
        }
        Ok(())
    }
}

fn collect_dependency_closure(
    action_id: &str,
    actions: &BTreeMap<String, &super::multi_study_context_artifact::MultiStudyDecisionAction>,
    state: &mut BTreeMap<String, u8>,
    closure: &mut BTreeSet<String>,
) -> Result<(), MultiStudyWorkflowError> {
    match state.get(action_id).copied() {
        Some(1) => {
            return Err(MultiStudyWorkflowError::InvalidArtifact(format!(
                "action dependency cycle includes {action_id}"
            )))
        }
        Some(2) => return Ok(()),
        _ => {}
    }
    let action = actions.get(action_id).ok_or_else(|| {
        MultiStudyWorkflowError::InvalidArtifact(format!("unknown action dependency {action_id}"))
    })?;
    state.insert(action_id.into(), 1);
    for dependency in &action.action.depends_on {
        collect_dependency_closure(dependency, actions, state, closure)?;
    }
    state.insert(action_id.into(), 2);
    closure.insert(action_id.into());
    Ok(())
}

fn dependencies_are_qualified(
    action_id: &str,
    actions: &BTreeMap<String, &super::multi_study_context_artifact::MultiStudyDecisionAction>,
    visiting: &mut BTreeSet<String>,
) -> Result<bool, MultiStudyWorkflowError> {
    if !visiting.insert(action_id.into()) {
        return Err(MultiStudyWorkflowError::InvalidArtifact(format!(
            "action dependency cycle includes {action_id}"
        )));
    }
    let action = actions.get(action_id).ok_or_else(|| {
        MultiStudyWorkflowError::InvalidArtifact(format!("unknown action dependency {action_id}"))
    })?;
    if action.disposition != MultiStudyActionDisposition::Qualified {
        visiting.remove(action_id);
        return Ok(false);
    }
    for dependency in &action.action.depends_on {
        if !dependencies_are_qualified(dependency, actions, visiting)? {
            visiting.remove(action_id);
            return Ok(false);
        }
    }
    visiting.remove(action_id);
    Ok(true)
}

fn route_for(action: &super::multi_study_context_artifact::MultiStudyDecisionAction) -> String {
    if action.action.autonomy_tier.requires_signed_preflight()
        || action.action.effects.contains(&Effect::InstrumentExecution)
    {
        "signed_instrument_preflight_and_authorization".into()
    } else if action.action.effects.contains(&Effect::ExternalDataAccess)
        || action.action.effects.contains(&Effect::FederationExport)
    {
        "researcher_approval_and_data_policy_gate".into()
    } else if action.action.autonomy_tier.requires_approval() {
        "researcher_approval_gate".into()
    } else if action.action.effects.contains(&Effect::ConsumeMaterial) {
        "assay_preflight_and_authorization".into()
    } else {
        "local_computation".into()
    }
}

fn consumes_material(
    action: &super::multi_study_context_artifact::MultiStudyDecisionAction,
) -> bool {
    action.action.effects.contains(&Effect::ConsumeMaterial)
}

fn consumes_instrument(
    action: &super::multi_study_context_artifact::MultiStudyDecisionAction,
) -> bool {
    action.action.effects.contains(&Effect::InstrumentExecution)
}

/// Compile a qualified multi-study context frontier into deterministic, dependency-safe waves.
pub fn plan_glioma_multi_study_workflow(
    request: &MultiStudyWorkflowRequest,
) -> Result<MultiStudyWorkflowPlan, MultiStudyWorkflowError> {
    if request.objective.trim().is_empty()
        || request.epoch == 0
        || request.maximum_waves == 0
        || request.maximum_waves > MAX_WAVES
        || request.maximum_tasks == 0
        || request.maximum_tasks > MAX_TASKS
        || request.minimum_independent_groups == 0
        || request.budget_units == 0
        || request.study_budgets.is_empty()
        || request.study_budgets.len() > MAX_STUDY_BUDGETS
        || request.objective != request.artifact.objective
        || request.epoch != request.artifact.epoch
    {
        return Err(MultiStudyWorkflowError::InvalidRequest(
            "objective, epoch, wave/task bounds, replication quorum, budget, and artifact binding are required".into(),
        ));
    }
    request
        .artifact
        .validate()
        .map_err(|error| MultiStudyWorkflowError::InvalidArtifact(error.to_string()))?;

    let mut budgets = request.study_budgets.clone();
    budgets.sort_by(|left, right| left.study_id.cmp(&right.study_id));
    if budgets
        .windows(2)
        .any(|pair| pair[0].study_id == pair[1].study_id)
        || budgets.iter().any(|budget| {
            budget.study_id.trim().is_empty()
                || budget.independent_group.trim().is_empty()
                || budget.compute_units == 0
                || budget.autonomy_ceiling < budget.approved_autonomy
        })
    {
        return Err(MultiStudyWorkflowError::InvalidRequest(
            "study budgets must be unique, positive, and autonomy-bounded".into(),
        ));
    }
    let artifact_studies = request
        .artifact
        .study_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if budgets.iter().any(|budget| {
        !artifact_studies.contains(&budget.study_id)
            || request
                .artifact
                .study_group
                .get(&budget.study_id)
                .map_or(true, |group| group != &budget.independent_group)
    }) {
        return Err(MultiStudyWorkflowError::InvalidRequest(
            "study budgets cannot introduce unknown studies or relabel independent groups".into(),
        ));
    }
    let mut budget_map = budgets
        .into_iter()
        .map(|budget| (budget.study_id.clone(), budget))
        .collect::<BTreeMap<_, _>>();
    let mut omissions = BTreeMap::new();
    let mut eligible_studies = Vec::new();
    for study_id in &request.artifact.eligible_study_order {
        let Some(budget) = budget_map.get(study_id) else {
            omissions.insert(format!("study:{study_id}"), "missing_local_budget".into());
            continue;
        };
        if !budget.policy_allowed {
            omissions.insert(format!("study:{study_id}"), "study_policy_denied".into());
        } else if !budget.local_only {
            omissions.insert(
                format!("study:{study_id}"),
                "raw_data_locality_not_declared".into(),
            );
        } else {
            eligible_studies.push(study_id.clone());
        }
    }
    let action_map = request
        .artifact
        .actions
        .iter()
        .map(|action| (action.action.action_id.clone(), action))
        .collect::<BTreeMap<_, _>>();
    let mut state = BTreeMap::new();
    let mut closure = BTreeSet::new();
    let mut blocked_frontier_actions = BTreeSet::new();
    for action_id in &request.artifact.frontier_order {
        let Some(action) = action_map.get(action_id) else {
            return Err(MultiStudyWorkflowError::InvalidArtifact(format!(
                "frontier references unknown action {action_id}"
            )));
        };
        if action.disposition != MultiStudyActionDisposition::Qualified {
            omissions.insert(
                format!("action:{action_id}"),
                "frontier_action_not_qualified".into(),
            );
            continue;
        }
        if !dependencies_are_qualified(action_id, &action_map, &mut BTreeSet::new())? {
            blocked_frontier_actions.insert(action_id.clone());
            omissions.insert(
                format!("action:{action_id}"),
                "dependency_action_not_qualified".into(),
            );
            continue;
        }
        collect_dependency_closure(action_id, &action_map, &mut state, &mut closure)?;
    }

    let mut indegree = closure
        .iter()
        .map(|action_id| (action_id.clone(), 0usize))
        .collect::<BTreeMap<_, _>>();
    let mut dependents = BTreeMap::<String, Vec<String>>::new();
    for action_id in &closure {
        let action = action_map[action_id];
        for dependency in &action.action.depends_on {
            if !closure.contains(dependency) {
                return Err(MultiStudyWorkflowError::InvalidArtifact(format!(
                    "dependency {dependency} is outside the closed action set"
                )));
            }
            *indegree
                .get_mut(action_id)
                .expect("closure contains action") += 1;
            dependents
                .entry(dependency.clone())
                .or_default()
                .push(action_id.clone());
        }
    }
    for values in dependents.values_mut() {
        values.sort();
    }
    let frontier_rank = request
        .artifact
        .frontier_order
        .iter()
        .enumerate()
        .map(|(rank, action_id)| (action_id.clone(), rank))
        .collect::<BTreeMap<_, _>>();
    let mut ready = indegree
        .iter()
        .filter_map(|(action_id, degree)| (*degree == 0).then_some(action_id.clone()))
        .collect::<Vec<_>>();
    let mut topological = Vec::with_capacity(closure.len());
    while !ready.is_empty() {
        ready.sort_by(|left, right| {
            frontier_rank
                .get(left)
                .copied()
                .unwrap_or(usize::MAX)
                .cmp(&frontier_rank.get(right).copied().unwrap_or(usize::MAX))
                .then_with(|| {
                    action_map[right]
                        .action
                        .priority_milli
                        .cmp(&action_map[left].action.priority_milli)
                })
                .then_with(|| left.cmp(right))
        });
        let action_id = ready.remove(0);
        topological.push(action_id.clone());
        for dependent in dependents.get(&action_id).into_iter().flatten() {
            let degree = indegree
                .get_mut(dependent)
                .expect("dependent is in closure");
            *degree -= 1;
            if *degree == 0 {
                ready.push(dependent.clone());
            }
        }
    }
    if topological.len() != closure.len() {
        return Err(MultiStudyWorkflowError::InvalidArtifact(
            "action dependency graph is cyclic".into(),
        ));
    }

    let mut tasks = Vec::new();
    let mut action_task_ids = BTreeMap::<(String, String), String>::new();
    let mut action_task_waves = BTreeMap::<String, usize>::new();
    let mut scheduled_actions = BTreeSet::new();
    let mut approval_actions = BTreeSet::new();
    let mut deferred_actions = BTreeSet::new();
    let mut blocked_actions = blocked_frontier_actions;
    let mut reserved_budget = 0u64;
    for action_id in topological {
        let action = action_map[&action_id];
        let dependencies_blocked = action.action.depends_on.iter().any(|dependency| {
            blocked_actions.contains(dependency) || deferred_actions.contains(dependency)
        });
        if dependencies_blocked {
            blocked_actions.insert(action_id.clone());
            omissions.insert(
                format!("action:{action_id}"),
                "dependency_not_schedulable".into(),
            );
            continue;
        }
        let mut candidate_studies = action
            .study_order
            .iter()
            .filter(|study_id| eligible_studies.binary_search(study_id).is_ok())
            .filter(|study_id| {
                action.action.depends_on.iter().all(|dependency| {
                    action_task_ids.contains_key(&(dependency.clone(), (*study_id).clone()))
                })
            })
            .cloned()
            .collect::<Vec<_>>();
        candidate_studies.sort_by(|left, right| {
            budget_map[left]
                .independent_group
                .cmp(&budget_map[right].independent_group)
                .then_with(|| left.cmp(right))
        });
        let action_cost = action.action.cost_units as u64;
        let material_need = consumes_material(action) as u64 * action_cost;
        let instrument_need = consumes_instrument(action) as u64 * action_cost;
        let mut autonomy_denied = false;
        candidate_studies.retain(|study_id| {
            let budget = &budget_map[study_id];
            if action.action.autonomy_tier > budget.autonomy_ceiling {
                autonomy_denied = true;
                omissions.insert(
                    format!("action:{action_id}:study:{study_id}"),
                    "autonomy_ceiling_denied".into(),
                );
                return false;
            }
            let available = action_cost <= budget.compute_units
                && material_need <= budget.material_units
                && instrument_need <= budget.instrument_units;
            if !available {
                omissions.insert(
                    format!("action:{action_id}:study:{study_id}"),
                    "local_resource_budget_exhausted".into(),
                );
            }
            available
        });
        let global_task_capacity = request.maximum_tasks.saturating_sub(tasks.len());
        let global_budget_capacity = request
            .budget_units
            .saturating_sub(reserved_budget)
            .checked_div(action_cost)
            .unwrap_or(0) as usize;
        let capacity = global_task_capacity.min(global_budget_capacity);
        let mut selected_studies = Vec::new();
        let mut selected_groups = BTreeSet::new();
        for study_id in &candidate_studies {
            let group = &budget_map[study_id].independent_group;
            if selected_studies.len() < capacity && selected_groups.insert(group.clone()) {
                selected_studies.push(study_id.clone());
            }
        }
        for study_id in &candidate_studies {
            if selected_studies.len() >= capacity {
                break;
            }
            if !selected_studies.contains(study_id) {
                selected_studies.push(study_id.clone());
            }
        }
        if selected_studies.len() < candidate_studies.len() {
            omissions.insert(
                format!("action:{action_id}"),
                if global_task_capacity <= global_budget_capacity {
                    "task_capacity_exceeded".into()
                } else {
                    "portfolio_budget_exhausted".into()
                },
            );
        }
        let candidate_groups = candidate_studies
            .iter()
            .filter_map(|study_id| {
                budget_map
                    .get(study_id)
                    .map(|budget| budget.independent_group.clone())
            })
            .collect::<BTreeSet<_>>();
        let required_groups = request
            .minimum_independent_groups
            .max(usize::from(request.require_replication) * 2);
        let selected_groups = selected_studies
            .iter()
            .filter_map(|study_id| {
                budget_map
                    .get(study_id)
                    .map(|budget| budget.independent_group.clone())
            })
            .collect::<BTreeSet<_>>();
        if selected_groups.len() < required_groups {
            if autonomy_denied && candidate_groups.len() < required_groups {
                blocked_actions.insert(action_id.clone());
                omissions.insert(
                    format!("action:{action_id}"),
                    "autonomy_policy_prevents_independent_group_quorum".into(),
                );
            } else {
                deferred_actions.insert(action_id.clone());
                omissions.insert(
                    format!("action:{action_id}"),
                    if candidate_groups.len() < required_groups {
                        "independent_group_quorum_unavailable".into()
                    } else {
                        "resource_or_task_capacity_prevents_quorum".into()
                    },
                );
            }
            continue;
        }
        let dependency_wave = action
            .action
            .depends_on
            .iter()
            .filter_map(|dependency| action_task_waves.get(dependency).copied())
            .max()
            .unwrap_or(0);
        let wave = dependency_wave + 1;
        if wave > request.maximum_waves {
            deferred_actions.insert(action_id.clone());
            omissions.insert(
                format!("action:{action_id}"),
                "wave_capacity_exceeded".into(),
            );
            continue;
        }
        let route = route_for(action);
        for study_id in selected_studies {
            let budget = budget_map
                .get_mut(&study_id)
                .expect("candidate study has budget");
            let disposition = if action.action.autonomy_tier.requires_approval()
                && action.action.autonomy_tier > budget.approved_autonomy
                || action.action.effects.contains(&Effect::ConsumeMaterial)
                || action.action.effects.contains(&Effect::InstrumentExecution)
                || action.action.effects.contains(&Effect::ExternalDataAccess)
                || action.action.effects.contains(&Effect::FederationExport)
                || action
                    .action
                    .depends_on
                    .iter()
                    .any(|dependency| approval_actions.contains(dependency))
            {
                MultiStudyTaskDisposition::ApprovalRequired
            } else {
                MultiStudyTaskDisposition::Scheduled
            };
            let task_id = format!("{action_id}@{study_id}");
            let mut dependencies = action
                .action
                .depends_on
                .iter()
                .filter_map(|dependency| {
                    action_task_ids
                        .get(&(dependency.clone(), study_id.clone()))
                        .cloned()
                })
                .collect::<Vec<_>>();
            dependencies.sort();
            tasks.push(MultiStudyWorkflowTask {
                task_id: task_id.clone(),
                action_id: action_id.clone(),
                study_id: study_id.clone(),
                independent_group: budget.independent_group.clone(),
                wave,
                cost_units: action.action.cost_units,
                autonomy_tier: action.action.autonomy_tier,
                effects: action.action.effects.clone(),
                depends_on: dependencies,
                route: route.clone(),
                disposition,
            });
            action_task_ids.insert((action_id.clone(), study_id), task_id);
            reserved_budget = reserved_budget.saturating_add(action_cost);
            budget.compute_units -= action_cost;
            budget.material_units = budget.material_units.saturating_sub(material_need);
            budget.instrument_units = budget.instrument_units.saturating_sub(instrument_need);
        }
        action_task_waves.insert(action_id.clone(), wave);
        scheduled_actions.insert(action_id.clone());
        if tasks
            .iter()
            .filter(|task| task.action_id == action_id)
            .any(|task| task.disposition == MultiStudyTaskDisposition::ApprovalRequired)
        {
            approval_actions.insert(action_id.clone());
        }
    }

    tasks.sort_by(|left, right| {
        left.wave
            .cmp(&right.wave)
            .then_with(|| left.task_id.cmp(&right.task_id))
    });
    let task_order = tasks
        .iter()
        .map(|task| task.task_id.clone())
        .collect::<Vec<_>>();
    let mut wave_order = Vec::new();
    for task in &tasks {
        while wave_order.len() < task.wave {
            wave_order.push(Vec::new());
        }
        wave_order[task.wave - 1].push(task.task_id.clone());
    }
    let mut scheduled_action_order = scheduled_actions.into_iter().collect::<Vec<_>>();
    scheduled_action_order.sort();
    let approval_action_order = approval_actions.into_iter().collect::<Vec<_>>();
    let deferred_action_order = deferred_actions.into_iter().collect::<Vec<_>>();
    let blocked_action_order = blocked_actions.into_iter().collect::<Vec<_>>();
    let disposition = if tasks.is_empty() {
        if !blocked_action_order.is_empty() {
            MultiStudyWorkflowDisposition::Blocked
        } else {
            MultiStudyWorkflowDisposition::Unresolved
        }
    } else if !approval_action_order.is_empty()
        || !deferred_action_order.is_empty()
        || !blocked_action_order.is_empty()
    {
        MultiStudyWorkflowDisposition::Partial
    } else {
        MultiStudyWorkflowDisposition::Ready
    };
    let next_route = if !approval_action_order.is_empty() {
        "researcher_approval_gate"
    } else if tasks
        .iter()
        .any(|task| task.route == "signed_instrument_preflight_and_authorization")
    {
        "glioma_instrument_preflight"
    } else if !tasks.is_empty() {
        "glioma_decision_operating_cycle"
    } else {
        "glioma_researcher_workbench"
    };
    let mut output = MultiStudyWorkflowPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        epoch: request.epoch,
        boundary: PRECLINICAL_BOUNDARY.into(),
        source_artifact_digest: request.artifact.digest.clone(),
        study_order: request.artifact.study_order.clone(),
        eligible_study_order: eligible_studies,
        task_order,
        wave_order,
        scheduled_action_order,
        approval_action_order,
        deferred_action_order,
        blocked_action_order,
        tasks,
        omissions,
        negative_evidence_order: request.artifact.negative_evidence_order.clone(),
        uncertainty_order: request.artifact.uncertainty_order.clone(),
        disposition,
        next_route: next_route.into(),
        budget_units: request.budget_units,
        budget_reserved_units: reserved_budget,
        digest: ContentHash::of_bytes(b"unsealed-glioma-multi-study-workflow"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| MultiStudyWorkflowError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p04_decision_context::multi_study_context_artifact::{
        MultiStudyContextDisposition, MultiStudyDecisionAction,
    };
    use crate::glioma::programs::p04_decision_context::DecisionContextArtifactCompatibility;
    use crate::glioma_engine::{GliomaModality, GliomaModelSystem, GliomaStageKind};
    use bioprism_foundation::{AutonomyTier, Effect};
    use std::collections::BTreeSet;

    fn hash(seed: &str) -> ContentHash {
        ContentHash::of_bytes(seed.as_bytes())
    }

    fn compatibility() -> DecisionContextArtifactCompatibility {
        DecisionContextArtifactCompatibility {
            contract_version: "glioma-context-compat/1".into(),
            consumer_order: vec![
                super::super::decision_context_artifact::DecisionContextArtifactConsumer::LocalAgent,
                super::super::decision_context_artifact::DecisionContextArtifactConsumer::ResearcherWorkbench,
                super::super::decision_context_artifact::DecisionContextArtifactConsumer::RustSdk,
                super::super::decision_context_artifact::DecisionContextArtifactConsumer::PythonSdk,
                super::super::decision_context_artifact::DecisionContextArtifactConsumer::TypeScriptSdk,
                super::super::decision_context_artifact::DecisionContextArtifactConsumer::McpClient,
            ],
            semantic_loss_order: Vec::new(),
            local_raw_data_required: false,
            clinical_decision_capable: false,
        }
    }

    fn action(
        id: &str,
        depends_on: Vec<String>,
        autonomy_tier: AutonomyTier,
    ) -> super::super::decision_context_artifact::DecisionContextArtifactAction {
        super::super::decision_context_artifact::DecisionContextArtifactAction {
            action_id: id.into(),
            claim_id: format!("claim-{id}"),
            kind:
                crate::glioma::programs::p04_decision_context::DecisionActionKind::ValidateMechanism,
            stage_kind: GliomaStageKind::ExperimentDesign,
            target_modality: GliomaModality::FunctionalPerturbation,
            target_model_system: GliomaModelSystem::Organoid,
            priority_milli: 900,
            cost_units: 2,
            depends_on,
            autonomy_tier,
            effects: BTreeSet::from([Effect::ExecuteLocalComputation]),
        }
    }

    fn artifact() -> MultiStudyDecisionContextArtifact {
        let action_a = action("action-a", Vec::new(), AutonomyTier::A1);
        let action_b = action("action-b", vec!["action-a".into()], AutonomyTier::A1);
        let actions = vec![
            MultiStudyDecisionAction {
                action: action_a,
                study_order: vec!["study-a".into(), "study-b".into()],
                independent_group_order: vec!["group-a".into(), "group-b".into()],
                support_milli: 1_000,
                disagreement_milli: 0,
                disposition: MultiStudyActionDisposition::Qualified,
                negative_study_order: Vec::new(),
                unknown_study_order: Vec::new(),
            },
            MultiStudyDecisionAction {
                action: action_b,
                study_order: vec!["study-a".into(), "study-b".into()],
                independent_group_order: vec!["group-a".into(), "group-b".into()],
                support_milli: 1_000,
                disagreement_milli: 0,
                disposition: MultiStudyActionDisposition::Qualified,
                negative_study_order: Vec::new(),
                unknown_study_order: Vec::new(),
            },
        ];
        let mut output = MultiStudyDecisionContextArtifact {
            feature_id: super::super::multi_study_context_artifact::FEATURE_ID.into(),
            output_schema: super::super::multi_study_context_artifact::OUTPUT_SCHEMA.into(),
            artifact_id: "glioma-multi-study-decision-context:epoch-3".into(),
            objective: "mechanism validation".into(),
            epoch: 3,
            boundary: PRECLINICAL_BOUNDARY.into(),
            study_order: vec!["study-a".into(), "study-b".into()],
            study_group: BTreeMap::from([
                ("study-a".into(), "group-a".into()),
                ("study-b".into(), "group-b".into()),
            ]),
            eligible_study_order: vec!["study-a".into(), "study-b".into()],
            omitted_study_order: Vec::new(),
            action_order: vec!["action-a".into(), "action-b".into()],
            frontier_order: vec!["action-b".into()],
            actions,
            omissions: BTreeMap::new(),
            negative_evidence_order: vec!["study-a:negative".into()],
            uncertainty_order: vec!["study-b:unknown".into()],
            disposition: MultiStudyContextDisposition::Qualified,
            compatibility: compatibility(),
            digest: hash("unsealed"),
        };
        let input = serde_json::json!({
            "feature_id": output.feature_id,
            "output_schema": output.output_schema,
            "artifact_id": output.artifact_id,
            "objective": output.objective,
            "epoch": output.epoch,
            "boundary": output.boundary,
            "study_order": output.study_order,
            "study_group": output.study_group,
            "eligible_study_order": output.eligible_study_order,
            "omitted_study_order": output.omitted_study_order,
            "action_order": output.action_order,
            "frontier_order": output.frontier_order,
            "actions": output.actions,
            "omissions": output.omissions,
            "negative_evidence_order": output.negative_evidence_order,
            "uncertainty_order": output.uncertainty_order,
            "disposition": output.disposition,
            "compatibility": output.compatibility,
        });
        output.digest = ContentHash::of_value(&input).unwrap();
        output
    }

    fn reseal(artifact: &mut MultiStudyDecisionContextArtifact) {
        let input = serde_json::json!({
            "feature_id": artifact.feature_id,
            "output_schema": artifact.output_schema,
            "artifact_id": artifact.artifact_id,
            "objective": artifact.objective,
            "epoch": artifact.epoch,
            "boundary": artifact.boundary,
            "study_order": artifact.study_order,
            "study_group": artifact.study_group,
            "eligible_study_order": artifact.eligible_study_order,
            "omitted_study_order": artifact.omitted_study_order,
            "action_order": artifact.action_order,
            "frontier_order": artifact.frontier_order,
            "actions": artifact.actions,
            "omissions": artifact.omissions,
            "negative_evidence_order": artifact.negative_evidence_order,
            "uncertainty_order": artifact.uncertainty_order,
            "disposition": artifact.disposition,
            "compatibility": artifact.compatibility,
        });
        artifact.digest = ContentHash::of_value(&input).unwrap();
    }

    fn request() -> MultiStudyWorkflowRequest {
        MultiStudyWorkflowRequest {
            objective: "mechanism validation".into(),
            epoch: 3,
            maximum_waves: 4,
            maximum_tasks: 16,
            minimum_independent_groups: 2,
            require_replication: true,
            budget_units: 16,
            artifact: artifact(),
            study_budgets: vec![
                MultiStudyStudyBudget {
                    study_id: "study-b".into(),
                    independent_group: "group-b".into(),
                    policy_allowed: true,
                    local_only: true,
                    compute_units: 10,
                    material_units: 10,
                    instrument_units: 10,
                    autonomy_ceiling: AutonomyTier::A1,
                    approved_autonomy: AutonomyTier::A1,
                },
                MultiStudyStudyBudget {
                    study_id: "study-a".into(),
                    independent_group: "group-a".into(),
                    policy_allowed: true,
                    local_only: true,
                    compute_units: 10,
                    material_units: 10,
                    instrument_units: 10,
                    autonomy_ceiling: AutonomyTier::A1,
                    approved_autonomy: AutonomyTier::A1,
                },
            ],
        }
    }

    #[test]
    fn schedules_dependency_safe_replicated_waves() {
        let plan = plan_glioma_multi_study_workflow(&request()).unwrap();
        assert_eq!(plan.disposition, MultiStudyWorkflowDisposition::Ready);
        assert_eq!(plan.tasks.len(), 4);
        assert_eq!(plan.wave_order.len(), 2);
        assert!(plan.wave_order[0]
            .iter()
            .all(|task| task.contains("action-a")));
        assert!(plan.wave_order[1]
            .iter()
            .all(|task| task.contains("action-b")));
        assert_eq!(plan.negative_evidence_order, vec!["study-a:negative"]);
        plan.validate().unwrap();
    }

    #[test]
    fn approval_is_explicit_for_higher_autonomy_actions() {
        let mut request = request();
        request.artifact.actions[0].action.autonomy_tier = AutonomyTier::A2;
        request.artifact.actions[1].action.autonomy_tier = AutonomyTier::A2;
        reseal(&mut request.artifact);
        for budget in &mut request.study_budgets {
            budget.autonomy_ceiling = AutonomyTier::A2;
            budget.approved_autonomy = AutonomyTier::A1;
        }
        let plan = plan_glioma_multi_study_workflow(&request).unwrap();
        assert_eq!(plan.disposition, MultiStudyWorkflowDisposition::Partial);
        assert_eq!(plan.approval_action_order, vec!["action-a", "action-b"]);
        assert!(plan
            .tasks
            .iter()
            .all(|task| task.disposition == MultiStudyTaskDisposition::ApprovalRequired));
        assert_eq!(plan.next_route, "researcher_approval_gate");
    }

    #[test]
    fn missing_group_budget_defers_replication_without_fabricating_support() {
        let mut request = request();
        request
            .study_budgets
            .retain(|budget| budget.study_id == "study-a");
        let plan = plan_glioma_multi_study_workflow(&request).unwrap();
        assert!(plan.tasks.is_empty());
        assert_eq!(plan.disposition, MultiStudyWorkflowDisposition::Blocked);
        assert!(plan
            .omissions
            .values()
            .any(|reason| reason == "independent_group_quorum_unavailable"));
    }

    #[test]
    fn dependency_cycle_is_rejected_as_invalid_artifact() {
        let mut request = request();
        request.artifact.actions[0].action.depends_on = vec!["action-b".into()];
        reseal(&mut request.artifact);
        let error = plan_glioma_multi_study_workflow(&request).unwrap_err();
        assert!(error.to_string().contains("cycle"));
    }

    #[test]
    fn global_budget_never_schedules_a_partial_replication_quorum() {
        let mut request = request();
        request.budget_units = 2;
        let plan = plan_glioma_multi_study_workflow(&request).unwrap();
        assert!(plan.tasks.is_empty());
        assert!(plan.scheduled_action_order.is_empty());
        assert_eq!(plan.budget_reserved_units, 0);
        assert!(plan
            .omissions
            .values()
            .any(|reason| reason == "resource_or_task_capacity_prevents_quorum"));
    }

    #[test]
    fn instrument_and_material_effects_route_through_authorization() {
        let mut request = request();
        for entry in &mut request.artifact.actions {
            entry.action.effects = BTreeSet::from([Effect::InstrumentExecution]);
            entry.action.autonomy_tier = AutonomyTier::A3;
        }
        reseal(&mut request.artifact);
        for budget in &mut request.study_budgets {
            budget.autonomy_ceiling = AutonomyTier::A3;
            budget.approved_autonomy = AutonomyTier::A3;
        }
        let plan = plan_glioma_multi_study_workflow(&request).unwrap();
        assert_eq!(plan.disposition, MultiStudyWorkflowDisposition::Partial);
        assert_eq!(plan.next_route, "researcher_approval_gate");
        assert!(plan.tasks.iter().all(|task| {
            task.route == "signed_instrument_preflight_and_authorization"
                && task.disposition == MultiStudyTaskDisposition::ApprovalRequired
        }));
    }

    #[test]
    fn tight_budget_prefers_distinct_replication_groups_over_duplicate_sites() {
        let mut request = request();
        request.budget_units = 4;
        request.artifact.study_order = vec!["study-a".into(), "study-b".into(), "study-c".into()];
        request.artifact.eligible_study_order = request.artifact.study_order.clone();
        request
            .artifact
            .study_group
            .insert("study-c".into(), "group-b".into());
        request
            .artifact
            .study_group
            .insert("study-b".into(), "group-a".into());
        for entry in &mut request.artifact.actions {
            entry.study_order = request.artifact.study_order.clone();
            entry.independent_group_order = vec!["group-a".into(), "group-b".into()];
        }
        request.study_budgets[0].independent_group = "group-a".into();
        request.study_budgets.push(MultiStudyStudyBudget {
            study_id: "study-c".into(),
            independent_group: "group-b".into(),
            policy_allowed: true,
            local_only: true,
            compute_units: 10,
            material_units: 10,
            instrument_units: 10,
            autonomy_ceiling: AutonomyTier::A1,
            approved_autonomy: AutonomyTier::A1,
        });
        reseal(&mut request.artifact);
        let plan = plan_glioma_multi_study_workflow(&request).unwrap();
        let first_action_studies = plan
            .tasks
            .iter()
            .filter(|task| task.action_id == "action-a")
            .map(|task| task.study_id.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(first_action_studies, BTreeSet::from(["study-a", "study-c"]));
        assert_eq!(plan.budget_reserved_units, 4);
        plan.validate().unwrap();
    }

    #[test]
    fn local_budget_cannot_relabel_a_study_to_fabricate_independence() {
        let mut request = request();
        request.study_budgets[0].independent_group = "fabricated-independent-group".into();
        let error = plan_glioma_multi_study_workflow(&request).unwrap_err();
        assert!(error.to_string().contains("relabel independent groups"));
    }

    #[test]
    fn qualified_frontier_is_blocked_when_a_prerequisite_is_underpowered() {
        let mut request = request();
        request.artifact.actions[0].disposition = MultiStudyActionDisposition::Underpowered;
        reseal(&mut request.artifact);
        let plan = plan_glioma_multi_study_workflow(&request).unwrap();
        assert!(plan.tasks.is_empty());
        assert_eq!(plan.blocked_action_order, vec!["action-b"]);
        assert_eq!(
            plan.omissions.get("action:action-b").map(String::as_str),
            Some("dependency_action_not_qualified")
        );
    }
}

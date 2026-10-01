//! Dependency-safe mechanism workflow compilation for preclinical glioma research.
//!
//! A feedback frontier becomes useful to a research engine only when it can be compiled into a
//! bounded, dependency-closed local workflow. This feature takes the returned P05 frontier and a
//! typed action catalog, closes prerequisites, computes deterministic topological order and
//! parallel execution waves, and refuses actions that violate risk, approval, provenance, or
//! budget policy. It produces an execution plan; it never runs an assay or moves raw data.

use super::feedback_replan::MechanismFeedbackReplan;
use crate::glioma_engine::{GliomaModality, GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P05-F13";
pub const OUTPUT_SCHEMA: &str = "GliomaMechanismWorkflow1@1";
pub const MAX_ACTIONS: usize = 1_024;
pub const MAX_DEPENDENCIES: usize = 64;
pub const MAX_WAVES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismWorkflowAction {
    pub action_id: String,
    pub mechanism_id: String,
    pub model_system: GliomaModelSystem,
    pub modality: GliomaModality,
    pub dependency_order: Vec<String>,
    pub cost_units: u64,
    pub risk_milli: u16,
    pub requires_approval: bool,
    pub input_artifacts: Vec<LocalArtifactRef>,
    pub execution_route: String,
    pub compensation_route: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanismWorkflowNodeStatus {
    Scheduled,
    Deferred,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismWorkflowNode {
    pub action: MechanismWorkflowAction,
    pub status: MechanismWorkflowNodeStatus,
    pub wave: Option<usize>,
    pub cumulative_cost_units: u64,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismWorkflowRequest {
    pub objective: String,
    pub replan: MechanismFeedbackReplan,
    pub actions: Vec<MechanismWorkflowAction>,
    pub budget_units: u64,
    pub max_actions: usize,
    pub max_waves: usize,
    pub maximum_risk_milli: u16,
    pub allow_approval_required: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanismWorkflowDisposition {
    Ready,
    Partial,
    DependencyBlocked,
    PolicyBlocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismWorkflowPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub replan_digest: ContentHash,
    pub node_order: Vec<String>,
    pub topological_order: Vec<String>,
    pub execution_waves: Vec<Vec<String>>,
    pub nodes: Vec<MechanismWorkflowNode>,
    pub scheduled_action_order: Vec<String>,
    pub deferred_action_order: Vec<String>,
    pub blocked_action_order: Vec<String>,
    pub total_cost_units: u64,
    pub budget_units: u64,
    pub budget_remaining_units: u64,
    pub critical_path_units: u64,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: MechanismWorkflowDisposition,
    pub next_route: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MechanismWorkflowError {
    #[error("mechanism workflow request is invalid: {0}")]
    InvalidRequest(String),
    #[error("mechanism workflow input is invalid: {0}")]
    InvalidInput(String),
    #[error("mechanism workflow output is invalid: {0}")]
    InvalidOutput(String),
    #[error("mechanism workflow digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique_nonempty(values: &[String]) -> bool {
    values.iter().all(|value| !value.trim().is_empty())
        && values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

pub(crate) fn digest_input(output: &MechanismWorkflowPlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "replan_digest": output.replan_digest,
        "node_order": output.node_order,
        "topological_order": output.topological_order,
        "execution_waves": output.execution_waves,
        "nodes": output.nodes,
        "scheduled_action_order": output.scheduled_action_order,
        "deferred_action_order": output.deferred_action_order,
        "blocked_action_order": output.blocked_action_order,
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

fn validate_action(action: &MechanismWorkflowAction) -> Result<(), MechanismWorkflowError> {
    if action.action_id.trim().is_empty()
        || action.mechanism_id.trim().is_empty()
        || action.cost_units == 0
        || action.risk_milli > 1_000
        || action.execution_route.trim().is_empty()
        || action.dependency_order.len() > MAX_DEPENDENCIES
        || !canonical(&action.dependency_order)
        || !unique_nonempty(&action.dependency_order)
        || action
            .dependency_order
            .iter()
            .any(|id| id == &action.action_id)
        || action
            .input_artifacts
            .iter()
            .any(|artifact| artifact.validate().is_err())
        || action
            .compensation_route
            .as_deref()
            .is_some_and(|route| route.trim().is_empty())
    {
        return Err(MechanismWorkflowError::InvalidInput(format!(
            "action {} has invalid identity, cost, risk, dependency, artifact, or route fields",
            action.action_id
        )));
    }
    Ok(())
}

fn validate_request(
    request: &MechanismWorkflowRequest,
) -> Result<BTreeMap<String, MechanismWorkflowAction>, MechanismWorkflowError> {
    if request.objective.trim().is_empty()
        || request.objective != request.replan.objective
        || request.actions.is_empty()
        || request.actions.len() > MAX_ACTIONS
        || request.budget_units == 0
        || request.max_actions == 0
        || request.max_actions > MAX_ACTIONS
        || request.max_waves == 0
        || request.max_waves > MAX_WAVES
        || request.maximum_risk_milli > 1_000
    {
        return Err(MechanismWorkflowError::InvalidRequest(
            "objective/replan binding, bounded action and wave counts, positive budget, and risk bounds are required".into(),
        ));
    }
    request
        .replan
        .validate()
        .map_err(|error| MechanismWorkflowError::InvalidInput(error.to_string()))?;
    let mut actions = BTreeMap::new();
    for action in &request.actions {
        validate_action(action)?;
        if actions
            .insert(action.action_id.clone(), action.clone())
            .is_some()
        {
            return Err(MechanismWorkflowError::InvalidInput(
                "action ids must be unique".into(),
            ));
        }
    }
    let candidate_ids = request
        .replan
        .candidate_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if candidate_ids != actions.keys().cloned().collect::<BTreeSet<_>>() {
        return Err(MechanismWorkflowError::InvalidInput(
            "the action catalog must exactly cover the feedback-replan candidate order".into(),
        ));
    }
    for action in actions.values() {
        if action
            .dependency_order
            .iter()
            .any(|dependency| !actions.contains_key(dependency))
        {
            return Err(MechanismWorkflowError::InvalidInput(format!(
                "action {} references an unknown dependency",
                action.action_id
            )));
        }
    }
    Ok(actions)
}

fn closure_for_seed(
    seeds: &[String],
    actions: &BTreeMap<String, MechanismWorkflowAction>,
) -> BTreeSet<String> {
    let mut closure = BTreeSet::new();
    let mut pending = seeds.to_vec();
    while let Some(action_id) = pending.pop() {
        if closure.insert(action_id.clone()) {
            if let Some(action) = actions.get(&action_id) {
                pending.extend(action.dependency_order.iter().cloned());
            }
        }
    }
    closure
}

fn topological_order(
    ids: &BTreeSet<String>,
    actions: &BTreeMap<String, MechanismWorkflowAction>,
) -> Result<Vec<String>, MechanismWorkflowError> {
    let mut indegree = ids
        .iter()
        .map(|id| {
            let count = actions
                .get(id)
                .expect("validated action")
                .dependency_order
                .iter()
                .filter(|dependency| ids.contains(*dependency))
                .count();
            (id.clone(), count)
        })
        .collect::<BTreeMap<_, _>>();
    let mut outgoing = BTreeMap::<String, Vec<String>>::new();
    for id in ids {
        for dependency in &actions.get(id).expect("validated action").dependency_order {
            if ids.contains(dependency) {
                outgoing
                    .entry(dependency.clone())
                    .or_default()
                    .push(id.clone());
            }
        }
    }
    for successors in outgoing.values_mut() {
        successors.sort();
    }
    let mut ready = indegree
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(id, _)| id.clone())
        .collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(ids.len());
    while let Some(id) = ready.pop_first() {
        order.push(id.clone());
        for successor in outgoing.get(&id).into_iter().flatten() {
            let count = indegree.get_mut(successor).expect("validated successor");
            *count = count.saturating_sub(1);
            if *count == 0 {
                ready.insert(successor.clone());
            }
        }
    }
    if order.len() != ids.len() {
        return Err(MechanismWorkflowError::InvalidInput(
            "mechanism action dependency graph contains a cycle".into(),
        ));
    }
    Ok(order)
}

fn score_index(replan: &MechanismFeedbackReplan) -> BTreeMap<String, (bool, Option<String>)> {
    replan
        .scores
        .iter()
        .map(|score| {
            (
                score.action_id.clone(),
                (score.eligible, score.exclusion_reason.clone()),
            )
        })
        .collect()
}

/// Compile a feedback frontier into a dependency-closed, budgeted local workflow.
pub fn compile_glioma_mechanism_workflow(
    request: &MechanismWorkflowRequest,
) -> Result<MechanismWorkflowPlan, MechanismWorkflowError> {
    let actions = validate_request(request)?;
    let scores = score_index(&request.replan);
    let seeds = request.replan.selected_action_order.clone();
    let scope = if seeds.is_empty() {
        actions.keys().cloned().collect::<BTreeSet<_>>()
    } else {
        closure_for_seed(&seeds, &actions)
    };
    let topological = topological_order(&scope, &actions)?;

    let mut blocked_reason = BTreeMap::<String, String>::new();
    for action_id in &scope {
        let action = actions.get(action_id).expect("validated action");
        let (eligible, exclusion_reason) = scores
            .get(action_id)
            .cloned()
            .expect("replan score coverage validated");
        if !eligible {
            blocked_reason.insert(
                action_id.clone(),
                exclusion_reason.unwrap_or_else(|| "feedback-replan-policy".into()),
            );
        } else if action.risk_milli > request.maximum_risk_milli {
            blocked_reason.insert(action_id.clone(), "risk-above-workflow-threshold".into());
        } else if action.requires_approval && !request.allow_approval_required {
            blocked_reason.insert(
                action_id.clone(),
                "approval-required-by-workflow-policy".into(),
            );
        }
    }
    let mut changed = true;
    while changed {
        changed = false;
        for action_id in &topological {
            if blocked_reason.contains_key(action_id) {
                continue;
            }
            let action = actions.get(action_id).expect("validated action");
            if let Some(dependency) = action
                .dependency_order
                .iter()
                .find(|dependency| blocked_reason.contains_key(*dependency))
            {
                blocked_reason.insert(
                    action_id.clone(),
                    format!("dependency-blocked:{dependency}"),
                );
                changed = true;
            }
        }
    }

    let mut deferred_reason = BTreeMap::<String, String>::new();
    let mut scheduled = Vec::new();
    let mut cumulative_by_id = BTreeMap::<String, u64>::new();
    let mut wave_by_id = BTreeMap::<String, usize>::new();
    let mut execution_waves = Vec::<Vec<String>>::new();
    let mut spent = 0_u64;
    for action_id in &topological {
        let action = actions.get(action_id).expect("validated action");
        let dependency_wave = action
            .dependency_order
            .iter()
            .filter_map(|dependency| wave_by_id.get(dependency))
            .copied()
            .max();
        let wave = dependency_wave.map_or(0, |value| value.saturating_add(1));
        let dependency_cost = action
            .dependency_order
            .iter()
            .filter_map(|dependency| cumulative_by_id.get(dependency))
            .copied()
            .max()
            .unwrap_or(0);
        let cumulative = dependency_cost.saturating_add(action.cost_units);
        wave_by_id.insert(action_id.clone(), wave);
        cumulative_by_id.insert(action_id.clone(), cumulative);
        if blocked_reason.contains_key(action_id) {
            continue;
        }
        if action
            .dependency_order
            .iter()
            .any(|dependency| !scheduled.iter().any(|id| id == dependency))
        {
            deferred_reason.insert(action_id.clone(), "dependency-not-scheduled".into());
        } else if wave >= request.max_waves {
            deferred_reason.insert(action_id.clone(), "wave-limit".into());
        } else if scheduled.len() >= request.max_actions {
            deferred_reason.insert(action_id.clone(), "action-limit".into());
        } else if spent.saturating_add(action.cost_units) > request.budget_units {
            deferred_reason.insert(action_id.clone(), "budget-limit".into());
        } else {
            spent = spent.saturating_add(action.cost_units);
            scheduled.push(action_id.clone());
            while execution_waves.len() <= wave {
                execution_waves.push(Vec::new());
            }
            execution_waves[wave].push(action_id.clone());
        }
    }
    execution_waves.retain(|wave| !wave.is_empty());
    for wave in &mut execution_waves {
        wave.sort();
    }
    let blocked_action_order = blocked_reason.keys().cloned().collect::<Vec<_>>();
    let deferred_action_order = deferred_reason.keys().cloned().collect::<Vec<_>>();
    let node_order = scope.iter().cloned().collect::<Vec<_>>();
    let nodes = node_order
        .iter()
        .map(|action_id| {
            let action = actions.get(action_id).expect("validated action").clone();
            let status = if blocked_reason.contains_key(action_id) {
                MechanismWorkflowNodeStatus::Blocked
            } else if deferred_reason.contains_key(action_id) {
                MechanismWorkflowNodeStatus::Deferred
            } else {
                MechanismWorkflowNodeStatus::Scheduled
            };
            let reason = blocked_reason
                .get(action_id)
                .or_else(|| deferred_reason.get(action_id))
                .cloned();
            MechanismWorkflowNode {
                wave: if status == MechanismWorkflowNodeStatus::Scheduled {
                    wave_by_id.get(action_id).copied()
                } else {
                    None
                },
                cumulative_cost_units: *cumulative_by_id
                    .get(action_id)
                    .expect("cumulative cost computed"),
                action,
                status,
                reason,
            }
        })
        .collect::<Vec<_>>();

    let mut negative_evidence = request.replan.negative_evidence.clone();
    negative_evidence.extend(
        blocked_reason
            .iter()
            .map(|(id, reason)| format!("workflow:{id}:blocked:{reason}")),
    );
    negative_evidence.sort();
    negative_evidence.dedup();
    let mut uncertainty = request.replan.uncertainty.clone();
    uncertainty.extend(
        deferred_reason
            .iter()
            .map(|(id, reason)| format!("workflow:{id}:deferred:{reason}")),
    );
    uncertainty.sort();
    uncertainty.dedup();
    let disposition = if scheduled.len() == scope.len() {
        MechanismWorkflowDisposition::Ready
    } else if scheduled.is_empty()
        && blocked_reason.values().any(|reason| {
            reason.contains("policy") || reason.contains("risk") || reason.contains("approval")
        })
    {
        MechanismWorkflowDisposition::PolicyBlocked
    } else if blocked_reason
        .values()
        .any(|reason| reason.starts_with("dependency-blocked:"))
    {
        MechanismWorkflowDisposition::DependencyBlocked
    } else {
        MechanismWorkflowDisposition::Partial
    };
    let next_route = if scheduled.is_empty() {
        "glioma_mechanism_action_plan"
    } else {
        "glioma_mechanism_operating_cycle"
    };
    let mut output = MechanismWorkflowPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        replan_digest: request.replan.digest.clone(),
        node_order,
        topological_order: topological,
        execution_waves,
        nodes,
        scheduled_action_order: scheduled,
        deferred_action_order,
        blocked_action_order,
        total_cost_units: spent,
        budget_units: request.budget_units,
        budget_remaining_units: request.budget_units.saturating_sub(spent),
        critical_path_units: cumulative_by_id.values().copied().max().unwrap_or(0),
        negative_evidence,
        uncertainty,
        disposition,
        next_route: next_route.into(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-mechanism-workflow"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| MechanismWorkflowError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

impl MechanismWorkflowPlan {
    pub fn validate(&self) -> Result<(), MechanismWorkflowError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.replan_digest.as_str().len() != 64
            || !canonical(&self.node_order)
            || !unique_nonempty(&self.node_order)
            || self.nodes.len() != self.node_order.len()
            || self.topological_order.len() != self.node_order.len()
            || self
                .execution_waves
                .iter()
                .any(|wave| wave.is_empty() || !canonical(wave) || !unique_nonempty(wave))
            || !canonical(&self.deferred_action_order)
            || !canonical(&self.blocked_action_order)
            || !unique_nonempty(&self.deferred_action_order)
            || !unique_nonempty(&self.blocked_action_order)
            || !unique_nonempty(&self.scheduled_action_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.next_route.trim().is_empty()
        {
            return Err(MechanismWorkflowError::InvalidOutput(
                "identity, ordering, node, wave, or digest shape is invalid".into(),
            ));
        }
        let node_ids = self.node_order.iter().cloned().collect::<BTreeSet<_>>();
        let topological_ids = self
            .topological_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let scheduled_ids = self
            .scheduled_action_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let deferred_ids = self
            .deferred_action_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let blocked_ids = self
            .blocked_action_order
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
        let node_action_order = self
            .nodes
            .iter()
            .map(|node| node.action.action_id.clone())
            .collect::<Vec<_>>();
        let node_action_ids = node_action_order.iter().cloned().collect::<BTreeSet<_>>();
        let status_scheduled = self
            .nodes
            .iter()
            .filter(|node| node.status == MechanismWorkflowNodeStatus::Scheduled)
            .map(|node| node.action.action_id.clone())
            .collect::<BTreeSet<_>>();
        let status_deferred = self
            .nodes
            .iter()
            .filter(|node| node.status == MechanismWorkflowNodeStatus::Deferred)
            .map(|node| node.action.action_id.clone())
            .collect::<BTreeSet<_>>();
        let status_blocked = self
            .nodes
            .iter()
            .filter(|node| node.status == MechanismWorkflowNodeStatus::Blocked)
            .map(|node| node.action.action_id.clone())
            .collect::<BTreeSet<_>>();
        let scheduled_topological = self
            .topological_order
            .iter()
            .filter(|id| scheduled_ids.contains(*id))
            .cloned()
            .collect::<Vec<_>>();
        if topological_ids != node_ids
            || node_action_order != self.node_order
            || node_action_ids != node_ids
            || partition != node_ids
            || scheduled_ids.len() + deferred_ids.len() + blocked_ids.len() != partition.len()
            || status_scheduled != scheduled_ids
            || status_deferred != deferred_ids
            || status_blocked != blocked_ids
            || scheduled_topological != self.scheduled_action_order
            || self
                .execution_waves
                .iter()
                .flatten()
                .cloned()
                .collect::<BTreeSet<_>>()
                != scheduled_ids
            || self.nodes.iter().any(|node| {
                node.action.action_id.trim().is_empty()
                    || node.action.action_id
                        != self
                            .node_order
                            .iter()
                            .find(|id| **id == node.action.action_id)
                            .cloned()
                            .unwrap_or_default()
                    || node.action.dependency_order.iter().any(|dependency| {
                        !node_ids.contains(dependency)
                            || self
                                .topological_order
                                .iter()
                                .position(|id| id == dependency)
                                >= self
                                    .topological_order
                                    .iter()
                                    .position(|id| id == &node.action.action_id)
                    })
                    || node.cumulative_cost_units < node.action.cost_units
                    || (node.status == MechanismWorkflowNodeStatus::Scheduled
                        && (node.wave.is_none()
                            || node.wave
                                != self.execution_waves.iter().position(|wave| {
                                    wave.iter().any(|id| id == &node.action.action_id)
                                })))
                    || (node.status != MechanismWorkflowNodeStatus::Scheduled
                        && node.wave.is_some())
            })
        {
            return Err(MechanismWorkflowError::InvalidOutput(
                "topology, status partition, wave coverage, dependency order, or node status is invalid".into(),
            ));
        }
        let scheduled_cost = self
            .nodes
            .iter()
            .filter(|node| node.status == MechanismWorkflowNodeStatus::Scheduled)
            .map(|node| node.action.cost_units)
            .sum::<u64>();
        if scheduled_cost != self.total_cost_units
            || self
                .total_cost_units
                .saturating_add(self.budget_remaining_units)
                != self.budget_units
            || self.total_cost_units > self.budget_units
        {
            return Err(MechanismWorkflowError::InvalidOutput(
                "scheduled cost and budget accounting do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| MechanismWorkflowError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(MechanismWorkflowError::Digest(
                "mechanism workflow digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::evidence_assimilation::AssimilatedMechanismStatus;
    use super::super::feedback_replan::{
        MechanismFeedbackActionScore, MechanismFeedbackDecision, MechanismFeedbackOutcome,
        MechanismFeedbackReplanDisposition,
    };
    use super::*;

    fn hash(seed: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"seed": seed})).unwrap()
    }

    fn replan(selected: Vec<&str>) -> MechanismFeedbackReplan {
        let mut output = MechanismFeedbackReplan {
            feature_id: super::super::feedback_replan::FEATURE_ID.into(),
            output_schema: super::super::feedback_replan::OUTPUT_SCHEMA.into(),
            objective: "compile local mechanism workflow".into(),
            prior_plan_digest: hash("prior"),
            mechanism_order: vec!["m1".into()],
            observation_order: vec![],
            decisions: vec![MechanismFeedbackDecision {
                mechanism_id: "m1".into(),
                prior_status: AssimilatedMechanismStatus::Unresolved,
                outcome_order: vec![MechanismFeedbackOutcome::Unresolved],
                next_action_id: selected.first().map(|id| (*id).into()),
                rationale: "retain local uncertainty".into(),
            }],
            candidate_order: vec!["bad".into(), "measure".into(), "prep".into()],
            ranked_action_order: vec!["measure".into(), "bad".into(), "prep".into()],
            selected_action_order: selected.into_iter().map(String::from).collect(),
            deferred_action_order: vec!["prep".into()],
            blocked_action_order: vec!["bad".into()],
            scores: vec![
                MechanismFeedbackActionScore {
                    action_id: "measure".into(),
                    mechanism_id: "m1".into(),
                    prior_priority_milli: 900,
                    feedback_adjustment_milli: 160,
                    observed_information_gain_milli: 200,
                    posterior_priority_milli: 1_000,
                    eligible: true,
                    exclusion_reason: None,
                },
                MechanismFeedbackActionScore {
                    action_id: "bad".into(),
                    mechanism_id: "m1".into(),
                    prior_priority_milli: 400,
                    feedback_adjustment_milli: 0,
                    observed_information_gain_milli: 0,
                    posterior_priority_milli: 400,
                    eligible: false,
                    exclusion_reason: Some("approval-required-by-policy".into()),
                },
                MechanismFeedbackActionScore {
                    action_id: "prep".into(),
                    mechanism_id: "m1".into(),
                    prior_priority_milli: 300,
                    feedback_adjustment_milli: 80,
                    observed_information_gain_milli: 0,
                    posterior_priority_milli: 380,
                    eligible: true,
                    exclusion_reason: None,
                },
            ],
            budget_units: 3,
            total_cost_units: 2,
            budget_remaining_units: 1,
            negative_evidence: vec![],
            uncertainty: vec!["m1:unresolved".into()],
            disposition: MechanismFeedbackReplanDisposition::Partial,
            next_route: "glioma_mechanism_action_plan".into(),
            digest: hash("unsealed"),
        };
        output.digest =
            ContentHash::of_value(&super::super::feedback_replan::digest_input(&output)).unwrap();
        output.validate().unwrap();
        output
    }

    fn action(
        action_id: &str,
        dependencies: &[&str],
        requires_approval: bool,
    ) -> MechanismWorkflowAction {
        MechanismWorkflowAction {
            action_id: action_id.into(),
            mechanism_id: "m1".into(),
            model_system: GliomaModelSystem::Organoid,
            modality: GliomaModality::Imaging,
            dependency_order: dependencies.iter().map(|id| (*id).into()).collect(),
            cost_units: if action_id == "measure" { 2 } else { 1 },
            risk_milli: 100,
            requires_approval,
            input_artifacts: vec![],
            execution_route: "local://glioma/mechanism".into(),
            compensation_route: Some("local://glioma/compensate".into()),
        }
    }

    #[test]
    fn closes_prerequisite_and_emits_parallel_execution_waves() {
        let request = MechanismWorkflowRequest {
            objective: "compile local mechanism workflow".into(),
            replan: replan(vec!["measure"]),
            actions: vec![
                action("bad", &[], true),
                action("measure", &["prep"], false),
                action("prep", &[], false),
            ],
            budget_units: 3,
            max_actions: 3,
            max_waves: 4,
            maximum_risk_milli: 500,
            allow_approval_required: false,
        };
        let plan = compile_glioma_mechanism_workflow(&request).unwrap();
        assert_eq!(plan.scheduled_action_order, vec!["prep", "measure"]);
        assert_eq!(plan.execution_waves, vec![vec!["prep"], vec!["measure"]]);
        assert_eq!(plan.disposition, MechanismWorkflowDisposition::Ready);
        plan.validate().unwrap();
    }

    #[test]
    fn refuses_dependency_cycle_before_any_schedule() {
        let mut feedback = replan(vec!["measure"]);
        feedback.candidate_order = vec!["a".into(), "b".into()];
        feedback.ranked_action_order = feedback.candidate_order.clone();
        feedback.selected_action_order = vec!["a".into()];
        feedback.deferred_action_order = vec!["b".into()];
        feedback.blocked_action_order = vec![];
        feedback.scores = vec![
            MechanismFeedbackActionScore {
                action_id: "a".into(),
                mechanism_id: "m1".into(),
                prior_priority_milli: 500,
                feedback_adjustment_milli: 0,
                observed_information_gain_milli: 0,
                posterior_priority_milli: 500,
                eligible: true,
                exclusion_reason: None,
            },
            MechanismFeedbackActionScore {
                action_id: "b".into(),
                mechanism_id: "m1".into(),
                prior_priority_milli: 400,
                feedback_adjustment_milli: 0,
                observed_information_gain_milli: 0,
                posterior_priority_milli: 400,
                eligible: true,
                exclusion_reason: None,
            },
        ];
        feedback.digest =
            ContentHash::of_value(&super::super::feedback_replan::digest_input(&feedback)).unwrap();
        feedback.validate().unwrap();
        let request = MechanismWorkflowRequest {
            objective: "compile local mechanism workflow".into(),
            replan: feedback,
            actions: vec![action("a", &["b"], false), action("b", &["a"], false)],
            budget_units: 3,
            max_actions: 3,
            max_waves: 4,
            maximum_risk_milli: 500,
            allow_approval_required: true,
        };
        let error = compile_glioma_mechanism_workflow(&request).unwrap_err();
        assert!(error.to_string().contains("cycle"));
    }
}

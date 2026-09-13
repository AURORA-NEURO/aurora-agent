//! Bounded dynamical mechanism simulation for preclinical glioma research.
//!
//! This feature turns a typed mechanism graph into a deterministic discrete-time state model.
//! Signed feedback edges, delayed influence, intervention pulses, stability windows, oscillation,
//! divergence, and intervention sensitivity are all explicit. It is a scientific planning and
//! simulation capability: it never claims that a simulated state is an observed biological fact,
//! never selects a clinical treatment, and never dispatches an assay.

use crate::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P05-F26";
pub const OUTPUT_SCHEMA: &str = "GliomaMechanismDynamics1@1";
pub const MAX_NODES: usize = 128;
pub const MAX_EDGES: usize = 1_024;
pub const MAX_INTERVENTIONS: usize = 128;
pub const MAX_STEPS: u16 = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismDynamicsRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub max_steps: u16,
    pub time_step_milli: u16,
    pub stability_window: u16,
    pub stability_delta_milli: u16,
    pub divergence_abs_milli: u16,
    pub max_selected_interventions: usize,
    pub budget_units: u32,
    pub risk_ceiling_milli: u16,
    pub sensitivity_delta_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismDynamicsNode {
    pub node_id: String,
    pub label: String,
    pub initial_state_milli: i32,
    pub drift_milli: i32,
    pub uncertainty_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismDynamicsEdge {
    pub edge_id: String,
    pub source_node: String,
    pub target_node: String,
    pub weight_milli: i32,
    pub lag_steps: u16,
    pub confidence_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismDynamicsIntervention {
    pub intervention_id: String,
    pub target_node: String,
    pub delta_milli: i32,
    pub start_step: u16,
    pub duration_steps: u16,
    pub cost_units: u32,
    pub risk_milli: u16,
    pub confidence_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismDynamicsState {
    pub node_id: String,
    pub state_milli: i32,
    pub derivative_milli: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismDynamicsStep {
    pub step: u16,
    pub states: Vec<MechanismDynamicsState>,
    pub derivative_l1_milli: u32,
    pub active_intervention_order: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanismDynamicsDisposition {
    Stable,
    Oscillatory,
    Diverging,
    BudgetBlocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismDynamicsSensitivity {
    pub intervention_id: String,
    pub target_node: String,
    pub baseline_terminal_state_milli: i32,
    pub perturbed_terminal_state_milli: i32,
    pub delta_terminal_state_milli: i32,
    pub delta_l1_milli: u32,
    pub informative: bool,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismDynamicsPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub node_order: Vec<String>,
    pub edge_order: Vec<String>,
    pub selected_intervention_order: Vec<String>,
    pub risk_blocked_order: Vec<String>,
    pub budget_blocked_order: Vec<String>,
    pub steps: Vec<MechanismDynamicsStep>,
    pub sensitivities: Vec<MechanismDynamicsSensitivity>,
    pub terminal_state: Vec<MechanismDynamicsState>,
    pub stability_step: Option<u16>,
    pub disposition: MechanismDynamicsDisposition,
    pub budget_remaining_units: u32,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MechanismDynamicsError {
    #[error("mechanism dynamics request is invalid: {0}")]
    InvalidRequest(String),
    #[error("mechanism dynamics node is invalid: {0}")]
    InvalidNode(String),
    #[error("mechanism dynamics edge is invalid: {0}")]
    InvalidEdge(String),
    #[error("mechanism dynamics intervention is invalid: {0}")]
    InvalidIntervention(String),
    #[error("mechanism dynamics output is invalid: {0}")]
    InvalidOutput(String),
    #[error("mechanism dynamics digest failed: {0}")]
    Digest(String),
}

#[derive(Debug, Clone)]
struct SimulationResult {
    steps: Vec<MechanismDynamicsStep>,
    terminal: Vec<i32>,
    disposition: MechanismDynamicsDisposition,
    stability_step: Option<u16>,
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn abs_i32(value: i32) -> u32 {
    value.unsigned_abs()
}

fn validate_request(request: &MechanismDynamicsRequest) -> Result<(), MechanismDynamicsError> {
    if request.objective.trim().is_empty()
        || request.max_steps == 0
        || request.max_steps > MAX_STEPS
        || request.time_step_milli == 0
        || request.time_step_milli > 1_000
        || request.stability_window == 0
        || request.stability_window > request.max_steps
        || request.stability_delta_milli > 1_000
        || request.divergence_abs_milli == 0
        || request.divergence_abs_milli > 1_000
        || request.max_selected_interventions == 0
        || request.max_selected_interventions > MAX_INTERVENTIONS
        || request.budget_units == 0
        || request.risk_ceiling_milli > 1_000
        || request.sensitivity_delta_milli == 0
    {
        return Err(MechanismDynamicsError::InvalidRequest(
            "objective, bounded step/time/stability/divergence limits, positive intervention and budget bounds, and sensitivity threshold are required".into(),
        ));
    }
    Ok(())
}

fn validate_nodes(
    nodes: &[MechanismDynamicsNode],
) -> Result<BTreeMap<String, MechanismDynamicsNode>, MechanismDynamicsError> {
    if nodes.is_empty() || nodes.len() > MAX_NODES {
        return Err(MechanismDynamicsError::InvalidNode(
            "at least one and at most the bounded number of nodes are required".into(),
        ));
    }
    let mut result = BTreeMap::new();
    for node in nodes {
        if node.node_id.trim().is_empty()
            || node.label.trim().is_empty()
            || node.initial_state_milli.unsigned_abs() > 1_000
            || node.drift_milli.unsigned_abs() > 1_000
            || node.uncertainty_milli > 1_000
            || result.insert(node.node_id.clone(), node.clone()).is_some()
        {
            return Err(MechanismDynamicsError::InvalidNode(
                "node identity, bounded state/drift/uncertainty, and uniqueness are required"
                    .into(),
            ));
        }
    }
    Ok(result)
}

fn validate_edges(
    edges: &[MechanismDynamicsEdge],
    nodes: &BTreeMap<String, MechanismDynamicsNode>,
    request: &MechanismDynamicsRequest,
) -> Result<Vec<MechanismDynamicsEdge>, MechanismDynamicsError> {
    if edges.len() > MAX_EDGES {
        return Err(MechanismDynamicsError::InvalidEdge(
            "edge count exceeds the bounded limit".into(),
        ));
    }
    let mut result = edges.to_vec();
    result.sort_by(|left, right| left.edge_id.cmp(&right.edge_id));
    for edge in &result {
        if edge.edge_id.trim().is_empty()
            || edge.source_node.trim().is_empty()
            || edge.target_node.trim().is_empty()
            || !nodes.contains_key(&edge.source_node)
            || !nodes.contains_key(&edge.target_node)
            || edge.weight_milli.unsigned_abs() > 1_000
            || edge.lag_steps > request.max_steps
            || edge.confidence_milli == 0
            || edge.confidence_milli > 1_000
        {
            return Err(MechanismDynamicsError::InvalidEdge(
                "edge identity, node bindings, signed weight, lag, and confidence bounds are required".into(),
            ));
        }
    }
    if result
        .windows(2)
        .any(|pair| pair[0].edge_id == pair[1].edge_id)
    {
        return Err(MechanismDynamicsError::InvalidEdge(
            "edge identities must be unique".into(),
        ));
    }
    Ok(result)
}

fn validate_interventions(
    interventions: &[MechanismDynamicsIntervention],
    nodes: &BTreeMap<String, MechanismDynamicsNode>,
    request: &MechanismDynamicsRequest,
) -> Result<Vec<MechanismDynamicsIntervention>, MechanismDynamicsError> {
    if interventions.len() > MAX_INTERVENTIONS {
        return Err(MechanismDynamicsError::InvalidIntervention(
            "intervention count exceeds the bounded limit".into(),
        ));
    }
    let mut result = interventions.to_vec();
    result.sort_by(|left, right| left.intervention_id.cmp(&right.intervention_id));
    for intervention in &result {
        if intervention.intervention_id.trim().is_empty()
            || intervention.target_node.trim().is_empty()
            || !nodes.contains_key(&intervention.target_node)
            || intervention.delta_milli.unsigned_abs() > 1_000
            || intervention.start_step > request.max_steps
            || intervention.duration_steps == 0
            || intervention.cost_units == 0
            || intervention.risk_milli > 1_000
            || intervention.confidence_milli == 0
            || intervention.confidence_milli > 1_000
        {
            return Err(MechanismDynamicsError::InvalidIntervention(
                "intervention identity, target, signed delta, schedule, positive cost, risk, and confidence bounds are required".into(),
            ));
        }
    }
    if result
        .windows(2)
        .any(|pair| pair[0].intervention_id == pair[1].intervention_id)
    {
        return Err(MechanismDynamicsError::InvalidIntervention(
            "intervention identities must be unique".into(),
        ));
    }
    Ok(result)
}

fn active_interventions(
    interventions: &[MechanismDynamicsIntervention],
    step: u16,
) -> Vec<&MechanismDynamicsIntervention> {
    interventions
        .iter()
        .filter(|intervention| {
            step >= intervention.start_step
                && step.saturating_sub(intervention.start_step) < intervention.duration_steps
        })
        .collect()
}

fn state_snapshot(
    nodes: &BTreeMap<String, MechanismDynamicsNode>,
    values: &[i32],
    previous: &[i32],
) -> Vec<MechanismDynamicsState> {
    nodes
        .keys()
        .enumerate()
        .map(|(index, node_id)| MechanismDynamicsState {
            node_id: node_id.clone(),
            state_milli: values[index],
            derivative_milli: values[index].saturating_sub(previous[index]),
        })
        .collect()
}

fn simulate(
    request: &MechanismDynamicsRequest,
    nodes: &BTreeMap<String, MechanismDynamicsNode>,
    edges: &[MechanismDynamicsEdge],
    interventions: &[MechanismDynamicsIntervention],
) -> SimulationResult {
    let node_ids = nodes.keys().cloned().collect::<Vec<_>>();
    let indices = node_ids
        .iter()
        .enumerate()
        .map(|(index, id)| (id.clone(), index))
        .collect::<BTreeMap<_, _>>();
    let initial = node_ids
        .iter()
        .map(|id| nodes[id].initial_state_milli)
        .collect::<Vec<_>>();
    let mut values = initial.clone();
    let mut previous = initial.clone();
    let mut history = vec![initial.clone()];
    let mut steps = Vec::new();
    let mut stable_count = 0_u16;
    let mut stability_step = None;
    let mut disposition = MechanismDynamicsDisposition::Unresolved;

    for step in 0..=request.max_steps {
        let active = active_interventions(interventions, step);
        let derivative_l1 = values
            .iter()
            .zip(previous.iter())
            .map(|(current, prior)| abs_i32(current.saturating_sub(*prior)))
            .sum::<u32>();
        steps.push(MechanismDynamicsStep {
            step,
            states: state_snapshot(nodes, &values, &previous),
            derivative_l1_milli: derivative_l1,
            active_intervention_order: active
                .iter()
                .map(|intervention| intervention.intervention_id.clone())
                .collect(),
        });
        if stable_count >= request.stability_window {
            disposition = MechanismDynamicsDisposition::Stable;
            stability_step = Some(step.saturating_sub(request.stability_window));
            break;
        }
        if step == request.max_steps {
            break;
        }

        let mut next = Vec::with_capacity(values.len());
        for (target_index, target_id) in node_ids.iter().enumerate() {
            let mut net = i64::from(nodes[target_id].drift_milli);
            for edge in edges.iter().filter(|edge| edge.target_node == *target_id) {
                let source_index = indices[&edge.source_node];
                let history_index = history
                    .len()
                    .saturating_sub(1 + usize::from(edge.lag_steps));
                let source_state = history
                    .get(history_index)
                    .and_then(|state| state.get(source_index))
                    .copied()
                    .unwrap_or(values[source_index]);
                let influence = i64::from(edge.weight_milli)
                    .saturating_mul(i64::from(source_state))
                    .saturating_div(1_000)
                    .saturating_mul(i64::from(edge.confidence_milli))
                    .saturating_div(1_000);
                net = net.saturating_add(influence);
            }
            for intervention in active
                .iter()
                .filter(|intervention| intervention.target_node == *target_id)
            {
                net = net.saturating_add(
                    i64::from(intervention.delta_milli)
                        .saturating_mul(i64::from(intervention.confidence_milli))
                        .saturating_div(1_000),
                );
            }
            let update = net
                .saturating_mul(i64::from(request.time_step_milli))
                .saturating_div(1_000);
            let candidate = i64::from(values[target_index]).saturating_add(update);
            next.push(candidate.clamp(-1_000, 1_000) as i32);
        }
        let derivative = next
            .iter()
            .zip(values.iter())
            .map(|(after, before)| abs_i32(after.saturating_sub(*before)))
            .sum::<u32>();
        if next
            .iter()
            .any(|value| value.unsigned_abs() >= u32::from(request.divergence_abs_milli))
        {
            values = next;
            disposition = MechanismDynamicsDisposition::Diverging;
            break;
        }
        if history
            .iter()
            .rev()
            .nth(1)
            .is_some_and(|prior| prior == &next)
            && derivative > u32::from(request.stability_delta_milli)
        {
            values = next;
            disposition = MechanismDynamicsDisposition::Oscillatory;
            break;
        }
        if derivative <= u32::from(request.stability_delta_milli) {
            stable_count = stable_count.saturating_add(1);
        } else {
            stable_count = 0;
        }
        previous = values;
        values = next;
        history.push(values.clone());
    }
    if disposition == MechanismDynamicsDisposition::Unresolved
        && steps.len() >= usize::from(request.max_steps)
    {
        disposition = MechanismDynamicsDisposition::Unresolved;
    }
    SimulationResult {
        steps,
        terminal: values,
        disposition,
        stability_step,
    }
}

fn digest_input(plan: &MechanismDynamicsPlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": plan.feature_id,
        "output_schema": plan.output_schema,
        "objective": plan.objective,
        "model_system": plan.model_system,
        "node_order": plan.node_order,
        "edge_order": plan.edge_order,
        "selected_intervention_order": plan.selected_intervention_order,
        "risk_blocked_order": plan.risk_blocked_order,
        "budget_blocked_order": plan.budget_blocked_order,
        "steps": plan.steps,
        "sensitivities": plan.sensitivities,
        "terminal_state": plan.terminal_state,
        "stability_step": plan.stability_step,
        "disposition": plan.disposition,
        "budget_remaining_units": plan.budget_remaining_units,
        "negative_evidence": plan.negative_evidence,
        "uncertainty": plan.uncertainty,
    })
}

impl MechanismDynamicsPlan {
    pub fn validate(&self) -> Result<(), MechanismDynamicsError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || !canonical(&self.node_order)
            || !canonical(&self.edge_order)
            || !canonical(&self.selected_intervention_order)
            || !canonical(&self.risk_blocked_order)
            || !canonical(&self.budget_blocked_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.steps.is_empty()
            || self
                .steps
                .windows(2)
                .any(|pair| pair[0].step >= pair[1].step)
            || self
                .steps
                .iter()
                .any(|step| !canonical(&step.active_intervention_order))
            || self.terminal_state.len() != self.node_order.len()
            || self
                .terminal_state
                .windows(2)
                .any(|pair| pair[0].node_id >= pair[1].node_id)
            || self
                .sensitivities
                .windows(2)
                .any(|pair| pair[0].intervention_id >= pair[1].intervention_id)
        {
            return Err(MechanismDynamicsError::InvalidOutput(
                "identity, canonical ordering, step trajectory, terminal state, or sensitivity invariants are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| MechanismDynamicsError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(MechanismDynamicsError::InvalidOutput(
                "mechanism dynamics digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Simulate a bounded signed mechanism network and rank intervention pulses by deterministic
/// sensitivity-per-cost. The result is a model-backed planning artifact, not an observation.
pub fn simulate_glioma_mechanism_dynamics(
    request: &MechanismDynamicsRequest,
    nodes: &[MechanismDynamicsNode],
    edges: &[MechanismDynamicsEdge],
    interventions: &[MechanismDynamicsIntervention],
) -> Result<MechanismDynamicsPlan, MechanismDynamicsError> {
    validate_request(request)?;
    let nodes = validate_nodes(nodes)?;
    let edges = validate_edges(edges, &nodes, request)?;
    let interventions = validate_interventions(interventions, &nodes, request)?;
    let mut negative_evidence = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for node in nodes.values() {
        if node.uncertainty_milli > request.stability_delta_milli {
            uncertainty.insert(format!("{}:initial-state-uncertainty", node.node_id));
        }
    }
    let mut scores = interventions
        .iter()
        .map(|intervention| {
            let score = u64::from(abs_i32(intervention.delta_milli))
                .saturating_mul(u64::from(intervention.confidence_milli))
                .saturating_mul(1_000_u64.saturating_sub(u64::from(intervention.risk_milli)))
                / u64::from(intervention.cost_units.max(1));
            (intervention.intervention_id.clone(), score)
        })
        .collect::<Vec<_>>();
    scores.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    let mut selected = Vec::new();
    let mut risk_blocked = BTreeSet::new();
    let mut budget_blocked = BTreeSet::new();
    let mut remaining_budget = request.budget_units;
    for (intervention_id, _) in scores {
        let intervention = interventions
            .iter()
            .find(|intervention| intervention.intervention_id == intervention_id)
            .expect("score was derived from interventions");
        if intervention.risk_milli > request.risk_ceiling_milli {
            risk_blocked.insert(intervention_id);
            uncertainty.insert(format!(
                "{}:risk-exceeds-ceiling",
                intervention.intervention_id
            ));
            continue;
        }
        if selected.len() >= request.max_selected_interventions {
            budget_blocked.insert(intervention_id);
            continue;
        }
        if u64::from(intervention.cost_units) > u64::from(remaining_budget) {
            budget_blocked.insert(intervention_id);
            uncertainty.insert(format!(
                "{}:budget-insufficient",
                intervention.intervention_id
            ));
            continue;
        }
        remaining_budget = remaining_budget.saturating_sub(intervention.cost_units);
        selected.push(intervention.clone());
    }
    selected.sort_by(|left, right| left.intervention_id.cmp(&right.intervention_id));
    let baseline = simulate(request, &nodes, &edges, &[]);
    let combined = simulate(request, &nodes, &edges, &selected);
    let sensitivities = selected
        .iter()
        .map(|intervention| {
            let leave_one_out = selected
                .iter()
                .filter(|candidate| candidate.intervention_id != intervention.intervention_id)
                .cloned()
                .collect::<Vec<_>>();
            let perturbed = simulate(request, &nodes, &edges, &leave_one_out);
            let target_index = nodes
                .keys()
                .position(|node_id| node_id == &intervention.target_node)
                .expect("validated target node");
            let delta_l1 = combined
                .terminal
                .iter()
                .zip(perturbed.terminal.iter())
                .map(|(combined, perturbed)| abs_i32(combined.saturating_sub(*perturbed)))
                .sum::<u32>();
            let target_delta =
                perturbed.terminal[target_index].saturating_sub(combined.terminal[target_index]);
            let informative = delta_l1 >= u32::from(request.sensitivity_delta_milli);
            MechanismDynamicsSensitivity {
                intervention_id: intervention.intervention_id.clone(),
                target_node: intervention.target_node.clone(),
                baseline_terminal_state_milli: baseline.terminal[target_index],
                perturbed_terminal_state_milli: perturbed.terminal[target_index],
                delta_terminal_state_milli: target_delta,
                delta_l1_milli: delta_l1,
                informative,
                rationale: if informative {
                    "leave-one-out terminal state changes beyond the declared sensitivity gate"
                        .into()
                } else {
                    "leave-one-out change remains below the declared sensitivity gate".into()
                },
            }
        })
        .collect::<Vec<_>>();
    if matches!(
        combined.disposition,
        MechanismDynamicsDisposition::Oscillatory
    ) {
        uncertainty.insert("combined-network-oscillation".into());
    }
    if matches!(
        combined.disposition,
        MechanismDynamicsDisposition::Diverging
    ) {
        negative_evidence.insert("combined-network-divergence".into());
    }
    let disposition = if selected.is_empty() && !budget_blocked.is_empty() {
        MechanismDynamicsDisposition::BudgetBlocked
    } else {
        combined.disposition
    };
    let terminal_state = nodes
        .keys()
        .enumerate()
        .map(|(index, node_id)| MechanismDynamicsState {
            node_id: node_id.clone(),
            state_milli: combined.terminal[index],
            derivative_milli: combined.terminal[index]
                .saturating_sub(nodes[node_id].initial_state_milli),
        })
        .collect::<Vec<_>>();
    let mut plan = MechanismDynamicsPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        node_order: nodes.keys().cloned().collect(),
        edge_order: edges.iter().map(|edge| edge.edge_id.clone()).collect(),
        selected_intervention_order: selected
            .iter()
            .map(|intervention| intervention.intervention_id.clone())
            .collect(),
        risk_blocked_order: risk_blocked.into_iter().collect(),
        budget_blocked_order: budget_blocked.into_iter().collect(),
        steps: combined.steps,
        sensitivities,
        terminal_state,
        stability_step: combined.stability_step,
        disposition,
        budget_remaining_units: remaining_budget,
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-mechanism-dynamics"),
    };
    plan.digest = ContentHash::of_value(&digest_input(&plan))
        .map_err(|error| MechanismDynamicsError::Digest(error.to_string()))?;
    plan.validate()?;
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> MechanismDynamicsRequest {
        MechanismDynamicsRequest {
            objective: "simulate hypoxia-driven invasion feedback".into(),
            model_system: GliomaModelSystem::Organoid,
            max_steps: 24,
            time_step_milli: 200,
            stability_window: 3,
            stability_delta_milli: 2,
            divergence_abs_milli: 1_000,
            max_selected_interventions: 2,
            budget_units: 8,
            risk_ceiling_milli: 800,
            sensitivity_delta_milli: 5,
        }
    }

    fn nodes() -> Vec<MechanismDynamicsNode> {
        vec![
            MechanismDynamicsNode {
                node_id: "hypoxia".into(),
                label: "hypoxia state".into(),
                initial_state_milli: 300,
                drift_milli: -30,
                uncertainty_milli: 20,
            },
            MechanismDynamicsNode {
                node_id: "invasion".into(),
                label: "invasion state".into(),
                initial_state_milli: 100,
                drift_milli: -10,
                uncertainty_milli: 20,
            },
        ]
    }

    fn edges() -> Vec<MechanismDynamicsEdge> {
        vec![
            MechanismDynamicsEdge {
                edge_id: "hypoxia-to-invasion".into(),
                source_node: "hypoxia".into(),
                target_node: "invasion".into(),
                weight_milli: 500,
                lag_steps: 0,
                confidence_milli: 900,
            },
            MechanismDynamicsEdge {
                edge_id: "invasion-to-hypoxia".into(),
                source_node: "invasion".into(),
                target_node: "hypoxia".into(),
                weight_milli: -100,
                lag_steps: 1,
                confidence_milli: 800,
            },
        ]
    }

    fn intervention(id: &str, delta: i32, risk: u16) -> MechanismDynamicsIntervention {
        MechanismDynamicsIntervention {
            intervention_id: id.into(),
            target_node: "hypoxia".into(),
            delta_milli: delta,
            start_step: 0,
            duration_steps: 6,
            cost_units: 2,
            risk_milli: risk,
            confidence_milli: 900,
        }
    }

    #[test]
    fn stable_feedback_network_ranks_intervention_and_is_replay_stable() {
        let request = request();
        let interventions = vec![intervention("oxygenation", -250, 200)];
        let first =
            simulate_glioma_mechanism_dynamics(&request, &nodes(), &edges(), &interventions)
                .unwrap();
        let mut reversed_nodes = nodes();
        reversed_nodes.reverse();
        let mut reversed_edges = edges();
        reversed_edges.reverse();
        let second = simulate_glioma_mechanism_dynamics(
            &request,
            &reversed_nodes,
            &reversed_edges,
            &interventions,
        )
        .unwrap();
        assert_eq!(first, second);
        assert!(first
            .selected_intervention_order
            .contains(&"oxygenation".into()));
        assert_eq!(first.sensitivities.len(), 1);
        first.validate().unwrap();
    }

    #[test]
    fn risk_and_budget_blocks_are_explicit() {
        let mut request = request();
        request.risk_ceiling_milli = 100;
        request.budget_units = 1;
        let plan = simulate_glioma_mechanism_dynamics(
            &request,
            &nodes(),
            &edges(),
            &[
                intervention("unsafe", -250, 900),
                intervention("expensive", -200, 50),
            ],
        )
        .unwrap();
        assert!(plan.risk_blocked_order.contains(&"unsafe".into()));
        assert!(plan.budget_blocked_order.contains(&"expensive".into()));
        assert_eq!(
            plan.disposition,
            MechanismDynamicsDisposition::BudgetBlocked
        );
    }

    #[test]
    fn unstable_network_remains_oscillatory_or_unresolved() {
        let mut request = request();
        request.time_step_milli = 1_000;
        request.max_steps = 16;
        let nodes = vec![MechanismDynamicsNode {
            node_id: "state".into(),
            label: "state".into(),
            initial_state_milli: 500,
            drift_milli: 0,
            uncertainty_milli: 0,
        }];
        let edges = vec![
            MechanismDynamicsEdge {
                edge_id: "self-negative-a".into(),
                source_node: "state".into(),
                target_node: "state".into(),
                weight_milli: -1_000,
                lag_steps: 0,
                confidence_milli: 1_000,
            },
            MechanismDynamicsEdge {
                edge_id: "self-negative-b".into(),
                source_node: "state".into(),
                target_node: "state".into(),
                weight_milli: -1_000,
                lag_steps: 0,
                confidence_milli: 1_000,
            },
        ];
        let plan = simulate_glioma_mechanism_dynamics(&request, &nodes, &edges, &[]).unwrap();
        assert!(matches!(
            plan.disposition,
            MechanismDynamicsDisposition::Oscillatory | MechanismDynamicsDisposition::Unresolved
        ));
    }
}

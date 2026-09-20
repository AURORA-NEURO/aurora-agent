//! Deterministic beam-search selection of resource-feasible protocol branches.
//!
//! A protocol often has several typed ways to acquire the same downstream artifact: a faster
//! imaging pass, a higher-information sequencing pass, or a lower-risk replicate. This feature
//! searches those alternatives against the real P07 resource scheduler instead of ranking them
//! from a static heuristic alone. It never executes a branch and never turns simulation into
//! biological evidence.

use super::simulator::{
    simulate_glioma_protocol, ProtocolDisposition, ProtocolSimulation, ProtocolSimulationRequest,
    ProtocolTask,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P07-F04";
pub const OUTPUT_SCHEMA: &str = "GliomaProtocolBranchOptimization1@1";
pub const MAX_CANDIDATES: usize = 1_024;
pub const MAX_BRANCHES: usize = 256;
pub const MAX_BEAM_WIDTH: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolBranchCandidate {
    pub candidate_id: String,
    pub task: ProtocolTask,
    pub expected_information_milli: u16,
    pub evidence_prior_milli: u16,
    pub cost_units: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolBranchWeights {
    pub information_milli: u16,
    pub feasibility_milli: u16,
    pub time_milli: u16,
    pub risk_milli: u16,
    pub cost_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolBranchOptimizationRequest {
    pub objective: String,
    pub base_protocol: ProtocolSimulationRequest,
    pub candidates: Vec<ProtocolBranchCandidate>,
    pub budget_units: u64,
    pub max_branches: usize,
    pub beam_width: usize,
    pub weights: ProtocolBranchWeights,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolBranchEvaluation {
    pub branch_id: String,
    pub selected_candidate_order: Vec<String>,
    pub simulation_digest: ContentHash,
    pub disposition: ProtocolDisposition,
    pub score_milli: u32,
    pub projected_cost_units: u64,
    pub expected_information_milli: u32,
    pub makespan_ticks: u32,
    pub risk_total_milli: u32,
    pub unscheduled_order: Vec<String>,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolBranchOptimizationDisposition {
    Qualified,
    Partial,
    BudgetBlocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolBranchOptimizationPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub baseline_simulation_digest: ContentHash,
    pub selected_branch_id: String,
    pub selected_candidate_order: Vec<String>,
    pub evaluation_order: Vec<String>,
    pub evaluations: Vec<ProtocolBranchEvaluation>,
    pub unresolved_candidate_order: Vec<String>,
    pub budget_remaining_units: u64,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub disposition: ProtocolBranchOptimizationDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProtocolBranchOptimizationError {
    #[error("protocol branch optimization request is invalid: {0}")]
    InvalidRequest(String),
    #[error("protocol branch candidate is invalid: {0}")]
    InvalidCandidate(String),
    #[error("protocol branch optimization output is invalid: {0}")]
    InvalidOutput(String),
    #[error("protocol branch simulation failed: {0}")]
    Simulation(String),
    #[error("protocol branch digest failed: {0}")]
    Digest(String),
}

#[derive(Debug, Clone)]
struct BranchState<'a> {
    choices: Vec<&'a ProtocolBranchCandidate>,
    projected_cost_units: u64,
    expected_information_milli: u32,
    evidence_prior_milli: u32,
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(plan: &ProtocolBranchOptimizationPlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": plan.feature_id,
        "output_schema": plan.output_schema,
        "objective": plan.objective,
        "baseline_simulation_digest": plan.baseline_simulation_digest,
        "selected_branch_id": plan.selected_branch_id,
        "selected_candidate_order": plan.selected_candidate_order,
        "evaluation_order": plan.evaluation_order,
        "evaluations": plan.evaluations,
        "unresolved_candidate_order": plan.unresolved_candidate_order,
        "budget_remaining_units": plan.budget_remaining_units,
        "uncertainty": plan.uncertainty,
        "negative_evidence": plan.negative_evidence,
        "disposition": plan.disposition,
    })
}

fn validate_request(
    request: &ProtocolBranchOptimizationRequest,
) -> Result<(), ProtocolBranchOptimizationError> {
    let weights = request.weights;
    let weight_total = u32::from(weights.information_milli)
        + u32::from(weights.feasibility_milli)
        + u32::from(weights.time_milli)
        + u32::from(weights.risk_milli)
        + u32::from(weights.cost_milli);
    if request.objective.trim().is_empty()
        || request.base_protocol.objective != request.objective
        || request.candidates.is_empty()
        || request.candidates.len() > MAX_CANDIDATES
        || request.budget_units == 0
        || request.max_branches == 0
        || request.max_branches > MAX_BRANCHES
        || request.beam_width == 0
        || request.beam_width > MAX_BEAM_WIDTH
        || weight_total == 0
        || weight_total > 5_000
    {
        return Err(ProtocolBranchOptimizationError::InvalidRequest(
            "objective must bind the base protocol, candidates and bounds must be finite, budget must be positive, and at least one score weight is required".into(),
        ));
    }
    Ok(())
}

fn validate_candidate(
    candidate: &ProtocolBranchCandidate,
    base_tasks: &BTreeMap<String, &ProtocolTask>,
    candidate_ids: &mut BTreeSet<String>,
) -> Result<(), ProtocolBranchOptimizationError> {
    if candidate.candidate_id.trim().is_empty()
        || !candidate_ids.insert(candidate.candidate_id.clone())
        || candidate.expected_information_milli > 1_000
        || candidate.evidence_prior_milli > 1_000
        || candidate.cost_units == 0
    {
        return Err(ProtocolBranchOptimizationError::InvalidCandidate(format!(
            "candidate {} has an invalid identity, information, prior, or cost bound",
            candidate.candidate_id
        )));
    }
    let base = base_tasks.get(&candidate.task.task_id).ok_or_else(|| {
        ProtocolBranchOptimizationError::InvalidCandidate(format!(
            "candidate {} replaces an unknown task {}",
            candidate.candidate_id, candidate.task.task_id
        ))
    })?;
    if candidate.task.model_system != base.model_system
        || candidate.task.output_schema != base.output_schema
        || candidate.task.depends_on != base.depends_on
        || candidate.task.task_id.trim().is_empty()
        || candidate.task.label.trim().is_empty()
        || candidate.task.resource_units == 0
        || candidate.task.duration_ticks == 0
        || candidate.task.risk_milli > 1_000
    {
        return Err(ProtocolBranchOptimizationError::InvalidCandidate(format!(
            "candidate {} changes model, output contract, dependency topology, or task bounds",
            candidate.candidate_id
        )));
    }
    Ok(())
}

fn branch_id(
    selected: &[&ProtocolBranchCandidate],
) -> Result<String, ProtocolBranchOptimizationError> {
    if selected.is_empty() {
        return Ok("baseline".into());
    }
    let ids = selected
        .iter()
        .map(|candidate| candidate.candidate_id.clone())
        .collect::<Vec<_>>();
    let digest = ContentHash::of_value(&serde_json::json!(ids))
        .map_err(|error| ProtocolBranchOptimizationError::Digest(error.to_string()))?;
    Ok(format!("branch-{}", &digest.as_str()[..16]))
}

fn apply_state(
    request: &ProtocolBranchOptimizationRequest,
    state: &BranchState<'_>,
) -> ProtocolSimulationRequest {
    let replacements = state
        .choices
        .iter()
        .map(|candidate| (candidate.task.task_id.clone(), candidate.task.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut protocol = request.base_protocol.clone();
    for task in &mut protocol.tasks {
        if let Some(replacement) = replacements.get(&task.task_id) {
            *task = replacement.clone();
        }
    }
    protocol
}

fn score_simulation(
    simulation: &ProtocolSimulation,
    state: &BranchState<'_>,
    request: &ProtocolBranchOptimizationRequest,
) -> u32 {
    let weights = request.weights;
    let feasibility = if simulation.disposition == ProtocolDisposition::Feasible {
        1_000u32
    } else {
        0
    };
    let time = if simulation.makespan_ticks >= request.base_protocol.max_ticks {
        0
    } else {
        ((u64::from(request.base_protocol.max_ticks - simulation.makespan_ticks) * 1_000)
            / u64::from(request.base_protocol.max_ticks)) as u32
    };
    let risk = 1_000u32.saturating_sub(
        (simulation.risk_total_milli.min(1_000) * 1_000)
            / u32::from(request.base_protocol.max_risk_milli.max(1)),
    );
    let cost = ((request
        .budget_units
        .saturating_sub(state.projected_cost_units)
        .min(request.budget_units)
        * 1_000)
        / request.budget_units) as u32;
    let information = state.expected_information_milli.min(1_000);
    let total_weight = u32::from(weights.information_milli)
        + u32::from(weights.feasibility_milli)
        + u32::from(weights.time_milli)
        + u32::from(weights.risk_milli)
        + u32::from(weights.cost_milli);
    let weighted = information * u32::from(weights.information_milli)
        + feasibility * u32::from(weights.feasibility_milli)
        + time * u32::from(weights.time_milli)
        + risk * u32::from(weights.risk_milli)
        + cost * u32::from(weights.cost_milli);
    weighted / total_weight.max(1)
}

fn validate_plan(
    plan: &ProtocolBranchOptimizationPlan,
) -> Result<(), ProtocolBranchOptimizationError> {
    if plan.feature_id != FEATURE_ID
        || plan.output_schema != OUTPUT_SCHEMA
        || plan.objective.trim().is_empty()
        || plan.baseline_simulation_digest.as_str().len() != 64
        || plan.selected_branch_id.trim().is_empty()
        || !canonical(&plan.selected_candidate_order)
        || !canonical(&plan.evaluation_order)
        || !canonical(&plan.unresolved_candidate_order)
        || !canonical(&plan.uncertainty)
        || !canonical(&plan.negative_evidence)
        || plan.evaluations.is_empty()
        || plan.evaluations.len() != plan.evaluation_order.len()
        || plan
            .evaluations
            .iter()
            .map(|evaluation| evaluation.branch_id.clone())
            .collect::<Vec<_>>()
            != plan.evaluation_order
        || plan.evaluations.iter().any(|evaluation| {
            evaluation.branch_id.trim().is_empty()
                || evaluation.simulation_digest.as_str().len() != 64
                || evaluation.score_milli > 1_000
                || evaluation
                    .selected_candidate_order
                    .windows(2)
                    .any(|pair| pair[0] >= pair[1])
                || evaluation.projected_cost_units == 0 && evaluation.branch_id != "baseline"
                || evaluation.rationale.trim().is_empty()
                || evaluation
                    .unscheduled_order
                    .windows(2)
                    .any(|pair| pair[0] > pair[1])
        })
    {
        return Err(ProtocolBranchOptimizationError::InvalidOutput(
            "identity, ordering, score, digest, or evaluation invariants are invalid".into(),
        ));
    }
    if !plan
        .evaluations
        .iter()
        .any(|evaluation| evaluation.branch_id == plan.selected_branch_id)
    {
        return Err(ProtocolBranchOptimizationError::InvalidOutput(
            "selected branch is absent from evaluations".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(plan))
        .map_err(|error| ProtocolBranchOptimizationError::Digest(error.to_string()))?;
    if expected != plan.digest {
        return Err(ProtocolBranchOptimizationError::InvalidOutput(
            "digest is not bound to the optimization plan".into(),
        ));
    }
    Ok(())
}

impl ProtocolBranchOptimizationPlan {
    pub fn validate(&self) -> Result<(), ProtocolBranchOptimizationError> {
        validate_plan(self)
    }
}

/// Select a bounded, resource-feasible protocol branch using deterministic beam search.
pub fn optimize_glioma_protocol_branches(
    request: &ProtocolBranchOptimizationRequest,
) -> Result<ProtocolBranchOptimizationPlan, ProtocolBranchOptimizationError> {
    validate_request(request)?;
    let baseline = simulate_glioma_protocol(&request.base_protocol)
        .map_err(|error| ProtocolBranchOptimizationError::Simulation(error.to_string()))?;
    let base_tasks = request
        .base_protocol
        .tasks
        .iter()
        .map(|task| (task.task_id.clone(), task))
        .collect::<BTreeMap<_, _>>();
    let mut candidate_ids = BTreeSet::new();
    let mut grouped = BTreeMap::<String, Vec<&ProtocolBranchCandidate>>::new();
    for candidate in &request.candidates {
        validate_candidate(candidate, &base_tasks, &mut candidate_ids)?;
        if u64::from(candidate.cost_units) <= request.budget_units {
            grouped
                .entry(candidate.task.task_id.clone())
                .or_default()
                .push(candidate);
        }
    }
    for candidates in grouped.values_mut() {
        candidates.sort_by(|left, right| left.candidate_id.cmp(&right.candidate_id));
    }
    let mut beam = vec![BranchState {
        choices: Vec::new(),
        projected_cost_units: 0,
        expected_information_milli: 0,
        evidence_prior_milli: 0,
    }];
    for candidates in grouped.values() {
        let mut expanded = Vec::new();
        for state in &beam {
            expanded.push(state.clone());
            for candidate in candidates {
                let cost = state
                    .projected_cost_units
                    .saturating_add(u64::from(candidate.cost_units));
                if cost > request.budget_units {
                    continue;
                }
                let mut choices = state.choices.clone();
                choices.push(*candidate);
                expanded.push(BranchState {
                    choices,
                    projected_cost_units: cost,
                    expected_information_milli: state
                        .expected_information_milli
                        .saturating_add(u32::from(candidate.expected_information_milli)),
                    evidence_prior_milli: state
                        .evidence_prior_milli
                        .saturating_add(u32::from(candidate.evidence_prior_milli)),
                });
            }
        }
        expanded.sort_by(|left, right| {
            right
                .expected_information_milli
                .cmp(&left.expected_information_milli)
                .then_with(|| right.evidence_prior_milli.cmp(&left.evidence_prior_milli))
                .then_with(|| left.projected_cost_units.cmp(&right.projected_cost_units))
                .then_with(|| {
                    let left_ids = left
                        .choices
                        .iter()
                        .map(|candidate| candidate.candidate_id.as_str())
                        .collect::<Vec<_>>();
                    let right_ids = right
                        .choices
                        .iter()
                        .map(|candidate| candidate.candidate_id.as_str())
                        .collect::<Vec<_>>();
                    left_ids.cmp(&right_ids)
                })
        });
        expanded.truncate(request.beam_width);
        beam = expanded;
    }
    if beam.is_empty() {
        return Err(ProtocolBranchOptimizationError::InvalidOutput(
            "beam search produced no baseline state".into(),
        ));
    }
    let mut states = beam;
    states.sort_by(|left, right| {
        let left_ids = left
            .choices
            .iter()
            .map(|candidate| candidate.candidate_id.as_str())
            .collect::<Vec<_>>();
        let right_ids = right
            .choices
            .iter()
            .map(|candidate| candidate.candidate_id.as_str())
            .collect::<Vec<_>>();
        left_ids.cmp(&right_ids)
    });
    states.truncate(request.max_branches);
    let mut evaluated_candidates = BTreeSet::new();
    let mut evaluations = Vec::new();
    let mut negative_evidence = Vec::new();
    for state in &states {
        let branch_protocol = apply_state(request, state);
        let simulation = simulate_glioma_protocol(&branch_protocol)
            .map_err(|error| ProtocolBranchOptimizationError::Simulation(error.to_string()))?;
        let branch_id = branch_id(&state.choices)?;
        let mut selected_candidate_order = state
            .choices
            .iter()
            .map(|candidate| {
                evaluated_candidates.insert(candidate.candidate_id.clone());
                candidate.candidate_id.clone()
            })
            .collect::<Vec<_>>();
        selected_candidate_order.sort();
        let score_milli = score_simulation(&simulation, state, request);
        if simulation.disposition != ProtocolDisposition::Feasible {
            negative_evidence.push(format!(
                "{branch_id}:{}",
                simulation.stop_conditions.join("|")
            ));
        }
        evaluations.push(ProtocolBranchEvaluation {
            branch_id,
            selected_candidate_order,
            simulation_digest: simulation.digest,
            disposition: simulation.disposition,
            score_milli,
            projected_cost_units: state.projected_cost_units,
            expected_information_milli: state.expected_information_milli,
            makespan_ticks: simulation.makespan_ticks,
            risk_total_milli: simulation.risk_total_milli,
            unscheduled_order: simulation.unscheduled_order,
            rationale: "branch was scored against the deterministic resource, horizon, risk, and approval simulator".into(),
        });
    }
    evaluations.sort_by(|left, right| left.branch_id.cmp(&right.branch_id));
    let mut ranking = evaluations.clone();
    ranking.sort_by(|left, right| {
        right
            .score_milli
            .cmp(&left.score_milli)
            .then_with(|| left.branch_id.cmp(&right.branch_id))
    });
    let selected = ranking.first().ok_or_else(|| {
        ProtocolBranchOptimizationError::InvalidOutput("no branch evaluation was produced".into())
    })?;
    let all_candidate_ids = request
        .candidates
        .iter()
        .map(|candidate| candidate.candidate_id.clone())
        .collect::<BTreeSet<_>>();
    let unresolved_candidate_order = all_candidate_ids
        .difference(&evaluated_candidates)
        .cloned()
        .collect::<Vec<_>>();
    let mut uncertainty = Vec::new();
    if states.len() < request.candidates.len().min(request.max_branches) {
        uncertainty.push("beam search pruned additional branch combinations".into());
    }
    if !unresolved_candidate_order.is_empty() {
        uncertainty
            .push("some alternatives were not evaluated because of beam or budget bounds".into());
    }
    if selected.disposition != ProtocolDisposition::Feasible {
        uncertainty.push(
            "the highest-scoring branch is not executable under current declared gates".into(),
        );
    }
    uncertainty.sort();
    negative_evidence.sort();
    let disposition = if selected.disposition == ProtocolDisposition::Feasible
        && unresolved_candidate_order.is_empty()
    {
        ProtocolBranchOptimizationDisposition::Qualified
    } else if selected.disposition == ProtocolDisposition::Feasible {
        ProtocolBranchOptimizationDisposition::Partial
    } else if evaluations
        .iter()
        .all(|evaluation| evaluation.disposition != ProtocolDisposition::Feasible)
        && evaluations.iter().any(|evaluation| {
            evaluation.disposition == ProtocolDisposition::CapacityBlocked
                || evaluation.disposition == ProtocolDisposition::RiskBlocked
        })
    {
        ProtocolBranchOptimizationDisposition::BudgetBlocked
    } else {
        ProtocolBranchOptimizationDisposition::Unresolved
    };
    let selected_candidate_order = selected.selected_candidate_order.clone();
    let mut plan = ProtocolBranchOptimizationPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        baseline_simulation_digest: baseline.digest,
        selected_branch_id: selected.branch_id.clone(),
        selected_candidate_order,
        evaluation_order: evaluations
            .iter()
            .map(|evaluation| evaluation.branch_id.clone())
            .collect(),
        evaluations,
        unresolved_candidate_order,
        budget_remaining_units: request
            .budget_units
            .saturating_sub(selected.projected_cost_units),
        uncertainty,
        negative_evidence,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-protocol-branch-optimization"),
    };
    plan.digest = ContentHash::of_value(&digest_input(&plan))
        .map_err(|error| ProtocolBranchOptimizationError::Digest(error.to_string()))?;
    validate_plan(&plan)?;
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p07_protocol_simulation::simulator::{
        ProtocolResource, ProtocolResourceKind,
    };
    use crate::glioma_engine::GliomaModelSystem;
    use bioprism_ids::ContentHash;

    fn protocol() -> ProtocolSimulationRequest {
        ProtocolSimulationRequest {
            objective: "choose an organoid invasion branch".into(),
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
                    duration_ticks: 3,
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
            randomization_seed: ContentHash::of_bytes(b"branch-seed"),
        }
    }

    fn candidate(id: &str, duration_ticks: u32, information: u16) -> ProtocolBranchCandidate {
        ProtocolBranchCandidate {
            candidate_id: id.into(),
            task: ProtocolTask {
                task_id: "assay".into(),
                label: format!("{id} invasion assay"),
                resource_kind: ProtocolResourceKind::Imaging,
                resource_units: 1,
                duration_ticks,
                depends_on: vec!["prepare".into()],
                model_system: GliomaModelSystem::Organoid,
                output_schema: "Assay1@1".into(),
                risk_milli: 200,
                requires_instrument: false,
            },
            expected_information_milli: information,
            evidence_prior_milli: 800,
            cost_units: 2,
        }
    }

    fn request(candidates: Vec<ProtocolBranchCandidate>) -> ProtocolBranchOptimizationRequest {
        ProtocolBranchOptimizationRequest {
            objective: "choose an organoid invasion branch".into(),
            base_protocol: protocol(),
            candidates,
            budget_units: 8,
            max_branches: 16,
            beam_width: 8,
            weights: ProtocolBranchWeights {
                information_milli: 400,
                feasibility_milli: 300,
                time_milli: 100,
                risk_milli: 100,
                cost_milli: 100,
            },
        }
    }

    #[test]
    fn branch_optimizer_prefers_feasible_high_information_alternative() {
        let plan = optimize_glioma_protocol_branches(&request(vec![
            candidate("slow-high-information", 10, 950),
            candidate("fast-moderate-information", 2, 700),
        ]))
        .unwrap();
        assert_eq!(
            plan.disposition,
            ProtocolBranchOptimizationDisposition::Qualified
        );
        assert_eq!(
            plan.selected_candidate_order,
            vec!["fast-moderate-information"]
        );
        assert_eq!(plan.selected_branch_id.starts_with("branch-"), true);
        plan.validate().unwrap();
    }

    #[test]
    fn branch_optimizer_preserves_digest_under_candidate_permutation() {
        let candidates = vec![candidate("a", 2, 800), candidate("b", 3, 700)];
        let first = optimize_glioma_protocol_branches(&request(candidates.clone())).unwrap();
        let mut reversed = candidates;
        reversed.reverse();
        let second = optimize_glioma_protocol_branches(&request(reversed)).unwrap();
        assert_eq!(first.digest, second.digest);
    }

    #[test]
    fn branch_optimizer_rejects_output_contract_changes() {
        let mut invalid = candidate("wrong", 2, 800);
        invalid.task.output_schema = "Wrong1@1".into();
        let error = optimize_glioma_protocol_branches(&request(vec![invalid])).unwrap_err();
        assert!(matches!(
            error,
            ProtocolBranchOptimizationError::InvalidCandidate(_)
        ));
    }
}

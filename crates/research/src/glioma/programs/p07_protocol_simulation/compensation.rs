//! Dependency-aware compensation planning after a preclinical protocol run.
//!
//! Protocol execution already fails closed when a local worker returns a failure or partial
//! result. This feature turns that outcome into a bounded, typed recovery portfolio: replacement
//! tasks must match the failed task's output contract, preserve completed/negative evidence, and
//! declare every dependency and downstream unlock. The planner never retries by itself, invents a
//! result, or dispatches a replacement task.

use super::execution::{ProtocolExecution, ProtocolTaskDisposition};
use super::simulator::{simulate_glioma_protocol, ProtocolResourceKind, ProtocolSimulationRequest};
use crate::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P07-F03";
pub const OUTPUT_SCHEMA: &str = "GliomaProtocolCompensation1@1";
pub const MAX_CANDIDATES: usize = 1_024;
pub const MAX_SELECTED: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolCompensationCandidate {
    pub candidate_id: String,
    pub replaces_task_id: String,
    pub output_schema: String,
    pub model_system: GliomaModelSystem,
    pub resource_kind: ProtocolResourceKind,
    pub resource_units: u16,
    pub duration_ticks: u32,
    pub cost_units: u32,
    pub risk_milli: u16,
    pub expected_information_milli: u16,
    pub depends_on: Vec<String>,
    pub unlocks_task_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolCompensationRequest {
    pub objective: String,
    pub protocol: ProtocolSimulationRequest,
    pub execution: ProtocolExecution,
    pub candidates: Vec<ProtocolCompensationCandidate>,
    pub budget_units: u64,
    pub max_selected: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolCompensationSelection {
    pub candidate_id: String,
    pub replaces_task_id: String,
    pub unlocks_task_order: Vec<String>,
    pub score_milli: u32,
    pub projected_cost_units: u64,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolCompensationDisposition {
    Qualified,
    Partial,
    BudgetBlocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolCompensationPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub protocol_digest: ContentHash,
    pub execution_digest: ContentHash,
    pub blocked_task_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub unresolved_task_order: Vec<String>,
    pub selections: Vec<ProtocolCompensationSelection>,
    pub preserved_completed_order: Vec<String>,
    pub preserved_negative_order: Vec<String>,
    pub budget_remaining_units: u64,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: ProtocolCompensationDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProtocolCompensationError {
    #[error("protocol compensation request is invalid: {0}")]
    InvalidRequest(String),
    #[error("protocol compensation candidate is invalid: {0}")]
    InvalidCandidate(String),
    #[error("protocol compensation input is invalid: {0}")]
    InvalidInput(String),
    #[error("protocol compensation output is invalid: {0}")]
    InvalidOutput(String),
    #[error("protocol compensation digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(plan: &ProtocolCompensationPlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": plan.feature_id,
        "output_schema": plan.output_schema,
        "objective": plan.objective,
        "protocol_digest": plan.protocol_digest,
        "execution_digest": plan.execution_digest,
        "blocked_task_order": plan.blocked_task_order,
        "selected_order": plan.selected_order,
        "unresolved_task_order": plan.unresolved_task_order,
        "selections": plan.selections,
        "preserved_completed_order": plan.preserved_completed_order,
        "preserved_negative_order": plan.preserved_negative_order,
        "budget_remaining_units": plan.budget_remaining_units,
        "negative_evidence": plan.negative_evidence,
        "uncertainty": plan.uncertainty,
        "disposition": plan.disposition,
    })
}

fn validate_request(
    request: &ProtocolCompensationRequest,
) -> Result<(), ProtocolCompensationError> {
    if request.objective.trim().is_empty()
        || request.protocol.objective != request.objective
        || request.candidates.is_empty()
        || request.candidates.len() > MAX_CANDIDATES
        || request.max_selected == 0
        || request.max_selected > MAX_SELECTED
        || request.budget_units == 0
    {
        return Err(ProtocolCompensationError::InvalidRequest(
            "objective must bind the protocol, candidates must be bounded, and budget/selection limits must be positive".into(),
        ));
    }
    Ok(())
}

fn validate_candidate(
    candidate: &ProtocolCompensationCandidate,
    task_ids: &BTreeSet<String>,
) -> Result<(), ProtocolCompensationError> {
    if candidate.candidate_id.trim().is_empty()
        || candidate.replaces_task_id.trim().is_empty()
        || candidate.output_schema.trim().is_empty()
        || !task_ids.contains(&candidate.replaces_task_id)
        || candidate.resource_units == 0
        || candidate.duration_ticks == 0
        || candidate.cost_units == 0
        || candidate.risk_milli > 1_000
        || candidate.expected_information_milli > 1_000
        || !canonical(&candidate.depends_on)
        || !canonical(&candidate.unlocks_task_order)
        || candidate.depends_on.iter().any(|id| !task_ids.contains(id))
        || candidate
            .unlocks_task_order
            .iter()
            .any(|id| !task_ids.contains(id))
    {
        return Err(ProtocolCompensationError::InvalidCandidate(format!(
            "candidate {} has an invalid identity, bounds, dependency list, or task reference",
            candidate.candidate_id
        )));
    }
    Ok(())
}

impl ProtocolCompensationPlan {
    pub fn validate(&self) -> Result<(), ProtocolCompensationError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.protocol_digest.as_str().len() != 64
            || self.execution_digest.as_str().len() != 64
            || !canonical(&self.blocked_task_order)
            || !canonical(&self.selected_order)
            || !canonical(&self.unresolved_task_order)
            || !canonical(&self.preserved_completed_order)
            || !canonical(&self.preserved_negative_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.selected_order.len() != self.selections.len()
            || self
                .selections
                .iter()
                .map(|selection| selection.candidate_id.clone())
                .collect::<Vec<_>>()
                != self.selected_order
            || self.selections.iter().any(|selection| {
                selection.candidate_id.trim().is_empty()
                    || selection.replaces_task_id.trim().is_empty()
                    || selection.score_milli > 1_000_000
                    || selection.projected_cost_units == 0
                    || selection.rationale.trim().is_empty()
                    || !canonical(&selection.unlocks_task_order)
            })
        {
            return Err(ProtocolCompensationError::InvalidOutput(
                "identity, ordering, selection, score, or limitation invariants are invalid".into(),
            ));
        }
        let mut selected_replacements = BTreeSet::new();
        for selection in &self.selections {
            if !selected_replacements.insert(selection.replaces_task_id.clone()) {
                return Err(ProtocolCompensationError::InvalidOutput(
                    "at most one compensation candidate may replace each task".into(),
                ));
            }
        }
        let blocked = self
            .blocked_task_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        if self
            .unresolved_task_order
            .iter()
            .any(|id| !blocked.contains(id))
            || self
                .selections
                .iter()
                .any(|selection| !blocked.contains(&selection.replaces_task_id))
        {
            return Err(ProtocolCompensationError::InvalidOutput(
                "selection and unresolved orders must remain within the blocked frontier".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ProtocolCompensationError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ProtocolCompensationError::InvalidOutput(
                "digest is not bound to the compensation plan".into(),
            ));
        }
        Ok(())
    }
}

/// Compile a bounded recovery portfolio from a failed or partial local protocol execution.
pub fn plan_glioma_protocol_compensation(
    request: &ProtocolCompensationRequest,
) -> Result<ProtocolCompensationPlan, ProtocolCompensationError> {
    validate_request(request)?;
    request
        .execution
        .validate()
        .map_err(|error| ProtocolCompensationError::InvalidInput(error.to_string()))?;
    let simulation = simulate_glioma_protocol(&request.protocol)
        .map_err(|error| ProtocolCompensationError::InvalidInput(error.to_string()))?;
    if simulation.digest != request.execution.protocol_digest {
        return Err(ProtocolCompensationError::InvalidInput(
            "execution is not bound to the declared protocol simulation".into(),
        ));
    }
    let task_map = request
        .protocol
        .tasks
        .iter()
        .map(|task| (task.task_id.clone(), task))
        .collect::<BTreeMap<_, _>>();
    let task_ids = task_map.keys().cloned().collect::<BTreeSet<_>>();
    let blocked_task_order = request
        .execution
        .task_results
        .iter()
        .filter(|result| {
            matches!(
                result.disposition,
                ProtocolTaskDisposition::Failed
                    | ProtocolTaskDisposition::Partial
                    | ProtocolTaskDisposition::Skipped
            )
        })
        .map(|result| result.task_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let blocked = blocked_task_order.iter().cloned().collect::<BTreeSet<_>>();
    let preserved_completed_order = request.execution.completed_order.clone();
    let preserved_negative_order = request.execution.negative_order.clone();
    let preserved = preserved_completed_order
        .iter()
        .chain(preserved_negative_order.iter())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut candidates_by_task = BTreeMap::<String, Vec<&ProtocolCompensationCandidate>>::new();
    let mut candidate_ids = BTreeSet::new();
    for candidate in &request.candidates {
        validate_candidate(candidate, &task_ids)?;
        if !candidate_ids.insert(candidate.candidate_id.clone()) {
            return Err(ProtocolCompensationError::InvalidCandidate(
                "candidate identifiers must be unique".into(),
            ));
        }
        let task = task_map.get(&candidate.replaces_task_id).ok_or_else(|| {
            ProtocolCompensationError::InvalidCandidate("replacement task is missing".into())
        })?;
        if task.output_schema != candidate.output_schema
            || task.model_system != candidate.model_system
            || task.resource_kind != candidate.resource_kind
            || task.resource_units != candidate.resource_units
            || candidate.risk_milli > task.risk_milli.saturating_add(250)
            || !blocked.contains(&candidate.replaces_task_id)
            || candidate
                .depends_on
                .iter()
                .any(|id| !preserved.contains(id))
        {
            continue;
        }
        candidates_by_task
            .entry(candidate.replaces_task_id.clone())
            .or_default()
            .push(candidate);
    }
    let mut ranked_by_task = BTreeMap::<String, Vec<(u32, &ProtocolCompensationCandidate)>>::new();
    for (task_id, candidates) in candidates_by_task {
        let mut ranked = candidates
            .into_iter()
            .map(|candidate| {
                let unlock_bonus = (candidate.unlocks_task_order.len() as u32).saturating_mul(125);
                let risk_penalty = u32::from(candidate.risk_milli).saturating_mul(2);
                let denominator = candidate
                    .cost_units
                    .saturating_add(candidate.duration_ticks)
                    .max(1);
                let score = (u32::from(candidate.expected_information_milli)
                    .saturating_mul(700)
                    .saturating_add(unlock_bonus)
                    .saturating_sub(risk_penalty))
                .saturating_mul(1_000)
                .saturating_div(denominator)
                .min(1_000_000);
                (score, candidate)
            })
            .collect::<Vec<_>>();
        ranked.sort_by(|left, right| {
            right
                .0
                .cmp(&left.0)
                .then_with(|| left.1.candidate_id.cmp(&right.1.candidate_id))
        });
        ranked_by_task.insert(task_id, ranked);
    }
    let mut selected = Vec::<ProtocolCompensationSelection>::new();
    let mut selected_order = Vec::new();
    let mut budget_remaining = request.budget_units;
    let mut unresolved = Vec::new();
    for task_id in &blocked_task_order {
        let Some(ranked) = ranked_by_task.get(task_id) else {
            unresolved.push(task_id.clone());
            continue;
        };
        let Some((score, candidate)) = ranked.iter().find(|(_, candidate)| {
            u64::from(candidate.cost_units) <= budget_remaining
                && selected.len() < request.max_selected
        }) else {
            unresolved.push(task_id.clone());
            continue;
        };
        budget_remaining -= u64::from(candidate.cost_units);
        selected_order.push(candidate.candidate_id.clone());
        selected.push(ProtocolCompensationSelection {
            candidate_id: candidate.candidate_id.clone(),
            replaces_task_id: task_id.clone(),
            unlocks_task_order: candidate.unlocks_task_order.clone(),
            score_milli: *score,
            projected_cost_units: u64::from(candidate.cost_units),
            rationale: "typed replacement preserves the original output contract and reopens a bounded downstream frontier".into(),
        });
    }
    selected_order.sort();
    selected.sort_by(|left, right| left.candidate_id.cmp(&right.candidate_id));
    unresolved.sort();
    let mut negative_evidence = request.execution.negative_evidence.clone();
    negative_evidence.extend(
        request
            .execution
            .failed_order
            .iter()
            .map(|id| format!("failed:{id}")),
    );
    negative_evidence.sort();
    negative_evidence.dedup();
    let mut uncertainty = request.execution.uncertainty.clone();
    uncertainty.extend(unresolved.iter().map(|id| format!("unresolved:{id}")));
    if !selected.is_empty() {
        uncertainty.push(
            "compensation remains a plan until a local executor returns typed results".into(),
        );
    }
    uncertainty.sort();
    uncertainty.dedup();
    let disposition = if blocked_task_order.is_empty() {
        ProtocolCompensationDisposition::Qualified
    } else if unresolved.is_empty() {
        ProtocolCompensationDisposition::Qualified
    } else if selected.is_empty() && !ranked_by_task.is_empty() {
        ProtocolCompensationDisposition::BudgetBlocked
    } else if !selected.is_empty() {
        ProtocolCompensationDisposition::Partial
    } else {
        ProtocolCompensationDisposition::Unresolved
    };
    let mut plan = ProtocolCompensationPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        protocol_digest: simulation.digest,
        execution_digest: request.execution.digest.clone(),
        blocked_task_order,
        selected_order,
        unresolved_task_order: unresolved,
        selections: selected,
        preserved_completed_order,
        preserved_negative_order,
        budget_remaining_units: budget_remaining,
        negative_evidence,
        uncertainty,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-protocol-compensation"),
    };
    plan.digest = ContentHash::of_value(&digest_input(&plan))
        .map_err(|error| ProtocolCompensationError::Digest(error.to_string()))?;
    plan.validate()?;
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p07_protocol_simulation::execution::{
        execute_glioma_protocol, DryRunGliomaProtocolExecutor, ProtocolExecutionRequest,
    };
    use crate::glioma::programs::p07_protocol_simulation::simulator::{
        ProtocolResource, ProtocolTask,
    };
    use crate::glioma_engine::GliomaModelSystem;
    use bioprism_ids::ContentHash;

    fn protocol() -> ProtocolSimulationRequest {
        ProtocolSimulationRequest {
            objective: "recover organoid invasion protocol".into(),
            model_system: GliomaModelSystem::Organoid,
            tasks: vec![
                ProtocolTask {
                    task_id: "setup".into(),
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
                    duration_ticks: 2,
                    depends_on: vec!["setup".into()],
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
            randomization_seed: ContentHash::of_bytes(b"seed"),
        }
    }

    fn execution() -> ProtocolExecution {
        let request = ProtocolExecutionRequest {
            protocol: protocol(),
            max_retries: 0,
            require_artifacts: true,
        };
        let mut executor = DryRunGliomaProtocolExecutor;
        let mut execution = execute_glioma_protocol(&request, &mut executor).unwrap();
        let assay = execution
            .task_results
            .iter_mut()
            .find(|result| result.task_id == "assay")
            .unwrap();
        assay.disposition = ProtocolTaskDisposition::Failed;
        assay.artifact = None;
        assay.note = "imaging gateway failed".into();
        execution.completed_order = vec!["setup".into()];
        execution.failed_order = vec!["assay".into()];
        execution.negative_order.clear();
        execution.partial_order.clear();
        execution.skipped_order.clear();
        execution.disposition = super::super::execution::ProtocolExecutionDisposition::Failed;
        execution.stop_reason = super::super::execution::ProtocolExecutionStopReason::TaskFailed;
        let digest_input = serde_json::json!({
            "feature_id": execution.feature_id,
            "output_schema": execution.output_schema,
            "protocol_digest": execution.protocol_digest,
            "task_order": execution.task_order,
            "task_results": execution.task_results,
            "completed_order": execution.completed_order,
            "negative_order": execution.negative_order,
            "partial_order": execution.partial_order,
            "failed_order": execution.failed_order,
            "skipped_order": execution.skipped_order,
            "retry_count": execution.retry_count,
            "uncertainty": execution.uncertainty,
            "negative_evidence": execution.negative_evidence,
            "disposition": execution.disposition,
            "stop_reason": execution.stop_reason,
        });
        execution.digest = ContentHash::of_value(&digest_input).unwrap();
        execution
    }

    #[test]
    fn chooses_contract_matching_compensation_and_preserves_completed_setup() {
        let plan = plan_glioma_protocol_compensation(&ProtocolCompensationRequest {
            objective: "recover organoid invasion protocol".into(),
            protocol: protocol(),
            execution: execution(),
            candidates: vec![ProtocolCompensationCandidate {
                candidate_id: "assay-retry-local".into(),
                replaces_task_id: "assay".into(),
                output_schema: "Assay1@1".into(),
                model_system: GliomaModelSystem::Organoid,
                resource_kind: ProtocolResourceKind::Imaging,
                resource_units: 1,
                duration_ticks: 2,
                cost_units: 3,
                risk_milli: 200,
                expected_information_milli: 900,
                depends_on: vec!["setup".into()],
                unlocks_task_order: vec!["assay".into()],
            }],
            budget_units: 5,
            max_selected: 4,
        })
        .unwrap();
        assert_eq!(plan.disposition, ProtocolCompensationDisposition::Qualified);
        assert_eq!(plan.selected_order, vec!["assay-retry-local"]);
        assert_eq!(plan.preserved_completed_order, vec!["setup"]);
        plan.validate().unwrap();
    }

    #[test]
    fn mismatched_output_contract_is_unresolved_not_fabricated() {
        let plan = plan_glioma_protocol_compensation(&ProtocolCompensationRequest {
            objective: "recover organoid invasion protocol".into(),
            protocol: protocol(),
            execution: execution(),
            candidates: vec![ProtocolCompensationCandidate {
                candidate_id: "wrong-contract".into(),
                replaces_task_id: "assay".into(),
                output_schema: "Wrong1@1".into(),
                model_system: GliomaModelSystem::Organoid,
                resource_kind: ProtocolResourceKind::Imaging,
                resource_units: 1,
                duration_ticks: 2,
                cost_units: 1,
                risk_milli: 100,
                expected_information_milli: 900,
                depends_on: vec!["setup".into()],
                unlocks_task_order: vec!["assay".into()],
            }],
            budget_units: 5,
            max_selected: 4,
        })
        .unwrap();
        assert!(plan.selected_order.is_empty());
        assert_eq!(plan.unresolved_task_order, vec!["assay"]);
        assert_eq!(
            plan.disposition,
            ProtocolCompensationDisposition::Unresolved
        );
    }

    #[test]
    fn candidate_input_permutation_preserves_digest() {
        let mut candidates = vec![
            ProtocolCompensationCandidate {
                candidate_id: "assay-retry-local".into(),
                replaces_task_id: "assay".into(),
                output_schema: "Assay1@1".into(),
                model_system: GliomaModelSystem::Organoid,
                resource_kind: ProtocolResourceKind::Imaging,
                resource_units: 1,
                duration_ticks: 2,
                cost_units: 3,
                risk_milli: 200,
                expected_information_milli: 900,
                depends_on: vec!["setup".into()],
                unlocks_task_order: vec!["assay".into()],
            },
            ProtocolCompensationCandidate {
                candidate_id: "assay-cheap".into(),
                replaces_task_id: "assay".into(),
                output_schema: "Assay1@1".into(),
                model_system: GliomaModelSystem::Organoid,
                resource_kind: ProtocolResourceKind::Imaging,
                resource_units: 1,
                duration_ticks: 2,
                cost_units: 3,
                risk_milli: 200,
                expected_information_milli: 900,
                depends_on: vec!["setup".into()],
                unlocks_task_order: vec!["assay".into()],
            },
        ];
        let request = |candidates| ProtocolCompensationRequest {
            objective: "recover organoid invasion protocol".into(),
            protocol: protocol(),
            execution: execution(),
            candidates,
            budget_units: 5,
            max_selected: 4,
        };
        let first = plan_glioma_protocol_compensation(&request(candidates.clone())).unwrap();
        candidates.reverse();
        let second = plan_glioma_protocol_compensation(&request(candidates)).unwrap();
        assert_eq!(first.digest, second.digest);
    }
}

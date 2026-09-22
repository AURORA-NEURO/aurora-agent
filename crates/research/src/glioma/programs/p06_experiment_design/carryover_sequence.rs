//! Carryover-aware assay sequence design for preclinical glioma workflows.
//!
//! Plate, instrument, and culture workflows can make the next observation less informative when
//! the preceding assay leaves a declared transition effect. This planner treats those transitions
//! as a weighted directed graph and chooses a bounded information-rich sequence with explicit
//! washout/carryover penalties. It is a design artifact only: it never schedules a plate, controls
//! an instrument, or interprets an observation.

use crate::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P06-F15";
pub const OUTPUT_SCHEMA: &str = "GliomaCarryoverSequenceDesign1@1";
pub const MAX_ACTIONS: usize = 256;
pub const MAX_SEQUENCE_LENGTH: usize = 1_024;
pub const SCORE_SCALE: u64 = 1_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CarryoverAction {
    pub action_id: String,
    pub label: String,
    pub feature_id: String,
    pub expected_information_milli: u32,
    pub cost_units: u32,
    pub risk_milli: u16,
    pub feasibility_milli: u16,
    pub max_repeats: u16,
    pub carryover_milli_by_next_action: BTreeMap<String, u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CarryoverSequenceRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub actions: Vec<CarryoverAction>,
    pub initial_carryover_milli_by_action: BTreeMap<String, u16>,
    pub sequence_length: usize,
    pub budget_units: u64,
    pub min_feasibility_milli: u16,
    pub risk_ceiling_milli: u16,
    pub carryover_penalty_milli: u16,
    pub risk_penalty_milli: u16,
    pub cost_penalty_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CarryoverSequenceStep {
    pub position: u16,
    pub action_id: String,
    pub label: String,
    pub carryover_milli_from_previous: u16,
    pub information_milli: u32,
    pub net_utility_milli: i64,
    pub projected_cost_units: u64,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CarryoverSequenceDisposition {
    Qualified,
    Partial,
    NoEligibleActions,
    BudgetBlocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CarryoverSequenceDesign {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub action_order: Vec<String>,
    pub sequence_order: Vec<String>,
    pub risk_blocked_order: Vec<String>,
    pub feasibility_blocked_order: Vec<String>,
    pub steps: Vec<CarryoverSequenceStep>,
    pub total_information_milli: u64,
    pub total_carryover_milli: u64,
    pub total_projected_cost_units: u64,
    pub budget_remaining_units: u64,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: CarryoverSequenceDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CarryoverSequenceError {
    #[error("carryover sequence request is invalid: {0}")]
    InvalidRequest(String),
    #[error("carryover sequence input is invalid: {0}")]
    InvalidInput(String),
    #[error("carryover sequence output is invalid: {0}")]
    InvalidOutput(String),
    #[error("carryover sequence digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &CarryoverSequenceDesign) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "model_system": output.model_system,
        "action_order": output.action_order,
        "sequence_order": output.sequence_order,
        "risk_blocked_order": output.risk_blocked_order,
        "feasibility_blocked_order": output.feasibility_blocked_order,
        "steps": output.steps,
        "total_information_milli": output.total_information_milli,
        "total_carryover_milli": output.total_carryover_milli,
        "total_projected_cost_units": output.total_projected_cost_units,
        "budget_remaining_units": output.budget_remaining_units,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn validate_request(request: &CarryoverSequenceRequest) -> Result<(), CarryoverSequenceError> {
    if request.objective.trim().is_empty()
        || request.actions.is_empty()
        || request.actions.len() > MAX_ACTIONS
        || request.sequence_length == 0
        || request.sequence_length > MAX_SEQUENCE_LENGTH
        || request.budget_units == 0
        || request.min_feasibility_milli > 1_000
        || request.risk_ceiling_milli > 1_000
        || request.carryover_penalty_milli > 1_000
        || request.risk_penalty_milli > 1_000
        || request.cost_penalty_milli > 1_000
    {
        return Err(CarryoverSequenceError::InvalidRequest(
            "objective, bounded actions/sequence, positive budget, and finite penalty/risk/feasibility gates are required".into(),
        ));
    }
    let action_ids = request
        .actions
        .iter()
        .map(|action| action.action_id.clone())
        .collect::<BTreeSet<_>>();
    if action_ids.len() != request.actions.len()
        || request
            .initial_carryover_milli_by_action
            .keys()
            .any(|id| !action_ids.contains(id))
        || request.initial_carryover_milli_by_action.len() != action_ids.len()
        || request
            .initial_carryover_milli_by_action
            .values()
            .any(|value| *value > 1_000)
    {
        return Err(CarryoverSequenceError::InvalidInput(
            "action ids must be unique and initial carryover must cover every action exactly once"
                .into(),
        ));
    }
    for action in &request.actions {
        if action.action_id.trim().is_empty()
            || action.label.trim().is_empty()
            || action.feature_id.trim().is_empty()
            || action.expected_information_milli == 0
            || action.cost_units == 0
            || action.risk_milli > 1_000
            || action.feasibility_milli > 1_000
            || action.max_repeats == 0
            || action.carryover_milli_by_next_action.len() != action_ids.len()
            || action
                .carryover_milli_by_next_action
                .keys()
                .any(|id| !action_ids.contains(id))
            || action
                .carryover_milli_by_next_action
                .values()
                .any(|value| *value > 1_000)
        {
            return Err(CarryoverSequenceError::InvalidInput(
                "action identity, positive information/cost, bounded risk/feasibility/repeats, and complete transition matrix are required".into(),
            ));
        }
    }
    Ok(())
}

fn validate_output(output: &CarryoverSequenceDesign) -> Result<(), CarryoverSequenceError> {
    if output.feature_id != FEATURE_ID
        || output.output_schema != OUTPUT_SCHEMA
        || output.objective.trim().is_empty()
        || !canonical(&output.action_order)
        || !canonical(&output.risk_blocked_order)
        || !canonical(&output.feasibility_blocked_order)
        || !canonical(&output.negative_evidence)
        || !canonical(&output.uncertainty)
        || output
            .steps
            .windows(2)
            .any(|pair| pair[0].position >= pair[1].position)
        || output.steps.iter().any(|step| {
            step.action_id.trim().is_empty()
                || step.label.trim().is_empty()
                || step.information_milli == 0
                || step.rationale.trim().is_empty()
        })
        || output.sequence_order.len() != output.steps.len()
    {
        return Err(CarryoverSequenceError::InvalidOutput(
            "identity, canonical ordering, sequence-step, and metric invariants are invalid".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(output))
        .map_err(|error| CarryoverSequenceError::Digest(error.to_string()))?;
    if expected != output.digest {
        return Err(CarryoverSequenceError::InvalidOutput(
            "digest is not bound to the carryover sequence design".into(),
        ));
    }
    Ok(())
}

impl CarryoverSequenceDesign {
    pub fn validate(&self) -> Result<(), CarryoverSequenceError> {
        validate_output(self)
    }
}

fn utility(
    action: &CarryoverAction,
    carryover_milli: u16,
    request: &CarryoverSequenceRequest,
) -> i64 {
    let information = u64::from(action.expected_information_milli)
        .saturating_mul(u64::from(action.feasibility_milli))
        / SCORE_SCALE;
    let carryover_penalty = u64::from(carryover_milli)
        .saturating_mul(u64::from(request.carryover_penalty_milli))
        / SCORE_SCALE;
    let risk_penalty = u64::from(action.risk_milli)
        .saturating_mul(u64::from(request.risk_penalty_milli))
        / SCORE_SCALE;
    let cost_penalty = u64::from(action.cost_units)
        .saturating_mul(u64::from(request.cost_penalty_milli))
        / SCORE_SCALE;
    i64::try_from(information).unwrap_or(i64::MAX)
        - i64::try_from(carryover_penalty).unwrap_or(i64::MAX)
        - i64::try_from(risk_penalty).unwrap_or(i64::MAX)
        - i64::try_from(cost_penalty).unwrap_or(i64::MAX)
}

/// Choose an information-rich assay order while minimizing declared transition carryover.
pub fn plan_glioma_carryover_sequence(
    request: &CarryoverSequenceRequest,
) -> Result<CarryoverSequenceDesign, CarryoverSequenceError> {
    validate_request(request)?;
    let action_order = request
        .actions
        .iter()
        .map(|action| action.action_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let risk_blocked_order = request
        .actions
        .iter()
        .filter(|action| action.risk_milli > request.risk_ceiling_milli)
        .map(|action| action.action_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let feasibility_blocked_order = request
        .actions
        .iter()
        .filter(|action| {
            action.risk_milli <= request.risk_ceiling_milli
                && action.feasibility_milli < request.min_feasibility_milli
        })
        .map(|action| action.action_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let eligible = request
        .actions
        .iter()
        .filter(|action| {
            action.risk_milli <= request.risk_ceiling_milli
                && action.feasibility_milli >= request.min_feasibility_milli
        })
        .collect::<Vec<_>>();
    let mut negative_evidence = risk_blocked_order
        .iter()
        .map(|id| format!("risk-gate-blocked:{id}"))
        .chain(
            feasibility_blocked_order
                .iter()
                .map(|id| format!("feasibility-gate-blocked:{id}")),
        )
        .collect::<Vec<_>>();
    let mut uncertainty = Vec::new();
    let mut steps = Vec::new();
    let mut sequence_order = Vec::new();
    let mut repeats = BTreeMap::<String, u16>::new();
    let mut previous: Option<String> = None;
    let mut total_cost = 0_u64;
    let mut total_information = 0_u64;
    let mut total_carryover = 0_u64;
    for position in 0..request.sequence_length {
        let mut best: Option<(i64, String, u16)> = None;
        for action in &eligible {
            let count = repeats.get(&action.action_id).copied().unwrap_or(0);
            if count >= action.max_repeats {
                continue;
            }
            let carryover = previous
                .as_ref()
                .and_then(|id| {
                    request
                        .actions
                        .iter()
                        .find(|candidate| candidate.action_id == *id)
                        .and_then(|candidate| {
                            candidate
                                .carryover_milli_by_next_action
                                .get(&action.action_id)
                        })
                        .copied()
                })
                .or_else(|| {
                    request
                        .initial_carryover_milli_by_action
                        .get(&action.action_id)
                        .copied()
                })
                .unwrap_or(0);
            let projected_cost = total_cost.saturating_add(u64::from(action.cost_units));
            if projected_cost > request.budget_units {
                continue;
            }
            let score = utility(action, carryover, request);
            let candidate = (score, action.action_id.clone(), carryover);
            if best
                .as_ref()
                .map(|current| candidate > *current)
                .unwrap_or(true)
            {
                best = Some(candidate);
            }
        }
        let Some((score, action_id, carryover)) = best else {
            uncertainty.push(format!("sequence-stopped-at-position:{position}"));
            break;
        };
        let action = eligible
            .iter()
            .find(|action| action.action_id == action_id)
            .expect("selected eligible action");
        total_cost = total_cost.saturating_add(u64::from(action.cost_units));
        total_information =
            total_information.saturating_add(u64::from(action.expected_information_milli));
        total_carryover = total_carryover.saturating_add(u64::from(carryover));
        *repeats.entry(action_id.clone()).or_default() += 1;
        sequence_order.push(action_id.clone());
        steps.push(CarryoverSequenceStep {
            position: position as u16,
            action_id,
            label: action.label.clone(),
            carryover_milli_from_previous: carryover,
            information_milli: action.expected_information_milli,
            net_utility_milli: score,
            projected_cost_units: total_cost,
            rationale: if carryover == 0 {
                "selected for information and no declared transition carryover".into()
            } else {
                "selected after penalizing the declared transition carryover".into()
            },
        });
        previous = Some(steps.last().expect("step appended").action_id.clone());
    }
    if steps.len() < request.sequence_length {
        uncertainty.push("requested-sequence-was-not-fully-realized".into());
    }
    if total_cost == request.budget_units {
        uncertainty.push("budget-exhausted-at-sequence-boundary".into());
    }
    negative_evidence.sort();
    uncertainty.sort();
    let disposition = if eligible.is_empty() {
        CarryoverSequenceDisposition::NoEligibleActions
    } else if steps.is_empty() && !uncertainty.is_empty() {
        CarryoverSequenceDisposition::BudgetBlocked
    } else if !uncertainty.is_empty()
        || !risk_blocked_order.is_empty()
        || !feasibility_blocked_order.is_empty()
    {
        CarryoverSequenceDisposition::Partial
    } else {
        CarryoverSequenceDisposition::Qualified
    };
    let mut output = CarryoverSequenceDesign {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        action_order,
        sequence_order,
        risk_blocked_order,
        feasibility_blocked_order,
        steps,
        total_information_milli: total_information,
        total_carryover_milli: total_carryover,
        total_projected_cost_units: total_cost,
        budget_remaining_units: request.budget_units.saturating_sub(total_cost),
        negative_evidence,
        uncertainty,
        disposition,
        digest: ContentHash::of_bytes(b"pending"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| CarryoverSequenceError::Digest(error.to_string()))?;
    validate_output(&output)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn action(id: &str, information: u32) -> CarryoverAction {
        CarryoverAction {
            action_id: id.into(),
            label: id.into(),
            feature_id: format!("feature-{id}"),
            expected_information_milli: information,
            cost_units: 2,
            risk_milli: 100,
            feasibility_milli: 900,
            max_repeats: 2,
            carryover_milli_by_next_action: BTreeMap::from([
                ("a".into(), if id == "a" { 900 } else { 50 }),
                ("b".into(), if id == "b" { 900 } else { 50 }),
            ]),
        }
    }

    fn request() -> CarryoverSequenceRequest {
        CarryoverSequenceRequest {
            objective: "order invasion readouts".into(),
            model_system: GliomaModelSystem::Organoid,
            actions: vec![action("a", 900), action("b", 700)],
            initial_carryover_milli_by_action: BTreeMap::from([("a".into(), 0), ("b".into(), 0)]),
            sequence_length: 4,
            budget_units: 16,
            min_feasibility_milli: 700,
            risk_ceiling_milli: 500,
            carryover_penalty_milli: 900,
            risk_penalty_milli: 100,
            cost_penalty_milli: 10,
        }
    }

    #[test]
    fn sequence_is_deterministic_and_valid() {
        let first = plan_glioma_carryover_sequence(&request()).expect("sequence");
        let second = plan_glioma_carryover_sequence(&request()).expect("sequence");
        assert_eq!(first, second);
        assert_eq!(first.steps.len(), 4);
        assert!(first.total_carryover_milli <= 1_800);
        first.validate().expect("valid output");
    }

    #[test]
    fn blocked_actions_remain_negative_evidence() {
        let mut input = request();
        input.actions[1].risk_milli = 900;
        let output = plan_glioma_carryover_sequence(&input).expect("sequence");
        assert_eq!(output.risk_blocked_order, vec!["b"]);
        assert!(output
            .negative_evidence
            .iter()
            .any(|entry| entry == "risk-gate-blocked:b"));
    }
}

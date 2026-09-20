//! Robust scenario-aware allocation for preclinical glioma experiment design.
//!
//! A one-model information score can select an assay that is brittle to mechanism misspecification
//! or nuisance variation. This feature treats each declared scenario as a first-class world,
//! computes lower-tail (maximin) information for every candidate, and allocates integer replicates
//! with diminishing marginal utility under explicit budget, risk, feasibility, and throughput
//! gates. It produces a runnable next batch; it does not invent outcomes or dispatch experiments.

use crate::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P06-F07";
pub const OUTPUT_SCHEMA: &str = "GliomaRobustExperimentDesign1@1";
pub const MAX_SCENARIOS: usize = 128;
pub const MAX_CANDIDATES: usize = 4_096;
pub const MAX_REPLICATES_PER_CANDIDATE: u16 = 10_000;
pub const MAX_TOTAL_REPLICATES: u32 = 1_000_000;
pub const UTILITY_SCALE: u64 = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RobustDesignScenario {
    pub scenario_id: String,
    pub label: String,
    pub weight_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RobustDesignCandidate {
    pub arm_id: String,
    pub label: String,
    pub feature_id: String,
    pub cost_units_per_replicate: u32,
    pub risk_milli: u16,
    pub feasibility_milli: u16,
    pub max_replicates: u16,
    pub utility_milli_by_scenario: BTreeMap<String, Vec<u64>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RobustExperimentDesignRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub scenarios: Vec<RobustDesignScenario>,
    pub candidates: Vec<RobustDesignCandidate>,
    pub budget_units: u64,
    pub max_selected_arms: usize,
    pub min_replicates_per_selected_arm: u16,
    pub max_total_replicates: u32,
    pub min_feasibility_milli: u16,
    pub risk_ceiling_milli: u16,
    pub min_robust_utility_milli: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RobustDesignActionKind {
    Allocate,
    Hold,
    RiskBlocked,
    FeasibilityBlocked,
    BudgetBlocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RobustCandidateAllocation {
    pub arm_id: String,
    pub label: String,
    pub feature_id: String,
    pub robust_utility_milli: u64,
    pub expected_utility_milli: u64,
    pub worst_case_scenario_id: String,
    pub scenario_utility_milli: BTreeMap<String, u64>,
    pub allocated_replicates: u16,
    pub projected_cost_units: u64,
    pub risk_milli: u16,
    pub feasibility_milli: u16,
    pub action: RobustDesignActionKind,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RobustExperimentDesignDisposition {
    Qualified,
    Partial,
    BudgetBlocked,
    NoEligibleActions,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RobustExperimentDesign {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub scenario_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub held_order: Vec<String>,
    pub risk_blocked_order: Vec<String>,
    pub feasibility_blocked_order: Vec<String>,
    pub budget_blocked_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub allocations: Vec<RobustCandidateAllocation>,
    pub budget_remaining_units: u64,
    pub total_allocated_replicates: u32,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: RobustExperimentDesignDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RobustExperimentDesignError {
    #[error("robust experiment design request is invalid: {0}")]
    InvalidRequest(String),
    #[error("robust experiment design input is invalid: {0}")]
    InvalidInput(String),
    #[error("robust experiment design output is invalid: {0}")]
    InvalidOutput(String),
    #[error("robust experiment design digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &RobustExperimentDesign) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "model_system": output.model_system,
        "scenario_order": output.scenario_order,
        "candidate_order": output.candidate_order,
        "selected_order": output.selected_order,
        "held_order": output.held_order,
        "risk_blocked_order": output.risk_blocked_order,
        "feasibility_blocked_order": output.feasibility_blocked_order,
        "budget_blocked_order": output.budget_blocked_order,
        "unresolved_order": output.unresolved_order,
        "allocations": output.allocations,
        "budget_remaining_units": output.budget_remaining_units,
        "total_allocated_replicates": output.total_allocated_replicates,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn validate_request(
    request: &RobustExperimentDesignRequest,
) -> Result<(), RobustExperimentDesignError> {
    if request.objective.trim().is_empty()
        || request.scenarios.is_empty()
        || request.scenarios.len() > MAX_SCENARIOS
        || request.candidates.is_empty()
        || request.candidates.len() > MAX_CANDIDATES
        || request.budget_units == 0
        || request.max_selected_arms == 0
        || request.max_selected_arms > request.candidates.len()
        || request.min_replicates_per_selected_arm == 0
        || request.min_replicates_per_selected_arm > MAX_REPLICATES_PER_CANDIDATE
        || request.max_total_replicates == 0
        || request.max_total_replicates > MAX_TOTAL_REPLICATES
        || request
            .max_selected_arms
            .saturating_mul(usize::from(request.min_replicates_per_selected_arm))
            > request.max_total_replicates as usize
        || request.min_feasibility_milli > 1_000
        || request.risk_ceiling_milli > 1_000
        || request.min_robust_utility_milli > UTILITY_SCALE
    {
        return Err(RobustExperimentDesignError::InvalidRequest(
            "objective, bounded scenarios/candidates, positive budget/selection/replicate limits, and finite feasibility/risk/utility gates are required".into(),
        ));
    }
    let mut scenario_ids = BTreeSet::new();
    let mut total_weight = 0_u32;
    for scenario in &request.scenarios {
        if scenario.scenario_id.trim().is_empty()
            || scenario.label.trim().is_empty()
            || scenario.weight_milli == 0
            || scenario.weight_milli > 1_000
            || !scenario_ids.insert(scenario.scenario_id.clone())
        {
            return Err(RobustExperimentDesignError::InvalidInput(
                "scenario ids/labels/weights must be non-empty, bounded, and unique".into(),
            ));
        }
        total_weight = total_weight.saturating_add(u32::from(scenario.weight_milli));
    }
    if total_weight != 1_000 {
        return Err(RobustExperimentDesignError::InvalidInput(
            "scenario weights must sum to exactly 1000 milli-units".into(),
        ));
    }
    let mut candidate_ids = BTreeSet::new();
    for candidate in &request.candidates {
        if candidate.arm_id.trim().is_empty()
            || candidate.label.trim().is_empty()
            || candidate.feature_id.trim().is_empty()
            || candidate.cost_units_per_replicate == 0
            || candidate.risk_milli > 1_000
            || candidate.feasibility_milli > 1_000
            || candidate.max_replicates == 0
            || candidate.max_replicates > MAX_REPLICATES_PER_CANDIDATE
            || !candidate_ids.insert(candidate.arm_id.clone())
            || candidate.utility_milli_by_scenario.len() != scenario_ids.len()
            || candidate
                .utility_milli_by_scenario
                .keys()
                .any(|id| !scenario_ids.contains(id))
            || candidate.utility_milli_by_scenario.values().any(|values| {
                values.len() != usize::from(candidate.max_replicates)
                    || values.iter().any(|value| *value > UTILITY_SCALE)
            })
        {
            return Err(RobustExperimentDesignError::InvalidInput(
                "candidate identity, resource/risk bounds, replicate horizon, or scenario utility matrix is invalid".into(),
            ));
        }
    }
    Ok(())
}

fn cumulative(candidate: &RobustDesignCandidate, scenario_id: &str, replicates: u16) -> u64 {
    candidate
        .utility_milli_by_scenario
        .get(scenario_id)
        .map(|values| {
            values
                .iter()
                .take(usize::from(replicates))
                .copied()
                .sum::<u64>()
                .min(UTILITY_SCALE)
        })
        .unwrap_or(0)
}

fn utility_summary(
    candidate: &RobustDesignCandidate,
    scenarios: &[RobustDesignScenario],
    replicates: u16,
) -> (u64, u64, String, BTreeMap<String, u64>) {
    let mut scenario_utility = BTreeMap::new();
    for scenario in scenarios {
        scenario_utility.insert(
            scenario.scenario_id.clone(),
            cumulative(candidate, &scenario.scenario_id, replicates),
        );
    }
    let (worst_case_scenario_id, robust_utility_milli) = scenario_utility
        .iter()
        .min_by(|(left_id, left_value), (right_id, right_value)| {
            left_value
                .cmp(right_value)
                .then_with(|| left_id.cmp(right_id))
        })
        .map(|(id, value)| (id.clone(), *value))
        .unwrap_or_else(|| (String::new(), 0));
    let expected = scenarios
        .iter()
        .map(|scenario| {
            scenario_utility
                .get(&scenario.scenario_id)
                .copied()
                .unwrap_or(0)
                .saturating_mul(u64::from(scenario.weight_milli))
        })
        .sum::<u64>()
        / 1_000;
    (
        robust_utility_milli,
        expected.min(UTILITY_SCALE),
        worst_case_scenario_id,
        scenario_utility,
    )
}

fn validate_output(output: &RobustExperimentDesign) -> Result<(), RobustExperimentDesignError> {
    if output.feature_id != FEATURE_ID
        || output.output_schema != OUTPUT_SCHEMA
        || output.objective.trim().is_empty()
        || !canonical(&output.scenario_order)
        || !canonical(&output.candidate_order)
        || !canonical(&output.selected_order)
        || !canonical(&output.held_order)
        || !canonical(&output.risk_blocked_order)
        || !canonical(&output.feasibility_blocked_order)
        || !canonical(&output.budget_blocked_order)
        || !canonical(&output.unresolved_order)
        || !canonical(&output.negative_evidence)
        || !canonical(&output.uncertainty)
        || output.allocations.len() != output.candidate_order.len()
        || output
            .allocations
            .windows(2)
            .any(|pair| pair[0].arm_id >= pair[1].arm_id)
        || output.allocations.iter().any(|allocation| {
            allocation.arm_id.trim().is_empty()
                || allocation.label.trim().is_empty()
                || allocation.feature_id.trim().is_empty()
                || allocation.scenario_utility_milli.is_empty()
                || allocation.robust_utility_milli > UTILITY_SCALE
                || allocation.expected_utility_milli > UTILITY_SCALE
                || allocation.allocated_replicates == 0
                    && matches!(allocation.action, RobustDesignActionKind::Allocate)
                || allocation.rationale.trim().is_empty()
        })
    {
        return Err(RobustExperimentDesignError::InvalidOutput(
            "identity, ordering, allocation cardinality, utility, or rationale invariants are invalid".into(),
        ));
    }
    let candidate_ids = output
        .candidate_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let allocation_ids = output
        .allocations
        .iter()
        .map(|allocation| allocation.arm_id.clone())
        .collect::<BTreeSet<_>>();
    if candidate_ids != allocation_ids {
        return Err(RobustExperimentDesignError::InvalidOutput(
            "candidate and allocation identities do not reconcile".into(),
        ));
    }
    let mut partitions = BTreeSet::new();
    for order in [
        &output.selected_order,
        &output.held_order,
        &output.risk_blocked_order,
        &output.feasibility_blocked_order,
        &output.budget_blocked_order,
        &output.unresolved_order,
    ] {
        for arm_id in order.iter() {
            if !candidate_ids.contains(arm_id) || !partitions.insert(arm_id) {
                return Err(RobustExperimentDesignError::InvalidOutput(
                    "candidate action orders do not partition the candidate set".into(),
                ));
            }
        }
    }
    if partitions.len() != candidate_ids.len() {
        return Err(RobustExperimentDesignError::InvalidOutput(
            "candidate action orders omit one or more candidates".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(output))
        .map_err(|error| RobustExperimentDesignError::Digest(error.to_string()))?;
    if expected != output.digest {
        return Err(RobustExperimentDesignError::InvalidOutput(
            "digest is not bound to the robust design".into(),
        ));
    }
    Ok(())
}

impl RobustExperimentDesign {
    pub fn validate(&self) -> Result<(), RobustExperimentDesignError> {
        validate_output(self)
    }
}

/// Allocate a robust next experimental batch using lower-tail scenario utility.
pub fn design_glioma_robust_experiment(
    request: &RobustExperimentDesignRequest,
) -> Result<RobustExperimentDesign, RobustExperimentDesignError> {
    validate_request(request)?;
    let scenario_order = request
        .scenarios
        .iter()
        .map(|scenario| scenario.scenario_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let candidate_order = request
        .candidates
        .iter()
        .map(|candidate| candidate.arm_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let eligible = request
        .candidates
        .iter()
        .filter(|candidate| {
            candidate.risk_milli <= request.risk_ceiling_milli
                && candidate.feasibility_milli >= request.min_feasibility_milli
        })
        .collect::<Vec<_>>();
    let mut eligible_ranked = eligible
        .iter()
        .map(|candidate| {
            let (robust, expected, _, _) = utility_summary(
                candidate,
                &request.scenarios,
                request.min_replicates_per_selected_arm,
            );
            (candidate, robust, expected)
        })
        .filter(|(_, robust, _)| *robust >= request.min_robust_utility_milli)
        .collect::<Vec<_>>();
    eligible_ranked.sort_by(|left, right| {
        right
            .1
            .cmp(&left.1)
            .then_with(|| right.2.cmp(&left.2))
            .then_with(|| left.0.arm_id.cmp(&right.0.arm_id))
    });
    let selected_candidates = eligible_ranked
        .into_iter()
        .take(request.max_selected_arms)
        .collect::<Vec<_>>();
    let mut allocations = BTreeMap::<String, u16>::new();
    let mut spent = 0_u64;
    let mut total_replicates = 0_u32;
    for (candidate, _, _) in &selected_candidates {
        let required_cost = u64::from(candidate.cost_units_per_replicate)
            .saturating_mul(u64::from(request.min_replicates_per_selected_arm));
        if total_replicates.saturating_add(u32::from(request.min_replicates_per_selected_arm))
            > request.max_total_replicates
            || spent.saturating_add(required_cost) > request.budget_units
        {
            continue;
        }
        allocations.insert(
            candidate.arm_id.clone(),
            request.min_replicates_per_selected_arm,
        );
        spent = spent.saturating_add(required_cost);
        total_replicates =
            total_replicates.saturating_add(u32::from(request.min_replicates_per_selected_arm));
    }
    loop {
        let next = request
            .candidates
            .iter()
            .filter(|candidate| allocations.contains_key(&candidate.arm_id))
            .filter_map(|candidate| {
                let current = *allocations.get(&candidate.arm_id)?;
                if current >= candidate.max_replicates
                    || total_replicates >= request.max_total_replicates
                    || spent.saturating_add(u64::from(candidate.cost_units_per_replicate))
                        > request.budget_units
                {
                    return None;
                }
                let (current_robust, _, _, _) =
                    utility_summary(candidate, &request.scenarios, current);
                let (next_robust, _, _, _) =
                    utility_summary(candidate, &request.scenarios, current.saturating_add(1));
                let marginal = next_robust.saturating_sub(current_robust);
                Some((candidate, marginal, current_robust))
            })
            .max_by(|left, right| {
                left.1
                    .cmp(&right.1)
                    .then_with(|| left.2.cmp(&right.2))
                    .then_with(|| right.0.arm_id.cmp(&left.0.arm_id))
            });
        let Some((candidate, marginal, _)) = next else {
            break;
        };
        if marginal == 0 {
            break;
        }
        let entry = allocations.entry(candidate.arm_id.clone()).or_default();
        *entry = entry.saturating_add(1);
        spent = spent.saturating_add(u64::from(candidate.cost_units_per_replicate));
        total_replicates = total_replicates.saturating_add(1);
    }
    let mut selected = Vec::new();
    let mut held = Vec::new();
    let mut risk_blocked = Vec::new();
    let mut feasibility_blocked = Vec::new();
    let mut budget_blocked = Vec::new();
    let mut unresolved = Vec::new();
    let mut negative_evidence = Vec::new();
    let mut uncertainty = Vec::new();
    for candidate in &request.candidates {
        let allocated = allocations.get(&candidate.arm_id).copied().unwrap_or(0);
        let (robust, expected, worst_case_scenario_id, scenario_utility_milli) = utility_summary(
            candidate,
            &request.scenarios,
            allocated.max(request.min_replicates_per_selected_arm),
        );
        let (action, rationale) = if allocated > 0 {
            selected.push(candidate.arm_id.clone());
            (
                RobustDesignActionKind::Allocate,
                format!("allocate {allocated} replicates using lower-tail scenario utility"),
            )
        } else if candidate.risk_milli > request.risk_ceiling_milli {
            risk_blocked.push(candidate.arm_id.clone());
            (
                RobustDesignActionKind::RiskBlocked,
                "risk exceeds the declared preclinical design ceiling".into(),
            )
        } else if candidate.feasibility_milli < request.min_feasibility_milli {
            feasibility_blocked.push(candidate.arm_id.clone());
            (
                RobustDesignActionKind::FeasibilityBlocked,
                "feasibility is below the declared execution gate".into(),
            )
        } else if robust < request.min_robust_utility_milli {
            unresolved.push(candidate.arm_id.clone());
            negative_evidence.push(format!("{}:robust-utility-below-gate", candidate.arm_id));
            (
                RobustDesignActionKind::Unresolved,
                "lower-tail scenario utility is below the declared information gate".into(),
            )
        } else if selected_candidates
            .iter()
            .any(|(selected, _, _)| selected.arm_id == candidate.arm_id)
        {
            budget_blocked.push(candidate.arm_id.clone());
            (
                RobustDesignActionKind::BudgetBlocked,
                "candidate met scientific gates but could not fit the bounded batch budget".into(),
            )
        } else {
            held.push(candidate.arm_id.clone());
            (
                RobustDesignActionKind::Hold,
                "candidate is deferred by the robust utility ranking".into(),
            )
        };
        if scenario_utility_milli.values().any(|value| *value == 0) {
            uncertainty.push(format!("{}:scenario-zero-utility", candidate.arm_id));
        }
        let _ = worst_case_scenario_id;
        let _ = action;
        let _ = rationale;
        let _ = expected;
        let _ = robust;
    }
    // Recompute reporting values from the final allocation map.
    let mut reported = Vec::new();
    for candidate in &request.candidates {
        let allocated = allocations.get(&candidate.arm_id).copied().unwrap_or(0);
        let evaluation_replicates = if allocated == 0 {
            request
                .min_replicates_per_selected_arm
                .min(candidate.max_replicates)
        } else {
            allocated
        };
        let (robust, expected, worst_case_scenario_id, scenario_utility_milli) =
            utility_summary(candidate, &request.scenarios, evaluation_replicates.max(1));
        let action = if allocated > 0 {
            RobustDesignActionKind::Allocate
        } else if candidate.risk_milli > request.risk_ceiling_milli {
            RobustDesignActionKind::RiskBlocked
        } else if candidate.feasibility_milli < request.min_feasibility_milli {
            RobustDesignActionKind::FeasibilityBlocked
        } else if robust < request.min_robust_utility_milli {
            RobustDesignActionKind::Unresolved
        } else if budget_blocked.contains(&candidate.arm_id) {
            RobustDesignActionKind::BudgetBlocked
        } else {
            RobustDesignActionKind::Hold
        };
        reported.push(RobustCandidateAllocation {
            arm_id: candidate.arm_id.clone(),
            label: candidate.label.clone(),
            feature_id: candidate.feature_id.clone(),
            robust_utility_milli: robust,
            expected_utility_milli: expected,
            worst_case_scenario_id,
            scenario_utility_milli,
            allocated_replicates: allocated,
            projected_cost_units: u64::from(allocated)
                .saturating_mul(u64::from(candidate.cost_units_per_replicate)),
            risk_milli: candidate.risk_milli,
            feasibility_milli: candidate.feasibility_milli,
            action,
            rationale: match action {
                RobustDesignActionKind::Allocate => "selected by robust lower-tail utility".into(),
                RobustDesignActionKind::RiskBlocked => "risk exceeds design ceiling".into(),
                RobustDesignActionKind::FeasibilityBlocked => {
                    "feasibility below design gate".into()
                }
                RobustDesignActionKind::Unresolved => {
                    "robust utility below information gate".into()
                }
                RobustDesignActionKind::Hold => "deferred by bounded selection".into(),
                RobustDesignActionKind::BudgetBlocked => "budget blocked".into(),
            },
        });
    }
    // All output rows are sorted before digesting so replay is independent of map iteration.
    selected.sort();
    held.sort();
    risk_blocked.sort();
    feasibility_blocked.sort();
    budget_blocked.sort();
    unresolved.sort();
    negative_evidence.sort();
    negative_evidence.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    reported.sort_by(|left, right| left.arm_id.cmp(&right.arm_id));
    let total_allocated_replicates = reported
        .iter()
        .map(|allocation| u32::from(allocation.allocated_replicates))
        .sum::<u32>();
    let spent = reported
        .iter()
        .map(|allocation| allocation.projected_cost_units)
        .sum::<u64>();
    let disposition = if !selected.is_empty() && unresolved.is_empty() && budget_blocked.is_empty()
    {
        RobustExperimentDesignDisposition::Qualified
    } else if !selected.is_empty() {
        RobustExperimentDesignDisposition::Partial
    } else if !budget_blocked.is_empty() {
        RobustExperimentDesignDisposition::BudgetBlocked
    } else if !unresolved.is_empty() {
        RobustExperimentDesignDisposition::Unresolved
    } else {
        RobustExperimentDesignDisposition::NoEligibleActions
    };
    let mut output = RobustExperimentDesign {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        scenario_order,
        candidate_order,
        selected_order: selected,
        held_order: held,
        risk_blocked_order: risk_blocked,
        feasibility_blocked_order: feasibility_blocked,
        budget_blocked_order: budget_blocked,
        unresolved_order: unresolved,
        allocations: reported,
        budget_remaining_units: request.budget_units.saturating_sub(spent),
        total_allocated_replicates,
        negative_evidence,
        uncertainty,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-robust-experiment-design"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| RobustExperimentDesignError::Digest(error.to_string()))?;
    validate_output(&output)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(arm_id: &str, strong: bool) -> RobustDesignCandidate {
        let first = if strong { 700 } else { 100 };
        let second = if strong { 600 } else { 50 };
        RobustDesignCandidate {
            arm_id: arm_id.into(),
            label: format!("{arm_id} assay"),
            feature_id: format!("assay-{arm_id}"),
            cost_units_per_replicate: 2,
            risk_milli: 200,
            feasibility_milli: 900,
            max_replicates: 3,
            utility_milli_by_scenario: BTreeMap::from([
                ("mechanism-a".into(), vec![first, first / 2, first / 4]),
                ("mechanism-b".into(), vec![second, second / 2, second / 4]),
            ]),
        }
    }

    fn request() -> RobustExperimentDesignRequest {
        RobustExperimentDesignRequest {
            objective: "robustly distinguish invasion mechanisms".into(),
            model_system: GliomaModelSystem::Organoid,
            scenarios: vec![
                RobustDesignScenario {
                    scenario_id: "mechanism-a".into(),
                    label: "mechanism A".into(),
                    weight_milli: 500,
                },
                RobustDesignScenario {
                    scenario_id: "mechanism-b".into(),
                    label: "mechanism B".into(),
                    weight_milli: 500,
                },
            ],
            candidates: vec![candidate("strong", true), candidate("weak", false)],
            budget_units: 8,
            max_selected_arms: 1,
            min_replicates_per_selected_arm: 2,
            max_total_replicates: 3,
            min_feasibility_milli: 700,
            risk_ceiling_milli: 500,
            min_robust_utility_milli: 400,
        }
    }

    #[test]
    fn robust_design_allocates_lower_tail_information() {
        let output = design_glioma_robust_experiment(&request()).unwrap();
        assert_eq!(
            output.disposition,
            RobustExperimentDesignDisposition::Partial
        );
        assert_eq!(output.selected_order, vec!["strong"]);
        assert!(output.total_allocated_replicates >= 2);
        output.validate().unwrap();
    }

    #[test]
    fn robust_design_keeps_weak_candidate_unresolved() {
        let mut request = request();
        request.max_selected_arms = 2;
        request.max_total_replicates = 4;
        let output = design_glioma_robust_experiment(&request).unwrap();
        assert!(output.unresolved_order.contains(&"weak".into()));
        assert!(output
            .negative_evidence
            .iter()
            .any(|item| item.contains("weak")));
    }
}

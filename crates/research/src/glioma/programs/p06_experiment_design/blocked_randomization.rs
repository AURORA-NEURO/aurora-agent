//! Confounding-aware blocked randomization for preclinical glioma experiments.
//!
//! This feature turns declared nuisance strata into an executable allocation matrix. It keeps
//! treatment arms balanced inside every block, spends remaining capacity where the expected
//! contrast precision gain per unit cost is largest, and exposes the exact blocks that could not
//! be filled. The planner is model-declared and local: it never observes outcomes, randomizes
//! animals, schedules a protocol, or makes a clinical decision.

use crate::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P06-F09";
pub const OUTPUT_SCHEMA: &str = "GliomaBlockedRandomizationDesign1@1";
pub const MAX_BLOCKS: usize = 1_024;
pub const MAX_ARMS: usize = 256;
pub const MAX_TOTAL_REPLICATES: u32 = 1_000_000;
pub const SCORE_SCALE: u64 = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RandomizationBlock {
    pub block_id: String,
    pub label: String,
    pub capacity: u32,
    pub weight_milli: u16,
    pub baseline_variance_milli: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RandomizationArm {
    pub arm_id: String,
    pub label: String,
    pub feature_id: String,
    pub cost_units_per_replicate: u32,
    pub expected_effect_milli: u32,
    pub risk_milli: u16,
    pub feasibility_milli: u16,
    pub max_replicates: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockedRandomizationRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub blocks: Vec<RandomizationBlock>,
    pub arms: Vec<RandomizationArm>,
    pub budget_units: u64,
    pub min_replicates_per_arm: u32,
    pub max_total_replicates: u32,
    pub min_feasibility_milli: u16,
    pub risk_ceiling_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockArmAllocation {
    pub block_id: String,
    pub arm_id: String,
    pub replicates: u32,
    pub projected_cost_units: u64,
    pub information_gain_milli: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockedRandomizationDisposition {
    Qualified,
    Partial,
    NoEligibleArms,
    CapacityBlocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockedRandomizationDesign {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub block_order: Vec<String>,
    pub arm_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub risk_blocked_order: Vec<String>,
    pub feasibility_blocked_order: Vec<String>,
    pub allocations: Vec<BlockArmAllocation>,
    pub balance_score_milli: u64,
    pub expected_precision_milli: u64,
    pub budget_remaining_units: u64,
    pub total_replicates: u32,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: BlockedRandomizationDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BlockedRandomizationError {
    #[error("blocked randomization request is invalid: {0}")]
    InvalidRequest(String),
    #[error("blocked randomization input is invalid: {0}")]
    InvalidInput(String),
    #[error("blocked randomization output is invalid: {0}")]
    InvalidOutput(String),
    #[error("blocked randomization digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &BlockedRandomizationDesign) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "model_system": output.model_system,
        "block_order": output.block_order,
        "arm_order": output.arm_order,
        "selected_order": output.selected_order,
        "risk_blocked_order": output.risk_blocked_order,
        "feasibility_blocked_order": output.feasibility_blocked_order,
        "allocations": output.allocations,
        "balance_score_milli": output.balance_score_milli,
        "expected_precision_milli": output.expected_precision_milli,
        "budget_remaining_units": output.budget_remaining_units,
        "total_replicates": output.total_replicates,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn validate_request(
    request: &BlockedRandomizationRequest,
) -> Result<(), BlockedRandomizationError> {
    if request.objective.trim().is_empty()
        || request.blocks.len() < 2
        || request.blocks.len() > MAX_BLOCKS
        || request.arms.len() < 2
        || request.arms.len() > MAX_ARMS
        || request.budget_units == 0
        || request.min_replicates_per_arm == 0
        || request.max_total_replicates == 0
        || request.max_total_replicates > MAX_TOTAL_REPLICATES
        || request.min_feasibility_milli > 1_000
        || request.risk_ceiling_milli > 1_000
    {
        return Err(BlockedRandomizationError::InvalidRequest(
            "objective, at least two bounded blocks and arms, positive budget/replicate limits, and finite risk/feasibility gates are required".into(),
        ));
    }
    let mut block_ids = BTreeSet::new();
    let mut weight = 0_u32;
    for block in &request.blocks {
        if block.block_id.trim().is_empty()
            || block.label.trim().is_empty()
            || block.capacity == 0
            || block.weight_milli == 0
            || block.baseline_variance_milli == 0
            || !block_ids.insert(block.block_id.clone())
        {
            return Err(BlockedRandomizationError::InvalidInput(
                "block identities, capacity, weight, variance, and uniqueness are required".into(),
            ));
        }
        weight = weight.saturating_add(u32::from(block.weight_milli));
    }
    if weight != 1_000 {
        return Err(BlockedRandomizationError::InvalidInput(
            "block weights must sum to exactly 1000 milli-units".into(),
        ));
    }
    let mut arm_ids = BTreeSet::new();
    for arm in &request.arms {
        if arm.arm_id.trim().is_empty()
            || arm.label.trim().is_empty()
            || arm.feature_id.trim().is_empty()
            || arm.cost_units_per_replicate == 0
            || arm.expected_effect_milli == 0
            || arm.risk_milli > 1_000
            || arm.feasibility_milli > 1_000
            || arm.max_replicates < request.min_replicates_per_arm
            || !arm_ids.insert(arm.arm_id.clone())
        {
            return Err(BlockedRandomizationError::InvalidInput(
                "arm identity, feature, positive cost/effect, bounded risk/feasibility, capacity, and uniqueness are required".into(),
            ));
        }
    }
    Ok(())
}

fn validate_output(output: &BlockedRandomizationDesign) -> Result<(), BlockedRandomizationError> {
    if output.feature_id != FEATURE_ID
        || output.output_schema != OUTPUT_SCHEMA
        || output.objective.trim().is_empty()
        || !canonical(&output.block_order)
        || !canonical(&output.arm_order)
        || !canonical(&output.selected_order)
        || !canonical(&output.risk_blocked_order)
        || !canonical(&output.feasibility_blocked_order)
        || !canonical(&output.negative_evidence)
        || !canonical(&output.uncertainty)
        || output.allocations.windows(2).any(|pair| {
            (pair[0].block_id.clone(), pair[0].arm_id.clone())
                >= (pair[1].block_id.clone(), pair[1].arm_id.clone())
        })
        || output.allocations.iter().any(|allocation| {
            allocation.block_id.trim().is_empty()
                || allocation.arm_id.trim().is_empty()
                || allocation.replicates == 0
        })
    {
        return Err(BlockedRandomizationError::InvalidOutput(
            "identity, canonical ordering, allocation, or metric invariants are invalid".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(output))
        .map_err(|error| BlockedRandomizationError::Digest(error.to_string()))?;
    if expected != output.digest {
        return Err(BlockedRandomizationError::InvalidOutput(
            "digest is not bound to the blocked randomization design".into(),
        ));
    }
    Ok(())
}

impl BlockedRandomizationDesign {
    pub fn validate(&self) -> Result<(), BlockedRandomizationError> {
        validate_output(self)
    }
}

fn marginal_information(arm: &RandomizationArm, block: &RandomizationBlock, n: u32) -> u64 {
    let effect = u64::from(arm.expected_effect_milli);
    let variance = u64::from(block.baseline_variance_milli);
    effect
        .saturating_mul(effect)
        .saturating_mul(u64::from(block.weight_milli))
        .saturating_mul(SCORE_SCALE)
        / ((variance.saturating_add(u64::from(n)).saturating_add(1)).saturating_mul(1_000))
}

/// Plan a balanced, variance-aware allocation across declared nuisance strata.
pub fn plan_glioma_blocked_randomization(
    request: &BlockedRandomizationRequest,
) -> Result<BlockedRandomizationDesign, BlockedRandomizationError> {
    validate_request(request)?;
    let block_order = request
        .blocks
        .iter()
        .map(|block| block.block_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let arm_order = request
        .arms
        .iter()
        .map(|arm| arm.arm_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let eligible = request
        .arms
        .iter()
        .filter(|arm| {
            arm.risk_milli <= request.risk_ceiling_milli
                && arm.feasibility_milli >= request.min_feasibility_milli
        })
        .collect::<Vec<_>>();
    let mut risk_blocked_order = request
        .arms
        .iter()
        .filter(|arm| arm.risk_milli > request.risk_ceiling_milli)
        .map(|arm| arm.arm_id.clone())
        .collect::<Vec<_>>();
    let mut feasibility_blocked_order = request
        .arms
        .iter()
        .filter(|arm| {
            arm.risk_milli <= request.risk_ceiling_milli
                && arm.feasibility_milli < request.min_feasibility_milli
        })
        .map(|arm| arm.arm_id.clone())
        .collect::<Vec<_>>();
    risk_blocked_order.sort();
    feasibility_blocked_order.sort();

    let mut selected = eligible.to_vec();
    selected.sort_by(|left, right| {
        let left_score = u64::from(left.expected_effect_milli)
            .saturating_mul(u64::from(left.feasibility_milli))
            / u64::from(left.cost_units_per_replicate);
        let right_score = u64::from(right.expected_effect_milli)
            .saturating_mul(u64::from(right.feasibility_milli))
            / u64::from(right.cost_units_per_replicate);
        right_score
            .cmp(&left_score)
            .then_with(|| left.arm_id.cmp(&right.arm_id))
    });
    if selected.len() > 8 {
        selected.truncate(8);
    }
    let mut selected_order = selected
        .iter()
        .map(|arm| arm.arm_id.clone())
        .collect::<Vec<_>>();
    selected_order.sort();
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
    if selected.len() < 2 {
        let mut output = BlockedRandomizationDesign {
            feature_id: FEATURE_ID.into(),
            output_schema: OUTPUT_SCHEMA.into(),
            objective: request.objective.clone(),
            model_system: request.model_system,
            block_order,
            arm_order,
            selected_order,
            risk_blocked_order,
            feasibility_blocked_order,
            allocations: Vec::new(),
            balance_score_milli: 0,
            expected_precision_milli: 0,
            budget_remaining_units: request.budget_units,
            total_replicates: 0,
            negative_evidence,
            uncertainty: vec!["at-least-two-eligible-arms-required-for-a-contrast".into()],
            disposition: BlockedRandomizationDisposition::NoEligibleArms,
            digest: ContentHash::of_bytes(b"pending"),
        };
        output.digest = ContentHash::of_value(&digest_input(&output))
            .map_err(|error| BlockedRandomizationError::Digest(error.to_string()))?;
        return Ok(output);
    }

    let mut counts = BTreeMap::<(String, String), u32>::new();
    let mut spent = 0_u64;
    let mut total = 0_u32;
    for block in &request.blocks {
        for arm in &selected {
            if block.capacity < request.min_replicates_per_arm
                || total.saturating_add(request.min_replicates_per_arm)
                    > request.max_total_replicates
                || spent.saturating_add(
                    u64::from(request.min_replicates_per_arm)
                        .saturating_mul(u64::from(arm.cost_units_per_replicate)),
                ) > request.budget_units
            {
                continue;
            }
            counts.insert(
                (block.block_id.clone(), arm.arm_id.clone()),
                request.min_replicates_per_arm,
            );
            total = total.saturating_add(request.min_replicates_per_arm);
            spent = spent.saturating_add(
                u64::from(request.min_replicates_per_arm)
                    .saturating_mul(u64::from(arm.cost_units_per_replicate)),
            );
        }
    }
    loop {
        let mut best: Option<(u64, String, String)> = None;
        for block in &request.blocks {
            let block_total = selected
                .iter()
                .map(|arm| {
                    counts
                        .get(&(block.block_id.clone(), arm.arm_id.clone()))
                        .copied()
                        .unwrap_or(0)
                })
                .sum::<u32>();
            for arm in &selected {
                let key = (block.block_id.clone(), arm.arm_id.clone());
                let current = counts.get(&key).copied().unwrap_or(0);
                if current >= arm.max_replicates
                    || block_total >= block.capacity
                    || total >= request.max_total_replicates
                    || spent.saturating_add(u64::from(arm.cost_units_per_replicate))
                        > request.budget_units
                {
                    continue;
                }
                let min_count = selected
                    .iter()
                    .map(|other| {
                        counts
                            .get(&(block.block_id.clone(), other.arm_id.clone()))
                            .copied()
                            .unwrap_or(0)
                    })
                    .min()
                    .unwrap_or(0);
                if current >= min_count.saturating_add(1) {
                    continue;
                }
                let arm_total = counts
                    .iter()
                    .filter(|((_, candidate_arm), _)| candidate_arm == &arm.arm_id)
                    .map(|(_, count)| *count)
                    .sum::<u32>();
                let global_min = selected
                    .iter()
                    .map(|other| {
                        counts
                            .iter()
                            .filter(|((_, candidate_arm), _)| candidate_arm == &other.arm_id)
                            .map(|(_, count)| *count)
                            .sum::<u32>()
                    })
                    .min()
                    .unwrap_or(0);
                if arm_total >= global_min.saturating_add(1) {
                    continue;
                }
                let information = marginal_information(arm, block, current);
                let value = information.saturating_mul(u64::from(arm.feasibility_milli))
                    / u64::from(arm.cost_units_per_replicate);
                let candidate = (value, block.block_id.clone(), arm.arm_id.clone());
                if best
                    .as_ref()
                    .map(|existing| candidate > *existing)
                    .unwrap_or(true)
                {
                    best = Some(candidate);
                }
            }
        }
        let Some((_, block_id, arm_id)) = best else {
            break;
        };
        let arm = selected
            .iter()
            .find(|arm| arm.arm_id == arm_id)
            .expect("selected arm");
        let key = (block_id, arm_id);
        *counts.entry(key).or_default() += 1;
        total = total.saturating_add(1);
        spent = spent.saturating_add(u64::from(arm.cost_units_per_replicate));
    }

    let mut allocations = Vec::new();
    for block in &request.blocks {
        for arm in &selected {
            let replicates = counts
                .get(&(block.block_id.clone(), arm.arm_id.clone()))
                .copied()
                .unwrap_or(0);
            if replicates > 0 {
                allocations.push(BlockArmAllocation {
                    block_id: block.block_id.clone(),
                    arm_id: arm.arm_id.clone(),
                    replicates,
                    projected_cost_units: u64::from(replicates)
                        .saturating_mul(u64::from(arm.cost_units_per_replicate)),
                    information_gain_milli: marginal_information(arm, block, replicates),
                });
            }
        }
    }
    allocations.sort_by(|left, right| {
        left.block_id
            .cmp(&right.block_id)
            .then_with(|| left.arm_id.cmp(&right.arm_id))
    });
    let arm_totals = selected
        .iter()
        .map(|arm| {
            allocations
                .iter()
                .filter(|allocation| allocation.arm_id == arm.arm_id)
                .map(|allocation| allocation.replicates)
                .sum::<u32>()
        })
        .collect::<Vec<_>>();
    let min_total = arm_totals.iter().copied().min().unwrap_or(0);
    let max_total = arm_totals.iter().copied().max().unwrap_or(0);
    let balance_score_milli = if max_total == 0 {
        0
    } else {
        u64::from(min_total).saturating_mul(1_000) / u64::from(max_total)
    };
    let expected_precision_milli = allocations
        .iter()
        .map(|allocation| allocation.information_gain_milli)
        .sum::<u64>()
        .min(SCORE_SCALE);
    if balance_score_milli < 900 {
        uncertainty.push("arm-count-balance-below-900-milli".into());
    }
    if total < request.max_total_replicates && spent < request.budget_units {
        uncertainty.push("capacity-or-arm-horizon-limited-before-global-budget".into());
    }
    if !risk_blocked_order.is_empty() || !feasibility_blocked_order.is_empty() {
        negative_evidence.push("eligible-set-is-not-the-full-declared-arm-set".into());
    }
    negative_evidence.sort();
    uncertainty.sort();
    let disposition = if allocations.is_empty() {
        BlockedRandomizationDisposition::CapacityBlocked
    } else if !uncertainty.is_empty() || selected.len() < eligible.len() {
        BlockedRandomizationDisposition::Partial
    } else {
        BlockedRandomizationDisposition::Qualified
    };
    let mut output = BlockedRandomizationDesign {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        block_order,
        arm_order,
        selected_order,
        risk_blocked_order,
        feasibility_blocked_order,
        allocations,
        balance_score_milli,
        expected_precision_milli,
        budget_remaining_units: request.budget_units.saturating_sub(spent),
        total_replicates: total,
        negative_evidence,
        uncertainty,
        disposition,
        digest: ContentHash::of_bytes(b"pending"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| BlockedRandomizationError::Digest(error.to_string()))?;
    validate_output(&output)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> BlockedRandomizationRequest {
        BlockedRandomizationRequest {
            objective: "separate invasion mechanisms".into(),
            model_system: GliomaModelSystem::Organoid,
            blocks: vec![
                RandomizationBlock {
                    block_id: "b1".into(),
                    label: "low oxygen".into(),
                    capacity: 12,
                    weight_milli: 500,
                    baseline_variance_milli: 20,
                },
                RandomizationBlock {
                    block_id: "b2".into(),
                    label: "standard oxygen".into(),
                    capacity: 12,
                    weight_milli: 500,
                    baseline_variance_milli: 30,
                },
            ],
            arms: vec![
                RandomizationArm {
                    arm_id: "control".into(),
                    label: "control".into(),
                    feature_id: "assay-control".into(),
                    cost_units_per_replicate: 2,
                    expected_effect_milli: 300,
                    risk_milli: 100,
                    feasibility_milli: 900,
                    max_replicates: 10,
                },
                RandomizationArm {
                    arm_id: "perturb".into(),
                    label: "perturbation".into(),
                    feature_id: "assay-perturb".into(),
                    cost_units_per_replicate: 3,
                    expected_effect_milli: 500,
                    risk_milli: 100,
                    feasibility_milli: 850,
                    max_replicates: 10,
                },
            ],
            budget_units: 36,
            min_replicates_per_arm: 1,
            max_total_replicates: 12,
            min_feasibility_milli: 700,
            risk_ceiling_milli: 500,
        }
    }

    #[test]
    fn plans_balanced_deterministic_allocation() {
        let first = plan_glioma_blocked_randomization(&request()).expect("plan");
        let second = plan_glioma_blocked_randomization(&request()).expect("plan");
        assert_eq!(first, second);
        assert_eq!(first.selected_order.len(), 2);
        assert!(first.balance_score_milli >= 900);
        first.validate().expect("valid output");
    }

    #[test]
    fn retains_risk_blocked_arm_as_negative_evidence() {
        let mut input = request();
        input.arms[1].risk_milli = 900;
        let output = plan_glioma_blocked_randomization(&input).expect("plan");
        assert_eq!(output.risk_blocked_order, vec!["perturb"]);
        assert!(output
            .negative_evidence
            .iter()
            .any(|entry| entry == "risk-gate-blocked:perturb"));
    }
}

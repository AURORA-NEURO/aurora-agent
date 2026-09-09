//! Autonomous adaptive-allocation campaigns for preclinical glioma experiments.
//!
//! P06's allocator computes a bounded next replicate batch from Beta posteriors. This module
//! closes the product loop around that planner: it dispatches one arm batch through a caller-owned
//! local adapter, merges the returned successes/failures, and replans with the updated posterior.
//! The campaign never treats a planned replicate as an observation, never moves raw biology, and
//! preserves underpowered, negative, risk-blocked, budget, retry, and executor-failure states.

use super::adaptive_allocation::{
    allocate_glioma_assays, AdaptiveAllocation, AdaptiveAllocationDisposition,
    AdaptiveAllocationRequest, AdaptiveArmObservation,
};
use crate::glioma_engine::LocalArtifactRef;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P06-F22";
pub const OUTPUT_SCHEMA: &str = "GliomaAdaptiveAllocationCampaign1@1";
pub const MAX_ROUNDS: u16 = 128;
pub const MAX_RETRIES: u8 = 8;
pub const MAX_BATCHES: usize = 32_768;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveAllocationCampaignRequest {
    pub allocation: AdaptiveAllocationRequest,
    pub arms: Vec<AdaptiveArmObservation>,
    pub max_rounds: u16,
    pub max_retries: u8,
    pub stop_on_negative: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveAllocationBatchObservation {
    pub batch_id: String,
    pub arm_id: String,
    pub requested_replicates: u32,
    pub successes: u32,
    pub failures: u32,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdaptiveAllocationCampaignExecutionFailure {
    pub reason: String,
    pub retryable: bool,
}

/// Institution-local assay, imaging, organoid, or computational gateways implement this seam.
/// The controller only accepts a de-identified aggregate batch with an exact replicate count.
pub trait AdaptiveAllocationCampaignExecutor {
    fn execute_arm(
        &mut self,
        arm: &AdaptiveArmObservation,
        requested_replicates: u32,
        round: u16,
        attempt: u8,
    ) -> Result<AdaptiveAllocationBatchObservation, AdaptiveAllocationCampaignExecutionFailure>;

    fn simulation_only(&self) -> bool {
        false
    }
}

/// Deterministic local sandbox adapter. It returns aggregate Bernoulli outcomes from the arm's
/// current posterior; this is a workflow rehearsal and must not be interpreted as biology.
#[derive(Debug, Default)]
pub struct DryRunAdaptiveAllocationCampaignExecutor;

impl AdaptiveAllocationCampaignExecutor for DryRunAdaptiveAllocationCampaignExecutor {
    fn execute_arm(
        &mut self,
        arm: &AdaptiveArmObservation,
        requested_replicates: u32,
        round: u16,
        attempt: u8,
    ) -> Result<AdaptiveAllocationBatchObservation, AdaptiveAllocationCampaignExecutionFailure>
    {
        let total = u64::from(arm.prior_alpha)
            .saturating_add(u64::from(arm.prior_beta))
            .saturating_add(u64::from(arm.successes))
            .saturating_add(u64::from(arm.failures));
        let successes_seen = u64::from(arm.prior_alpha).saturating_add(u64::from(arm.successes));
        let success_rate_milli = if total == 0 {
            500
        } else {
            successes_seen
                .saturating_mul(1_000)
                .checked_div(total)
                .unwrap_or(500)
        };
        let successes = u64::from(requested_replicates)
            .saturating_mul(success_rate_milli)
            .saturating_div(1_000) as u32;
        let failures = requested_replicates.saturating_sub(successes);
        let batch_id = format!(
            "dry-run-adaptive-allocation:{}:{}:{}",
            arm.arm_id, round, attempt
        );
        let content_hash = ContentHash::of_value(&serde_json::json!({
            "batch_id": batch_id,
            "arm_id": arm.arm_id,
            "requested_replicates": requested_replicates,
            "successes": successes,
            "failures": failures,
            "simulation_only": true,
        }))
        .map_err(|error| AdaptiveAllocationCampaignExecutionFailure {
            reason: format!("dry-run adaptive allocation digest failed: {error}"),
            retryable: false,
        })?;
        Ok(AdaptiveAllocationBatchObservation {
            batch_id,
            arm_id: arm.arm_id.clone(),
            requested_replicates,
            successes,
            failures,
            artifact: LocalArtifactRef {
                artifact_id: format!("dry-run-adaptive-allocation:{}", arm.arm_id),
                content_hash,
                content_type: "application/vnd.aurora.glioma.adaptive-allocation+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
        })
    }

    fn simulation_only(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveAllocationCampaignRound {
    pub round: u16,
    pub allocation: AdaptiveAllocation,
    pub selected_arm_order: Vec<String>,
    pub requested_replicates: Vec<(String, u32)>,
    pub accepted_batch_order: Vec<String>,
    pub retry_count: u32,
    pub cost_units: u64,
    pub budget_before_units: u64,
    pub budget_after_units: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdaptiveAllocationCampaignDisposition {
    Qualified,
    Partial,
    Negative,
    BudgetBlocked,
    Failed,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdaptiveAllocationCampaignStopReason {
    Qualified,
    Negative,
    BudgetExhausted,
    NoAllocation,
    MaxRounds,
    ExecutorFailed,
    NoProgress,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveAllocationCampaign {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: crate::glioma_engine::GliomaModelSystem,
    pub budget_units: u64,
    pub rounds: Vec<AdaptiveAllocationCampaignRound>,
    pub final_arms: Vec<AdaptiveArmObservation>,
    pub batches: Vec<AdaptiveAllocationBatchObservation>,
    pub completed_arm_order: Vec<String>,
    pub failed_arm_order: Vec<String>,
    pub accepted_batch_order: Vec<String>,
    pub retry_count: u32,
    pub budget_spent_units: u64,
    pub remaining_budget_units: u64,
    pub final_allocation: AdaptiveAllocation,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub simulation_only: bool,
    pub disposition: AdaptiveAllocationCampaignDisposition,
    pub stop_reason: AdaptiveAllocationCampaignStopReason,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AdaptiveAllocationCampaignError {
    #[error("adaptive-allocation campaign request is invalid: {0}")]
    InvalidRequest(String),
    #[error("adaptive-allocation campaign planning failed: {0}")]
    Planning(String),
    #[error("adaptive-allocation campaign execution failed: {0}")]
    Execution(String),
    #[error("adaptive-allocation campaign output is invalid: {0}")]
    InvalidOutput(String),
    #[error("adaptive-allocation campaign digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(campaign: &AdaptiveAllocationCampaign) -> serde_json::Value {
    serde_json::json!({
        "feature_id": campaign.feature_id,
        "output_schema": campaign.output_schema,
        "objective": campaign.objective,
        "model_system": campaign.model_system,
        "budget_units": campaign.budget_units,
        "rounds": campaign.rounds,
        "final_arms": campaign.final_arms,
        "batches": campaign.batches,
        "completed_arm_order": campaign.completed_arm_order,
        "failed_arm_order": campaign.failed_arm_order,
        "accepted_batch_order": campaign.accepted_batch_order,
        "retry_count": campaign.retry_count,
        "budget_spent_units": campaign.budget_spent_units,
        "remaining_budget_units": campaign.remaining_budget_units,
        "final_allocation": campaign.final_allocation,
        "negative_evidence": campaign.negative_evidence,
        "uncertainty": campaign.uncertainty,
        "simulation_only": campaign.simulation_only,
        "disposition": campaign.disposition,
        "stop_reason": campaign.stop_reason,
    })
}

fn validate_batch(
    batch: &AdaptiveAllocationBatchObservation,
    arm_id: &str,
    requested_replicates: u32,
) -> Result<(), AdaptiveAllocationCampaignError> {
    batch
        .artifact
        .validate()
        .map_err(|error| AdaptiveAllocationCampaignError::Execution(error.to_string()))?;
    if batch.batch_id.trim().is_empty()
        || batch.arm_id != arm_id
        || batch.requested_replicates != requested_replicates
        || requested_replicates == 0
        || batch.successes.saturating_add(batch.failures) != requested_replicates
        || !batch.artifact.local_only
        || batch.artifact.contains_human_data
        || batch.artifact.contains_direct_identifiers
    {
        return Err(AdaptiveAllocationCampaignError::Execution(
            "executor returned an invalid arm-bound batch contract".into(),
        ));
    }
    Ok(())
}

fn update_arm(
    arms: &mut [AdaptiveArmObservation],
    batch: &AdaptiveAllocationBatchObservation,
) -> Result<(), AdaptiveAllocationCampaignError> {
    let arm = arms
        .iter_mut()
        .find(|arm| arm.arm_id == batch.arm_id)
        .ok_or_else(|| {
            AdaptiveAllocationCampaignError::Execution("returned arm is unknown".into())
        })?;
    arm.successes = arm.successes.saturating_add(batch.successes);
    arm.failures = arm.failures.saturating_add(batch.failures);
    Ok(())
}

fn allocation_request(
    request: &AdaptiveAllocationCampaignRequest,
    remaining_budget_units: u64,
) -> AdaptiveAllocationRequest {
    let mut allocation = request.allocation.clone();
    allocation.budget_units = remaining_budget_units;
    allocation.max_selected_arms = 1;
    allocation
}

fn is_budget_blocked(
    allocation: &AdaptiveAllocation,
    arms: &[AdaptiveArmObservation],
    remaining_budget_units: u64,
) -> bool {
    allocation.posteriors.iter().any(|posterior| {
        if posterior.is_control || posterior.recommended_replicates == 0 {
            return false;
        }
        let Some(arm) = arms.iter().find(|arm| arm.arm_id == posterior.arm_id) else {
            return false;
        };
        posterior.allocated_replicates == 0 && u64::from(arm.cost_units) > remaining_budget_units
    })
}

impl AdaptiveAllocationCampaign {
    pub fn validate(&self) -> Result<(), AdaptiveAllocationCampaignError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self
                .budget_spent_units
                .saturating_add(self.remaining_budget_units)
                != self.budget_units
            || !canonical(&self.completed_arm_order)
            || !canonical(&self.failed_arm_order)
            || !canonical(&self.accepted_batch_order)
            || self
                .completed_arm_order
                .iter()
                .any(|id| self.failed_arm_order.binary_search(id).is_ok())
            || self.batches.len() > MAX_BATCHES
            || self
                .rounds
                .windows(2)
                .any(|pair| pair[0].round >= pair[1].round)
            || self.rounds.iter().any(|round| {
                round.round == 0
                    || round.selected_arm_order.len() != 1
                    || round.requested_replicates.len() != 1
                    || round.accepted_batch_order.len() > 1
                    || round.cost_units
                        != round
                            .budget_before_units
                            .saturating_sub(round.budget_after_units)
            })
        {
            return Err(AdaptiveAllocationCampaignError::InvalidOutput(
                "identity, budget, canonical partition, or round invariants are invalid".into(),
            ));
        }
        self.final_allocation
            .validate()
            .map_err(|error| AdaptiveAllocationCampaignError::InvalidOutput(error.to_string()))?;
        if self.final_allocation.objective != self.objective
            || self.final_allocation.model_system != self.model_system
        {
            return Err(AdaptiveAllocationCampaignError::InvalidOutput(
                "final allocation does not reconcile with campaign identity".into(),
            ));
        }
        let mut batch_ids = BTreeSet::new();
        for batch in &self.batches {
            if !batch_ids.insert(batch.batch_id.clone())
                || batch.batch_id.trim().is_empty()
                || batch.requested_replicates == 0
                || batch.successes.saturating_add(batch.failures) != batch.requested_replicates
            {
                return Err(AdaptiveAllocationCampaignError::InvalidOutput(
                    "batch identity or replicate accounting is invalid".into(),
                ));
            }
            batch.artifact.validate().map_err(|error| {
                AdaptiveAllocationCampaignError::InvalidOutput(error.to_string())
            })?;
        }
        if batch_ids.iter().cloned().collect::<Vec<_>>() != self.accepted_batch_order {
            return Err(AdaptiveAllocationCampaignError::InvalidOutput(
                "accepted batch order does not reconcile".into(),
            ));
        }
        let mut arm_ids = BTreeSet::new();
        for arm in &self.final_arms {
            if !arm_ids.insert(arm.arm_id.clone())
                || arm.arm_id.trim().is_empty()
                || arm.label.trim().is_empty()
                || arm.artifact.validate().is_err()
            {
                return Err(AdaptiveAllocationCampaignError::InvalidOutput(
                    "final arm identity or artifact is invalid".into(),
                ));
            }
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| AdaptiveAllocationCampaignError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(AdaptiveAllocationCampaignError::InvalidOutput(
                "campaign digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Execute one adaptive replicate batch at a time, updating the Beta posterior after every
/// accepted aggregate result.
pub fn execute_glioma_adaptive_allocation_campaign<E: AdaptiveAllocationCampaignExecutor>(
    request: &AdaptiveAllocationCampaignRequest,
    executor: &mut E,
) -> Result<AdaptiveAllocationCampaign, AdaptiveAllocationCampaignError> {
    if request.max_rounds == 0
        || request.max_rounds > MAX_ROUNDS
        || request.max_retries > MAX_RETRIES
        || request.arms.is_empty()
    {
        return Err(AdaptiveAllocationCampaignError::InvalidRequest(
            "positive bounded rounds, retries, and arms are required".into(),
        ));
    }
    let mut arms = request.arms.clone();
    allocate_glioma_assays(&request.allocation, &arms)
        .map_err(|error| AdaptiveAllocationCampaignError::InvalidRequest(error.to_string()))?;
    let mut batches = Vec::new();
    let mut rounds = Vec::new();
    let mut completed = BTreeSet::new();
    let mut failed = BTreeSet::new();
    let mut accepted_batch_ids = BTreeSet::new();
    let mut negative_evidence = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut retry_count = 0_u32;
    let mut budget_spent = 0_u64;
    let mut stop_reason = AdaptiveAllocationCampaignStopReason::MaxRounds;

    for round_number in 1..=request.max_rounds {
        let remaining = request.allocation.budget_units.saturating_sub(budget_spent);
        let plan_request = allocation_request(request, remaining);
        let allocation = allocate_glioma_assays(&plan_request, &arms)
            .map_err(|error| AdaptiveAllocationCampaignError::Planning(error.to_string()))?;
        negative_evidence.extend(allocation.negative_evidence.iter().cloned());
        uncertainty.extend(allocation.uncertainty.iter().cloned());
        if allocation.selected_order.is_empty() {
            stop_reason = if remaining == 0 || is_budget_blocked(&allocation, &arms, remaining) {
                AdaptiveAllocationCampaignStopReason::BudgetExhausted
            } else if request.stop_on_negative
                && allocation.disposition == AdaptiveAllocationDisposition::Negative
            {
                AdaptiveAllocationCampaignStopReason::Negative
            } else {
                AdaptiveAllocationCampaignStopReason::NoAllocation
            };
            break;
        }
        let arm_id = allocation.selected_order[0].clone();
        let posterior = allocation
            .posteriors
            .iter()
            .find(|posterior| posterior.arm_id == arm_id)
            .ok_or_else(|| {
                AdaptiveAllocationCampaignError::Planning(
                    "selected arm is absent from posterior output".into(),
                )
            })?;
        let requested_replicates = posterior.allocated_replicates;
        let arm = arms
            .iter()
            .find(|arm| arm.arm_id == arm_id)
            .ok_or_else(|| {
                AdaptiveAllocationCampaignError::Planning("selected arm is absent".into())
            })?;
        let cost_units = u64::from(requested_replicates).saturating_mul(u64::from(arm.cost_units));
        if cost_units == 0 || cost_units > remaining {
            return Err(AdaptiveAllocationCampaignError::Planning(
                "selected allocation does not fit the remaining budget".into(),
            ));
        }
        let budget_before = remaining;
        let mut accepted = None;
        let mut round_retries = 0_u32;
        for attempt in 1..=request.max_retries.saturating_add(1) {
            match executor.execute_arm(arm, requested_replicates, round_number, attempt) {
                Ok(batch) => {
                    validate_batch(&batch, &arm_id, requested_replicates)?;
                    if accepted_batch_ids.contains(&batch.batch_id) {
                        return Err(AdaptiveAllocationCampaignError::Execution(
                            "executor returned a duplicate batch identity".into(),
                        ));
                    }
                    accepted = Some(batch);
                    break;
                }
                Err(error) => {
                    if error.reason.trim().is_empty() {
                        return Err(AdaptiveAllocationCampaignError::Execution(
                            "executor returned an empty failure reason".into(),
                        ));
                    }
                    if error.retryable && attempt <= request.max_retries {
                        retry_count = retry_count.saturating_add(1);
                        round_retries = round_retries.saturating_add(1);
                        continue;
                    }
                    failed.insert(arm_id.clone());
                    stop_reason = AdaptiveAllocationCampaignStopReason::ExecutorFailed;
                    break;
                }
            }
        }
        budget_spent = budget_spent.saturating_add(cost_units);
        let budget_after = request.allocation.budget_units.saturating_sub(budget_spent);
        let mut accepted_batch_order = Vec::new();
        if let Some(batch) = accepted {
            update_arm(&mut arms, &batch)?;
            accepted_batch_ids.insert(batch.batch_id.clone());
            accepted_batch_order.push(batch.batch_id.clone());
            batches.push(batch);
            completed.insert(arm_id.clone());
        }
        rounds.push(AdaptiveAllocationCampaignRound {
            round: round_number,
            allocation,
            selected_arm_order: vec![arm_id.clone()],
            requested_replicates: vec![(arm_id.clone(), requested_replicates)],
            accepted_batch_order,
            retry_count: round_retries,
            cost_units,
            budget_before_units: budget_before,
            budget_after_units: budget_after,
        });
        if failed.contains(&arm_id) {
            break;
        }
        if !completed.contains(&arm_id) {
            stop_reason = AdaptiveAllocationCampaignStopReason::NoProgress;
            break;
        }
        if budget_after == 0 {
            stop_reason = AdaptiveAllocationCampaignStopReason::BudgetExhausted;
            break;
        }
    }

    let remaining = request.allocation.budget_units.saturating_sub(budget_spent);
    let final_request = allocation_request(request, remaining);
    let final_allocation = allocate_glioma_assays(&final_request, &arms)
        .map_err(|error| AdaptiveAllocationCampaignError::Planning(error.to_string()))?;
    negative_evidence.extend(final_allocation.negative_evidence.iter().cloned());
    uncertainty.extend(final_allocation.uncertainty.iter().cloned());
    if final_allocation.disposition == AdaptiveAllocationDisposition::Negative
        && final_allocation.selected_order.is_empty()
        && request.stop_on_negative
    {
        stop_reason = AdaptiveAllocationCampaignStopReason::Negative;
    }
    let disposition = match stop_reason {
        AdaptiveAllocationCampaignStopReason::Negative => {
            AdaptiveAllocationCampaignDisposition::Negative
        }
        AdaptiveAllocationCampaignStopReason::BudgetExhausted => {
            AdaptiveAllocationCampaignDisposition::BudgetBlocked
        }
        AdaptiveAllocationCampaignStopReason::ExecutorFailed => {
            AdaptiveAllocationCampaignDisposition::Failed
        }
        AdaptiveAllocationCampaignStopReason::NoAllocation
            if final_allocation.disposition == AdaptiveAllocationDisposition::Negative =>
        {
            AdaptiveAllocationCampaignDisposition::Negative
        }
        AdaptiveAllocationCampaignStopReason::NoAllocation
            if final_allocation.disposition == AdaptiveAllocationDisposition::BudgetBlocked =>
        {
            AdaptiveAllocationCampaignDisposition::BudgetBlocked
        }
        _ if final_allocation.disposition == AdaptiveAllocationDisposition::Unresolved => {
            AdaptiveAllocationCampaignDisposition::Unresolved
        }
        _ => AdaptiveAllocationCampaignDisposition::Partial,
    };
    let mut output = AdaptiveAllocationCampaign {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.allocation.objective.clone(),
        model_system: request.allocation.model_system,
        budget_units: request.allocation.budget_units,
        rounds,
        final_arms: arms,
        batches,
        completed_arm_order: completed.into_iter().collect(),
        failed_arm_order: failed.into_iter().collect(),
        accepted_batch_order: accepted_batch_ids.into_iter().collect(),
        retry_count,
        budget_spent_units: budget_spent,
        remaining_budget_units: remaining,
        final_allocation,
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        simulation_only: executor.simulation_only(),
        disposition,
        stop_reason,
        digest: ContentHash::of_bytes(b"unsealed-glioma-adaptive-allocation-campaign"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| AdaptiveAllocationCampaignError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: format!("local:{id}"),
            content_hash: ContentHash::of_bytes(id.as_bytes()),
            content_type: "application/json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn arm(id: &str, successes: u32, failures: u32, cost_units: u32) -> AdaptiveArmObservation {
        AdaptiveArmObservation {
            arm_id: id.into(),
            label: id.into(),
            artifact: artifact(id),
            model_system: GliomaModelSystem::Organoid,
            successes,
            failures,
            prior_alpha: 1,
            prior_beta: 1,
            risk_milli: 100,
            cost_units,
        }
    }

    fn request() -> AdaptiveAllocationCampaignRequest {
        AdaptiveAllocationCampaignRequest {
            allocation: AdaptiveAllocationRequest {
                objective: "allocate organoid invasion replicates".into(),
                model_system: GliomaModelSystem::Organoid,
                control_arm_id: "control".into(),
                target_effect_milli: 100,
                min_replicates_per_arm: 4,
                min_probability_milli: 700,
                max_posterior_uncertainty_milli: 200,
                exploration_weight_milli: 300,
                max_selected_arms: 1,
                max_new_replicates: 2,
                budget_units: 8,
                risk_ceiling_milli: 700,
            },
            arms: vec![arm("control", 1, 1, 1), arm("egfr", 1, 1, 1)],
            max_rounds: 3,
            max_retries: 1,
            stop_on_negative: true,
        }
    }

    #[test]
    fn campaign_replans_after_each_returned_batch_and_replays() {
        let request = request();
        let mut first_executor = DryRunAdaptiveAllocationCampaignExecutor;
        let mut second_executor = DryRunAdaptiveAllocationCampaignExecutor;
        let first =
            execute_glioma_adaptive_allocation_campaign(&request, &mut first_executor).unwrap();
        let second =
            execute_glioma_adaptive_allocation_campaign(&request, &mut second_executor).unwrap();
        assert_eq!(first, second);
        assert!(!first.rounds.is_empty());
        assert!(!first.batches.is_empty());
        assert_eq!(first.rounds.len(), first.batches.len());
        assert!(first
            .final_arms
            .iter()
            .any(|arm| arm.successes + arm.failures > 2));
        first.validate().unwrap();
    }

    #[test]
    fn budget_exhaustion_is_explicit_before_dispatch() {
        let mut request = request();
        request.allocation.budget_units = 1;
        request.arms[1].cost_units = 2;
        let mut executor = DryRunAdaptiveAllocationCampaignExecutor;
        let output = execute_glioma_adaptive_allocation_campaign(&request, &mut executor).unwrap();
        assert_eq!(output.rounds.len(), 0);
        assert_eq!(
            output.stop_reason,
            AdaptiveAllocationCampaignStopReason::BudgetExhausted
        );
        assert_eq!(
            output.disposition,
            AdaptiveAllocationCampaignDisposition::BudgetBlocked
        );
        output.validate().unwrap();
    }

    struct RetryOnce {
        failed: bool,
    }

    impl AdaptiveAllocationCampaignExecutor for RetryOnce {
        fn execute_arm(
            &mut self,
            arm: &AdaptiveArmObservation,
            requested_replicates: u32,
            _round: u16,
            _attempt: u8,
        ) -> Result<AdaptiveAllocationBatchObservation, AdaptiveAllocationCampaignExecutionFailure>
        {
            if !self.failed {
                self.failed = true;
                return Err(AdaptiveAllocationCampaignExecutionFailure {
                    reason: "temporary local gateway outage".into(),
                    retryable: true,
                });
            }
            Ok(AdaptiveAllocationBatchObservation {
                batch_id: "retry-success".into(),
                arm_id: arm.arm_id.clone(),
                requested_replicates,
                successes: requested_replicates,
                failures: 0,
                artifact: artifact("retry-success"),
            })
        }
    }

    #[test]
    fn retryable_gateway_failure_is_bounded_and_recorded() {
        let request = request();
        let mut executor = RetryOnce { failed: false };
        let output = execute_glioma_adaptive_allocation_campaign(&request, &mut executor).unwrap();
        assert_eq!(output.retry_count, 1);
        assert_eq!(output.rounds[0].retry_count, 1);
        assert!(!output.rounds.is_empty());
        output.validate().unwrap();
    }
}

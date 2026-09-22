//! Closed-loop execution for sequential preclinical glioma experiment design.
//!
//! The sequential planner is useful only when its decisions are applied to new observations and
//! replanned. This module supplies that loop without pretending that a local simulation is a
//! laboratory: every batch is aggregate-only, arm-bound, local, and returned through an explicit
//! executor seam. A governed institution can implement the seam for its own assay gateway; the
//! default executor is deterministic and simulation-only.

use super::sequential_design::{
    plan_glioma_sequential_design, SequentialArmObservation, SequentialDesignDisposition,
    SequentialDesignPlan, SequentialDesignRequest,
};
use crate::glioma_engine::LocalArtifactRef;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P06-F32";
pub const OUTPUT_SCHEMA: &str = "GliomaSequentialCampaign1@1";
pub const MAX_RETRIES: u8 = 8;
pub const MAX_BATCHES: usize = 32_768;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SequentialCampaignRequest {
    pub design: SequentialDesignRequest,
    pub arms: Vec<SequentialArmObservation>,
    pub max_retries: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SequentialBatchObservation {
    pub batch_id: String,
    pub arm_id: String,
    pub requested_replicates: u32,
    pub successes: u32,
    pub failures: u32,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SequentialCampaignExecutionFailure {
    pub reason: String,
    pub retryable: bool,
}

/// Institution-local assay, imaging, organoid, or computational gateways implement this seam.
/// The controller accepts only an aggregate batch with exact replicate accounting.
pub trait SequentialCampaignExecutor {
    fn execute_arm(
        &mut self,
        arm: &SequentialArmObservation,
        requested_replicates: u32,
        round: u16,
        attempt: u8,
    ) -> Result<SequentialBatchObservation, SequentialCampaignExecutionFailure>;

    fn simulation_only(&self) -> bool {
        false
    }
}

/// Deterministic local sandbox adapter. It models the declared arm posterior and never contacts
/// hardware or treats simulated observations as biological evidence.
#[derive(Debug, Default)]
pub struct DryRunSequentialCampaignExecutor;

impl SequentialCampaignExecutor for DryRunSequentialCampaignExecutor {
    fn execute_arm(
        &mut self,
        arm: &SequentialArmObservation,
        requested_replicates: u32,
        round: u16,
        attempt: u8,
    ) -> Result<SequentialBatchObservation, SequentialCampaignExecutionFailure> {
        let total = u64::from(arm.prior_alpha)
            .saturating_add(u64::from(arm.prior_beta))
            .saturating_add(u64::from(arm.successes))
            .saturating_add(u64::from(arm.failures));
        let successes_seen = u64::from(arm.prior_alpha).saturating_add(u64::from(arm.successes));
        let success_rate_milli = successes_seen
            .saturating_mul(1_000)
            .checked_div(total.max(1))
            .unwrap_or(500);
        let successes = u64::from(requested_replicates)
            .saturating_mul(success_rate_milli)
            .saturating_div(1_000) as u32;
        let failures = requested_replicates.saturating_sub(successes);
        let batch_id = format!("dry-run-sequential:{}:{}:{}", arm.arm_id, round, attempt);
        let content_hash = ContentHash::of_value(&serde_json::json!({
            "batch_id": batch_id,
            "arm_id": arm.arm_id,
            "requested_replicates": requested_replicates,
            "successes": successes,
            "failures": failures,
            "simulation_only": true,
        }))
        .map_err(|error| SequentialCampaignExecutionFailure {
            reason: format!("dry-run sequential digest failed: {error}"),
            retryable: false,
        })?;
        Ok(SequentialBatchObservation {
            batch_id,
            arm_id: arm.arm_id.clone(),
            requested_replicates,
            successes,
            failures,
            artifact: LocalArtifactRef {
                artifact_id: format!("dry-run-sequential:{}", arm.arm_id),
                content_hash,
                content_type: "application/vnd.aurora.glioma.sequential-campaign+json".into(),
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
pub struct SequentialCampaignRound {
    pub round: u16,
    pub plan: SequentialDesignPlan,
    pub executed_arm_order: Vec<String>,
    pub requested_replicates: Vec<(String, u32)>,
    pub accepted_batch_order: Vec<String>,
    pub retry_count: u32,
    pub cost_units: u64,
    pub budget_before_units: u64,
    pub budget_after_units: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SequentialCampaignStopReason {
    SuccessStop,
    FutilityStop,
    RiskBlocked,
    BudgetExhausted,
    NoSelection,
    MaxRounds,
    ExecutorFailed,
    NoProgress,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SequentialCampaignDisposition {
    Success,
    Futility,
    Continue,
    BudgetBlocked,
    Failed,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SequentialCampaign {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: crate::glioma_engine::GliomaModelSystem,
    pub budget_units: u64,
    pub rounds: Vec<SequentialCampaignRound>,
    pub final_arms: Vec<SequentialArmObservation>,
    pub batches: Vec<SequentialBatchObservation>,
    pub completed_arm_order: Vec<String>,
    pub failed_arm_order: Vec<String>,
    pub accepted_batch_order: Vec<String>,
    pub retry_count: u32,
    pub budget_spent_units: u64,
    pub remaining_budget_units: u64,
    pub final_plan: SequentialDesignPlan,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub simulation_only: bool,
    pub disposition: SequentialCampaignDisposition,
    pub stop_reason: SequentialCampaignStopReason,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SequentialCampaignError {
    #[error("sequential campaign request is invalid: {0}")]
    InvalidRequest(String),
    #[error("sequential campaign planning failed: {0}")]
    Planning(String),
    #[error("sequential campaign execution failed: {0}")]
    Execution(String),
    #[error("sequential campaign output is invalid: {0}")]
    InvalidOutput(String),
    #[error("sequential campaign digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(campaign: &SequentialCampaign) -> serde_json::Value {
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
        "final_plan": campaign.final_plan,
        "negative_evidence": campaign.negative_evidence,
        "uncertainty": campaign.uncertainty,
        "simulation_only": campaign.simulation_only,
        "disposition": campaign.disposition,
        "stop_reason": campaign.stop_reason,
    })
}

fn validate_batch(
    batch: &SequentialBatchObservation,
    arm_id: &str,
    requested_replicates: u32,
) -> Result<(), SequentialCampaignError> {
    batch
        .artifact
        .validate()
        .map_err(|error| SequentialCampaignError::Execution(error.to_string()))?;
    if batch.batch_id.trim().is_empty()
        || batch.arm_id != arm_id
        || batch.requested_replicates != requested_replicates
        || requested_replicates == 0
        || batch.successes.saturating_add(batch.failures) != requested_replicates
        || !batch.artifact.local_only
        || batch.artifact.contains_human_data
        || batch.artifact.contains_direct_identifiers
    {
        return Err(SequentialCampaignError::Execution(
            "executor returned an invalid arm-bound aggregate batch".into(),
        ));
    }
    Ok(())
}

fn update_arm(
    arms: &mut [SequentialArmObservation],
    batch: &SequentialBatchObservation,
) -> Result<(), SequentialCampaignError> {
    let arm = arms
        .iter_mut()
        .find(|arm| arm.arm_id == batch.arm_id)
        .ok_or_else(|| SequentialCampaignError::Execution("returned arm is unknown".into()))?;
    arm.successes = arm.successes.saturating_add(batch.successes);
    arm.failures = arm.failures.saturating_add(batch.failures);
    Ok(())
}

impl SequentialCampaign {
    pub fn validate(&self) -> Result<(), SequentialCampaignError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self
                .budget_spent_units
                .saturating_add(self.remaining_budget_units)
                != self.budget_units
            || !canonical(&self.completed_arm_order)
            || !canonical(&self.failed_arm_order)
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
                    || round.executed_arm_order.len() > round.plan.selected_order.len()
                    || round.executed_arm_order
                        != round.plan.selected_order[..round.executed_arm_order.len()]
                    || round.requested_replicates.len() != round.executed_arm_order.len()
                    || round.accepted_batch_order.len() > round.executed_arm_order.len()
                    || round.cost_units
                        != round
                            .budget_before_units
                            .saturating_sub(round.budget_after_units)
            })
        {
            return Err(SequentialCampaignError::InvalidOutput(
                "identity, budget, canonical partition, or round invariants are invalid".into(),
            ));
        }
        self.final_plan
            .validate()
            .map_err(|error| SequentialCampaignError::InvalidOutput(error.to_string()))?;
        let mut batch_ids = BTreeSet::new();
        for batch in &self.batches {
            if !batch_ids.insert(batch.batch_id.clone())
                || batch.batch_id.trim().is_empty()
                || batch.requested_replicates == 0
                || batch.successes.saturating_add(batch.failures) != batch.requested_replicates
            {
                return Err(SequentialCampaignError::InvalidOutput(
                    "batch identity or replicate accounting is invalid".into(),
                ));
            }
            batch
                .artifact
                .validate()
                .map_err(|error| SequentialCampaignError::InvalidOutput(error.to_string()))?;
        }
        if self
            .batches
            .iter()
            .map(|batch| batch.batch_id.clone())
            .collect::<Vec<_>>()
            != self.accepted_batch_order
        {
            return Err(SequentialCampaignError::InvalidOutput(
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
                return Err(SequentialCampaignError::InvalidOutput(
                    "final arm identity or artifact is invalid".into(),
                ));
            }
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| SequentialCampaignError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(SequentialCampaignError::InvalidOutput(
                "campaign digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Execute selected aggregate batches, update arm counts, and replan until a declared stopping
/// condition or bounded campaign limit is reached.
pub fn execute_glioma_sequential_campaign<E: SequentialCampaignExecutor>(
    request: &SequentialCampaignRequest,
    executor: &mut E,
) -> Result<SequentialCampaign, SequentialCampaignError> {
    if request.max_retries > MAX_RETRIES || request.arms.is_empty() {
        return Err(SequentialCampaignError::InvalidRequest(
            "positive bounded arm set and retry limit are required".into(),
        ));
    }
    plan_glioma_sequential_design(&request.design, &request.arms)
        .map_err(|error| SequentialCampaignError::InvalidRequest(error.to_string()))?;
    let mut arms = request.arms.clone();
    let mut rounds = Vec::new();
    let mut batches = Vec::new();
    let mut completed = BTreeSet::new();
    let mut failed = BTreeSet::new();
    let mut accepted_batch_ids = Vec::new();
    let mut negative_evidence = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut retry_count = 0_u32;
    let mut budget_spent = 0_u64;
    let mut stop_reason = SequentialCampaignStopReason::MaxRounds;
    let mut latest_plan = plan_glioma_sequential_design(&request.design, &arms)
        .map_err(|error| SequentialCampaignError::Planning(error.to_string()))?;

    for round_number in 1..=request.design.max_rounds {
        let remaining = request.design.budget_units.saturating_sub(budget_spent);
        if remaining == 0 {
            stop_reason = SequentialCampaignStopReason::BudgetExhausted;
            break;
        }
        let mut plan_request = request.design.clone();
        plan_request.budget_units = remaining;
        let plan = plan_glioma_sequential_design(&plan_request, &arms)
            .map_err(|error| SequentialCampaignError::Planning(error.to_string()))?;
        negative_evidence.extend(plan.negative_evidence.iter().cloned());
        uncertainty.extend(plan.uncertainty.iter().cloned());
        latest_plan = plan.clone();
        if plan.selected_order.is_empty() {
            stop_reason = match plan.disposition {
                SequentialDesignDisposition::Success => SequentialCampaignStopReason::SuccessStop,
                SequentialDesignDisposition::Futility => SequentialCampaignStopReason::FutilityStop,
                SequentialDesignDisposition::RiskBlocked => {
                    SequentialCampaignStopReason::RiskBlocked
                }
                SequentialDesignDisposition::BudgetBlocked => {
                    SequentialCampaignStopReason::BudgetExhausted
                }
                _ => SequentialCampaignStopReason::NoSelection,
            };
            break;
        }

        let budget_before = remaining;
        let mut executed_arm_order = Vec::new();
        let mut requested_replicates = Vec::new();
        let mut accepted_for_round = Vec::new();
        let mut round_retries = 0_u32;
        let mut round_cost = 0_u64;
        let mut execution_failed = false;
        for arm_id in &plan.selected_order {
            let decision = plan
                .decisions
                .iter()
                .find(|decision| decision.arm_id == *arm_id)
                .ok_or_else(|| {
                    SequentialCampaignError::Planning(
                        "selected arm is absent from sequential decision output".into(),
                    )
                })?;
            if decision.planned_replicates == 0 {
                return Err(SequentialCampaignError::Planning(
                    "selected arm has no planned replicates".into(),
                ));
            }
            let arm = arms
                .iter()
                .find(|arm| arm.arm_id == *arm_id)
                .ok_or_else(|| {
                    SequentialCampaignError::Planning(
                        "selected arm is absent from observations".into(),
                    )
                })?;
            let cost =
                u64::from(decision.planned_replicates).saturating_mul(u64::from(arm.cost_units));
            if round_cost.saturating_add(cost) > budget_before {
                return Err(SequentialCampaignError::Planning(
                    "selected sequential batch exceeds the remaining budget".into(),
                ));
            }
            executed_arm_order.push(arm_id.clone());
            requested_replicates.push((arm_id.clone(), decision.planned_replicates));
            round_cost = round_cost.saturating_add(cost);
            let mut accepted = None;
            for attempt in 1..=request.max_retries.saturating_add(1) {
                match executor.execute_arm(arm, decision.planned_replicates, round_number, attempt)
                {
                    Ok(batch) => {
                        validate_batch(&batch, arm_id, decision.planned_replicates)?;
                        if accepted_batch_ids.contains(&batch.batch_id) {
                            return Err(SequentialCampaignError::Execution(
                                "executor returned a duplicate batch identity".into(),
                            ));
                        }
                        accepted = Some(batch);
                        break;
                    }
                    Err(error) => {
                        if error.reason.trim().is_empty() {
                            return Err(SequentialCampaignError::Execution(
                                "executor returned an empty failure reason".into(),
                            ));
                        }
                        if error.retryable && attempt <= request.max_retries {
                            retry_count = retry_count.saturating_add(1);
                            round_retries = round_retries.saturating_add(1);
                            continue;
                        }
                        failed.insert(arm_id.clone());
                        execution_failed = true;
                        break;
                    }
                }
            }
            let Some(batch) = accepted else {
                break;
            };
            update_arm(&mut arms, &batch)?;
            accepted_batch_ids.push(batch.batch_id.clone());
            accepted_for_round.push(batch.batch_id.clone());
            completed.insert(arm_id.clone());
            batches.push(batch);
        }
        budget_spent = budget_spent.saturating_add(round_cost);
        let budget_after = request.design.budget_units.saturating_sub(budget_spent);
        rounds.push(SequentialCampaignRound {
            round: round_number,
            plan,
            executed_arm_order,
            requested_replicates,
            accepted_batch_order: accepted_for_round,
            retry_count: round_retries,
            cost_units: round_cost,
            budget_before_units: budget_before,
            budget_after_units: budget_after,
        });
        if execution_failed {
            stop_reason = SequentialCampaignStopReason::ExecutorFailed;
            break;
        }
        if completed.is_empty() {
            stop_reason = SequentialCampaignStopReason::NoProgress;
            break;
        }
        if budget_after == 0 {
            stop_reason = SequentialCampaignStopReason::BudgetExhausted;
            break;
        }
    }

    let remaining = request.design.budget_units.saturating_sub(budget_spent);
    if remaining > 0 {
        let mut final_request = request.design.clone();
        final_request.budget_units = remaining;
        latest_plan = plan_glioma_sequential_design(&final_request, &arms)
            .map_err(|error| SequentialCampaignError::Planning(error.to_string()))?;
        negative_evidence.extend(latest_plan.negative_evidence.iter().cloned());
        uncertainty.extend(latest_plan.uncertainty.iter().cloned());
    }
    if matches!(stop_reason, SequentialCampaignStopReason::MaxRounds) {
        stop_reason = match latest_plan.disposition {
            SequentialDesignDisposition::Success => SequentialCampaignStopReason::SuccessStop,
            SequentialDesignDisposition::Futility => SequentialCampaignStopReason::FutilityStop,
            SequentialDesignDisposition::RiskBlocked => SequentialCampaignStopReason::RiskBlocked,
            SequentialDesignDisposition::BudgetBlocked => {
                SequentialCampaignStopReason::BudgetExhausted
            }
            _ => SequentialCampaignStopReason::MaxRounds,
        };
    }
    let disposition = match stop_reason {
        SequentialCampaignStopReason::SuccessStop => SequentialCampaignDisposition::Success,
        SequentialCampaignStopReason::FutilityStop => SequentialCampaignDisposition::Futility,
        SequentialCampaignStopReason::RiskBlocked
        | SequentialCampaignStopReason::BudgetExhausted => {
            SequentialCampaignDisposition::BudgetBlocked
        }
        SequentialCampaignStopReason::ExecutorFailed => SequentialCampaignDisposition::Failed,
        _ if latest_plan.disposition == SequentialDesignDisposition::Continue => {
            SequentialCampaignDisposition::Continue
        }
        _ => SequentialCampaignDisposition::Unresolved,
    };
    let mut output = SequentialCampaign {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.design.objective.clone(),
        model_system: request.design.model_system,
        budget_units: request.design.budget_units,
        rounds,
        final_arms: arms,
        batches,
        completed_arm_order: completed.into_iter().collect(),
        failed_arm_order: failed.into_iter().collect(),
        accepted_batch_order: accepted_batch_ids,
        retry_count,
        budget_spent_units: budget_spent,
        remaining_budget_units: remaining,
        final_plan: latest_plan,
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        simulation_only: executor.simulation_only(),
        disposition,
        stop_reason,
        digest: ContentHash::of_bytes(b"unsealed-glioma-sequential-campaign"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| SequentialCampaignError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma_engine::GliomaModelSystem;

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

    fn arm(id: &str, successes: u32, failures: u32) -> SequentialArmObservation {
        SequentialArmObservation {
            arm_id: id.into(),
            label: id.into(),
            artifact: artifact(id),
            model_system: GliomaModelSystem::Organoid,
            successes,
            failures,
            prior_alpha: 1,
            prior_beta: 1,
            risk_milli: 200,
            cost_units: 1,
        }
    }

    fn request() -> SequentialCampaignRequest {
        SequentialCampaignRequest {
            design: SequentialDesignRequest {
                objective: "close the replicate loop for organoid invasion".into(),
                model_system: GliomaModelSystem::Organoid,
                endpoint: "invasion_fraction".into(),
                control_arm_id: "control".into(),
                target_effect_milli: 150,
                success_probability_milli: 750,
                futility_probability_milli: 700,
                min_replicates_per_arm: 3,
                max_new_replicates_per_round: 2,
                max_rounds: 4,
                max_selected_arms: 2,
                budget_units: 12,
                risk_ceiling_milli: 800,
                exploration_weight_milli: 400,
            },
            arms: vec![arm("control", 1, 1), arm("candidate", 1, 0)],
            max_retries: 1,
        }
    }

    #[test]
    fn dry_run_campaign_replans_and_is_replay_stable() {
        let request = request();
        let mut first_executor = DryRunSequentialCampaignExecutor;
        let mut second_executor = DryRunSequentialCampaignExecutor;
        let first = execute_glioma_sequential_campaign(&request, &mut first_executor).unwrap();
        let second = execute_glioma_sequential_campaign(&request, &mut second_executor).unwrap();
        assert_eq!(first, second);
        assert!(!first.rounds.is_empty());
        assert!(!first.batches.is_empty());
        assert!(first
            .final_arms
            .iter()
            .any(|arm| arm.successes + arm.failures > 2));
        first.validate().unwrap();
    }

    #[test]
    fn prequalified_success_stops_without_dispatching_a_new_batch() {
        let mut request = request();
        request.arms = vec![arm("control", 3, 7), arm("candidate", 10, 0)];
        let mut executor = DryRunSequentialCampaignExecutor;
        let output = execute_glioma_sequential_campaign(&request, &mut executor).unwrap();
        assert_eq!(
            output.stop_reason,
            SequentialCampaignStopReason::SuccessStop
        );
        assert_eq!(output.disposition, SequentialCampaignDisposition::Success);
        assert!(output.rounds.is_empty());
        assert!(output.batches.is_empty());
    }

    #[derive(Default)]
    struct FailAfterFirst {
        calls: u8,
        delegate: DryRunSequentialCampaignExecutor,
    }

    impl SequentialCampaignExecutor for FailAfterFirst {
        fn execute_arm(
            &mut self,
            arm: &SequentialArmObservation,
            requested_replicates: u32,
            round: u16,
            attempt: u8,
        ) -> Result<SequentialBatchObservation, SequentialCampaignExecutionFailure> {
            self.calls = self.calls.saturating_add(1);
            if self.calls > 1 {
                return Err(SequentialCampaignExecutionFailure {
                    reason: "gateway stopped after one accepted arm".into(),
                    retryable: false,
                });
            }
            self.delegate
                .execute_arm(arm, requested_replicates, round, attempt)
        }
    }

    #[test]
    fn partial_executor_failure_is_reconciled_without_fabricating_the_second_arm() {
        let mut executor = FailAfterFirst::default();
        let output = execute_glioma_sequential_campaign(&request(), &mut executor).unwrap();
        assert_eq!(
            output.stop_reason,
            SequentialCampaignStopReason::ExecutorFailed
        );
        assert_eq!(output.disposition, SequentialCampaignDisposition::Failed);
        assert_eq!(output.batches.len(), 1);
        assert_eq!(output.rounds[0].accepted_batch_order.len(), 1);
        output.validate().unwrap();
    }
}

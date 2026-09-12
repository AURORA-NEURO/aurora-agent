//! Closed-loop autonomous federated benchmark campaigns for preclinical glioma research.
//!
//! The consensus analyzer answers whether the currently available site aggregates support a
//! claim.  This module turns that analysis into a bounded research workflow: it ranks typed
//! follow-up actions, asks institution-local executors for one new aggregate at a time, and
//! replans after every batch.  Raw traces never cross the federation boundary.  A dry-run worker
//! is intentionally synthetic and is useful only for replay and integration tests.

use super::consensus::{
    analyze_federated_benchmark, FederatedBenchmarkConsensus, FederatedBenchmarkDisposition,
    FederatedBenchmarkRequest, FederatedBenchmarkSite,
};
use crate::glioma_engine::LocalArtifactRef;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P12-F10";
pub const OUTPUT_SCHEMA: &str = "GliomaFederatedBenchmarkCampaign1@1";
pub const MAX_ROUNDS: u16 = 32;
pub const MAX_ACTIONS: usize = 256;
pub const MAX_RETRIES: u8 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedBenchmarkActionKind {
    ReplicateSite,
    RecalibrateSite,
    ResolveHeterogeneity,
    ExpandCoverage,
    AuditInfluentialSite,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedBenchmarkAction {
    pub action_id: String,
    pub kind: FederatedBenchmarkActionKind,
    pub target_site_id: Option<String>,
    pub cost_units: u32,
    pub expected_information_milli: u64,
    pub expected_effect_milli: u64,
    pub feasibility_milli: u16,
    pub risk_milli: u16,
    pub requested_replicates: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedBenchmarkCampaignRequest {
    pub benchmark: FederatedBenchmarkRequest,
    pub initial_sites: Vec<FederatedBenchmarkSite>,
    pub actions: Vec<FederatedBenchmarkAction>,
    pub budget_units: u64,
    pub max_rounds: u16,
    pub max_retries: u8,
    pub stop_on_qualified: bool,
    pub stop_on_negative: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FederatedBenchmarkExecutionFailure {
    pub reason: String,
    pub retryable: bool,
}

/// Institution-local workers implement this seam.  The only value returned to the campaign is a
/// typed, aggregate-only site result; the worker owns raw traces, instruments, and credentials.
pub trait FederatedBenchmarkCampaignExecutor {
    fn execute_action(
        &mut self,
        action: &FederatedBenchmarkAction,
        request: &FederatedBenchmarkRequest,
        attempt: u8,
    ) -> Result<FederatedBenchmarkSite, FederatedBenchmarkExecutionFailure>;
}

/// Deterministic sandbox worker.  Its scores are synthetic and must never be interpreted as
/// biological evidence.
#[derive(Debug, Default)]
pub struct DryRunFederatedBenchmarkCampaignExecutor;

impl FederatedBenchmarkCampaignExecutor for DryRunFederatedBenchmarkCampaignExecutor {
    fn execute_action(
        &mut self,
        action: &FederatedBenchmarkAction,
        request: &FederatedBenchmarkRequest,
        attempt: u8,
    ) -> Result<FederatedBenchmarkSite, FederatedBenchmarkExecutionFailure> {
        let target = action.target_site_id.as_deref().unwrap_or("coverage");
        let kind_bonus = match action.kind {
            FederatedBenchmarkActionKind::ReplicateSite => 28_i64,
            FederatedBenchmarkActionKind::RecalibrateSite => 18,
            FederatedBenchmarkActionKind::ResolveHeterogeneity => 8,
            FederatedBenchmarkActionKind::ExpandCoverage => 35,
            FederatedBenchmarkActionKind::AuditInfluentialSite => 12,
        };
        let score = (500_i64
            + kind_bonus
            + action.expected_effect_milli.min(500) as i64
            + i64::from(attempt) * 3)
            .clamp(0, 1_000) as u64;
        let site_id = format!("dry-run:{}:{}", action.action_id, target);
        let study_id = format!("dry-run-study:{}", action.action_id);
        let content_hash = ContentHash::of_value(&serde_json::json!({
            "site_id": site_id,
            "study_id": study_id,
            "action_id": action.action_id,
            "attempt": attempt,
            "score": score,
            "simulation_only": true,
        }))
        .map_err(|error| FederatedBenchmarkExecutionFailure {
            reason: format!("dry-run aggregate digest failed: {error}"),
            retryable: false,
        })?;
        Ok(FederatedBenchmarkSite {
            site_id,
            study_id,
            capability_id: request.capability_id.clone(),
            benchmark_world: request.benchmark_world.clone(),
            metric_name: request.metric_name.clone(),
            model_system: request.model_system,
            artifact: LocalArtifactRef {
                artifact_id: format!("dry-run-federated:{}", action.action_id),
                content_hash,
                content_type: "application/vnd.aurora.glioma.federated-benchmark+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
            baseline_score_milli: 500,
            candidate_score_milli: score,
            uncertainty_milli: 80,
            replicate_count: action.requested_replicates.max(1),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedBenchmarkCampaignRound {
    pub round: u16,
    pub action_order: Vec<String>,
    pub returned_site_order: Vec<String>,
    pub failed_action_order: Vec<String>,
    pub consensus_disposition: FederatedBenchmarkDisposition,
    pub cost_units: u32,
    pub budget_before_units: u64,
    pub budget_after_units: u64,
    pub retry_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedBenchmarkCampaignDisposition {
    Qualified,
    Negative,
    Heterogeneous,
    Partial,
    BudgetBlocked,
    Failed,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedBenchmarkCampaignStopReason {
    Qualified,
    Negative,
    BudgetExhausted,
    NoEligibleActions,
    MaxRounds,
    ExecutorFailed,
    NoProgress,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedBenchmarkCampaign {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub rounds: Vec<FederatedBenchmarkCampaignRound>,
    pub sites: Vec<FederatedBenchmarkSite>,
    pub completed_action_order: Vec<String>,
    pub failed_action_order: Vec<String>,
    pub retry_count: u32,
    pub budget_spent_units: u64,
    pub remaining_budget_units: u64,
    pub final_consensus: FederatedBenchmarkConsensus,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: FederatedBenchmarkCampaignDisposition,
    pub stop_reason: FederatedBenchmarkCampaignStopReason,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedBenchmarkCampaignError {
    #[error("federated benchmark campaign request is invalid: {0}")]
    InvalidRequest(String),
    #[error("federated benchmark campaign planning failed: {0}")]
    Planning(String),
    #[error("federated benchmark campaign execution failed: {0}")]
    Execution(String),
    #[error("federated benchmark campaign output is invalid: {0}")]
    InvalidOutput(String),
    #[error("federated benchmark campaign digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(campaign: &FederatedBenchmarkCampaign) -> serde_json::Value {
    serde_json::json!({
        "feature_id": campaign.feature_id,
        "output_schema": campaign.output_schema,
        "objective": campaign.objective,
        "rounds": campaign.rounds,
        "sites": campaign.sites,
        "completed_action_order": campaign.completed_action_order,
        "failed_action_order": campaign.failed_action_order,
        "retry_count": campaign.retry_count,
        "budget_spent_units": campaign.budget_spent_units,
        "remaining_budget_units": campaign.remaining_budget_units,
        "final_consensus": campaign.final_consensus,
        "negative_evidence": campaign.negative_evidence,
        "uncertainty": campaign.uncertainty,
        "disposition": campaign.disposition,
        "stop_reason": campaign.stop_reason,
    })
}

fn validate_request(
    request: &FederatedBenchmarkCampaignRequest,
) -> Result<(), FederatedBenchmarkCampaignError> {
    if request.max_rounds == 0
        || request.max_rounds > MAX_ROUNDS
        || request.max_retries > MAX_RETRIES
        || request.budget_units == 0
        || request.actions.is_empty()
        || request.actions.len() > MAX_ACTIONS
        || request.initial_sites.is_empty()
    {
        return Err(FederatedBenchmarkCampaignError::InvalidRequest(
            "bounded rounds/retries/budget, initial aggregate sites, and actions are required"
                .into(),
        ));
    }
    let mut action_ids = BTreeSet::new();
    for action in &request.actions {
        if action.action_id.trim().is_empty()
            || !action_ids.insert(action.action_id.clone())
            || action.cost_units == 0
            || action.feasibility_milli > 1_000
            || action.risk_milli > 1_000
            || action.requested_replicates == 0
        {
            return Err(FederatedBenchmarkCampaignError::InvalidRequest(
                "action identity, uniqueness, cost, feasibility, risk, and replicate bounds are invalid".into(),
            ));
        }
    }
    analyze_federated_benchmark(&request.benchmark, &request.initial_sites)
        .map_err(|error| FederatedBenchmarkCampaignError::InvalidRequest(error.to_string()))?;
    Ok(())
}

fn action_score(
    action: &FederatedBenchmarkAction,
    consensus: &FederatedBenchmarkConsensus,
) -> u128 {
    let pressure = match consensus.disposition {
        FederatedBenchmarkDisposition::Heterogeneous => 1_500_u128,
        FederatedBenchmarkDisposition::Unresolved => 1_250,
        FederatedBenchmarkDisposition::Negative => 1_000,
        FederatedBenchmarkDisposition::Qualified => 250,
    };
    let value = u128::from(action.expected_information_milli)
        .saturating_add(u128::from(action.expected_effect_milli))
        .saturating_mul(pressure)
        .saturating_mul(u128::from(action.feasibility_milli.max(1)));
    let penalty = u128::from(action.cost_units)
        .saturating_mul(u128::from(1_000_u16.saturating_add(action.risk_milli)));
    value.saturating_sub(penalty)
}

impl FederatedBenchmarkCampaign {
    pub fn validate(&self) -> Result<(), FederatedBenchmarkCampaignError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.rounds.len() > MAX_ROUNDS as usize
            || !canonical(&self.completed_action_order)
            || !canonical(&self.failed_action_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self
                .completed_action_order
                .iter()
                .any(|id| self.failed_action_order.binary_search(id).is_ok())
        {
            return Err(FederatedBenchmarkCampaignError::InvalidOutput(
                "identity, canonical partitions, or evidence fields are invalid".into(),
            ));
        }
        let mut seen_rounds = BTreeSet::new();
        let mut seen_sites = BTreeSet::new();
        let mut spent = 0_u64;
        let mut retries = 0_u32;
        for round in &self.rounds {
            if round.round == 0
                || !seen_rounds.insert(round.round)
                || !canonical(&round.action_order)
                || !canonical(&round.returned_site_order)
                || !canonical(&round.failed_action_order)
                || round.budget_after_units > round.budget_before_units
                || u64::from(round.cost_units)
                    != round
                        .budget_before_units
                        .saturating_sub(round.budget_after_units)
            {
                return Err(FederatedBenchmarkCampaignError::InvalidOutput(
                    "round ordering or budget invariants are invalid".into(),
                ));
            }
            for site_id in &round.returned_site_order {
                if !seen_sites.insert(site_id.clone()) {
                    return Err(FederatedBenchmarkCampaignError::InvalidOutput(
                        "a returned site was emitted in more than one round".into(),
                    ));
                }
            }
            spent = spent.saturating_add(u64::from(round.cost_units));
            retries = retries.saturating_add(round.retry_count);
        }
        if spent != self.budget_spent_units || retries != self.retry_count {
            return Err(FederatedBenchmarkCampaignError::InvalidOutput(
                "budget or retry count does not reconcile with rounds".into(),
            ));
        }
        self.final_consensus
            .validate()
            .map_err(|error| FederatedBenchmarkCampaignError::InvalidOutput(error.to_string()))?;
        let mut site_ids = BTreeSet::new();
        let mut study_ids = BTreeSet::new();
        for site in &self.sites {
            site.artifact.validate().map_err(|error| {
                FederatedBenchmarkCampaignError::InvalidOutput(error.to_string())
            })?;
            if !site_ids.insert(site.site_id.clone()) || !study_ids.insert(site.study_id.clone()) {
                return Err(FederatedBenchmarkCampaignError::InvalidOutput(
                    "campaign sites must have unique site and study identities".into(),
                ));
            }
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| FederatedBenchmarkCampaignError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(FederatedBenchmarkCampaignError::InvalidOutput(
                "campaign digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_returned_site(
    site: &FederatedBenchmarkSite,
    request: &FederatedBenchmarkRequest,
    existing: &[FederatedBenchmarkSite],
) -> Result<(), FederatedBenchmarkCampaignError> {
    site.artifact
        .validate()
        .map_err(|error| FederatedBenchmarkCampaignError::Execution(error.to_string()))?;
    if site.site_id.trim().is_empty()
        || site.study_id.trim().is_empty()
        || site.capability_id != request.capability_id
        || site.benchmark_world != request.benchmark_world
        || site.metric_name != request.metric_name
        || site.model_system != request.model_system
        || site.replicate_count == 0
        || site.uncertainty_milli == 0
        || existing
            .iter()
            .any(|old| old.site_id == site.site_id || old.study_id == site.study_id)
    {
        return Err(FederatedBenchmarkCampaignError::Execution(
            "executor returned a duplicate, unbound, or incomplete aggregate site".into(),
        ));
    }
    Ok(())
}

/// Execute a bounded autonomous campaign.  Every successful action adds exactly one returned
/// aggregate site, then the same deterministic consensus and action ranking are rerun.
pub fn execute_federated_benchmark_campaign<E: FederatedBenchmarkCampaignExecutor>(
    request: &FederatedBenchmarkCampaignRequest,
    executor: &mut E,
) -> Result<FederatedBenchmarkCampaign, FederatedBenchmarkCampaignError> {
    validate_request(request)?;
    let mut sites = request.initial_sites.clone();
    let mut completed = BTreeSet::new();
    let mut failed = BTreeSet::new();
    let mut rounds = Vec::new();
    let mut negative_evidence = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut budget_spent = 0_u64;
    let mut retry_count = 0_u32;
    let mut stop_reason = FederatedBenchmarkCampaignStopReason::MaxRounds;

    for round_number in 1..=request.max_rounds {
        let consensus = analyze_federated_benchmark(&request.benchmark, &sites)
            .map_err(|error| FederatedBenchmarkCampaignError::Planning(error.to_string()))?;
        negative_evidence.extend(consensus.negative_evidence.iter().cloned());
        uncertainty.extend(consensus.uncertainty.iter().cloned());
        if request.stop_on_qualified
            && consensus.disposition == FederatedBenchmarkDisposition::Qualified
        {
            stop_reason = FederatedBenchmarkCampaignStopReason::Qualified;
            break;
        }
        if request.stop_on_negative
            && consensus.disposition == FederatedBenchmarkDisposition::Negative
        {
            stop_reason = FederatedBenchmarkCampaignStopReason::Negative;
            break;
        }
        let remaining = request.budget_units.saturating_sub(budget_spent);
        if remaining == 0 {
            stop_reason = FederatedBenchmarkCampaignStopReason::BudgetExhausted;
            break;
        }
        let mut eligible = request
            .actions
            .iter()
            .filter(|action| {
                !completed.contains(&action.action_id)
                    && !failed.contains(&action.action_id)
                    && u64::from(action.cost_units) <= remaining
                    && action.feasibility_milli > 0
            })
            .collect::<Vec<_>>();
        eligible.sort_by(|left, right| {
            action_score(right, &consensus)
                .cmp(&action_score(left, &consensus))
                .then_with(|| left.action_id.cmp(&right.action_id))
        });
        if eligible.is_empty() {
            let budget_blocked = request.actions.iter().any(|action| {
                !completed.contains(&action.action_id)
                    && !failed.contains(&action.action_id)
                    && action.feasibility_milli > 0
                    && u64::from(action.cost_units) > remaining
            });
            stop_reason = if budget_blocked {
                FederatedBenchmarkCampaignStopReason::BudgetExhausted
            } else {
                FederatedBenchmarkCampaignStopReason::NoEligibleActions
            };
            break;
        }
        let mut selected = Vec::new();
        let mut selected_cost = 0_u64;
        for action in eligible {
            let cost = u64::from(action.cost_units);
            if selected.is_empty() || selected_cost.saturating_add(cost) <= remaining {
                selected_cost = selected_cost.saturating_add(cost);
                selected.push(action);
            }
            if selected.len() >= 8 {
                break;
            }
        }
        if selected.is_empty() {
            stop_reason = FederatedBenchmarkCampaignStopReason::BudgetExhausted;
            break;
        }
        let before_budget = remaining;
        let mut returned_site_order = Vec::new();
        let mut failed_action_order = Vec::new();
        let mut round_retry_count = 0_u32;
        let mut progress = false;
        for action in selected.iter().copied() {
            let mut returned = None;
            for attempt in 1..=request.max_retries.saturating_add(1) {
                match executor.execute_action(action, &request.benchmark, attempt) {
                    Ok(site) => {
                        validate_returned_site(&site, &request.benchmark, &sites)?;
                        returned = Some(site);
                        break;
                    }
                    Err(error) => {
                        if error.reason.trim().is_empty() {
                            return Err(FederatedBenchmarkCampaignError::Execution(
                                "executor returned an empty failure reason".into(),
                            ));
                        }
                        if error.retryable && attempt <= request.max_retries {
                            retry_count = retry_count.saturating_add(1);
                            round_retry_count = round_retry_count.saturating_add(1);
                            continue;
                        }
                        failed_action_order.push(action.action_id.clone());
                        failed.insert(action.action_id.clone());
                        break;
                    }
                }
            }
            if let Some(site) = returned {
                returned_site_order.push(site.site_id.clone());
                sites.push(site);
                completed.insert(action.action_id.clone());
                progress = true;
            } else {
                break;
            }
        }
        returned_site_order.sort();
        failed_action_order.sort();
        budget_spent = budget_spent.saturating_add(selected_cost);
        let after_budget = request.budget_units.saturating_sub(budget_spent);
        let updated = analyze_federated_benchmark(&request.benchmark, &sites)
            .map_err(|error| FederatedBenchmarkCampaignError::Planning(error.to_string()))?;
        rounds.push(FederatedBenchmarkCampaignRound {
            round: round_number,
            action_order: selected
                .iter()
                .map(|action| action.action_id.clone())
                .collect(),
            returned_site_order,
            failed_action_order: failed_action_order.clone(),
            consensus_disposition: updated.disposition,
            cost_units: selected_cost.min(u64::from(u32::MAX)) as u32,
            budget_before_units: before_budget,
            budget_after_units: after_budget,
            retry_count: round_retry_count,
        });
        if !failed_action_order.is_empty() {
            stop_reason = FederatedBenchmarkCampaignStopReason::ExecutorFailed;
            break;
        }
        if !progress {
            stop_reason = FederatedBenchmarkCampaignStopReason::NoProgress;
            break;
        }
        if request.stop_on_qualified
            && updated.disposition == FederatedBenchmarkDisposition::Qualified
        {
            stop_reason = FederatedBenchmarkCampaignStopReason::Qualified;
            break;
        }
        if request.stop_on_negative
            && updated.disposition == FederatedBenchmarkDisposition::Negative
        {
            stop_reason = FederatedBenchmarkCampaignStopReason::Negative;
            break;
        }
        if after_budget == 0 {
            stop_reason = FederatedBenchmarkCampaignStopReason::BudgetExhausted;
            break;
        }
    }

    let final_consensus = analyze_federated_benchmark(&request.benchmark, &sites)
        .map_err(|error| FederatedBenchmarkCampaignError::Planning(error.to_string()))?;
    negative_evidence.extend(final_consensus.negative_evidence.iter().cloned());
    uncertainty.extend(final_consensus.uncertainty.iter().cloned());
    let disposition = match stop_reason {
        FederatedBenchmarkCampaignStopReason::Qualified => {
            FederatedBenchmarkCampaignDisposition::Qualified
        }
        FederatedBenchmarkCampaignStopReason::Negative => {
            FederatedBenchmarkCampaignDisposition::Negative
        }
        FederatedBenchmarkCampaignStopReason::BudgetExhausted => {
            FederatedBenchmarkCampaignDisposition::BudgetBlocked
        }
        FederatedBenchmarkCampaignStopReason::ExecutorFailed => {
            FederatedBenchmarkCampaignDisposition::Failed
        }
        _ => match final_consensus.disposition {
            FederatedBenchmarkDisposition::Heterogeneous => {
                FederatedBenchmarkCampaignDisposition::Heterogeneous
            }
            FederatedBenchmarkDisposition::Negative => {
                FederatedBenchmarkCampaignDisposition::Negative
            }
            FederatedBenchmarkDisposition::Qualified => {
                FederatedBenchmarkCampaignDisposition::Partial
            }
            FederatedBenchmarkDisposition::Unresolved => {
                FederatedBenchmarkCampaignDisposition::Unresolved
            }
        },
    };
    let mut output = FederatedBenchmarkCampaign {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.benchmark.objective.clone(),
        rounds,
        sites,
        completed_action_order: completed.into_iter().collect(),
        failed_action_order: failed.into_iter().collect(),
        retry_count,
        budget_spent_units: budget_spent,
        remaining_budget_units: request.budget_units.saturating_sub(budget_spent),
        final_consensus,
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        stop_reason,
        digest: ContentHash::of_bytes(b"unsealed-glioma-federated-benchmark-campaign"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| FederatedBenchmarkCampaignError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p12_federated_benchmarking::consensus::FederatedBenchmarkSiteDisposition;
    use crate::glioma_engine::GliomaModelSystem;

    fn hash(id: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"id": id})).unwrap()
    }
    fn site(id: &str, score: u64, replicates: u16) -> FederatedBenchmarkSite {
        FederatedBenchmarkSite {
            site_id: format!("site-{id}"),
            study_id: format!("study-{id}"),
            capability_id: "glioma:invasion-model".into(),
            benchmark_world: "glioma-world-v1".into(),
            metric_name: "holdout_auc".into(),
            model_system: GliomaModelSystem::Organoid,
            artifact: LocalArtifactRef {
                artifact_id: format!("artifact-{id}"),
                content_hash: hash(id),
                content_type: "application/vnd.aurora.glioma-benchmark+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
            baseline_score_milli: 500,
            candidate_score_milli: score,
            uncertainty_milli: 30,
            replicate_count: replicates,
        }
    }
    fn request() -> FederatedBenchmarkCampaignRequest {
        FederatedBenchmarkCampaignRequest {
            benchmark: FederatedBenchmarkRequest {
                objective: "compare invasion model improvements".into(),
                capability_id: "glioma:invasion-model".into(),
                benchmark_world: "glioma-world-v1".into(),
                metric_name: "holdout_auc".into(),
                model_system: GliomaModelSystem::Organoid,
                minimum_sites: 3,
                minimum_replicates_per_site: 3,
                effect_threshold_milli: 50,
                max_i2_milli: 250,
                min_signal_to_noise_milli: 500,
                max_site_spread_milli: 120,
                max_leave_one_out_shift_milli: 100,
            },
            initial_sites: vec![site("a", 620, 4)],
            actions: vec![
                FederatedBenchmarkAction {
                    action_id: "expand-a".into(),
                    kind: FederatedBenchmarkActionKind::ExpandCoverage,
                    target_site_id: None,
                    cost_units: 2,
                    expected_information_milli: 900,
                    expected_effect_milli: 200,
                    feasibility_milli: 900,
                    risk_milli: 50,
                    requested_replicates: 4,
                },
                FederatedBenchmarkAction {
                    action_id: "replicate-a".into(),
                    kind: FederatedBenchmarkActionKind::ReplicateSite,
                    target_site_id: Some("site-a".into()),
                    cost_units: 2,
                    expected_information_milli: 800,
                    expected_effect_milli: 100,
                    feasibility_milli: 850,
                    risk_milli: 70,
                    requested_replicates: 4,
                },
            ],
            budget_units: 4,
            max_rounds: 3,
            max_retries: 1,
            stop_on_qualified: false,
            stop_on_negative: false,
        }
    }

    #[test]
    fn campaign_replans_from_aggregate_sites_and_is_replay_stable() {
        let request = request();
        let mut first_executor = DryRunFederatedBenchmarkCampaignExecutor;
        let mut second_executor = DryRunFederatedBenchmarkCampaignExecutor;
        let first = execute_federated_benchmark_campaign(&request, &mut first_executor).unwrap();
        let second = execute_federated_benchmark_campaign(&request, &mut second_executor).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.completed_action_order.len(), 2);
        assert_eq!(first.sites.len(), 3);
        first.validate().unwrap();
    }

    #[test]
    fn returned_site_cannot_move_raw_or_duplicate_identity() {
        struct BadExecutor;
        impl FederatedBenchmarkCampaignExecutor for BadExecutor {
            fn execute_action(
                &mut self,
                _action: &FederatedBenchmarkAction,
                request: &FederatedBenchmarkRequest,
                _attempt: u8,
            ) -> Result<FederatedBenchmarkSite, FederatedBenchmarkExecutionFailure> {
                let mut output = site("a", 600, 4);
                output.capability_id = request.capability_id.clone();
                Ok(output)
            }
        }
        let mut executor = BadExecutor;
        let error = execute_federated_benchmark_campaign(&request(), &mut executor).unwrap_err();
        assert!(error.to_string().contains("duplicate"));
    }

    #[test]
    fn campaign_preserves_unresolved_consensus_when_budget_stops() {
        let mut request = request();
        request.actions.truncate(1);
        request.budget_units = 1;
        let mut executor = DryRunFederatedBenchmarkCampaignExecutor;
        let output = execute_federated_benchmark_campaign(&request, &mut executor).unwrap();
        assert_eq!(
            output.disposition,
            FederatedBenchmarkCampaignDisposition::BudgetBlocked
        );
        assert_eq!(
            output.stop_reason,
            FederatedBenchmarkCampaignStopReason::BudgetExhausted
        );
        assert!(output
            .final_consensus
            .contributions
            .iter()
            .all(|item| item.disposition == FederatedBenchmarkSiteDisposition::Included));
    }
}

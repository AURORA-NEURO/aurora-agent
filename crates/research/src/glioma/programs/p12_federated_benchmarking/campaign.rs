//! Closed-loop autonomous federated benchmark campaigns for preclinical glioma research.
//!
//! The consensus analyzer answers whether the currently available site aggregates support a
//! claim.  This module turns that analysis into a bounded research workflow: it ranks typed
//! follow-up actions, asks institution-local executors for one new aggregate at a time, and
//! replans after every batch.  Raw traces never cross the federation boundary.  A dry-run worker
//! is intentionally synthetic and is useful only for replay and integration tests.

use super::consensus::{
    FederatedBenchmarkConsensus, FederatedBenchmarkDisposition, FederatedBenchmarkRequest,
    FederatedBenchmarkSite, analyze_federated_benchmark,
};
use crate::glioma_engine::LocalArtifactRef;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P12-F10";
pub const OUTPUT_SCHEMA: &str = "GliomaFederatedBenchmarkCampaign1@2";
pub const MAX_ROUNDS: u16 = 32;
pub const MAX_ACTIONS: usize = 256;
pub const MAX_RETRIES: u8 = 6;
const PORTFOLIO_BEAM_WIDTH: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum BenchmarkEvidenceGap {
    SiteCoverage,
    ReplicateFloor,
    Heterogeneity,
    Influence,
    Signal,
}

/// Convert consensus diagnostics into the evidence gaps an autonomous portfolio must close.
/// Action kind and target diversity alone are insufficient: two different actions can still spend
/// a round without addressing the missing-site, heterogeneity, influence, or signal gate that
/// currently blocks a federated conclusion.
fn evidence_gaps(consensus: &FederatedBenchmarkConsensus) -> BTreeSet<BenchmarkEvidenceGap> {
    let mut gaps = BTreeSet::new();
    for evidence in consensus
        .negative_evidence
        .iter()
        .chain(consensus.uncertainty.iter())
    {
        if evidence.contains("minimum-site-count") || evidence.contains("site-count") {
            gaps.insert(BenchmarkEvidenceGap::SiteCoverage);
        }
        if evidence.contains("replicate-floor") {
            gaps.insert(BenchmarkEvidenceGap::ReplicateFloor);
        }
        if evidence.contains("heterogeneity")
            || evidence.contains("direction-contradiction")
            || evidence.contains("score-spread")
        {
            gaps.insert(BenchmarkEvidenceGap::Heterogeneity);
        }
        if evidence.contains("leave-one-site") || evidence.contains("influence") {
            gaps.insert(BenchmarkEvidenceGap::Influence);
        }
        if evidence.contains("signal-threshold") || evidence.contains("underperforms") {
            gaps.insert(BenchmarkEvidenceGap::Signal);
        }
    }
    gaps
}

fn action_covers_gap(action: &FederatedBenchmarkAction, gap: BenchmarkEvidenceGap) -> bool {
    match gap {
        BenchmarkEvidenceGap::SiteCoverage => {
            matches!(action.kind, FederatedBenchmarkActionKind::ExpandCoverage)
        }
        BenchmarkEvidenceGap::ReplicateFloor => {
            matches!(action.kind, FederatedBenchmarkActionKind::ReplicateSite)
        }
        BenchmarkEvidenceGap::Heterogeneity => matches!(
            action.kind,
            FederatedBenchmarkActionKind::ResolveHeterogeneity
                | FederatedBenchmarkActionKind::RecalibrateSite
                | FederatedBenchmarkActionKind::ReplicateSite
        ),
        BenchmarkEvidenceGap::Influence => matches!(
            action.kind,
            FederatedBenchmarkActionKind::AuditInfluentialSite
                | FederatedBenchmarkActionKind::RecalibrateSite
        ),
        BenchmarkEvidenceGap::Signal => matches!(
            action.kind,
            FederatedBenchmarkActionKind::ReplicateSite
                | FederatedBenchmarkActionKind::ExpandCoverage
                | FederatedBenchmarkActionKind::ResolveHeterogeneity
        ),
    }
}

#[derive(Clone)]
struct BenchmarkPortfolioState {
    selected_indices: Vec<usize>,
    spent: u64,
    utility: u128,
}

fn portfolio_state_better(
    left: &BenchmarkPortfolioState,
    right: &BenchmarkPortfolioState,
    actions: &[&FederatedBenchmarkAction],
) -> bool {
    if left.utility != right.utility {
        return left.utility > right.utility;
    }
    if left.selected_indices.len() != right.selected_indices.len() {
        return left.selected_indices.len() > right.selected_indices.len();
    }
    if left.spent != right.spent {
        return left.spent < right.spent;
    }
    left.selected_indices
        .iter()
        .map(|index| actions[*index].action_id.as_str())
        .cmp(
            right
                .selected_indices
                .iter()
                .map(|index| actions[*index].action_id.as_str()),
        )
        == std::cmp::Ordering::Less
}

fn benchmark_portfolio_utility(
    selected_indices: &[usize],
    actions: &[&FederatedBenchmarkAction],
    consensus: &FederatedBenchmarkConsensus,
) -> u128 {
    let gaps = evidence_gaps(consensus);
    let mut covered_gaps = BTreeSet::new();
    let mut kind_counts = BTreeMap::<FederatedBenchmarkActionKind, u64>::new();
    let mut target_counts = BTreeMap::<Option<&str>, u64>::new();
    for index in selected_indices {
        let action = actions[*index];
        *kind_counts.entry(action.kind).or_default() += 1;
        *target_counts
            .entry(action.target_site_id.as_deref())
            .or_default() += 1;
    }
    selected_indices
        .iter()
        .map(|index| {
            let action = actions[*index];
            let kind_diversity = 1_000_u64 / kind_counts[&action.kind];
            let target_diversity = 1_000_u64 / target_counts[&action.target_site_id.as_deref()];
            let diversity = (kind_diversity + target_diversity) / 2;
            let newly_covered = gaps
                .iter()
                .filter(|gap| action_covers_gap(action, **gap) && !covered_gaps.contains(*gap))
                .count() as u64;
            for gap in gaps.iter().filter(|gap| action_covers_gap(action, **gap)) {
                covered_gaps.insert(*gap);
            }
            // Closing an unresolved consortium gate is intentionally worth up to 2x a
            // same-round raw score: a high-information action that leaves the required site
            // coverage unresolved is not a productive autonomous research step.
            let gap_multiplier_milli = 1_000_u64.saturating_add((newly_covered > 0) as u64 * 1_000);
            action_score(action, consensus)
                .saturating_mul(u128::from(diversity))
                .saturating_mul(u128::from(gap_multiplier_milli))
                / 1_000
        })
        .sum()
}

fn select_benchmark_portfolio<'a>(
    eligible: &[&'a FederatedBenchmarkAction],
    consensus: &FederatedBenchmarkConsensus,
    remaining: u64,
) -> Vec<&'a FederatedBenchmarkAction> {
    let mut beam = vec![BenchmarkPortfolioState {
        selected_indices: Vec::new(),
        spent: 0,
        utility: 0,
    }];
    for index in 0..eligible.len() {
        let mut expanded = beam.clone();
        for state in &beam {
            if state.selected_indices.len() >= 8 {
                continue;
            }
            let cost = u64::from(eligible[index].cost_units);
            if state.spent.saturating_add(cost) > remaining {
                continue;
            }
            let mut selected_indices = state.selected_indices.clone();
            selected_indices.push(index);
            let spent = state.spent.saturating_add(cost);
            let utility = benchmark_portfolio_utility(&selected_indices, eligible, consensus);
            expanded.push(BenchmarkPortfolioState {
                selected_indices,
                spent,
                utility,
            });
        }
        expanded.sort_by(|left, right| {
            if portfolio_state_better(left, right, eligible) {
                std::cmp::Ordering::Less
            } else if portfolio_state_better(right, left, eligible) {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        });
        let mut seen = BTreeSet::new();
        expanded.retain(|state| seen.insert(state.selected_indices.clone()));
        expanded.truncate(PORTFOLIO_BEAM_WIDTH);
        beam = expanded;
    }
    let selected = beam
        .iter()
        .max_by(|left, right| {
            if portfolio_state_better(left, right, eligible) {
                std::cmp::Ordering::Greater
            } else if portfolio_state_better(right, left, eligible) {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .map(|state| state.selected_indices.clone())
        .unwrap_or_default();
    selected.into_iter().map(|index| eligible[index]).collect()
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
        let selected = select_benchmark_portfolio(&eligible, &consensus, remaining);
        if selected.is_empty() {
            stop_reason = FederatedBenchmarkCampaignStopReason::BudgetExhausted;
            break;
        }
        let before_budget = remaining;
        let mut returned_site_order = Vec::new();
        let mut failed_action_order = Vec::new();
        let mut round_retry_count = 0_u32;
        let mut progress = false;
        let mut round_spent = 0_u64;
        let mut budget_exhausted_during_execution = false;
        for action in selected.iter().copied() {
            let action_cost = u64::from(action.cost_units);
            if round_spent.saturating_add(action_cost) > remaining {
                budget_exhausted_during_execution = true;
                break;
            }
            let mut returned = None;
            for attempt in 1..=request.max_retries.saturating_add(1) {
                if round_spent.saturating_add(action_cost) > remaining {
                    budget_exhausted_during_execution = true;
                    failed_action_order.push(action.action_id.clone());
                    failed.insert(action.action_id.clone());
                    break;
                }
                round_spent = round_spent.saturating_add(action_cost);
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
        budget_spent = budget_spent.saturating_add(round_spent);
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
            cost_units: round_spent.min(u64::from(u32::MAX)) as u32,
            budget_before_units: before_budget,
            budget_after_units: after_budget,
            retry_count: round_retry_count,
        });
        if budget_exhausted_during_execution {
            stop_reason = FederatedBenchmarkCampaignStopReason::BudgetExhausted;
            break;
        }
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
    fn portfolio_beam_prefers_complementary_low_cost_actions() {
        let mut request = request();
        request.max_rounds = 1;
        request.budget_units = 4;
        request.actions = vec![
            FederatedBenchmarkAction {
                action_id: "expensive-single".into(),
                kind: FederatedBenchmarkActionKind::ReplicateSite,
                target_site_id: Some("site-a".into()),
                cost_units: 4,
                expected_information_milli: 1_000,
                expected_effect_milli: 300,
                feasibility_milli: 950,
                risk_milli: 20,
                requested_replicates: 4,
            },
            FederatedBenchmarkAction {
                action_id: "cheap-coverage".into(),
                kind: FederatedBenchmarkActionKind::ExpandCoverage,
                target_site_id: None,
                cost_units: 2,
                expected_information_milli: 820,
                expected_effect_milli: 120,
                feasibility_milli: 900,
                risk_milli: 20,
                requested_replicates: 4,
            },
            FederatedBenchmarkAction {
                action_id: "cheap-heterogeneity".into(),
                kind: FederatedBenchmarkActionKind::ResolveHeterogeneity,
                target_site_id: Some("site-a".into()),
                cost_units: 2,
                expected_information_milli: 820,
                expected_effect_milli: 120,
                feasibility_milli: 900,
                risk_milli: 20,
                requested_replicates: 4,
            },
        ];
        let mut executor = DryRunFederatedBenchmarkCampaignExecutor;
        let output = execute_federated_benchmark_campaign(&request, &mut executor).unwrap();
        assert_eq!(
            output.completed_action_order,
            vec!["cheap-coverage", "cheap-heterogeneity"]
        );
        assert!(!output
            .completed_action_order
            .contains(&"expensive-single".into()));
        output.validate().unwrap();
    }

    #[test]
    fn portfolio_closes_missing_site_evidence_before_a_redundant_high_score_action() {
        let mut request = request();
        request.benchmark.max_leave_one_out_shift_milli = 1_000;
        let consensus =
            analyze_federated_benchmark(&request.benchmark, &request.initial_sites).unwrap();
        let actions = [
            FederatedBenchmarkAction {
                action_id: "audit-only".into(),
                kind: FederatedBenchmarkActionKind::AuditInfluentialSite,
                target_site_id: Some("site-a".into()),
                cost_units: 1,
                expected_information_milli: 1_000,
                expected_effect_milli: 300,
                feasibility_milli: 950,
                risk_milli: 20,
                requested_replicates: 3,
            },
            FederatedBenchmarkAction {
                action_id: "coverage-action".into(),
                kind: FederatedBenchmarkActionKind::ExpandCoverage,
                target_site_id: None,
                cost_units: 1,
                expected_information_milli: 700,
                expected_effect_milli: 100,
                feasibility_milli: 900,
                risk_milli: 20,
                requested_replicates: 3,
            },
        ];
        let eligible = actions.iter().collect::<Vec<_>>();
        assert_eq!(
            evidence_gaps(&consensus),
            BTreeSet::from([BenchmarkEvidenceGap::SiteCoverage])
        );
        assert!(
            benchmark_portfolio_utility(&[1], &eligible, &consensus)
                > benchmark_portfolio_utility(&[0], &eligible, &consensus)
        );
        let selected = select_benchmark_portfolio(&eligible, &consensus, 1);
        assert_eq!(
            selected
                .iter()
                .map(|action| action.action_id.as_str())
                .collect::<Vec<_>>(),
            vec!["coverage-action"]
        );
    }

    #[test]
    fn portfolio_distinguishes_replicate_floor_from_new_site_coverage() {
        let mut request = request();
        request.benchmark.minimum_sites = 1;
        request.benchmark.minimum_replicates_per_site = 3;
        request.benchmark.max_leave_one_out_shift_milli = 1_000;
        request.initial_sites[0].replicate_count = 1;
        request.initial_sites.push(site("b", 650, 3));
        let consensus =
            analyze_federated_benchmark(&request.benchmark, &request.initial_sites).unwrap();
        assert_eq!(
            evidence_gaps(&consensus),
            BTreeSet::from([BenchmarkEvidenceGap::ReplicateFloor])
        );
        let actions = [
            FederatedBenchmarkAction {
                action_id: "expand-new-site".into(),
                kind: FederatedBenchmarkActionKind::ExpandCoverage,
                target_site_id: None,
                cost_units: 1,
                expected_information_milli: 1_000,
                expected_effect_milli: 300,
                feasibility_milli: 950,
                risk_milli: 20,
                requested_replicates: 3,
            },
            FederatedBenchmarkAction {
                action_id: "replicate-underpowered-site".into(),
                kind: FederatedBenchmarkActionKind::ReplicateSite,
                target_site_id: Some("site-a".into()),
                cost_units: 1,
                expected_information_milli: 600,
                expected_effect_milli: 100,
                feasibility_milli: 900,
                risk_milli: 20,
                requested_replicates: 3,
            },
        ];
        let eligible = actions.iter().collect::<Vec<_>>();
        let selected = select_benchmark_portfolio(&eligible, &consensus, 1);
        assert_eq!(
            selected
                .iter()
                .map(|action| action.action_id.as_str())
                .collect::<Vec<_>>(),
            vec!["replicate-underpowered-site"]
        );
    }

    #[test]
    fn retry_attempts_consume_federated_budget() {
        struct RetryOnce {
            calls: u8,
        }

        impl FederatedBenchmarkCampaignExecutor for RetryOnce {
            fn execute_action(
                &mut self,
                action: &FederatedBenchmarkAction,
                request: &FederatedBenchmarkRequest,
                attempt: u8,
            ) -> Result<FederatedBenchmarkSite, FederatedBenchmarkExecutionFailure> {
                self.calls = self.calls.saturating_add(1);
                if self.calls == 1 {
                    return Err(FederatedBenchmarkExecutionFailure {
                        reason: "transient site worker outage".into(),
                        retryable: true,
                    });
                }
                let mut dry_run = DryRunFederatedBenchmarkCampaignExecutor;
                dry_run.execute_action(action, request, attempt)
            }
        }

        let mut request = request();
        request.actions.truncate(1);
        request.budget_units = 4;
        request.max_rounds = 1;
        let mut executor = RetryOnce { calls: 0 };
        let output = execute_federated_benchmark_campaign(&request, &mut executor).unwrap();
        assert_eq!(output.retry_count, 1);
        assert_eq!(output.rounds[0].cost_units, 4);
        assert_eq!(output.budget_spent_units, 4);
        assert_eq!(output.remaining_budget_units, 0);
        assert_eq!(output.completed_action_order, vec!["expand-a"]);
        output.validate().unwrap();
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
        assert!(
            output
                .final_consensus
                .contributions
                .iter()
                .all(|item| item.disposition == FederatedBenchmarkSiteDisposition::Included)
        );
    }
}

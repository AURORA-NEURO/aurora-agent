//! Influence-aware, aggregate-only planning for federated preclinical glioma benchmarks.
//!
//! The consensus feature answers what the current consortium supports. This planner answers the
//! operational research question that follows: which additional independent sites are worth
//! funding next, under a hard budget and privacy-risk ceiling, if their expected aggregate scores
//! are treated conservatively? It uses bounded beam search over site portfolios, replays the real
//! consensus analyzer for every projected portfolio, and labels projections as scenarios rather
//! than observations. No raw trace, patient data, or clinical decision crosses this boundary.

use super::consensus::{
    analyze_federated_benchmark, FederatedBenchmarkConsensus, FederatedBenchmarkDisposition,
    FederatedBenchmarkRequest, FederatedBenchmarkSite,
};
use crate::glioma_engine::LocalArtifactRef;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P12-F12";
pub const OUTPUT_SCHEMA: &str = "GliomaFederatedBenchmarkSitePlanner1@1";
pub const MAX_CANDIDATES: usize = 128;
pub const MAX_NEW_SITES: usize = 16;
pub const MAX_BEAM_WIDTH: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedBenchmarkCandidate {
    pub candidate_id: String,
    pub site_id: String,
    pub study_id: String,
    pub independence_group: String,
    pub artifact: LocalArtifactRef,
    pub baseline_score_milli: u64,
    pub expected_candidate_score_milli: u64,
    pub uncertainty_milli: u64,
    pub replicate_count: u16,
    pub cost_units: u32,
    pub privacy_risk_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedBenchmarkSitePlannerRequest {
    pub benchmark: FederatedBenchmarkRequest,
    pub current_sites: Vec<FederatedBenchmarkSite>,
    pub candidates: Vec<FederatedBenchmarkCandidate>,
    pub budget_units: u64,
    pub max_new_sites: usize,
    pub beam_width: usize,
    pub privacy_budget_milli: u32,
    pub conservatism_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedBenchmarkCandidateScore {
    pub candidate_id: String,
    pub projected_disposition: FederatedBenchmarkDisposition,
    pub marginal_utility_milli: u64,
    pub cost_units: u32,
    pub privacy_risk_milli: u16,
    pub projected_signal_to_noise_milli: u64,
    pub projected_i2_milli: u16,
    pub projected_leave_one_out_shift_milli: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedBenchmarkPlanDisposition {
    ReadyToValidate,
    NeedsMoreSites,
    BudgetBlocked,
    NoAdmissiblePlan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedBenchmarkSitePlan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub baseline: FederatedBenchmarkConsensus,
    pub projected: FederatedBenchmarkConsensus,
    pub candidate_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub deferred_order: Vec<String>,
    pub ranking: Vec<FederatedBenchmarkCandidateScore>,
    pub next_action_order: Vec<String>,
    pub selected_cost_units: u64,
    pub selected_privacy_risk_milli: u32,
    pub budget_units: u64,
    pub remaining_budget_units: u64,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: FederatedBenchmarkPlanDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedBenchmarkSitePlannerError {
    #[error("federated benchmark site-planner request is invalid: {0}")]
    InvalidRequest(String),
    #[error("federated benchmark site-planner input is invalid: {0}")]
    InvalidInput(String),
    #[error("federated benchmark site-planner consensus projection failed: {0}")]
    Consensus(String),
    #[error("federated benchmark site-planner output is invalid: {0}")]
    InvalidOutput(String),
    #[error("federated benchmark site-planner digest failed: {0}")]
    Digest(String),
}

#[derive(Debug, Clone)]
struct BeamState {
    selected: Vec<usize>,
    cost_units: u64,
    privacy_risk_milli: u32,
    projected: FederatedBenchmarkConsensus,
    utility: i128,
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn disposition_rank(disposition: FederatedBenchmarkDisposition) -> i128 {
    match disposition {
        FederatedBenchmarkDisposition::Qualified => 4,
        FederatedBenchmarkDisposition::Heterogeneous => 2,
        FederatedBenchmarkDisposition::Unresolved => 1,
        FederatedBenchmarkDisposition::Negative => 0,
    }
}

fn utility(consensus: &FederatedBenchmarkConsensus, cost: u64, privacy: u32) -> i128 {
    disposition_rank(consensus.disposition) * 1_000_000_000_000_i128
        + i128::from(consensus.signal_to_noise_milli.min(1_000_000)) * 1_000_000
        - i128::from(consensus.i2_milli) * 10_000
        - i128::from(consensus.max_leave_one_out_shift_milli.min(1_000_000)) * 10_000
        - i128::from(consensus.site_spread_milli.min(1_000_000)) * 1_000
        - i128::from(privacy) * 100
        - i128::from(cost) * 10
}

fn digest_input(output: &FederatedBenchmarkSitePlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "baseline": output.baseline,
        "projected": output.projected,
        "candidate_order": output.candidate_order,
        "selected_order": output.selected_order,
        "deferred_order": output.deferred_order,
        "ranking": output.ranking,
        "next_action_order": output.next_action_order,
        "selected_cost_units": output.selected_cost_units,
        "selected_privacy_risk_milli": output.selected_privacy_risk_milli,
        "budget_units": output.budget_units,
        "remaining_budget_units": output.remaining_budget_units,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn conservative_site(
    candidate: &FederatedBenchmarkCandidate,
    request: &FederatedBenchmarkSitePlannerRequest,
) -> FederatedBenchmarkSite {
    let uncertainty_penalty = candidate
        .uncertainty_milli
        .saturating_mul(u64::from(request.conservatism_milli))
        / 1_000;
    FederatedBenchmarkSite {
        site_id: candidate.site_id.clone(),
        study_id: candidate.study_id.clone(),
        capability_id: request.benchmark.capability_id.clone(),
        benchmark_world: request.benchmark.benchmark_world.clone(),
        metric_name: request.benchmark.metric_name.clone(),
        model_system: request.benchmark.model_system,
        artifact: candidate.artifact.clone(),
        baseline_score_milli: candidate.baseline_score_milli,
        candidate_score_milli: candidate
            .expected_candidate_score_milli
            .saturating_sub(uncertainty_penalty),
        uncertainty_milli: candidate.uncertainty_milli,
        replicate_count: candidate.replicate_count,
    }
}

fn project(
    request: &FederatedBenchmarkSitePlannerRequest,
    candidates: &[FederatedBenchmarkCandidate],
    selected: &[usize],
) -> Result<FederatedBenchmarkConsensus, FederatedBenchmarkSitePlannerError> {
    let mut sites = request.current_sites.clone();
    sites.extend(
        selected
            .iter()
            .map(|index| conservative_site(&candidates[*index], request)),
    );
    analyze_federated_benchmark(&request.benchmark, &sites)
        .map_err(|error| FederatedBenchmarkSitePlannerError::Consensus(error.to_string()))
}

fn validate_request(
    request: &FederatedBenchmarkSitePlannerRequest,
) -> Result<(), FederatedBenchmarkSitePlannerError> {
    if request.budget_units == 0
        || request.max_new_sites == 0
        || request.max_new_sites > MAX_NEW_SITES
        || request.beam_width == 0
        || request.beam_width > MAX_BEAM_WIDTH
        || request.privacy_budget_milli == 0
        || request.conservatism_milli > 1_000
        || request.candidates.is_empty()
        || request.candidates.len() > MAX_CANDIDATES
        || request.current_sites.is_empty()
    {
        return Err(FederatedBenchmarkSitePlannerError::InvalidRequest(
            "bounded current sites, candidates, budget, privacy, beam, and conservatism settings are required".into(),
        ));
    }
    let mut site_ids = BTreeSet::new();
    let mut study_ids = BTreeSet::new();
    let mut candidate_ids = BTreeSet::new();
    let current_site_ids = request
        .current_sites
        .iter()
        .map(|site| site.site_id.clone())
        .collect::<BTreeSet<_>>();
    for candidate in &request.candidates {
        candidate
            .artifact
            .validate()
            .map_err(|error| FederatedBenchmarkSitePlannerError::InvalidInput(error.to_string()))?;
        if candidate.candidate_id.trim().is_empty()
            || candidate.site_id.trim().is_empty()
            || candidate.study_id.trim().is_empty()
            || candidate.independence_group.trim().is_empty()
            || candidate.baseline_score_milli > super::consensus::MAX_SCORE_MILLI
            || candidate.expected_candidate_score_milli > super::consensus::MAX_SCORE_MILLI
            || candidate.uncertainty_milli == 0
            || candidate.uncertainty_milli > super::consensus::MAX_SCORE_MILLI
            || candidate.replicate_count == 0
            || candidate.cost_units == 0
            || candidate.privacy_risk_milli > 1_000
            || !candidate_ids.insert(candidate.candidate_id.clone())
            || !site_ids.insert(candidate.site_id.clone())
            || !study_ids.insert(candidate.study_id.clone())
            || current_site_ids.contains(&candidate.site_id)
        {
            return Err(FederatedBenchmarkSitePlannerError::InvalidInput(
                "candidate identity, independence, score, replicate, cost, privacy, or site uniqueness is invalid".into(),
            ));
        }
    }
    Ok(())
}

impl FederatedBenchmarkSitePlan {
    pub fn validate(&self) -> Result<(), FederatedBenchmarkSitePlannerError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || !canonical(&self.candidate_order)
            || !canonical(&self.selected_order)
            || !canonical(&self.deferred_order)
            || !canonical(&self.next_action_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.selected_cost_units > self.budget_units
            || self.remaining_budget_units != self.budget_units - self.selected_cost_units
            || self
                .ranking
                .windows(2)
                .any(|pair| pair[0].marginal_utility_milli < pair[1].marginal_utility_milli)
            || self.ranking.iter().any(|score| {
                score.candidate_id.trim().is_empty()
                    || score.projected_i2_milli > 1_000
                    || score.privacy_risk_milli > 1_000
            })
        {
            return Err(FederatedBenchmarkSitePlannerError::InvalidOutput(
                "identity, ordering, budget, ranking, privacy, or score bounds are invalid".into(),
            ));
        }
        self.baseline.validate().map_err(|error| {
            FederatedBenchmarkSitePlannerError::InvalidOutput(error.to_string())
        })?;
        self.projected.validate().map_err(|error| {
            FederatedBenchmarkSitePlannerError::InvalidOutput(error.to_string())
        })?;
        let candidates = self.candidate_order.iter().collect::<BTreeSet<_>>();
        let selected = self.selected_order.iter().collect::<BTreeSet<_>>();
        let deferred = self.deferred_order.iter().collect::<BTreeSet<_>>();
        let ranked = self
            .ranking
            .iter()
            .map(|score| &score.candidate_id)
            .collect::<BTreeSet<_>>();
        if candidates != selected.union(&deferred).cloned().collect::<BTreeSet<_>>()
            || candidates != ranked
            || selected.intersection(&deferred).next().is_some()
            || self
                .next_action_order
                .iter()
                .any(|action| !action.starts_with("add-federated-site:"))
        {
            return Err(FederatedBenchmarkSitePlannerError::InvalidOutput(
                "candidate, selected, deferred, ranking, or action partitions do not reconcile"
                    .into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| FederatedBenchmarkSitePlannerError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(FederatedBenchmarkSitePlannerError::InvalidOutput(
                "site plan digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Plan a conservative, privacy- and influence-aware consortium expansion without moving raw data.
pub fn plan_federated_benchmark_sites(
    request: &FederatedBenchmarkSitePlannerRequest,
) -> Result<FederatedBenchmarkSitePlan, FederatedBenchmarkSitePlannerError> {
    validate_request(request)?;
    let mut candidates = request.candidates.clone();
    candidates.sort_by(|left, right| left.candidate_id.cmp(&right.candidate_id));
    let baseline = analyze_federated_benchmark(&request.benchmark, &request.current_sites)
        .map_err(|error| FederatedBenchmarkSitePlannerError::Consensus(error.to_string()))?;

    let mut ranking = Vec::with_capacity(candidates.len());
    for (index, candidate) in candidates.iter().enumerate() {
        let projected = project(request, &candidates, &[index])?;
        let marginal = utility(
            &projected,
            u64::from(candidate.cost_units),
            u32::from(candidate.privacy_risk_milli),
        )
        .saturating_sub(utility(&baseline, 0, 0));
        ranking.push(FederatedBenchmarkCandidateScore {
            candidate_id: candidate.candidate_id.clone(),
            projected_disposition: projected.disposition,
            marginal_utility_milli: marginal.max(0).min(u64::MAX as i128) as u64,
            cost_units: candidate.cost_units,
            privacy_risk_milli: candidate.privacy_risk_milli,
            projected_signal_to_noise_milli: projected.signal_to_noise_milli,
            projected_i2_milli: projected.i2_milli,
            projected_leave_one_out_shift_milli: projected.max_leave_one_out_shift_milli,
        });
    }
    ranking.sort_by(|left, right| {
        right
            .marginal_utility_milli
            .cmp(&left.marginal_utility_milli)
            .then_with(|| left.cost_units.cmp(&right.cost_units))
            .then_with(|| left.candidate_id.cmp(&right.candidate_id))
    });

    let mut beam = vec![BeamState {
        selected: Vec::new(),
        cost_units: 0,
        privacy_risk_milli: 0,
        projected: baseline.clone(),
        utility: utility(&baseline, 0, 0),
    }];
    let mut best = beam[0].clone();
    for _depth in 0..request.max_new_sites {
        let mut expanded = Vec::new();
        for state in &beam {
            let start = state.selected.last().map_or(0, |index| index + 1);
            for index in start..candidates.len() {
                let candidate = &candidates[index];
                if state.selected.iter().any(|selected_index| {
                    candidates[*selected_index].independence_group == candidate.independence_group
                }) {
                    continue;
                }
                let cost = state
                    .cost_units
                    .saturating_add(u64::from(candidate.cost_units));
                let privacy = state
                    .privacy_risk_milli
                    .saturating_add(u32::from(candidate.privacy_risk_milli));
                if cost > request.budget_units || privacy > request.privacy_budget_milli {
                    continue;
                }
                let mut selected = state.selected.clone();
                selected.push(index);
                let projected = project(request, &candidates, &selected)?;
                let value = utility(&projected, cost, privacy);
                expanded.push(BeamState {
                    selected,
                    cost_units: cost,
                    privacy_risk_milli: privacy,
                    projected,
                    utility: value,
                });
            }
        }
        if expanded.is_empty() {
            break;
        }
        expanded.sort_by(|left, right| {
            right
                .utility
                .cmp(&left.utility)
                .then_with(|| left.cost_units.cmp(&right.cost_units))
                .then_with(|| left.selected.cmp(&right.selected))
        });
        expanded.truncate(request.beam_width);
        for state in &expanded {
            if state.utility > best.utility
                || (state.utility == best.utility && state.cost_units < best.cost_units)
            {
                best = state.clone();
            }
        }
        beam = expanded;
    }

    let selected_order = best
        .selected
        .iter()
        .map(|index| candidates[*index].candidate_id.clone())
        .collect::<Vec<_>>();
    let selected_set = selected_order.iter().collect::<BTreeSet<_>>();
    let candidate_order = candidates
        .iter()
        .map(|candidate| candidate.candidate_id.clone())
        .collect::<Vec<_>>();
    let deferred_order = candidate_order
        .iter()
        .filter(|id| !selected_set.contains(id))
        .cloned()
        .collect::<Vec<_>>();
    let mut next_action_order = selected_order
        .iter()
        .map(|id| format!("add-federated-site:{id}"))
        .collect::<Vec<_>>();
    if next_action_order.is_empty()
        && baseline.disposition != FederatedBenchmarkDisposition::Qualified
    {
        next_action_order = ranking
            .iter()
            .take(3)
            .map(|score| format!("add-federated-site:{}", score.candidate_id))
            .collect();
    }
    next_action_order.sort();

    let mut negative_evidence = best.projected.negative_evidence.clone();
    let mut uncertainty = best.projected.uncertainty.clone();
    uncertainty.push("projected-consensus-is-a-conservative-scenario-not-an-observation".into());
    uncertainty.sort();
    uncertainty.dedup();
    if best.projected.disposition != FederatedBenchmarkDisposition::Qualified
        && selected_order.is_empty()
    {
        negative_evidence.push("no-budget-admissible-portfolio-clears-consensus-gates".into());
    }
    negative_evidence.sort();
    negative_evidence.dedup();
    let disposition = if best.projected.disposition == FederatedBenchmarkDisposition::Qualified {
        FederatedBenchmarkPlanDisposition::ReadyToValidate
    } else if best.cost_units >= request.budget_units {
        FederatedBenchmarkPlanDisposition::BudgetBlocked
    } else if !selected_order.is_empty() {
        FederatedBenchmarkPlanDisposition::NeedsMoreSites
    } else {
        FederatedBenchmarkPlanDisposition::NoAdmissiblePlan
    };
    let mut output = FederatedBenchmarkSitePlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.benchmark.objective.clone(),
        baseline,
        projected: best.projected,
        candidate_order,
        selected_order,
        deferred_order,
        ranking,
        next_action_order,
        selected_cost_units: best.cost_units,
        selected_privacy_risk_milli: best.privacy_risk_milli,
        budget_units: request.budget_units,
        remaining_budget_units: request.budget_units.saturating_sub(best.cost_units),
        negative_evidence,
        uncertainty,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-federated-site-plan"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| FederatedBenchmarkSitePlannerError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma_engine::GliomaModelSystem;

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: format!("artifact-{id}"),
            content_hash: ContentHash::of_bytes(id.as_bytes()),
            content_type: "application/vnd.aurora.glioma.federated-plan+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn benchmark() -> FederatedBenchmarkRequest {
        FederatedBenchmarkRequest {
            objective: "validate an organoid invasion model across sites".into(),
            capability_id: "glioma:invasion-model".into(),
            benchmark_world: "glioma-world-v1".into(),
            metric_name: "holdout_auc".into(),
            model_system: GliomaModelSystem::Organoid,
            minimum_sites: 3,
            minimum_replicates_per_site: 3,
            effect_threshold_milli: 80,
            max_i2_milli: 250,
            min_signal_to_noise_milli: 500,
            max_site_spread_milli: 160,
            max_leave_one_out_shift_milli: 100,
        }
    }

    fn current(id: &str, score: u64) -> FederatedBenchmarkSite {
        FederatedBenchmarkSite {
            site_id: format!("site-{id}"),
            study_id: format!("study-{id}"),
            capability_id: "glioma:invasion-model".into(),
            benchmark_world: "glioma-world-v1".into(),
            metric_name: "holdout_auc".into(),
            model_system: GliomaModelSystem::Organoid,
            artifact: artifact(id),
            baseline_score_milli: 500,
            candidate_score_milli: score,
            uncertainty_milli: 45,
            replicate_count: 4,
        }
    }

    fn candidate(id: &str, score: u64, cost: u32) -> FederatedBenchmarkCandidate {
        FederatedBenchmarkCandidate {
            candidate_id: format!("candidate-{id}"),
            site_id: format!("candidate-site-{id}"),
            study_id: format!("candidate-study-{id}"),
            independence_group: format!("group-{id}"),
            artifact: artifact(&format!("candidate-{id}")),
            baseline_score_milli: 500,
            expected_candidate_score_milli: score,
            uncertainty_milli: 30,
            replicate_count: 4,
            cost_units: cost,
            privacy_risk_milli: 100,
        }
    }

    fn request() -> FederatedBenchmarkSitePlannerRequest {
        FederatedBenchmarkSitePlannerRequest {
            benchmark: benchmark(),
            current_sites: vec![current("a", 600), current("b", 615)],
            candidates: vec![candidate("c", 625, 4), candidate("d", 900, 9)],
            budget_units: 12,
            max_new_sites: 2,
            beam_width: 8,
            privacy_budget_milli: 500,
            conservatism_milli: 500,
        }
    }

    #[test]
    fn planner_finds_conservative_qualified_portfolio_and_replays() {
        let first = plan_federated_benchmark_sites(&request()).unwrap();
        let second = plan_federated_benchmark_sites(&request()).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            first.disposition,
            FederatedBenchmarkPlanDisposition::ReadyToValidate
        );
        assert!(!first.selected_order.is_empty());
        assert!(first.projected.disposition == FederatedBenchmarkDisposition::Qualified);
        first.validate().unwrap();
    }

    #[test]
    fn planner_keeps_budget_and_scenario_uncertainty_explicit() {
        let mut request = request();
        request.budget_units = 1;
        let plan = plan_federated_benchmark_sites(&request).unwrap();
        assert!(matches!(
            plan.disposition,
            FederatedBenchmarkPlanDisposition::BudgetBlocked
                | FederatedBenchmarkPlanDisposition::NoAdmissiblePlan
                | FederatedBenchmarkPlanDisposition::NeedsMoreSites
        ));
        assert!(plan
            .uncertainty
            .iter()
            .any(|item| item.contains("scenario")));
        assert!(!plan.next_action_order.is_empty());
    }

    #[test]
    fn planner_never_selects_two_candidates_from_one_independence_group() {
        let mut request = request();
        request.candidates[1].independence_group = request.candidates[0].independence_group.clone();
        let plan = plan_federated_benchmark_sites(&request).unwrap();
        assert!(plan.selected_order.len() <= 1);
    }
}

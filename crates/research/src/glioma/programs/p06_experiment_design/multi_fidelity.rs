//! Cost-aware multi-fidelity optimization for preclinical glioma experiments.
//!
//! This module is a bounded surrogate-and-selection engine for screening, mechanistic, and
//! validation model systems.  It does not pretend that a cheap cell-line observation is a
//! validation result: it estimates cross-fidelity transfer bias from paired designs, carries the
//! resulting uncertainty into an acquisition score, and blocks high-fidelity work without a
//! declared lower-fidelity support path.  The optimizer is deliberately integer-only and
//! content-addressed so a campaign can be replayed on a disconnected local workstation.

use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P06-F20";
pub const OUTPUT_SCHEMA: &str = "GliomaMultiFidelityOptimization1@1";
pub const MAX_CANDIDATES: usize = 4_096;
pub const MAX_OBSERVATIONS: usize = 32_768;
pub const MAX_SELECTIONS: usize = 128;
const SCORE_SCALE: u64 = 1_000;
const BEAM_WIDTH: usize = 64;

/// Fidelity is a research execution scale, not a claim of biological truth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FidelityLevel {
    Screening,
    Mechanistic,
    Validation,
}

impl FidelityLevel {
    const fn ordinal(self) -> u8 {
        match self {
            Self::Screening => 0,
            Self::Mechanistic => 1,
            Self::Validation => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OptimizationDirection {
    Maximize,
    Minimize,
}

/// The selection policy is explicit so a consortium can review the trade-off between exploitation,
/// exploration, transfer learning, cost, and risk without changing the candidate contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiFidelityOptimizationRequest {
    pub objective: String,
    pub direction: OptimizationDirection,
    pub budget_units: u64,
    pub max_selections: usize,
    pub min_replicates_per_candidate: u16,
    pub exploration_weight_milli: u16,
    pub exploitation_weight_milli: u16,
    pub transfer_weight_milli: u16,
    pub risk_penalty_milli: u16,
    pub cost_penalty_milli: u16,
    pub max_risk_milli: u16,
    pub min_transfer_reliability_milli: u16,
    pub baseline_milli: Option<i64>,
}

/// A candidate is an independently runnable condition.  `design_id` links equivalent conditions
/// across fidelity levels; `parent_candidate_id` declares the lower-fidelity support route.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FidelityCandidate {
    pub candidate_id: String,
    pub design_id: String,
    pub fidelity: FidelityLevel,
    pub model_system: GliomaModelSystem,
    pub dose_milli: u32,
    pub combination_milli: u32,
    pub cost_units: u32,
    pub risk_milli: u16,
    pub parent_candidate_id: Option<String>,
    pub max_replicates: u16,
}

/// One independent local result.  The optimizer consumes only the value and declared uncertainty;
/// raw bytes remain behind the local artifact reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FidelityObservation {
    pub observation_id: String,
    pub candidate_id: String,
    pub replicate_index: u16,
    pub outcome_milli: i64,
    pub uncertainty_milli: u64,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EstimateSource {
    Observed,
    Transferred,
    Neighborhood,
    Prior,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FidelityEstimate {
    pub candidate_id: String,
    pub design_id: String,
    pub fidelity: FidelityLevel,
    pub model_system: GliomaModelSystem,
    pub observed_replicates: u16,
    pub posterior_mean_milli: i64,
    pub posterior_uncertainty_milli: u64,
    pub expected_improvement_milli: u64,
    pub exploration_milli: u64,
    pub transfer_value_milli: u64,
    pub acquisition_milli: u64,
    pub source: EstimateSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FidelityCalibration {
    pub lower_fidelity: FidelityLevel,
    pub higher_fidelity: FidelityLevel,
    pub paired_design_order: Vec<String>,
    pub pair_count: u16,
    pub bias_milli: i64,
    pub residual_milli: u64,
    pub reliability_milli: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultiFidelityDisposition {
    Qualified,
    Partial,
    BudgetBlocked,
    NoEligibleCandidates,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiFidelityOptimizationPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub direction: OptimizationDirection,
    pub candidate_order: Vec<String>,
    pub observed_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub deferred_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub estimates: Vec<FidelityEstimate>,
    pub calibrations: Vec<FidelityCalibration>,
    pub budget_units: u64,
    pub budget_used_units: u64,
    pub budget_remaining_units: u64,
    pub best_observed_milli: Option<i64>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: MultiFidelityDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MultiFidelityOptimizationError {
    #[error("multi-fidelity request is invalid: {0}")]
    InvalidRequest(String),
    #[error("multi-fidelity candidate is invalid: {0}")]
    InvalidCandidate(String),
    #[error("multi-fidelity observation is invalid: {0}")]
    InvalidObservation(String),
    #[error("multi-fidelity output is invalid: {0}")]
    InvalidOutput(String),
    #[error("multi-fidelity digest failed: {0}")]
    Digest(String),
}

#[derive(Debug, Clone, Copy)]
struct Aggregate {
    mean: i64,
    uncertainty: u64,
    replicates: u16,
}

#[derive(Debug, Clone)]
struct ScoredCandidate {
    candidate: FidelityCandidate,
    estimate: FidelityEstimate,
    eligible: bool,
    block_reason: Option<String>,
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn clamp_i64(value: i128) -> i64 {
    value.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64
}

fn abs_i128(value: i128) -> u128 {
    value.unsigned_abs()
}

fn weighted_mean(values: &[(i64, u64)]) -> i64 {
    let mut numerator = 0_i128;
    let mut denominator = 0_u128;
    for (value, uncertainty) in values {
        let weight = 1_000_000_u128 / u128::from((*uncertainty).max(1));
        numerator = numerator.saturating_add(i128::from(*value).saturating_mul(weight as i128));
        denominator = denominator.saturating_add(weight);
    }
    if denominator == 0 {
        0
    } else {
        clamp_i64(numerator / denominator as i128)
    }
}

fn aggregate(observations: &[&FidelityObservation]) -> Aggregate {
    let values = observations
        .iter()
        .map(|observation| (observation.outcome_milli, observation.uncertainty_milli))
        .collect::<Vec<_>>();
    let mean = weighted_mean(&values);
    let mut spread = 0_u128;
    let mut declared = 0_u128;
    for observation in observations {
        spread = spread.saturating_add(abs_i128(
            i128::from(observation.outcome_milli) - i128::from(mean),
        ));
        declared = declared.saturating_add(u128::from(observation.uncertainty_milli));
    }
    let n = observations.len().max(1) as u128;
    Aggregate {
        mean,
        uncertainty: spread
            .saturating_div(n)
            .max(declared.saturating_div(n))
            .min(u128::from(u64::MAX)) as u64,
        replicates: observations.len().min(usize::from(u16::MAX)) as u16,
    }
}

fn median_i64(values: &mut [i64]) -> i64 {
    if values.is_empty() {
        0
    } else {
        values.sort_unstable();
        values[values.len() / 2]
    }
}

fn median_u64(values: &mut [u64]) -> u64 {
    if values.is_empty() {
        0
    } else {
        values.sort_unstable();
        values[values.len() / 2]
    }
}

fn calibration_for_pair(
    lower: FidelityLevel,
    higher: FidelityLevel,
    by_design: &BTreeMap<String, BTreeMap<FidelityLevel, Aggregate>>,
) -> FidelityCalibration {
    let mut designs = Vec::new();
    let mut differences = Vec::new();
    for (design_id, levels) in by_design {
        if let (Some(low), Some(high)) = (levels.get(&lower), levels.get(&higher)) {
            designs.push(design_id.clone());
            differences.push(high.mean.saturating_sub(low.mean));
        }
    }
    let bias = median_i64(&mut differences);
    let mut residuals = by_design
        .values()
        .filter_map(|levels| {
            let low = levels.get(&lower)?;
            let high = levels.get(&higher)?;
            Some(abs_i128(i128::from(high.mean) - i128::from(low.mean) - i128::from(bias)) as u64)
        })
        .collect::<Vec<_>>();
    let residual = median_u64(&mut residuals);
    let pair_count = designs.len().min(usize::from(u16::MAX)) as u16;
    let reliability = if pair_count == 0 {
        0
    } else {
        let support = u64::from(pair_count).saturating_mul(250).min(SCORE_SCALE);
        let noise_penalty = residual.min(SCORE_SCALE);
        support
            .saturating_mul(SCORE_SCALE.saturating_sub(noise_penalty))
            .checked_div(SCORE_SCALE)
            .unwrap_or(0)
            .min(SCORE_SCALE) as u16
    };
    designs.sort();
    FidelityCalibration {
        lower_fidelity: lower,
        higher_fidelity: higher,
        paired_design_order: designs,
        pair_count,
        bias_milli: bias,
        residual_milli: residual,
        reliability_milli: reliability,
    }
}

fn coordinate_distance_milli(left: &FidelityCandidate, right: &FidelityCandidate) -> u64 {
    let dose = u64::from(left.dose_milli.abs_diff(right.dose_milli)).min(SCORE_SCALE);
    let combination =
        u64::from(left.combination_milli.abs_diff(right.combination_milli)).min(SCORE_SCALE);
    dose.saturating_add(combination)
        .saturating_div(2)
        .min(SCORE_SCALE)
}

fn normalized_improvement(
    direction: OptimizationDirection,
    mean: i64,
    uncertainty: u64,
    baseline: i64,
) -> u64 {
    let delta = match direction {
        OptimizationDirection::Maximize => i128::from(mean) - i128::from(baseline),
        OptimizationDirection::Minimize => i128::from(baseline) - i128::from(mean),
    };
    let optimistic = delta.saturating_add(i128::from(uncertainty));
    optimistic.max(0).min(i128::from(u64::MAX)) as u64
}

fn acquisition(
    request: &MultiFidelityOptimizationRequest,
    estimate: &FidelityEstimate,
    candidate: &FidelityCandidate,
) -> u64 {
    let positive = |weight: u16, value: u64| {
        u64::from(weight)
            .saturating_mul(value.min(SCORE_SCALE))
            .saturating_div(SCORE_SCALE)
    };
    let cost_penalty = u64::from(request.cost_penalty_milli)
        .saturating_mul(u64::from(candidate.cost_units).min(SCORE_SCALE))
        .saturating_div(SCORE_SCALE);
    let risk_penalty = u64::from(request.risk_penalty_milli)
        .saturating_mul(u64::from(candidate.risk_milli))
        .saturating_div(SCORE_SCALE);
    positive(
        request.exploitation_weight_milli,
        estimate.expected_improvement_milli,
    )
    .saturating_add(positive(
        request.exploration_weight_milli,
        estimate.exploration_milli,
    ))
    .saturating_add(positive(
        request.transfer_weight_milli,
        estimate.transfer_value_milli,
    ))
    .saturating_sub(cost_penalty)
    .saturating_sub(risk_penalty)
}

fn digest_input(plan: &MultiFidelityOptimizationPlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": plan.feature_id,
        "output_schema": plan.output_schema,
        "objective": plan.objective,
        "direction": plan.direction,
        "candidate_order": plan.candidate_order,
        "observed_order": plan.observed_order,
        "selected_order": plan.selected_order,
        "deferred_order": plan.deferred_order,
        "blocked_order": plan.blocked_order,
        "estimates": plan.estimates,
        "calibrations": plan.calibrations,
        "budget_units": plan.budget_units,
        "budget_used_units": plan.budget_used_units,
        "budget_remaining_units": plan.budget_remaining_units,
        "best_observed_milli": plan.best_observed_milli,
        "negative_evidence": plan.negative_evidence,
        "uncertainty": plan.uncertainty,
        "disposition": plan.disposition,
    })
}

impl MultiFidelityOptimizationPlan {
    pub fn validate(&self) -> Result<(), MultiFidelityOptimizationError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.candidate_order.is_empty()
            || !canonical(&self.candidate_order)
            || !canonical(&self.observed_order)
            || !canonical(&self.selected_order)
            || !canonical(&self.deferred_order)
            || !canonical(&self.blocked_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.budget_used_units > self.budget_units
            || self.budget_remaining_units != self.budget_units - self.budget_used_units
            || self.estimates.len() != self.candidate_order.len()
            || self
                .estimates
                .windows(2)
                .any(|pair| pair[0].candidate_id >= pair[1].candidate_id)
            || self.calibrations.windows(2).any(|pair| {
                (pair[0].lower_fidelity, pair[0].higher_fidelity)
                    >= (pair[1].lower_fidelity, pair[1].higher_fidelity)
            })
            || self.estimates.iter().any(|estimate| {
                estimate.transfer_value_milli > SCORE_SCALE
                    || estimate.acquisition_milli > u64::from(u16::MAX)
            })
        {
            return Err(MultiFidelityOptimizationError::InvalidOutput(
                "identity, ordering, budget, estimate, or calibration invariants are invalid"
                    .into(),
            ));
        }
        let candidate_set = self.candidate_order.iter().collect::<BTreeSet<_>>();
        for (label, values) in [
            ("observed", &self.observed_order),
            ("selected", &self.selected_order),
            ("deferred", &self.deferred_order),
            ("blocked", &self.blocked_order),
        ] {
            if values.iter().any(|value| !candidate_set.contains(value)) {
                return Err(MultiFidelityOptimizationError::InvalidOutput(format!(
                    "{label} order contains an unknown candidate"
                )));
            }
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| MultiFidelityOptimizationError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(MultiFidelityOptimizationError::InvalidOutput(
                "multi-fidelity digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &MultiFidelityOptimizationRequest,
) -> Result<(), MultiFidelityOptimizationError> {
    if request.objective.trim().is_empty()
        || request.budget_units == 0
        || request.max_selections == 0
        || request.max_selections > MAX_SELECTIONS
        || request.min_replicates_per_candidate == 0
        || u64::from(request.exploration_weight_milli) > SCORE_SCALE
        || u64::from(request.exploitation_weight_milli) > SCORE_SCALE
        || u64::from(request.transfer_weight_milli) > SCORE_SCALE
        || u64::from(request.risk_penalty_milli) > SCORE_SCALE
        || u64::from(request.cost_penalty_milli) > SCORE_SCALE
        || u64::from(request.max_risk_milli) > SCORE_SCALE
        || u64::from(request.min_transfer_reliability_milli) > SCORE_SCALE
        || request
            .exploration_weight_milli
            .saturating_add(request.exploitation_weight_milli)
            .saturating_add(request.transfer_weight_milli)
            == 0
    {
        return Err(MultiFidelityOptimizationError::InvalidRequest(
            "objective, positive budget/selection/replicate floors, and bounded non-zero score weights are required".into(),
        ));
    }
    Ok(())
}

fn validate_candidates(
    candidates: &[FidelityCandidate],
    request: &MultiFidelityOptimizationRequest,
) -> Result<BTreeMap<String, FidelityCandidate>, MultiFidelityOptimizationError> {
    if candidates.is_empty() || candidates.len() > MAX_CANDIDATES {
        return Err(MultiFidelityOptimizationError::InvalidCandidate(
            "candidate count must be non-zero and bounded".into(),
        ));
    }
    let mut by_id = BTreeMap::new();
    for candidate in candidates {
        if candidate.candidate_id.trim().is_empty()
            || candidate.design_id.trim().is_empty()
            || candidate.cost_units == 0
            || u64::from(candidate.risk_milli) > SCORE_SCALE
            || candidate.max_replicates == 0
            || !by_id
                .insert(candidate.candidate_id.clone(), candidate.clone())
                .is_none()
        {
            return Err(MultiFidelityOptimizationError::InvalidCandidate(
                "candidate ids/design ids must be non-empty and unique, with positive cost and replicate bounds".into(),
            ));
        }
    }
    for candidate in by_id.values() {
        if let Some(parent_id) = &candidate.parent_candidate_id {
            let Some(parent) = by_id.get(parent_id) else {
                return Err(MultiFidelityOptimizationError::InvalidCandidate(format!(
                    "{} names a missing parent candidate",
                    candidate.candidate_id
                )));
            };
            if parent.design_id != candidate.design_id
                || parent.fidelity.ordinal() >= candidate.fidelity.ordinal()
            {
                return Err(MultiFidelityOptimizationError::InvalidCandidate(format!(
                    "{} parent must share its design and be strictly lower fidelity",
                    candidate.candidate_id
                )));
            }
        } else if candidate.fidelity != FidelityLevel::Screening {
            return Err(MultiFidelityOptimizationError::InvalidCandidate(format!(
                "{} high-fidelity candidate needs an explicit lower-fidelity parent",
                candidate.candidate_id
            )));
        }
        if u64::from(candidate.cost_units) > request.budget_units {
            // Keep an over-budget candidate in the catalog so the output can explain why it was
            // blocked; the selection budget itself remains hard.
        }
    }
    Ok(by_id)
}

fn validate_observations<'a>(
    observations: &'a [FidelityObservation],
    candidates: &BTreeMap<String, FidelityCandidate>,
) -> Result<BTreeMap<String, Vec<&'a FidelityObservation>>, MultiFidelityOptimizationError> {
    if observations.len() > MAX_OBSERVATIONS {
        return Err(MultiFidelityOptimizationError::InvalidObservation(
            "observation count exceeds the deterministic bound".into(),
        ));
    }
    let mut seen_ids = BTreeSet::new();
    let mut seen_replicates = BTreeSet::new();
    let mut grouped = BTreeMap::<String, Vec<&FidelityObservation>>::new();
    for observation in observations {
        if observation.observation_id.trim().is_empty()
            || !seen_ids.insert(observation.observation_id.clone())
            || !candidates.contains_key(&observation.candidate_id)
            || !seen_replicates.insert((
                observation.candidate_id.clone(),
                observation.replicate_index,
            ))
            || observation.uncertainty_milli == 0
        {
            return Err(MultiFidelityOptimizationError::InvalidObservation(
                "observation ids and candidate/replicate keys must be unique, known, and positively uncertain".into(),
            ));
        }
        observation.artifact.validate().map_err(|error| {
            MultiFidelityOptimizationError::InvalidObservation(error.to_string())
        })?;
        grouped
            .entry(observation.candidate_id.clone())
            .or_default()
            .push(observation);
    }
    Ok(grouped)
}

fn select_beam(
    scored: &[ScoredCandidate],
    request: &MultiFidelityOptimizationRequest,
) -> BTreeSet<String> {
    #[derive(Clone)]
    struct State {
        ids: BTreeSet<String>,
        designs: BTreeSet<String>,
        cost: u64,
        score: u64,
    }
    let mut states = vec![State {
        ids: BTreeSet::new(),
        designs: BTreeSet::new(),
        cost: 0,
        score: 0,
    }];
    let eligible = scored
        .iter()
        .filter(|entry| entry.eligible)
        .collect::<Vec<_>>();
    for entry in eligible {
        let mut expanded = states.clone();
        for state in &states {
            if state.ids.len() >= request.max_selections
                || state
                    .cost
                    .saturating_add(u64::from(entry.candidate.cost_units))
                    > request.budget_units
            {
                continue;
            }
            let new_design = !state.designs.contains(&entry.candidate.design_id);
            let diversity_bonus = if new_design { 25 } else { 0 };
            let mut ids = state.ids.clone();
            ids.insert(entry.candidate.candidate_id.clone());
            let mut designs = state.designs.clone();
            designs.insert(entry.candidate.design_id.clone());
            expanded.push(State {
                ids,
                designs,
                cost: state.cost + u64::from(entry.candidate.cost_units),
                score: state
                    .score
                    .saturating_add(entry.estimate.acquisition_milli)
                    .saturating_add(diversity_bonus),
            });
        }
        expanded.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| left.cost.cmp(&right.cost))
                .then_with(|| left.ids.iter().cmp(right.ids.iter()))
        });
        expanded.dedup_by(|left, right| left.ids == right.ids);
        expanded.truncate(BEAM_WIDTH);
        states = expanded;
    }
    states
        .into_iter()
        .max_by(|left, right| {
            left.score
                .cmp(&right.score)
                .then_with(|| right.cost.cmp(&left.cost))
                .then_with(|| right.ids.iter().cmp(left.ids.iter()))
        })
        .map(|state| state.ids)
        .unwrap_or_default()
}

/// Compile a deterministic, cost-aware next-batch plan across fidelity levels.
pub fn plan_glioma_multi_fidelity_optimization(
    request: &MultiFidelityOptimizationRequest,
    candidates: &[FidelityCandidate],
    observations: &[FidelityObservation],
) -> Result<MultiFidelityOptimizationPlan, MultiFidelityOptimizationError> {
    validate_request(request)?;
    let candidates = validate_candidates(candidates, request)?;
    let grouped = validate_observations(observations, &candidates)?;
    let mut aggregates = BTreeMap::<String, Aggregate>::new();
    for (candidate_id, rows) in &grouped {
        aggregates.insert(candidate_id.clone(), aggregate(rows));
    }
    let mut by_design = BTreeMap::<String, BTreeMap<FidelityLevel, Aggregate>>::new();
    for (candidate_id, aggregate) in &aggregates {
        let candidate = &candidates[candidate_id];
        by_design
            .entry(candidate.design_id.clone())
            .or_default()
            .insert(candidate.fidelity, *aggregate);
    }
    let mut calibrations = Vec::new();
    for lower in [FidelityLevel::Screening, FidelityLevel::Mechanistic] {
        for higher in [FidelityLevel::Mechanistic, FidelityLevel::Validation] {
            if lower.ordinal() < higher.ordinal() {
                calibrations.push(calibration_for_pair(lower, higher, &by_design));
            }
        }
    }
    calibrations
        .sort_by_key(|calibration| (calibration.lower_fidelity, calibration.higher_fidelity));
    let calibration_map = calibrations
        .iter()
        .map(|calibration| {
            (
                (calibration.lower_fidelity, calibration.higher_fidelity),
                calibration,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let best_observed = aggregates
        .values()
        .map(|value| value.mean)
        .reduce(|left, right| match request.direction {
            OptimizationDirection::Maximize => left.max(right),
            OptimizationDirection::Minimize => left.min(right),
        });
    let baseline = request.baseline_milli.or(best_observed).unwrap_or(0);
    let mut uncertainty = BTreeSet::new();
    let mut negative_evidence = BTreeSet::new();
    let mut scored = Vec::with_capacity(candidates.len());
    for candidate in candidates.values() {
        let direct = aggregates.get(&candidate.candidate_id).copied();
        let parent = candidate
            .parent_candidate_id
            .as_ref()
            .and_then(|parent_id| aggregates.get(parent_id).copied());
        let transfer = candidate
            .parent_candidate_id
            .as_ref()
            .and_then(|parent_id| {
                let parent_candidate = candidates.get(parent_id)?;
                let calibration =
                    calibration_map.get(&(parent_candidate.fidelity, candidate.fidelity))?;
                if calibration.reliability_milli < request.min_transfer_reliability_milli {
                    return None;
                }
                parent.map(|aggregate| {
                    (
                        clamp_i64(
                            i128::from(aggregate.mean)
                                .saturating_add(i128::from(calibration.bias_milli)),
                        ),
                        aggregate
                            .uncertainty
                            .saturating_add(calibration.residual_milli),
                        calibration.reliability_milli,
                    )
                })
            });
        let mut neighbor_values = Vec::new();
        for other in candidates.values() {
            if other.candidate_id == candidate.candidate_id
                || other.fidelity != candidate.fidelity
                || other.model_system != candidate.model_system
            {
                continue;
            }
            if let Some(other_aggregate) = aggregates.get(&other.candidate_id) {
                let distance = coordinate_distance_milli(candidate, other);
                let weight = SCORE_SCALE.saturating_sub(distance);
                if weight > 0 {
                    neighbor_values.push((
                        other_aggregate.mean,
                        weight,
                        other_aggregate.uncertainty,
                    ));
                }
            }
        }
        let (mean, uncertainty_milli, source, transfer_value) = if let Some(direct) = direct {
            (direct.mean, direct.uncertainty, EstimateSource::Observed, 0)
        } else if let Some((mean, uncertainty_milli, reliability)) = transfer {
            (
                mean,
                uncertainty_milli,
                EstimateSource::Transferred,
                u64::from(reliability),
            )
        } else if !neighbor_values.is_empty() {
            let values = neighbor_values
                .iter()
                .map(|(mean, weight, _)| (*mean, (*weight).max(1)))
                .collect::<Vec<_>>();
            let spread = neighbor_values
                .iter()
                .map(|(other_mean, _, other_uncertainty)| {
                    abs_i128(i128::from(*other_mean) - i128::from(weighted_mean(&values)))
                        .saturating_add(u128::from(*other_uncertainty)) as u64
                })
                .max()
                .unwrap_or(0);
            (
                weighted_mean(&values),
                spread,
                EstimateSource::Neighborhood,
                0,
            )
        } else {
            uncertainty.insert(format!("{}:prior-only-estimate", candidate.candidate_id));
            (baseline, SCORE_SCALE, EstimateSource::Prior, 0)
        };
        let expected_improvement =
            normalized_improvement(request.direction, mean, uncertainty_milli, baseline)
                .min(SCORE_SCALE);
        let exploration = uncertainty_milli.min(SCORE_SCALE);
        let mut estimate = FidelityEstimate {
            candidate_id: candidate.candidate_id.clone(),
            design_id: candidate.design_id.clone(),
            fidelity: candidate.fidelity,
            model_system: candidate.model_system,
            observed_replicates: direct.map(|value| value.replicates).unwrap_or(0),
            posterior_mean_milli: mean,
            posterior_uncertainty_milli: uncertainty_milli,
            expected_improvement_milli: expected_improvement,
            exploration_milli: exploration,
            transfer_value_milli: transfer_value.min(SCORE_SCALE),
            acquisition_milli: 0,
            source,
        };
        estimate.acquisition_milli = acquisition(request, &estimate, candidate);
        let replicate_cap_reached = direct
            .map(|value| value.replicates >= candidate.max_replicates)
            .unwrap_or(false);
        let parent_support_ok =
            if direct.is_some() || candidate.fidelity == FidelityLevel::Screening {
                true
            } else {
                candidate
                    .parent_candidate_id
                    .as_ref()
                    .and_then(|parent_id| aggregates.get(parent_id))
                    .is_some_and(|aggregate| {
                        aggregate.replicates >= request.min_replicates_per_candidate
                    })
                    && calibration_map
                        .get(&(
                            candidates[candidate.parent_candidate_id.as_ref().unwrap()].fidelity,
                            candidate.fidelity,
                        ))
                        .is_some_and(|calibration| {
                            calibration.reliability_milli >= request.min_transfer_reliability_milli
                        })
            };
        let mut block_reason = None;
        if candidate.risk_milli > request.max_risk_milli {
            block_reason = Some("risk-ceiling-exceeded".into());
        } else if u64::from(candidate.cost_units) > request.budget_units {
            block_reason = Some("single-candidate-cost-exceeds-budget".into());
        } else if replicate_cap_reached {
            block_reason = Some("replicate-cap-reached".into());
        } else if !parent_support_ok {
            block_reason = Some("lower-fidelity-support-not-qualified".into());
            uncertainty.insert(format!(
                "{}:await-lower-fidelity-support",
                candidate.candidate_id
            ));
        }
        if estimate.source == EstimateSource::Prior {
            negative_evidence.insert(format!("{}:no-observed-support", candidate.candidate_id));
        }
        scored.push(ScoredCandidate {
            candidate: candidate.clone(),
            estimate,
            eligible: block_reason.is_none(),
            block_reason,
        });
    }
    let selected_set = select_beam(&scored, request);
    let candidate_order = candidates.keys().cloned().collect::<Vec<_>>();
    let observed_order = aggregates.keys().cloned().collect::<Vec<_>>();
    let mut selected_order = selected_set.iter().cloned().collect::<Vec<_>>();
    selected_order.sort();
    let mut deferred_order = Vec::new();
    let mut blocked_order = Vec::new();
    let mut estimates = Vec::new();
    let mut budget_used = 0_u64;
    for entry in scored {
        if selected_set.contains(&entry.candidate.candidate_id) {
            budget_used = budget_used.saturating_add(u64::from(entry.candidate.cost_units));
        } else if entry.eligible {
            deferred_order.push(entry.candidate.candidate_id.clone());
        } else {
            blocked_order.push(entry.candidate.candidate_id.clone());
            if let Some(reason) = entry.block_reason {
                negative_evidence.insert(format!("{}:{reason}", entry.candidate.candidate_id));
            }
        }
        estimates.push(entry.estimate);
    }
    deferred_order.sort();
    blocked_order.sort();
    estimates.sort_by(|left, right| left.candidate_id.cmp(&right.candidate_id));
    if selected_order.is_empty() {
        negative_evidence.insert("no-eligible-candidate-selected-under-budget".into());
    }
    let disposition = if selected_order.is_empty() && !blocked_order.is_empty() {
        MultiFidelityDisposition::NoEligibleCandidates
    } else if selected_order.is_empty() {
        MultiFidelityDisposition::BudgetBlocked
    } else if estimates
        .iter()
        .any(|estimate| estimate.source == EstimateSource::Prior)
        || !blocked_order.is_empty()
    {
        MultiFidelityDisposition::Partial
    } else {
        MultiFidelityDisposition::Qualified
    };
    let mut plan = MultiFidelityOptimizationPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        direction: request.direction,
        candidate_order,
        observed_order,
        selected_order,
        deferred_order,
        blocked_order,
        estimates,
        calibrations,
        budget_units: request.budget_units,
        budget_used_units: budget_used,
        budget_remaining_units: request.budget_units.saturating_sub(budget_used),
        best_observed_milli: best_observed,
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-multi-fidelity"),
    };
    plan.digest = ContentHash::of_value(&digest_input(&plan))
        .map_err(|error| MultiFidelityOptimizationError::Digest(error.to_string()))?;
    plan.validate()?;
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> MultiFidelityOptimizationRequest {
        MultiFidelityOptimizationRequest {
            objective: "optimize preclinical invasion suppression".into(),
            direction: OptimizationDirection::Maximize,
            budget_units: 8,
            max_selections: 1,
            min_replicates_per_candidate: 1,
            exploration_weight_milli: 250,
            exploitation_weight_milli: 500,
            transfer_weight_milli: 250,
            risk_penalty_milli: 100,
            cost_penalty_milli: 10,
            max_risk_milli: 800,
            min_transfer_reliability_milli: 200,
            baseline_milli: None,
        }
    }

    fn candidate(
        id: &str,
        design: &str,
        fidelity: FidelityLevel,
        parent: Option<&str>,
        cost: u32,
        dose: u32,
    ) -> FidelityCandidate {
        FidelityCandidate {
            candidate_id: id.into(),
            design_id: design.into(),
            fidelity,
            model_system: match fidelity {
                FidelityLevel::Screening => GliomaModelSystem::CellLine,
                FidelityLevel::Mechanistic => GliomaModelSystem::Organoid,
                FidelityLevel::Validation => GliomaModelSystem::MouseModel,
            },
            dose_milli: dose,
            combination_milli: 0,
            cost_units: cost,
            risk_milli: 100,
            parent_candidate_id: parent.map(str::to_owned),
            max_replicates: 4,
        }
    }

    fn observation(id: &str, candidate_id: &str, outcome: i64) -> FidelityObservation {
        FidelityObservation {
            observation_id: id.into(),
            candidate_id: candidate_id.into(),
            replicate_index: 0,
            outcome_milli: outcome,
            uncertainty_milli: 10,
            artifact: LocalArtifactRef {
                artifact_id: format!("local:{id}"),
                content_hash: ContentHash::of_bytes(id.as_bytes()),
                content_type: "application/vnd.aurora.glioma-fidelity+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
        }
    }

    #[test]
    fn transfer_calibration_unlocks_a_validation_candidate() {
        let candidates = vec![
            candidate(
                "screen-a",
                "design-a",
                FidelityLevel::Screening,
                None,
                1,
                100,
            ),
            candidate(
                "screen-b",
                "design-b",
                FidelityLevel::Screening,
                None,
                1,
                200,
            ),
            candidate(
                "valid-a",
                "design-a",
                FidelityLevel::Validation,
                Some("screen-a"),
                4,
                100,
            ),
            candidate(
                "valid-b",
                "design-b",
                FidelityLevel::Validation,
                Some("screen-b"),
                4,
                200,
            ),
        ];
        let observations = vec![
            observation("obs-screen-a", "screen-a", 400),
            observation("obs-screen-b", "screen-b", 300),
            observation("obs-valid-b", "valid-b", 500),
        ];
        let plan = plan_glioma_multi_fidelity_optimization(&request(), &candidates, &observations)
            .unwrap();
        assert_eq!(plan.selected_order, vec!["valid-a"]);
        assert_eq!(plan.disposition, MultiFidelityDisposition::Qualified);
        assert_eq!(plan.calibrations[1].pair_count, 1);
        assert_eq!(plan.estimates[2].source, EstimateSource::Transferred);
        plan.validate().unwrap();
    }

    #[test]
    fn validation_without_lower_fidelity_support_is_blocked() {
        let candidates = vec![
            candidate(
                "screen-a",
                "design-a",
                FidelityLevel::Screening,
                None,
                1,
                100,
            ),
            candidate(
                "valid-a",
                "design-a",
                FidelityLevel::Validation,
                Some("screen-a"),
                4,
                100,
            ),
        ];
        let plan = plan_glioma_multi_fidelity_optimization(&request(), &candidates, &[]).unwrap();
        assert!(plan.blocked_order.contains(&"valid-a".into()));
        assert_eq!(plan.selected_order, vec!["screen-a"]);
        assert!(plan
            .uncertainty
            .iter()
            .any(|reason| reason.contains("await-lower-fidelity")));
    }

    #[test]
    fn input_permutation_replays_identically_and_null_support_is_visible() {
        let mut screen_a = candidate(
            "screen-a",
            "design-a",
            FidelityLevel::Screening,
            None,
            1,
            100,
        );
        screen_a.model_system = GliomaModelSystem::Organoid;
        let candidates = vec![
            screen_a,
            candidate(
                "screen-b",
                "design-b",
                FidelityLevel::Screening,
                None,
                1,
                900,
            ),
        ];
        let observations = vec![observation("obs-b", "screen-b", 100)];
        let first = plan_glioma_multi_fidelity_optimization(&request(), &candidates, &observations)
            .unwrap();
        let second = plan_glioma_multi_fidelity_optimization(
            &request(),
            &candidates,
            &[observations[0].clone()],
        )
        .unwrap();
        assert_eq!(first, second);
        assert!(first
            .negative_evidence
            .iter()
            .any(|reason| reason.contains("no-observed-support")));
    }
}

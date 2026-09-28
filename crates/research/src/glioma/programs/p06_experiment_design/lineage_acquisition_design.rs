//! Candidate-specific expected reduction in uncertainty of glioma lineage-propagation operators.
//!
//! P10-F02 supplies paired control/treatment operator draws from an independent-unit bootstrap.
//! Each assay candidate supplies calibrated outcome likelihoods under each draw. This module targets
//! posterior variance of the treatment-minus-control operator contrasts per assay cost and does
//! not infer biology from an assay label.

use super::state_stratified_campaign::{
    CandidateSpecificAcquisitionPriority, StratifiedAssayCandidate,
};
use crate::glioma::programs::p10_interpretation_replication::lineage_propagation::{
    LineagePropagationAnalysis, LineagePropagationDisposition,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = super::adaptive_information_campaign::FEATURE_ID;
pub const OUTPUT_SCHEMA: &str = "GliomaLineageAssayAcquisition1@5";
const PROBABILITY_MILLION: u64 = 1_000_000;
const LIKELIHOOD_MILLI: u64 = 1_000;
const VARIANCE_PRECISION: u128 = 1_000_000;
const MAX_CANDIDATES: usize = 4_096;
const MAX_JOINT_CANDIDATES: usize = 64;
const MAX_JOINT_OUTCOMES_PER_ACTION: usize = 32;
const MAX_JOINT_ACQUISITION_WORK: u128 = 50_000_000;
const MAX_ACQUISITION_WORK: u128 = 250_000_000;
const MAX_ROLLOUT_CANDIDATES: usize = 64;
const MAX_ROLLOUT_WORK: u128 = 50_000_000;
/// Refuse sequential updates retaining less than 10% of the finite bootstrap ensemble by ESS.
pub const MIN_POSTERIOR_EFFECTIVE_SAMPLE_FRACTION_MILLI: u16 = 100;

/// Calibrated outcome likelihoods for one candidate, indexed in the exact P10 bootstrap-draw order.
/// For each draw, probabilities across this model's outcomes must sum to 1,000.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineagePropagationAssayResponseModel {
    pub action_id: String,
    pub analysis_digest: ContentHash,
    pub model_version: String,
    /// Content hash of the local preclinical calibration evidence used for these likelihoods.
    pub calibration_source_digest: ContentHash,
    pub calibration_unit_count: u32,
    pub outcomes: Vec<LineagePropagationOutcomeLikelihood>,
}

/// Calibrated joint outcome likelihoods for a pair of distinct, single-use assays.
///
/// Action IDs are stored in lexical order. For each P10 draw, probabilities over the Cartesian
/// product of the two assays' declared outcomes sum to 1,000. Pair marginals must exactly match
/// the corresponding single-assay response models. Sequential campaigns use this distribution
/// as a first-order Markov observation model conditioned on the most recent assay and outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineagePropagationJointAssayResponseModel {
    pub first_action_id: String,
    pub second_action_id: String,
    pub analysis_digest: ContentHash,
    pub model_version: String,
    /// Content hash of the paired local preclinical calibration evidence.
    pub calibration_source_digest: ContentHash,
    pub calibration_unit_count: u32,
    pub outcomes: Vec<LineagePropagationJointOutcomeLikelihood>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineagePropagationJointOutcomeLikelihood {
    pub first_outcome_id: String,
    pub second_outcome_id: String,
    pub likelihood_milli_by_bootstrap_draw: Vec<u16>,
}

/// The latest observed assay needed to resume a first-order joint-response campaign.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineagePropagationObservedAssay {
    pub action_id: String,
    pub outcome_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineagePropagationResponseDependence {
    ConditionalIndependence,
    FirstOrderPairCalibration,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineagePropagationOutcomeLikelihood {
    pub outcome_id: String,
    pub likelihood_milli_by_bootstrap_draw: Vec<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineagePropagationOutcomeProbability {
    pub outcome_id: String,
    pub probability_million: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineagePropagationAssayScore {
    pub action_id: String,
    pub state_id: String,
    pub calibration_source_digest: ContentHash,
    pub model_version: String,
    pub cost_units: u32,
    /// P06's estimated probability that this assay will successfully produce an interpretable result.
    pub feasibility_milli: u16,
    /// Fraction of the target-weighted treatment-minus-control coefficient variance expected to be removed.
    pub expected_variance_reduction_milli: u16,
    /// Conditional variance-reduction milli-points per assay cost unit, scaled by 1,000,000.
    pub reduction_per_cost_million: u64,
    /// Conditional information value multiplied by feasibility: expected realized variance-
    /// reduction milli-points per assay cost unit, scaled by 1,000,000.
    pub expected_realized_reduction_per_cost_million: u64,
    /// Variance and expected residual variance are in coefficient-ppm², scaled by 1,000,000.
    pub prior_operator_variance_q: u128,
    pub expected_posterior_operator_variance_q: u128,
    /// Minimum predicted ESS / particle count over outcomes with nonzero predictive probability,
    /// expressed in milli-fractions (1,000 means the full ensemble remains effective).
    pub minimum_predicted_effective_sample_fraction_milli: u16,
    pub predicted_outcome_probabilities: Vec<LineagePropagationOutcomeProbability>,
}

/// Research estimand weights for destination-state components of the treatment-minus-control
/// propagation operator. The vector is aligned to `LineagePropagationAnalysis.state_order`;
/// zero excludes a destination, while nonzero weights define a normalized linear composite over
/// the included destinations. The default averages all destinations equally.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineagePropagationAcquisitionTarget {
    pub destination_state_order: Vec<String>,
    pub destination_state_weights_milli: Vec<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineagePropagationAcquisitionPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub analysis_digest: ContentHash,
    /// Linear destination-state estimand whose full covariance is used to calculate candidate value.
    pub target: LineagePropagationAcquisitionTarget,
    /// Dependence assumption used to predict outcomes and update the P10 posterior.
    pub response_dependence: LineagePropagationResponseDependence,
    /// These exported candidate scores are immediate one-step diagnostics. The live instrument
    /// campaign may select with a two-assay rollout and records that horizon on each campaign round.
    /// In joint mode, the previous observation conditioning the current candidate scores.
    pub conditioned_on: Option<LineagePropagationObservedAssay>,
    /// Current posterior mass over the exported P10 bootstrap draws, totaling 1,000,000.
    pub particle_weights_million: Vec<u32>,
    pub candidate_order: Vec<String>,
    pub scores: Vec<LineagePropagationAssayScore>,
    pub digest: ContentHash,
}

/// Mutable, run-local sequential posterior over P10 bootstrap draws. The P06 campaign asks it for
/// the current candidate value each round, then assimilates the exact local assay outcome before
/// the next acquisition decision.
#[derive(Debug, Clone)]
pub struct LineagePropagationAcquisitionPolicy {
    analysis: LineagePropagationAnalysis,
    target: LineagePropagationAcquisitionTarget,
    response_models: BTreeMap<String, LineagePropagationAssayResponseModel>,
    joint_response_models:
        Option<BTreeMap<(String, String), LineagePropagationJointAssayResponseModel>>,
    last_observation: Option<LineagePropagationObservedAssay>,
    response_history: Vec<LineagePropagationObservedAssay>,
    particle_weights_million: Vec<u32>,
    max_rounds: u16,
    minimum_effective_sample_fraction_milli: u16,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LineagePropagationAcquisitionError {
    #[error("lineage acquisition request is invalid: {0}")]
    InvalidRequest(String),
    #[error("lineage acquisition input is invalid: {0}")]
    InvalidInput(String),
    #[error("lineage acquisition outcome is outside the supported propagation model")]
    OutOfModelOutcome,
    #[error(
        "lineage propagation posterior is unstable: effective sample fraction {effective_sample_fraction_milli} milli is below required {minimum_milli} milli"
    )]
    LowEffectiveSampleSize {
        effective_sample_fraction_milli: u16,
        minimum_milli: u16,
    },
    #[error("lineage acquisition output is invalid: {0}")]
    InvalidOutput(String),
    #[error("lineage acquisition digest failed: {0}")]
    Digest(String),
}

fn digest_input(plan: &LineagePropagationAcquisitionPlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": plan.feature_id,
        "output_schema": plan.output_schema,
        "analysis_digest": plan.analysis_digest,
        "target": plan.target,
        "response_dependence": plan.response_dependence,
        "conditioned_on": plan.conditioned_on,
        "particle_weights_million": plan.particle_weights_million,
        "candidate_order": plan.candidate_order,
        "scores": plan.scores,
    })
}

fn uniform_particle_weights(count: usize) -> Vec<u32> {
    let base = PROBABILITY_MILLION / count as u64;
    let remainder = PROBABILITY_MILLION % count as u64;
    (0..count)
        .map(|index| (base + u64::from((index as u64) < remainder)) as u32)
        .collect()
}

fn uniform_destination_weights(
    analysis: &LineagePropagationAnalysis,
) -> LineagePropagationAcquisitionTarget {
    LineagePropagationAcquisitionTarget {
        destination_state_order: analysis.state_order.clone(),
        destination_state_weights_milli: vec![1_000; analysis.state_order.len()],
    }
}

fn validate_acquisition_target(
    analysis: &LineagePropagationAnalysis,
    target: &LineagePropagationAcquisitionTarget,
) -> Result<(), LineagePropagationAcquisitionError> {
    let weights = &target.destination_state_weights_milli;
    if target.destination_state_order != analysis.state_order
        || weights.len() != analysis.state_order.len()
        || weights.iter().any(|weight| *weight > 1_000)
        || weights.iter().all(|weight| *weight == 0)
    {
        return Err(LineagePropagationAcquisitionError::InvalidRequest(
            "destination-state target weights must align with P10 state order, lie in 0..=1000 milli, and include at least one nonzero target".into(),
        ));
    }
    Ok(())
}

fn weighted_operator_variance_q(
    values_by_dimension: &[Vec<i64>],
    particle_weights_million: &[u32],
    destination_state_weights_milli: &[u16],
) -> u128 {
    if values_by_dimension.len() != destination_state_weights_milli.len()
        || values_by_dimension
            .iter()
            .any(|values| values.len() != particle_weights_million.len())
    {
        return 0;
    }
    let total_target_weight = destination_state_weights_milli
        .iter()
        .map(|weight| u128::from(*weight))
        .sum::<u128>();
    if total_target_weight == 0 {
        return 0;
    }

    // Form the scalar target separately for every paired bootstrap draw before taking variance.
    // This retains cross-destination covariance: summing marginal variances can badly misstate
    // uncertainty when destination-state effects move together or in opposite directions.
    let target_values = (0..particle_weights_million.len())
        .map(|particle_index| {
            let weighted_sum = values_by_dimension
                .iter()
                .zip(destination_state_weights_milli)
                .map(|(values, target_weight)| {
                    i128::from(values[particle_index]) * i128::from(*target_weight)
                })
                .sum::<i128>();
            // P10 has at most eight states and each coefficient is a bounded ppm estimate, so
            // this product is comfortably within i64 even at the declared maximum weight.
            weighted_sum as i64
        })
        .collect::<Vec<_>>();
    weighted_variance_q(&target_values, particle_weights_million)
        / total_target_weight.saturating_mul(total_target_weight)
}

fn source_operator_values_by_dimension(
    analysis: &LineagePropagationAnalysis,
    source_index: usize,
) -> Vec<Vec<i64>> {
    let states = analysis.state_order.len();
    (0..states)
        .map(|destination| {
            analysis
                .bootstrap_draws
                .iter()
                .map(|draw| {
                    draw.treatment_coefficients_ppm[destination * states + source_index] as i64
                        - draw.control_coefficients_ppm[destination * states + source_index] as i64
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn normalize_weights_million(
    raw_weights: &[u128],
) -> Result<Vec<u32>, LineagePropagationAcquisitionError> {
    let total = raw_weights.iter().sum::<u128>();
    if total == 0 {
        return Err(LineagePropagationAcquisitionError::OutOfModelOutcome);
    }
    let mut assigned = 0_u128;
    let mut normalized = raw_weights
        .iter()
        .enumerate()
        .map(|(index, weight)| {
            let numerator = *weight * u128::from(PROBABILITY_MILLION);
            let floor = numerator / total;
            assigned += floor;
            (index, floor, numerator % total)
        })
        .collect::<Vec<_>>();
    let remainder_count = (u128::from(PROBABILITY_MILLION) - assigned) as usize;
    let mut order = (0..normalized.len()).collect::<Vec<_>>();
    order.sort_by(|left, right| {
        normalized[*right]
            .2
            .cmp(&normalized[*left].2)
            .then_with(|| normalized[*left].0.cmp(&normalized[*right].0))
    });
    for index in order.into_iter().take(remainder_count) {
        normalized[index].1 += 1;
    }
    Ok(normalized
        .into_iter()
        .map(|(_, weight, _)| weight as u32)
        .collect())
}

fn weighted_variance_q(values: &[i64], weights: &[u32]) -> u128 {
    let total = weights
        .iter()
        .map(|weight| u128::from(*weight))
        .sum::<u128>();
    if total == 0 || values.len() != weights.len() {
        return 0;
    }
    let first_moment = values
        .iter()
        .zip(weights)
        .map(|(value, weight)| i128::from(*value) * i128::from(*weight))
        .sum::<i128>();
    let second_moment = values
        .iter()
        .zip(weights)
        .map(|(value, weight)| {
            let magnitude = i128::from(*value).unsigned_abs();
            magnitude * magnitude * u128::from(*weight)
        })
        .sum::<u128>();
    let centered = second_moment.saturating_mul(total).saturating_sub(
        first_moment
            .unsigned_abs()
            .saturating_mul(first_moment.unsigned_abs()),
    );
    centered.saturating_mul(VARIANCE_PRECISION) / total.saturating_mul(total)
}

fn effective_sample_fraction_milli(weights: &[u32]) -> u16 {
    if weights.is_empty() {
        return 0;
    }
    let total = weights
        .iter()
        .map(|weight| u128::from(*weight))
        .sum::<u128>();
    let sum_squares = weights
        .iter()
        .map(|weight| u128::from(*weight) * u128::from(*weight))
        .sum::<u128>();
    if total == 0 || sum_squares == 0 {
        return 0;
    }
    (total.saturating_mul(total).saturating_mul(1_000)
        / (sum_squares.saturating_mul(weights.len() as u128)))
    .min(1_000) as u16
}

fn validate_models(
    analysis: &LineagePropagationAnalysis,
    candidates: &[StratifiedAssayCandidate],
    models: &[LineagePropagationAssayResponseModel],
    max_rounds: u16,
) -> Result<(), LineagePropagationAcquisitionError> {
    if analysis.disposition != LineagePropagationDisposition::Qualified {
        return Err(LineagePropagationAcquisitionError::InvalidInput(
            "only a qualified P10 lineage-propagation model can guide assay acquisition".into(),
        ));
    }
    analysis
        .validate()
        .map_err(|error| LineagePropagationAcquisitionError::InvalidInput(error.to_string()))?;
    if candidates.is_empty()
        || candidates.len() > MAX_CANDIDATES
        || models.len() != candidates.len()
        || max_rounds == 0
    {
        return Err(LineagePropagationAcquisitionError::InvalidRequest(
            "candidate/model coverage, candidate count, or campaign horizon is invalid".into(),
        ));
    }
    let model_by_action = models
        .iter()
        .map(|model| (model.action_id.as_str(), model))
        .collect::<BTreeMap<_, _>>();
    let candidate_ids = candidates
        .iter()
        .map(|candidate| candidate.action.action_id.as_str())
        .collect::<BTreeSet<_>>();
    if model_by_action.len() != models.len()
        || candidate_ids.len() != candidates.len()
        || candidate_ids != model_by_action.keys().copied().collect::<BTreeSet<_>>()
    {
        return Err(LineagePropagationAcquisitionError::InvalidInput(
            "every unique P06 candidate must have exactly one calibrated response model".into(),
        ));
    }

    let draws = analysis.bootstrap_draws.len() as u128;
    let mut work = 0_u128;
    for candidate in candidates {
        let action = &candidate.action;
        let model = model_by_action[action.action_id.as_str()];
        if !analysis.state_order.contains(&candidate.stratum_id)
            || action.cost_units == 0
            || action.feasibility_milli > 1_000
            || action.outcomes.len() < 2
            || model.analysis_digest != analysis.digest
            || model.model_version.trim().is_empty()
            || model.calibration_unit_count < 2
            || model.outcomes.len() != action.outcomes.len()
        {
            return Err(LineagePropagationAcquisitionError::InvalidInput(format!(
                "candidate {} lacks a matching state, assay cost/outcomes, P10 digest, or calibration model",
                action.action_id
            )));
        }
        let declared_outcomes = action
            .outcomes
            .iter()
            .map(|outcome| outcome.outcome_id.as_str())
            .collect::<BTreeSet<_>>();
        let modeled_outcomes = model
            .outcomes
            .iter()
            .map(|outcome| outcome.outcome_id.as_str())
            .collect::<BTreeSet<_>>();
        if declared_outcomes.len() != action.outcomes.len()
            || declared_outcomes != modeled_outcomes
            || model.outcomes.iter().any(|outcome| {
                outcome.outcome_id.trim().is_empty()
                    || outcome.likelihood_milli_by_bootstrap_draw.len()
                        != analysis.bootstrap_draws.len()
                    || outcome
                        .likelihood_milli_by_bootstrap_draw
                        .iter()
                        .any(|likelihood| u64::from(*likelihood) > LIKELIHOOD_MILLI)
            })
        {
            return Err(LineagePropagationAcquisitionError::InvalidInput(format!(
                "candidate {} outcome IDs or bootstrap likelihood dimensions do not match",
                action.action_id
            )));
        }
        for draw_index in 0..analysis.bootstrap_draws.len() {
            let probability_sum = model
                .outcomes
                .iter()
                .map(|outcome| u64::from(outcome.likelihood_milli_by_bootstrap_draw[draw_index]))
                .sum::<u64>();
            if probability_sum != LIKELIHOOD_MILLI {
                return Err(LineagePropagationAcquisitionError::InvalidInput(format!(
                    "candidate {} likelihoods for bootstrap draw {draw_index} must sum to 1000",
                    action.action_id
                )));
            }
        }
        let coefficient_dimensions = (2 * analysis.state_order.len()) as u128;
        work = work.saturating_add(
            draws
                .saturating_mul(action.outcomes.len() as u128)
                .saturating_mul(coefficient_dimensions)
                .saturating_mul(u128::from(max_rounds)),
        );
    }
    if work > MAX_ACQUISITION_WORK {
        return Err(LineagePropagationAcquisitionError::InvalidRequest(format!(
            "bounded sequential acquisition scoring requires {work} evaluations, above {MAX_ACQUISITION_WORK}"
        )));
    }
    Ok(())
}

fn validate_response_model_probabilities(
    model: &LineagePropagationAssayResponseModel,
    particle_count: usize,
) -> Result<(), LineagePropagationAcquisitionError> {
    let outcome_ids = model
        .outcomes
        .iter()
        .map(|outcome| outcome.outcome_id.as_str())
        .collect::<BTreeSet<_>>();
    if model.action_id.trim().is_empty()
        || model.model_version.trim().is_empty()
        || model.calibration_unit_count < 2
        || model.outcomes.len() < 2
        || outcome_ids.len() != model.outcomes.len()
        || model.outcomes.iter().any(|outcome| {
            outcome.outcome_id.trim().is_empty()
                || outcome.likelihood_milli_by_bootstrap_draw.len() != particle_count
                || outcome
                    .likelihood_milli_by_bootstrap_draw
                    .iter()
                    .any(|likelihood| u64::from(*likelihood) > LIKELIHOOD_MILLI)
        })
        || (0..particle_count).any(|draw| {
            model
                .outcomes
                .iter()
                .map(|outcome| u64::from(outcome.likelihood_milli_by_bootstrap_draw[draw]))
                .sum::<u64>()
                != LIKELIHOOD_MILLI
        })
    {
        return Err(LineagePropagationAcquisitionError::InvalidInput(
            "calibrated outcome likelihood model is malformed or not normalized per particle"
                .into(),
        ));
    }
    Ok(())
}

fn joint_pair_key(first_action_id: &str, second_action_id: &str) -> (String, String) {
    if first_action_id < second_action_id {
        (first_action_id.into(), second_action_id.into())
    } else {
        (second_action_id.into(), first_action_id.into())
    }
}

fn validate_joint_response_models(
    analysis: &LineagePropagationAnalysis,
    candidates: &[StratifiedAssayCandidate],
    response_models: &[LineagePropagationAssayResponseModel],
    joint_models: &[LineagePropagationJointAssayResponseModel],
) -> Result<
    BTreeMap<(String, String), LineagePropagationJointAssayResponseModel>,
    LineagePropagationAcquisitionError,
> {
    if candidates.len() < 2
        || candidates.len() > MAX_JOINT_CANDIDATES
        || candidates.iter().any(|candidate| {
            candidate.action.max_replicates != 1
                || candidate.action.outcomes.is_empty()
                || candidate.action.outcomes.len() > MAX_JOINT_OUTCOMES_PER_ACTION
        })
    {
        return Err(LineagePropagationAcquisitionError::InvalidRequest(
            format!(
                "joint response mode requires 2..={MAX_JOINT_CANDIDATES} single-use actions with 1..={MAX_JOINT_OUTCOMES_PER_ACTION} outcomes each"
            ),
        ));
    }

    let response_by_action = response_models
        .iter()
        .map(|model| (model.action_id.as_str(), model))
        .collect::<BTreeMap<_, _>>();
    let candidate_by_action = candidates
        .iter()
        .map(|candidate| (candidate.action.action_id.as_str(), candidate))
        .collect::<BTreeMap<_, _>>();
    let required_pair_count = candidates.len().saturating_mul(candidates.len() - 1) / 2;
    if joint_models.len() != required_pair_count {
        return Err(LineagePropagationAcquisitionError::InvalidInput(
            "joint response calibration must cover every unordered pair of candidate actions"
                .into(),
        ));
    }

    let mut normalized = BTreeMap::new();
    let mut total_work = 0_u128;
    for model in joint_models {
        let key = joint_pair_key(&model.first_action_id, &model.second_action_id);
        if model.first_action_id >= model.second_action_id
            || model.analysis_digest != analysis.digest
            || model.model_version.trim().is_empty()
            || model.calibration_unit_count < 2
            || model.outcomes.is_empty()
            || normalized.contains_key(&key)
        {
            return Err(LineagePropagationAcquisitionError::InvalidInput(
                "joint response models require a unique lexical action pair, matching P10 digest, version, and independent calibration units".into(),
            ));
        }
        let first_candidate = candidate_by_action
            .get(model.first_action_id.as_str())
            .ok_or_else(|| {
                LineagePropagationAcquisitionError::InvalidInput(
                    "joint response model references an unknown first action".into(),
                )
            })?;
        let second_candidate = candidate_by_action
            .get(model.second_action_id.as_str())
            .ok_or_else(|| {
                LineagePropagationAcquisitionError::InvalidInput(
                    "joint response model references an unknown second action".into(),
                )
            })?;
        let first_response = response_by_action[model.first_action_id.as_str()];
        let second_response = response_by_action[model.second_action_id.as_str()];
        let first_outcomes = first_candidate
            .action
            .outcomes
            .iter()
            .map(|outcome| outcome.outcome_id.as_str())
            .collect::<BTreeSet<_>>();
        let second_outcomes = second_candidate
            .action
            .outcomes
            .iter()
            .map(|outcome| outcome.outcome_id.as_str())
            .collect::<BTreeSet<_>>();
        let allowed_pairs = first_outcomes
            .iter()
            .flat_map(|first| second_outcomes.iter().map(move |second| (*first, *second)))
            .collect::<BTreeSet<_>>();
        let supplied_pairs = model
            .outcomes
            .iter()
            .map(|outcome| {
                (
                    outcome.first_outcome_id.as_str(),
                    outcome.second_outcome_id.as_str(),
                )
            })
            .collect::<BTreeSet<_>>();
        if allowed_pairs.len() != first_outcomes.len().saturating_mul(second_outcomes.len())
            || supplied_pairs.len() != model.outcomes.len()
            || supplied_pairs != allowed_pairs
            || model.outcomes.iter().any(|outcome| {
                outcome.first_outcome_id.trim().is_empty()
                    || outcome.second_outcome_id.trim().is_empty()
                    || outcome.likelihood_milli_by_bootstrap_draw.len()
                        != analysis.bootstrap_draws.len()
                    || outcome
                        .likelihood_milli_by_bootstrap_draw
                        .iter()
                        .any(|likelihood| u64::from(*likelihood) > LIKELIHOOD_MILLI)
            })
        {
            return Err(LineagePropagationAcquisitionError::InvalidInput(
                "joint response outcomes must uniquely cover the exact Cartesian product and P10 draw count".into(),
            ));
        }
        total_work = total_work.saturating_add(
            (analysis.bootstrap_draws.len() as u128).saturating_mul(model.outcomes.len() as u128),
        );
        for draw_index in 0..analysis.bootstrap_draws.len() {
            let joint_total = model
                .outcomes
                .iter()
                .map(|outcome| u64::from(outcome.likelihood_milli_by_bootstrap_draw[draw_index]))
                .sum::<u64>();
            if joint_total != LIKELIHOOD_MILLI {
                return Err(LineagePropagationAcquisitionError::InvalidInput(format!(
                    "joint likelihoods for action pair {} / {} and draw {draw_index} must sum to 1,000",
                    model.first_action_id, model.second_action_id
                )));
            }
            for expected in &first_response.outcomes {
                let marginal = model
                    .outcomes
                    .iter()
                    .filter(|outcome| outcome.first_outcome_id == expected.outcome_id)
                    .map(|outcome| {
                        u64::from(outcome.likelihood_milli_by_bootstrap_draw[draw_index])
                    })
                    .sum::<u64>();
                if marginal != u64::from(expected.likelihood_milli_by_bootstrap_draw[draw_index]) {
                    return Err(LineagePropagationAcquisitionError::InvalidInput(
                        "joint first-action marginals must exactly match single-assay calibration"
                            .into(),
                    ));
                }
            }
            for expected in &second_response.outcomes {
                let marginal = model
                    .outcomes
                    .iter()
                    .filter(|outcome| outcome.second_outcome_id == expected.outcome_id)
                    .map(|outcome| {
                        u64::from(outcome.likelihood_milli_by_bootstrap_draw[draw_index])
                    })
                    .sum::<u64>();
                if marginal != u64::from(expected.likelihood_milli_by_bootstrap_draw[draw_index]) {
                    return Err(LineagePropagationAcquisitionError::InvalidInput(
                        "joint second-action marginals must exactly match single-assay calibration"
                            .into(),
                    ));
                }
            }
        }
        normalized.insert(key, model.clone());
    }
    if total_work > MAX_JOINT_ACQUISITION_WORK {
        return Err(LineagePropagationAcquisitionError::InvalidRequest(format!(
            "joint response validation requires {total_work} draw/outcome evaluations, above {MAX_JOINT_ACQUISITION_WORK}"
        )));
    }
    Ok(normalized)
}

fn normalize_likelihoods_milli(raw: &[u64], outcome_ids: &[String]) -> Vec<u16> {
    let total = raw.iter().map(|value| u128::from(*value)).sum::<u128>();
    if total == 0 || raw.len() != outcome_ids.len() {
        return vec![0; raw.len()];
    }
    let mut assigned = 0_u64;
    let mut rows = raw
        .iter()
        .zip(outcome_ids)
        .enumerate()
        .map(|(index, (value, outcome_id))| {
            let scaled = u128::from(*value) * u128::from(LIKELIHOOD_MILLI);
            let floor = (scaled / total) as u64;
            assigned += floor;
            (index, floor, scaled % total, outcome_id.as_str())
        })
        .collect::<Vec<_>>();
    let mut order = (0..rows.len()).collect::<Vec<_>>();
    order.sort_by(|left, right| {
        rows[*right]
            .2
            .cmp(&rows[*left].2)
            .then_with(|| rows[*left].3.cmp(rows[*right].3))
    });
    for index in order
        .into_iter()
        .take((LIKELIHOOD_MILLI - assigned) as usize)
    {
        rows[index].1 += 1;
    }
    let mut normalized = vec![0_u16; raw.len()];
    for (index, value, _, _) in rows {
        normalized[index] = value as u16;
    }
    normalized
}

fn conditional_response_model(
    previous: &LineagePropagationObservedAssay,
    action_id: &str,
    response_models: &BTreeMap<String, LineagePropagationAssayResponseModel>,
    joint_models: &BTreeMap<(String, String), LineagePropagationJointAssayResponseModel>,
) -> Result<LineagePropagationAssayResponseModel, LineagePropagationAcquisitionError> {
    let marginal = response_models
        .get(action_id)
        .ok_or(LineagePropagationAcquisitionError::OutOfModelOutcome)?;
    // A single-use action cannot occur twice in an executable campaign. Keeping its marginal
    // model available lets end-of-run plans display all historical candidates consistently.
    if previous.action_id == action_id {
        return Ok(marginal.clone());
    }
    let key = joint_pair_key(&previous.action_id, action_id);
    let joint = joint_models
        .get(&key)
        .ok_or(LineagePropagationAcquisitionError::OutOfModelOutcome)?;
    let previous_is_first = previous.action_id == joint.first_action_id;
    let mut likelihoods_by_outcome = marginal
        .outcomes
        .iter()
        .map(|outcome| {
            (
                outcome.outcome_id.clone(),
                Vec::with_capacity(outcome.likelihood_milli_by_bootstrap_draw.len()),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let ordered_outcomes = marginal
        .outcomes
        .iter()
        .map(|outcome| outcome.outcome_id.clone())
        .collect::<Vec<_>>();

    for draw_index in 0..marginal.outcomes.first().map_or(0, |outcome| {
        outcome.likelihood_milli_by_bootstrap_draw.len()
    }) {
        let raw = marginal
            .outcomes
            .iter()
            .map(|outcome| {
                joint
                    .outcomes
                    .iter()
                    .find(|joint_outcome| {
                        if previous_is_first {
                            joint_outcome.first_outcome_id == previous.outcome_id
                                && joint_outcome.second_outcome_id == outcome.outcome_id
                        } else {
                            joint_outcome.second_outcome_id == previous.outcome_id
                                && joint_outcome.first_outcome_id == outcome.outcome_id
                        }
                    })
                    .map_or(0, |joint_outcome| {
                        u64::from(joint_outcome.likelihood_milli_by_bootstrap_draw[draw_index])
                    })
            })
            .collect::<Vec<_>>();
        let conditional = if raw.iter().all(|likelihood| *likelihood == 0) {
            // P(previous outcome | draw) is zero here, so Bayes assigns that draw zero posterior
            // mass. Retain a normalized fallback likelihood for schema validity only.
            normalize_likelihoods_milli(
                &marginal
                    .outcomes
                    .iter()
                    .map(|outcome| {
                        u64::from(outcome.likelihood_milli_by_bootstrap_draw[draw_index])
                    })
                    .collect::<Vec<_>>(),
                &ordered_outcomes,
            )
        } else {
            normalize_likelihoods_milli(&raw, &ordered_outcomes)
        };
        for (outcome_id, likelihood) in ordered_outcomes.iter().zip(conditional) {
            likelihoods_by_outcome
                .get_mut(outcome_id)
                .expect("outcome map was built from the same ordered outcomes")
                .push(likelihood);
        }
    }

    Ok(LineagePropagationAssayResponseModel {
        action_id: action_id.into(),
        analysis_digest: marginal.analysis_digest.clone(),
        model_version: format!(
            "{}|given:{}={}",
            joint.model_version, previous.action_id, previous.outcome_id
        ),
        calibration_source_digest: joint.calibration_source_digest.clone(),
        calibration_unit_count: joint.calibration_unit_count,
        outcomes: ordered_outcomes
            .into_iter()
            .map(|outcome_id| LineagePropagationOutcomeLikelihood {
                likelihood_milli_by_bootstrap_draw: likelihoods_by_outcome
                    .remove(&outcome_id)
                    .expect("outcome map was built from the same ordered outcomes"),
                outcome_id,
            })
            .collect(),
    })
}

/// Update the normalized posterior over P10 bootstrap operator draws after one observed assay.
/// A zero-mass observation is returned as a typed out-of-model result rather than smoothing away.
pub fn update_lineage_propagation_particle_weights(
    particle_weights_million: &[u32],
    response_model: &LineagePropagationAssayResponseModel,
    observed_outcome_id: &str,
) -> Result<Vec<u32>, LineagePropagationAcquisitionError> {
    validate_response_model_probabilities(response_model, particle_weights_million.len())?;
    if particle_weights_million.is_empty()
        || particle_weights_million.len()
            != response_model.outcomes.first().map_or(0, |outcome| {
                outcome.likelihood_milli_by_bootstrap_draw.len()
            })
        || particle_weights_million
            .iter()
            .map(|weight| u64::from(*weight))
            .sum::<u64>()
            != PROBABILITY_MILLION
    {
        return Err(LineagePropagationAcquisitionError::InvalidInput(
            "particle posterior must align with the calibrated model and sum to 1,000,000".into(),
        ));
    }
    let outcome = response_model
        .outcomes
        .iter()
        .find(|outcome| outcome.outcome_id == observed_outcome_id)
        .ok_or(LineagePropagationAcquisitionError::OutOfModelOutcome)?;
    let unnormalized = particle_weights_million
        .iter()
        .zip(&outcome.likelihood_milli_by_bootstrap_draw)
        .map(|(weight, likelihood)| u128::from(*weight) * u128::from(*likelihood))
        .collect::<Vec<_>>();
    normalize_weights_million(&unnormalized)
}

fn score_candidate(
    analysis: &LineagePropagationAnalysis,
    candidate: &StratifiedAssayCandidate,
    model: &LineagePropagationAssayResponseModel,
    particle_weights_million: &[u32],
    target: &LineagePropagationAcquisitionTarget,
) -> LineagePropagationAssayScore {
    let source_index = analysis
        .state_order
        .iter()
        .position(|state| state == &candidate.stratum_id)
        .expect("candidate state was validated");
    let values_by_dimension = source_operator_values_by_dimension(analysis, source_index);
    let prior_variance = weighted_operator_variance_q(
        &values_by_dimension,
        particle_weights_million,
        &target.destination_state_weights_milli,
    );
    let mut predicted_outcome_probabilities = Vec::with_capacity(model.outcomes.len());
    let mut outcome_masses_raw = Vec::with_capacity(model.outcomes.len());
    let mut minimum_predicted_effective_sample_fraction_milli = 1_000_u16;
    for outcome in &model.outcomes {
        let mass = particle_weights_million
            .iter()
            .zip(&outcome.likelihood_milli_by_bootstrap_draw)
            .map(|(weight, likelihood)| u128::from(*weight) * u128::from(*likelihood))
            .sum::<u128>();
        outcome_masses_raw.push(mass);
    }
    let outcome_probabilities = normalize_weights_million(&outcome_masses_raw)
        .expect("per-draw outcome probabilities sum to 1000");
    for (outcome, probability) in model.outcomes.iter().zip(outcome_probabilities) {
        predicted_outcome_probabilities.push(LineagePropagationOutcomeProbability {
            outcome_id: outcome.outcome_id.clone(),
            probability_million: probability,
        });
    }
    predicted_outcome_probabilities.sort_by(|left, right| left.outcome_id.cmp(&right.outcome_id));

    let expected_posterior_variance = model
        .outcomes
        .iter()
        .zip(&outcome_masses_raw)
        .map(|(outcome, raw_mass)| {
            if *raw_mass == 0 {
                return 0;
            }
            let posterior = particle_weights_million
                .iter()
                .zip(&outcome.likelihood_milli_by_bootstrap_draw)
                .map(|(weight, likelihood)| u128::from(*weight) * u128::from(*likelihood))
                .collect::<Vec<_>>();
            let posterior = normalize_weights_million(&posterior)
                .expect("positive outcome mass has a posterior");
            minimum_predicted_effective_sample_fraction_milli =
                minimum_predicted_effective_sample_fraction_milli
                    .min(effective_sample_fraction_milli(&posterior));
            let posterior_variance = weighted_operator_variance_q(
                &values_by_dimension,
                &posterior,
                &target.destination_state_weights_milli,
            );
            let probability_million = raw_mass / u128::from(LIKELIHOOD_MILLI);
            posterior_variance.saturating_mul(probability_million)
        })
        .sum::<u128>()
        / u128::from(PROBABILITY_MILLION);
    let expected_posterior_variance = expected_posterior_variance.min(prior_variance);
    let reduction = prior_variance.saturating_sub(expected_posterior_variance);
    let reduction_milli = if prior_variance == 0 {
        0
    } else {
        (reduction.saturating_mul(u128::from(LIKELIHOOD_MILLI)) / prior_variance)
            .min(u128::from(LIKELIHOOD_MILLI)) as u16
    };
    let reduction_per_cost_million = u64::from(reduction_milli).saturating_mul(PROBABILITY_MILLION)
        / u64::from(candidate.action.cost_units);
    let expected_realized_reduction_per_cost_million = reduction_per_cost_million
        .saturating_mul(u64::from(candidate.action.feasibility_milli))
        / LIKELIHOOD_MILLI;
    LineagePropagationAssayScore {
        action_id: candidate.action.action_id.clone(),
        state_id: candidate.stratum_id.clone(),
        calibration_source_digest: model.calibration_source_digest.clone(),
        model_version: model.model_version.clone(),
        cost_units: candidate.action.cost_units,
        feasibility_milli: candidate.action.feasibility_milli,
        expected_variance_reduction_milli: reduction_milli,
        reduction_per_cost_million,
        expected_realized_reduction_per_cost_million,
        prior_operator_variance_q: prior_variance,
        expected_posterior_operator_variance_q: expected_posterior_variance,
        minimum_predicted_effective_sample_fraction_milli,
        predicted_outcome_probabilities,
    }
}

impl LineagePropagationAcquisitionPolicy {
    pub fn new(
        analysis: LineagePropagationAnalysis,
        candidates: &[StratifiedAssayCandidate],
        response_models: &[LineagePropagationAssayResponseModel],
        posterior_particle_weights_million: &[u32],
        max_rounds: u16,
    ) -> Result<Self, LineagePropagationAcquisitionError> {
        let target = uniform_destination_weights(&analysis);
        Self::new_with_target_and_minimum_effective_sample_fraction(
            analysis,
            candidates,
            response_models,
            target,
            posterior_particle_weights_million,
            max_rounds,
            MIN_POSTERIOR_EFFECTIVE_SAMPLE_FRACTION_MILLI,
        )
    }

    pub fn new_with_minimum_effective_sample_fraction(
        analysis: LineagePropagationAnalysis,
        candidates: &[StratifiedAssayCandidate],
        response_models: &[LineagePropagationAssayResponseModel],
        posterior_particle_weights_million: &[u32],
        max_rounds: u16,
        minimum_effective_sample_fraction_milli: u16,
    ) -> Result<Self, LineagePropagationAcquisitionError> {
        let target = uniform_destination_weights(&analysis);
        Self::new_with_target_and_minimum_effective_sample_fraction(
            analysis,
            candidates,
            response_models,
            target,
            posterior_particle_weights_million,
            max_rounds,
            minimum_effective_sample_fraction_milli,
        )
    }

    /// Construct a sequential P06 selector for an explicit destination-state estimand.
    /// The target state order must exactly match the fitted P10 lineage analysis.
    pub fn new_with_target(
        analysis: LineagePropagationAnalysis,
        candidates: &[StratifiedAssayCandidate],
        response_models: &[LineagePropagationAssayResponseModel],
        target: LineagePropagationAcquisitionTarget,
        posterior_particle_weights_million: &[u32],
        max_rounds: u16,
    ) -> Result<Self, LineagePropagationAcquisitionError> {
        Self::new_with_target_and_minimum_effective_sample_fraction(
            analysis,
            candidates,
            response_models,
            target,
            posterior_particle_weights_million,
            max_rounds,
            MIN_POSTERIOR_EFFECTIVE_SAMPLE_FRACTION_MILLI,
        )
    }

    pub fn new_with_target_and_minimum_effective_sample_fraction(
        analysis: LineagePropagationAnalysis,
        candidates: &[StratifiedAssayCandidate],
        response_models: &[LineagePropagationAssayResponseModel],
        target: LineagePropagationAcquisitionTarget,
        posterior_particle_weights_million: &[u32],
        max_rounds: u16,
        minimum_effective_sample_fraction_milli: u16,
    ) -> Result<Self, LineagePropagationAcquisitionError> {
        validate_models(&analysis, candidates, response_models, max_rounds)?;
        validate_acquisition_target(&analysis, &target)?;
        if minimum_effective_sample_fraction_milli == 0
            || minimum_effective_sample_fraction_milli > 1_000
        {
            return Err(LineagePropagationAcquisitionError::InvalidRequest(
                "minimum effective-sample fraction must be in 1..=1000 milli".into(),
            ));
        }
        let particle_weights_million = if posterior_particle_weights_million.is_empty() {
            uniform_particle_weights(analysis.bootstrap_draws.len())
        } else {
            if posterior_particle_weights_million.len() != analysis.bootstrap_draws.len()
                || posterior_particle_weights_million
                    .iter()
                    .map(|weight| u64::from(*weight))
                    .sum::<u64>()
                    != PROBABILITY_MILLION
            {
                return Err(LineagePropagationAcquisitionError::InvalidInput(
                    "posterior particle weights must align with P10 draws and sum to 1,000,000"
                        .into(),
                ));
            }
            posterior_particle_weights_million.to_vec()
        };
        let response_models = response_models
            .iter()
            .cloned()
            .map(|model| (model.action_id.clone(), model))
            .collect();
        Ok(Self {
            analysis,
            target,
            response_models,
            joint_response_models: None,
            last_observation: None,
            response_history: Vec::new(),
            particle_weights_million,
            max_rounds,
            minimum_effective_sample_fraction_milli,
        })
    }

    /// Construct a sequential P06 selector using complete pairwise assay calibration.
    ///
    /// Each pair model must exactly reproduce the single-assay marginals and covers every
    /// unordered candidate pair. Replays in this mode require the ordered prior assay history;
    /// supplied posterior weights are checked against a deterministic replay of that history.
    pub fn new_with_target_and_joint_response_models(
        analysis: LineagePropagationAnalysis,
        candidates: &[StratifiedAssayCandidate],
        response_models: &[LineagePropagationAssayResponseModel],
        joint_response_models: &[LineagePropagationJointAssayResponseModel],
        target: LineagePropagationAcquisitionTarget,
        initial_history: &[LineagePropagationObservedAssay],
        posterior_particle_weights_million: &[u32],
        max_rounds: u16,
    ) -> Result<Self, LineagePropagationAcquisitionError> {
        validate_models(&analysis, candidates, response_models, max_rounds)?;
        validate_acquisition_target(&analysis, &target)?;
        let joint_response_models = validate_joint_response_models(
            &analysis,
            candidates,
            response_models,
            joint_response_models,
        )?;
        let response_models = response_models
            .iter()
            .cloned()
            .map(|model| (model.action_id.clone(), model))
            .collect::<BTreeMap<_, _>>();
        let candidate_by_action = candidates
            .iter()
            .map(|candidate| (candidate.action.action_id.as_str(), candidate))
            .collect::<BTreeMap<_, _>>();
        if initial_history.is_empty() && !posterior_particle_weights_million.is_empty() {
            return Err(LineagePropagationAcquisitionError::InvalidInput(
                "joint-response resumption requires the ordered prior assay history; an unexplained particle posterior is insufficient".into(),
            ));
        }
        if initial_history.len() > candidates.len() {
            return Err(LineagePropagationAcquisitionError::InvalidRequest(
                "joint-response history cannot exceed the number of single-use candidate actions"
                    .into(),
            ));
        }

        let mut particle_weights_million = uniform_particle_weights(analysis.bootstrap_draws.len());
        let mut last_observation = None;
        let mut observed_actions = BTreeSet::new();
        for observation in initial_history {
            if observation.action_id.trim().is_empty()
                || observation.outcome_id.trim().is_empty()
                || !observed_actions.insert(observation.action_id.as_str())
            {
                return Err(LineagePropagationAcquisitionError::InvalidInput(
                    "joint-response history must contain unique, nonempty single-use observations in execution order".into(),
                ));
            }
            let candidate = candidate_by_action
                .get(observation.action_id.as_str())
                .ok_or_else(|| {
                    LineagePropagationAcquisitionError::InvalidInput(
                        "joint-response history references an unknown candidate action".into(),
                    )
                })?;
            if !candidate
                .action
                .outcomes
                .iter()
                .any(|outcome| outcome.outcome_id == observation.outcome_id)
            {
                return Err(LineagePropagationAcquisitionError::OutOfModelOutcome);
            }
            let response_model = match &last_observation {
                Some(previous) => conditional_response_model(
                    previous,
                    &observation.action_id,
                    &response_models,
                    &joint_response_models,
                )?,
                None => response_models[&observation.action_id].clone(),
            };
            particle_weights_million = update_lineage_propagation_particle_weights(
                &particle_weights_million,
                &response_model,
                &observation.outcome_id,
            )?;
            let effective_fraction = effective_sample_fraction_milli(&particle_weights_million);
            if effective_fraction < MIN_POSTERIOR_EFFECTIVE_SAMPLE_FRACTION_MILLI {
                return Err(LineagePropagationAcquisitionError::LowEffectiveSampleSize {
                    effective_sample_fraction_milli: effective_fraction,
                    minimum_milli: MIN_POSTERIOR_EFFECTIVE_SAMPLE_FRACTION_MILLI,
                });
            }
            last_observation = Some(observation.clone());
        }
        if !posterior_particle_weights_million.is_empty()
            && posterior_particle_weights_million != particle_weights_million
        {
            return Err(LineagePropagationAcquisitionError::InvalidInput(
                "supplied joint-response posterior does not equal deterministic replay of the ordered assay history".into(),
            ));
        }

        Ok(Self {
            analysis,
            target,
            response_models,
            joint_response_models: Some(joint_response_models),
            last_observation,
            response_history: initial_history.to_vec(),
            particle_weights_million,
            max_rounds,
            minimum_effective_sample_fraction_milli: MIN_POSTERIOR_EFFECTIVE_SAMPLE_FRACTION_MILLI,
        })
    }

    pub fn plan(
        &self,
        candidates: &[StratifiedAssayCandidate],
    ) -> Result<LineagePropagationAcquisitionPlan, LineagePropagationAcquisitionError> {
        let models = candidates
            .iter()
            .map(
                |candidate| match (&self.last_observation, &self.joint_response_models) {
                    (Some(previous), Some(joint_models)) => conditional_response_model(
                        previous,
                        &candidate.action.action_id,
                        &self.response_models,
                        joint_models,
                    ),
                    _ => self
                        .response_models
                        .get(&candidate.action.action_id)
                        .cloned()
                        .ok_or(LineagePropagationAcquisitionError::OutOfModelOutcome),
                },
            )
            .collect::<Result<Vec<_>, _>>()?;
        let mut plan = plan_glioma_lineage_assay_acquisition_for_target(
            &self.analysis,
            candidates,
            &models,
            &self.target,
            &self.particle_weights_million,
            self.max_rounds,
        )?;
        plan.response_dependence = if self.joint_response_models.is_some() {
            LineagePropagationResponseDependence::FirstOrderPairCalibration
        } else {
            LineagePropagationResponseDependence::ConditionalIndependence
        };
        plan.conditioned_on = self.last_observation.clone();
        plan.digest = ContentHash::of_value(&digest_input(&plan))
            .map_err(|error| LineagePropagationAcquisitionError::Digest(error.to_string()))?;
        plan.validate()?;
        Ok(plan)
    }

    pub fn particle_weights_million(&self) -> &[u32] {
        &self.particle_weights_million
    }

    pub fn analysis_digest(&self) -> &ContentHash {
        &self.analysis.digest
    }

    pub fn response_dependence(&self) -> LineagePropagationResponseDependence {
        if self.joint_response_models.is_some() {
            LineagePropagationResponseDependence::FirstOrderPairCalibration
        } else {
            LineagePropagationResponseDependence::ConditionalIndependence
        }
    }

    pub fn response_history(&self) -> &[LineagePropagationObservedAssay] {
        &self.response_history
    }

    pub(crate) fn score(
        &self,
        candidate: &StratifiedAssayCandidate,
    ) -> Option<LineagePropagationAssayScore> {
        let model = match (&self.last_observation, &self.joint_response_models) {
            (Some(previous), Some(joint_models)) => conditional_response_model(
                previous,
                &candidate.action.action_id,
                &self.response_models,
                joint_models,
            )
            .ok()?,
            _ => self
                .response_models
                .get(&candidate.action.action_id)?
                .clone(),
        };
        let score = score_candidate(
            &self.analysis,
            candidate,
            &model,
            &self.particle_weights_million,
            &self.target,
        );
        (score.minimum_predicted_effective_sample_fraction_milli
            >= self.minimum_effective_sample_fraction_milli)
            .then_some(score)
    }

    /// Score each eligible first assay using a bounded two-assay adaptive rollout. The second
    /// assay may come from any source-state stratum: all strata share the same P10 bootstrap draw,
    /// so an observed outcome can change the posterior value of a different state's assay. The
    /// second assay is selected independently for every possible first result, then averaged under
    /// the first assay's posterior predictive distribution. This is a practical finite-horizon
    /// heuristic, not a general optimality guarantee.
    pub(crate) fn candidate_specific_priorities_with_lookahead(
        &self,
        candidates: &[StratifiedAssayCandidate],
        remaining_budget_units: u64,
        remaining_rounds: u16,
    ) -> Result<
        BTreeMap<(String, String), CandidateSpecificAcquisitionPriority>,
        LineagePropagationAcquisitionError,
    > {
        if candidates.is_empty() {
            return Ok(BTreeMap::new());
        }
        if candidates.len() > MAX_ROLLOUT_CANDIDATES {
            return Err(LineagePropagationAcquisitionError::InvalidRequest(format!(
                "two-assay adaptive rollout accepts at most {MAX_ROLLOUT_CANDIDATES} eligible candidates"
            )));
        }
        if candidates.iter().any(|candidate| {
            u64::from(candidate.action.cost_units) > remaining_budget_units
                || candidate.action.cost_units == 0
                || candidate.action.max_replicates != 1
        }) {
            return Err(LineagePropagationAcquisitionError::InvalidInput(
                "rollout candidates must be budget-feasible, positive-cost, single-use assays"
                    .into(),
            ));
        }
        let distinct = candidates
            .iter()
            .map(|candidate| {
                (
                    candidate.stratum_id.as_str(),
                    candidate.action.action_id.as_str(),
                )
            })
            .collect::<BTreeSet<_>>();
        let distinct_action_ids = candidates
            .iter()
            .map(|candidate| candidate.action.action_id.as_str())
            .collect::<BTreeSet<_>>();
        if distinct.len() != candidates.len() || distinct_action_ids.len() != candidates.len() {
            return Err(LineagePropagationAcquisitionError::InvalidInput(
                "rollout candidates must have globally unique single-use assay identities".into(),
            ));
        }
        let max_outcomes = candidates
            .iter()
            .map(|candidate| candidate.action.outcomes.len())
            .max()
            .unwrap_or(0);
        let states = self.analysis.state_order.len();
        // The campaign-level objective is the equal-source-weight sum of target-weighted
        // treatment/control transition variance across the complete P10 operator. This keeps a
        // gain in one stratum on the same scale as a follow-up gain in another stratum.
        let total_prior_variance_q = (0..states)
            .map(|source_index| {
                let values = source_operator_values_by_dimension(&self.analysis, source_index);
                weighted_operator_variance_q(
                    &values,
                    &self.particle_weights_million,
                    &self.target.destination_state_weights_milli,
                )
            })
            .sum::<u128>();
        let work = (candidates.len() as u128)
            .saturating_mul(candidates.len() as u128)
            .saturating_mul(max_outcomes as u128)
            .saturating_mul(self.analysis.bootstrap_draws.len() as u128)
            .saturating_mul((states as u128).saturating_mul(states as u128));
        if work > MAX_ROLLOUT_WORK {
            return Err(LineagePropagationAcquisitionError::InvalidRequest(format!(
                "two-assay adaptive rollout requires an estimated {work} particle/state operations, above the {MAX_ROLLOUT_WORK} work budget"
            )));
        }

        let mut priorities = BTreeMap::new();
        for first in candidates {
            let first_model = match (&self.last_observation, &self.joint_response_models) {
                (Some(previous), Some(joint_models)) => conditional_response_model(
                    previous,
                    &first.action.action_id,
                    &self.response_models,
                    joint_models,
                )?,
                _ => self
                    .response_models
                    .get(&first.action.action_id)
                    .cloned()
                    .ok_or(LineagePropagationAcquisitionError::OutOfModelOutcome)?,
            };
            let first_score = score_candidate(
                &self.analysis,
                first,
                &first_model,
                &self.particle_weights_million,
                &self.target,
            );
            if first_score.minimum_predicted_effective_sample_fraction_milli
                < self.minimum_effective_sample_fraction_milli
            {
                continue;
            }
            let prior_variance = first_score.prior_operator_variance_q;
            let first_gain =
                prior_variance.saturating_sub(first_score.expected_posterior_operator_variance_q);
            let first_feasibility = u128::from(first.action.feasibility_milli);
            let mut expected_followup_gain_q = 0_u128;
            let mut expected_followup_cost_units = 0_u128;
            let mut minimum_ess = first_score.minimum_predicted_effective_sample_fraction_milli;
            let mut used_second_assay = false;

            if remaining_rounds >= 2
                && first_feasibility > 0
                && remaining_budget_units > u64::from(first.action.cost_units)
            {
                for predicted in &first_score.predicted_outcome_probabilities {
                    if predicted.probability_million == 0 {
                        continue;
                    }
                    let posterior = update_lineage_propagation_particle_weights(
                        &self.particle_weights_million,
                        &first_model,
                        &predicted.outcome_id,
                    )?;
                    if effective_sample_fraction_milli(&posterior)
                        < self.minimum_effective_sample_fraction_milli
                    {
                        continue;
                    }
                    let observation = LineagePropagationObservedAssay {
                        action_id: first.action.action_id.clone(),
                        outcome_id: predicted.outcome_id.clone(),
                    };
                    let followup_budget =
                        remaining_budget_units.saturating_sub(u64::from(first.action.cost_units));
                    let mut best_followup: Option<(u128, u32, u16, String)> = None;
                    for second in candidates.iter().filter(|candidate| {
                        candidate.action.action_id != first.action.action_id
                            && u64::from(candidate.action.cost_units) <= followup_budget
                            && !self
                                .response_history
                                .iter()
                                .any(|seen| seen.action_id == candidate.action.action_id)
                    }) {
                        let second_model = if let Some(joint_models) = &self.joint_response_models {
                            match conditional_response_model(
                                &observation,
                                &second.action.action_id,
                                &self.response_models,
                                joint_models,
                            ) {
                                Ok(model) => model,
                                Err(LineagePropagationAcquisitionError::OutOfModelOutcome) => {
                                    continue;
                                }
                                Err(error) => return Err(error),
                            }
                        } else {
                            let Some(model) = self.response_models.get(&second.action.action_id)
                            else {
                                continue;
                            };
                            model.clone()
                        };
                        let score = score_candidate(
                            &self.analysis,
                            second,
                            &second_model,
                            &posterior,
                            &self.target,
                        );
                        if score.minimum_predicted_effective_sample_fraction_milli
                            < self.minimum_effective_sample_fraction_milli
                        {
                            continue;
                        }
                        let gain_q = score
                            .prior_operator_variance_q
                            .saturating_sub(score.expected_posterior_operator_variance_q);
                        if gain_q == 0 || second.action.feasibility_milli == 0 {
                            continue;
                        }
                        let realized_gain_q =
                            gain_q.saturating_mul(u128::from(second.action.feasibility_milli));
                        let replacement = best_followup.as_ref().is_none_or(
                            |(best_realized_gain, best_cost, _, best_action_id)| {
                                realized_gain_q.saturating_mul(u128::from(*best_cost))
                                    > best_realized_gain
                                        .saturating_mul(u128::from(second.action.cost_units))
                                    || (realized_gain_q.saturating_mul(u128::from(*best_cost))
                                        == best_realized_gain
                                            .saturating_mul(u128::from(second.action.cost_units))
                                        && (second.action.cost_units, &second.action.action_id)
                                            < (*best_cost, best_action_id))
                            },
                        );
                        if replacement {
                            best_followup = Some((
                                realized_gain_q,
                                second.action.cost_units,
                                score.minimum_predicted_effective_sample_fraction_milli,
                                second.action.action_id.clone(),
                            ));
                        }
                    }
                    if let Some((gain_feasibility_scaled, cost, ess, _)) = best_followup {
                        used_second_assay = true;
                        minimum_ess = minimum_ess.min(ess);
                        expected_followup_gain_q = expected_followup_gain_q.saturating_add(
                            gain_feasibility_scaled
                                .saturating_mul(u128::from(predicted.probability_million)),
                        );
                        expected_followup_cost_units = expected_followup_cost_units.saturating_add(
                            u128::from(cost)
                                .saturating_mul(u128::from(predicted.probability_million)),
                        );
                    }
                }
            }

            let expected_realized_gain_q = first_gain
                .saturating_add(
                    expected_followup_gain_q
                        / (u128::from(PROBABILITY_MILLION) * u128::from(LIKELIHOOD_MILLI)),
                )
                .saturating_mul(first_feasibility)
                / u128::from(LIKELIHOOD_MILLI);
            let expected_cost_units = u128::from(first.action.cost_units)
                .saturating_add(
                    (expected_followup_cost_units / u128::from(PROBABILITY_MILLION))
                        .saturating_mul(first_feasibility)
                        / u128::from(LIKELIHOOD_MILLI),
                )
                .max(1);
            let expected_reduction_milli = if total_prior_variance_q == 0 {
                0
            } else {
                (expected_realized_gain_q.saturating_mul(u128::from(LIKELIHOOD_MILLI))
                    / total_prior_variance_q)
                    .min(u128::from(LIKELIHOOD_MILLI)) as u16
            };
            priorities.insert(
                (first.stratum_id.clone(), first.action.action_id.clone()),
                CandidateSpecificAcquisitionPriority {
                    horizon_assays: if used_second_assay { 2 } else { 1 },
                    expected_variance_reduction_milli: expected_reduction_milli,
                    reduction_per_cost_million: u64::from(expected_reduction_milli)
                        .saturating_mul(PROBABILITY_MILLION)
                        / u64::try_from(expected_cost_units).unwrap_or(u64::MAX),
                    minimum_predicted_effective_sample_fraction_milli: minimum_ess,
                },
            );
        }
        Ok(priorities)
    }

    pub(crate) fn assimilate_outcome(
        &mut self,
        action_id: &str,
        outcome_id: &str,
    ) -> Result<(), LineagePropagationAcquisitionError> {
        let model = match (&self.last_observation, &self.joint_response_models) {
            (Some(previous), Some(joint_models)) => conditional_response_model(
                previous,
                action_id,
                &self.response_models,
                joint_models,
            )?,
            _ => self
                .response_models
                .get(action_id)
                .cloned()
                .ok_or(LineagePropagationAcquisitionError::OutOfModelOutcome)?,
        };
        let updated_weights = update_lineage_propagation_particle_weights(
            &self.particle_weights_million,
            &model,
            outcome_id,
        )?;
        let effective_sample_fraction_milli = effective_sample_fraction_milli(&updated_weights);
        if effective_sample_fraction_milli < self.minimum_effective_sample_fraction_milli {
            return Err(LineagePropagationAcquisitionError::LowEffectiveSampleSize {
                effective_sample_fraction_milli,
                minimum_milli: self.minimum_effective_sample_fraction_milli,
            });
        }
        self.particle_weights_million = updated_weights;
        if self.joint_response_models.is_some() {
            let observation = LineagePropagationObservedAssay {
                action_id: action_id.into(),
                outcome_id: outcome_id.into(),
            };
            self.last_observation = Some(observation.clone());
            self.response_history.push(observation);
        }
        Ok(())
    }
}

/// Rank preclinical glioma assay candidates by calibrated expected reduction in lineage-operator
/// uncertainty per assay cost. `posterior_particle_weights_million` may be empty for the uniform
/// P10 bootstrap prior, or supplied from prior observations for sequential replanning.
pub fn plan_glioma_lineage_assay_acquisition(
    analysis: &LineagePropagationAnalysis,
    candidates: &[StratifiedAssayCandidate],
    response_models: &[LineagePropagationAssayResponseModel],
    posterior_particle_weights_million: &[u32],
    max_rounds: u16,
) -> Result<LineagePropagationAcquisitionPlan, LineagePropagationAcquisitionError> {
    let target = uniform_destination_weights(analysis);
    plan_glioma_lineage_assay_acquisition_for_target(
        analysis,
        candidates,
        response_models,
        &target,
        posterior_particle_weights_million,
        max_rounds,
    )
}

/// Rank candidates against a preclinical glioma research estimand that weights the destination
/// states of the P10 treatment-minus-control propagation operator.
pub fn plan_glioma_lineage_assay_acquisition_for_target(
    analysis: &LineagePropagationAnalysis,
    candidates: &[StratifiedAssayCandidate],
    response_models: &[LineagePropagationAssayResponseModel],
    target: &LineagePropagationAcquisitionTarget,
    posterior_particle_weights_million: &[u32],
    max_rounds: u16,
) -> Result<LineagePropagationAcquisitionPlan, LineagePropagationAcquisitionError> {
    validate_models(analysis, candidates, response_models, max_rounds)?;
    validate_acquisition_target(analysis, target)?;
    let particle_weights = if posterior_particle_weights_million.is_empty() {
        uniform_particle_weights(analysis.bootstrap_draws.len())
    } else {
        if posterior_particle_weights_million.len() != analysis.bootstrap_draws.len()
            || posterior_particle_weights_million
                .iter()
                .map(|weight| u64::from(*weight))
                .sum::<u64>()
                != PROBABILITY_MILLION
        {
            return Err(LineagePropagationAcquisitionError::InvalidInput(
                "posterior particle weights must align with P10 draws and sum to 1,000,000".into(),
            ));
        }
        posterior_particle_weights_million.to_vec()
    };
    let models = response_models
        .iter()
        .map(|model| (model.action_id.as_str(), model))
        .collect::<BTreeMap<_, _>>();
    let mut candidate_order = candidates
        .iter()
        .map(|candidate| candidate.action.action_id.clone())
        .collect::<Vec<_>>();
    candidate_order.sort();
    let mut scores = candidates
        .iter()
        .map(|candidate| {
            score_candidate(
                analysis,
                candidate,
                models[candidate.action.action_id.as_str()],
                &particle_weights,
                target,
            )
        })
        .collect::<Vec<_>>();
    scores.sort_by(|left, right| {
        right
            .expected_realized_reduction_per_cost_million
            .cmp(&left.expected_realized_reduction_per_cost_million)
            .then_with(|| {
                right
                    .reduction_per_cost_million
                    .cmp(&left.reduction_per_cost_million)
            })
            .then_with(|| {
                right
                    .expected_variance_reduction_milli
                    .cmp(&left.expected_variance_reduction_milli)
            })
            .then_with(|| left.action_id.cmp(&right.action_id))
    });
    let mut plan = LineagePropagationAcquisitionPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        analysis_digest: analysis.digest.clone(),
        target: target.clone(),
        response_dependence: LineagePropagationResponseDependence::ConditionalIndependence,
        conditioned_on: None,
        particle_weights_million: particle_weights,
        candidate_order,
        scores,
        digest: ContentHash::of_bytes(b"unsealed-lineage-propagation-acquisition-plan"),
    };
    plan.digest = ContentHash::of_value(&digest_input(&plan))
        .map_err(|error| LineagePropagationAcquisitionError::Digest(error.to_string()))?;
    plan.validate()?;
    Ok(plan)
}

impl LineagePropagationAcquisitionPlan {
    pub fn validate(&self) -> Result<(), LineagePropagationAcquisitionError> {
        let ids = self
            .scores
            .iter()
            .map(|score| score.action_id.as_str())
            .collect::<BTreeSet<_>>();
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || (self.response_dependence
                == LineagePropagationResponseDependence::ConditionalIndependence
                && self.conditioned_on.is_some())
            || self.conditioned_on.as_ref().is_some_and(|observation| {
                observation.action_id.trim().is_empty() || observation.outcome_id.trim().is_empty()
            })
            || self.target.destination_state_order.is_empty()
            || self.target.destination_state_order.len()
                != self.target.destination_state_weights_milli.len()
            || self
                .target
                .destination_state_order
                .iter()
                .any(|state| state.trim().is_empty())
            || self
                .target
                .destination_state_order
                .iter()
                .collect::<BTreeSet<_>>()
                .len()
                != self.target.destination_state_order.len()
            || self
                .target
                .destination_state_weights_milli
                .iter()
                .any(|weight| *weight > 1_000)
            || self
                .target
                .destination_state_weights_milli
                .iter()
                .all(|weight| *weight == 0)
            || self.scores.is_empty()
            || self.candidate_order.len() != self.scores.len()
            || ids.len() != self.scores.len()
            || self
                .candidate_order
                .iter()
                .map(String::as_str)
                .collect::<BTreeSet<_>>()
                != ids
            || self.particle_weights_million.is_empty()
            || self
                .particle_weights_million
                .iter()
                .map(|weight| u64::from(*weight))
                .sum::<u64>()
                != PROBABILITY_MILLION
            || self.scores.iter().any(|score| {
                score.action_id.trim().is_empty()
                    || score.state_id.trim().is_empty()
                    || score.model_version.trim().is_empty()
                    || score.cost_units == 0
                    || score.feasibility_milli > 1_000
                    || score.expected_variance_reduction_milli > LIKELIHOOD_MILLI as u16
                    || score.reduction_per_cost_million
                        != u64::from(score.expected_variance_reduction_milli)
                            .saturating_mul(PROBABILITY_MILLION)
                            / u64::from(score.cost_units)
                    || score.expected_realized_reduction_per_cost_million
                        != score
                            .reduction_per_cost_million
                            .saturating_mul(u64::from(score.feasibility_milli))
                            / LIKELIHOOD_MILLI
                    || score.expected_posterior_operator_variance_q
                        > score.prior_operator_variance_q
                    || score.minimum_predicted_effective_sample_fraction_milli > 1_000
                    || score.predicted_outcome_probabilities.is_empty()
                    || score
                        .predicted_outcome_probabilities
                        .iter()
                        .map(|outcome| u64::from(outcome.probability_million))
                        .sum::<u64>()
                        != PROBABILITY_MILLION
            })
            || self.scores.windows(2).any(|pair| {
                pair[0].expected_realized_reduction_per_cost_million
                    < pair[1].expected_realized_reduction_per_cost_million
                    || (pair[0].expected_realized_reduction_per_cost_million
                        == pair[1].expected_realized_reduction_per_cost_million
                        && pair[0].reduction_per_cost_million < pair[1].reduction_per_cost_million)
                    || (pair[0].expected_realized_reduction_per_cost_million
                        == pair[1].expected_realized_reduction_per_cost_million
                        && pair[0].reduction_per_cost_million == pair[1].reduction_per_cost_million
                        && pair[0].expected_variance_reduction_milli
                            < pair[1].expected_variance_reduction_milli)
            })
        {
            return Err(LineagePropagationAcquisitionError::InvalidOutput(
                "candidate identities, posterior mass, feasibility-adjusted score ordering, or variance bounds are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| LineagePropagationAcquisitionError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(LineagePropagationAcquisitionError::InvalidOutput(
                "lineage acquisition plan digest mismatch".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p06_experiment_design::information_design::{
        DesignAction, DesignOutcome,
    };
    use crate::glioma::programs::p10_interpretation_replication::lineage_propagation::{
        analyze_glioma_lineage_propagation, LineagePropagationBootstrapDraw,
        LineagePropagationRequest, LineagePropagationSnapshot,
    };
    use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};

    fn artifact(label: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: format!("local:{label}"),
            content_hash: ContentHash::of_bytes(label.as_bytes()),
            content_type: "application/octet-stream".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn analysis() -> LineagePropagationAnalysis {
        let request = LineagePropagationRequest {
            objective: "estimate finite-interval glioma lineage-state perturbation contrasts"
                .into(),
            model_system: GliomaModelSystem::Organoid,
            control_arm: "control".into(),
            treatment_arm: "perturbation".into(),
            state_order: vec!["npc_like".into(), "mes_like".into()],
            min_units_per_arm: 2,
            min_lineages_per_arm: 4,
            ridge_penalty_ppm: 1,
            max_coefficient_ppm: 5_000_000,
            max_prediction_error_ppm: 300_000,
            bootstrap_replicates: 99,
            bootstrap_seed: ContentHash::of_bytes(b"lineage-acquisition-test-seed"),
            confidence_level_milli: 900,
            minimum_effect_ppm: 25_000,
        };
        let mut rows = Vec::new();
        let unit_matrices = [
            ("control", [[8, 1], [2, 9]]),
            ("control", [[9, 2], [1, 8]]),
            ("control", [[7, 3], [3, 7]]),
            ("perturbation", [[6, 3], [4, 7]]),
            ("perturbation", [[7, 4], [3, 6]]),
            ("perturbation", [[5, 2], [5, 8]]),
        ];
        for (unit_index, (arm, matrix)) in unit_matrices.into_iter().enumerate() {
            for source in 0..2 {
                let lineage = format!("{arm}-u{unit_index}-l{source}");
                let mut values = vec![0_u32; 2];
                values[source] = 100;
                for (step, day) in [0_u32, 7, 14].into_iter().enumerate() {
                    rows.push(LineagePropagationSnapshot {
                        observation_id: format!("{lineage}-t{day}"),
                        experimental_unit_id: format!("{arm}-u{unit_index}"),
                        lineage_id: lineage.clone(),
                        arm_id: arm.into(),
                        model_system: GliomaModelSystem::Organoid,
                        assay_batch_id: format!("assay-batch-{unit_index}"),
                        timepoint_day: day,
                        state_counts: values.clone(),
                        capture_fraction_ppm: 1_000_000,
                        artifact: artifact(&format!("{lineage}-{day}")),
                    });
                    if step < 2 {
                        values = (0..2)
                            .map(|to| {
                                (0..2)
                                    .map(|from| matrix[to][from] * values[from])
                                    .sum::<u32>()
                                    / 10
                            })
                            .collect();
                    }
                }
            }
        }
        analyze_glioma_lineage_propagation(&request, &rows).unwrap()
    }

    fn candidate(action_id: &str, cost_units: u32) -> StratifiedAssayCandidate {
        let outcome = |id: &str, a: u16, b: u16| DesignOutcome {
            outcome_id: id.into(),
            label: id.into(),
            probability_milli_by_mechanism: [("mechanism_a".into(), a), ("mechanism_b".into(), b)]
                .into(),
        };
        StratifiedAssayCandidate {
            stratum_id: "npc_like".into(),
            action: DesignAction {
                action_id: action_id.into(),
                feature_id: "GAF-GLIOMA-P06-F19".into(),
                label: format!("{action_id} lineage assay"),
                outcomes: vec![outcome("high", 900, 100), outcome("low", 100, 900)],
                feasibility_milli: 1_000,
                risk_milli: 0,
                cost_units,
                max_replicates: 1,
            },
            negative_outcome_order: vec!["low".into()],
        }
    }

    fn response_model(
        analysis: &LineagePropagationAnalysis,
        candidate: &StratifiedAssayCandidate,
        informative: bool,
    ) -> LineagePropagationAssayResponseModel {
        let source = analysis
            .state_order
            .iter()
            .position(|state| state == &candidate.stratum_id)
            .unwrap();
        response_model_for_destination(analysis, candidate, source, informative)
    }

    fn response_model_for_destination(
        analysis: &LineagePropagationAnalysis,
        candidate: &StratifiedAssayCandidate,
        destination: usize,
        informative: bool,
    ) -> LineagePropagationAssayResponseModel {
        let states = analysis.state_order.len();
        let source = analysis
            .state_order
            .iter()
            .position(|state| state == &candidate.stratum_id)
            .unwrap();
        let mut likelihoods = analysis
            .bootstrap_draws
            .iter()
            .map(|draw| {
                let flat_index = destination * states + source;
                draw.treatment_coefficients_ppm[flat_index] as i64
                    - draw.control_coefficients_ppm[flat_index] as i64
            })
            .collect::<Vec<_>>();
        let sorted = {
            let mut values = likelihoods.clone();
            values.sort();
            values
        };
        let median = sorted[sorted.len() / 2];
        let high = likelihoods
            .iter_mut()
            .map(|contrast| {
                if informative && *contrast >= median {
                    900
                } else if informative {
                    100
                } else {
                    500
                }
            })
            .collect::<Vec<_>>();
        let low = high
            .iter()
            .map(|probability| 1_000 - probability)
            .collect::<Vec<_>>();
        debug_assert_eq!(states, 2);
        LineagePropagationAssayResponseModel {
            action_id: candidate.action.action_id.clone(),
            analysis_digest: analysis.digest.clone(),
            model_version: "calibrated-organoid-assay-v1".into(),
            calibration_source_digest: ContentHash::of_bytes(
                format!("calibration:{}", candidate.action.action_id).as_bytes(),
            ),
            calibration_unit_count: 12,
            outcomes: vec![
                LineagePropagationOutcomeLikelihood {
                    outcome_id: "high".into(),
                    likelihood_milli_by_bootstrap_draw: high,
                },
                LineagePropagationOutcomeLikelihood {
                    outcome_id: "low".into(),
                    likelihood_milli_by_bootstrap_draw: low,
                },
            ],
        }
    }

    fn perfectly_correlated_pair_model(
        analysis: &LineagePropagationAnalysis,
        first: &LineagePropagationAssayResponseModel,
        second: &LineagePropagationAssayResponseModel,
    ) -> LineagePropagationJointAssayResponseModel {
        assert!(first.action_id < second.action_id);
        assert_eq!(
            first.outcomes[0].likelihood_milli_by_bootstrap_draw,
            second.outcomes[0].likelihood_milli_by_bootstrap_draw
        );
        let zeros = vec![0; analysis.bootstrap_draws.len()];
        LineagePropagationJointAssayResponseModel {
            first_action_id: first.action_id.clone(),
            second_action_id: second.action_id.clone(),
            analysis_digest: analysis.digest.clone(),
            model_version: "paired-correlated-assay-v1".into(),
            calibration_source_digest: ContentHash::of_bytes(b"paired-calibration"),
            calibration_unit_count: 24,
            outcomes: vec![
                LineagePropagationJointOutcomeLikelihood {
                    first_outcome_id: "high".into(),
                    second_outcome_id: "high".into(),
                    likelihood_milli_by_bootstrap_draw: first.outcomes[0]
                        .likelihood_milli_by_bootstrap_draw
                        .clone(),
                },
                LineagePropagationJointOutcomeLikelihood {
                    first_outcome_id: "high".into(),
                    second_outcome_id: "low".into(),
                    likelihood_milli_by_bootstrap_draw: zeros.clone(),
                },
                LineagePropagationJointOutcomeLikelihood {
                    first_outcome_id: "low".into(),
                    second_outcome_id: "high".into(),
                    likelihood_milli_by_bootstrap_draw: zeros,
                },
                LineagePropagationJointOutcomeLikelihood {
                    first_outcome_id: "low".into(),
                    second_outcome_id: "low".into(),
                    likelihood_milli_by_bootstrap_draw: first.outcomes[1]
                        .likelihood_milli_by_bootstrap_draw
                        .clone(),
                },
            ],
        }
    }

    #[test]
    fn acquisition_prefers_a_calibrated_informative_assay_per_cost_and_is_order_invariant() {
        let analysis = analysis();
        assert_eq!(analysis.bootstrap_draws.len(), 99);
        let informative = candidate("informative", 1);
        let uninformative = candidate("uninformative", 1);
        let candidates = vec![informative.clone(), uninformative.clone()];
        let models = vec![
            response_model(&analysis, &informative, true),
            response_model(&analysis, &uninformative, false),
        ];
        let target = LineagePropagationAcquisitionTarget {
            destination_state_order: analysis.state_order.clone(),
            destination_state_weights_milli: vec![1_000, 0],
        };
        let plan = plan_glioma_lineage_assay_acquisition_for_target(
            &analysis,
            &candidates,
            &models,
            &target,
            &[],
            4,
        )
        .unwrap();
        assert_eq!(plan.scores[0].action_id, "informative");
        assert_eq!(plan.target, target);
        assert!(plan.scores[0].expected_variance_reduction_milli > 0);
        assert_eq!(
            plan.scores[0].expected_realized_reduction_per_cost_million,
            plan.scores[0].reduction_per_cost_million
        );
        assert_eq!(plan.scores[1].expected_variance_reduction_milli, 0);
        assert!(plan.scores[0].minimum_predicted_effective_sample_fraction_milli >= 100);

        // This assay is calibrated to the NPC-like destination only. It must not be called
        // informative for the default all-destination composite when the paired destinations
        // cancel in that declared research estimand.
        let equal_destination_plan =
            plan_glioma_lineage_assay_acquisition(&analysis, &candidates, &models, &[], 4).unwrap();
        assert!(equal_destination_plan
            .scores
            .iter()
            .all(|score| score.expected_variance_reduction_milli == 0));

        let reversed = plan_glioma_lineage_assay_acquisition_for_target(
            &analysis,
            &[uninformative, informative],
            &[models[1].clone(), models[0].clone()],
            &target,
            &[],
            4,
        )
        .unwrap();
        assert_eq!(plan.digest, reversed.digest);
    }

    #[test]
    fn feasibility_changes_expected_realized_value_and_candidate_order() {
        let analysis = analysis();
        let reliable = candidate("reliable", 1);
        let mut unreliable = candidate("unreliable", 1);
        unreliable.action.feasibility_milli = 200;
        let candidates = vec![unreliable.clone(), reliable.clone()];
        let models = vec![
            response_model(&analysis, &unreliable, true),
            response_model(&analysis, &reliable, true),
        ];

        let plan =
            plan_glioma_lineage_assay_acquisition(&analysis, &candidates, &models, &[], 4).unwrap();

        assert_eq!(plan.scores[0].action_id, "reliable");
        assert_eq!(plan.scores[1].action_id, "unreliable");
        assert_eq!(
            plan.scores[0].reduction_per_cost_million,
            plan.scores[1].reduction_per_cost_million
        );
        assert_eq!(
            plan.scores[0].expected_realized_reduction_per_cost_million,
            plan.scores[0].reduction_per_cost_million
        );
        assert_eq!(
            plan.scores[1].expected_realized_reduction_per_cost_million,
            plan.scores[1].reduction_per_cost_million / 5
        );
        plan.validate().unwrap();
    }

    #[test]
    fn explicit_destination_estimand_is_bound_and_must_match_p10_state_order() {
        let analysis = analysis();
        let candidate = candidate("targeted", 1);
        let models = vec![response_model(&analysis, &candidate, true)];
        let target = LineagePropagationAcquisitionTarget {
            destination_state_order: analysis.state_order.clone(),
            destination_state_weights_milli: vec![0, 1_000],
        };

        let plan = plan_glioma_lineage_assay_acquisition_for_target(
            &analysis,
            std::slice::from_ref(&candidate),
            &models,
            &target,
            &[],
            2,
        )
        .unwrap();
        assert_eq!(plan.target, target);
        plan.validate().unwrap();

        let policy = LineagePropagationAcquisitionPolicy::new_with_target(
            analysis.clone(),
            std::slice::from_ref(&candidate),
            &models,
            target.clone(),
            &[],
            2,
        )
        .unwrap();
        assert_eq!(policy.plan(&[candidate.clone()]).unwrap().target, target);

        let wrong_state_order = LineagePropagationAcquisitionTarget {
            destination_state_order: vec!["mes_like".into(), "npc_like".into()],
            destination_state_weights_milli: vec![0, 1_000],
        };
        assert!(plan_glioma_lineage_assay_acquisition_for_target(
            &analysis,
            std::slice::from_ref(&candidate),
            &models,
            &wrong_state_order,
            &[],
            2,
        )
        .is_err());

        let all_zero = LineagePropagationAcquisitionTarget {
            destination_state_order: analysis.state_order.clone(),
            destination_state_weights_milli: vec![0, 0],
        };
        assert!(plan_glioma_lineage_assay_acquisition_for_target(
            &analysis,
            &[candidate],
            &models,
            &all_zero,
            &[],
            2,
        )
        .is_err());
    }

    #[test]
    fn estimand_weights_direct_assay_selection_to_the_glioma_destination_of_interest() {
        let mut analysis = analysis();
        analysis.bootstrap_draws = (0..4)
            .map(|index| {
                let first_destination = if index % 2 == 0 { 0 } else { 1_000_000 };
                let second_destination = if (index / 2) % 2 == 0 { 0 } else { 1_000_000 };
                LineagePropagationBootstrapDraw {
                    replicate_index: index + 1,
                    control_coefficients_ppm: vec![0; 4],
                    treatment_coefficients_ppm: vec![first_destination, 0, second_destination, 0],
                }
            })
            .collect();
        let npc_candidate = candidate("assay-npc-to-npc", 1);
        let mes_candidate = candidate("assay-npc-to-mes", 1);
        let likelihood_model = |candidate: &StratifiedAssayCandidate, high: Vec<u16>| {
            let low = high.iter().map(|probability| 1_000 - probability).collect();
            LineagePropagationAssayResponseModel {
                action_id: candidate.action.action_id.clone(),
                analysis_digest: analysis.digest.clone(),
                model_version: "synthetic-target-specific-v1".into(),
                calibration_source_digest: ContentHash::of_bytes(
                    candidate.action.action_id.as_bytes(),
                ),
                calibration_unit_count: 24,
                outcomes: vec![
                    LineagePropagationOutcomeLikelihood {
                        outcome_id: "high".into(),
                        likelihood_milli_by_bootstrap_draw: high,
                    },
                    LineagePropagationOutcomeLikelihood {
                        outcome_id: "low".into(),
                        likelihood_milli_by_bootstrap_draw: low,
                    },
                ],
            }
        };
        let npc_model = likelihood_model(&npc_candidate, vec![900, 100, 900, 100]);
        let mes_model = likelihood_model(&mes_candidate, vec![900, 900, 100, 100]);
        let particles = vec![250_000; 4];
        let npc_target = LineagePropagationAcquisitionTarget {
            destination_state_order: analysis.state_order.clone(),
            destination_state_weights_milli: vec![1_000, 0],
        };
        let mes_target = LineagePropagationAcquisitionTarget {
            destination_state_order: analysis.state_order.clone(),
            destination_state_weights_milli: vec![0, 1_000],
        };

        let npc_target_npc_assay = score_candidate(
            &analysis,
            &npc_candidate,
            &npc_model,
            &particles,
            &npc_target,
        );
        let npc_target_mes_assay = score_candidate(
            &analysis,
            &mes_candidate,
            &mes_model,
            &particles,
            &npc_target,
        );
        let mes_target_npc_assay = score_candidate(
            &analysis,
            &npc_candidate,
            &npc_model,
            &particles,
            &mes_target,
        );
        let mes_target_mes_assay = score_candidate(
            &analysis,
            &mes_candidate,
            &mes_model,
            &particles,
            &mes_target,
        );

        assert!(
            npc_target_npc_assay.expected_variance_reduction_milli
                > npc_target_mes_assay.expected_variance_reduction_milli
        );
        assert!(
            mes_target_mes_assay.expected_variance_reduction_milli
                > mes_target_npc_assay.expected_variance_reduction_milli
        );
        assert_eq!(
            weighted_operator_variance_q(
                &[vec![0, 1_000_000], vec![0, 1_000_000]],
                &particles,
                &[1_000, 0],
            ),
            weighted_variance_q(&[0, 1_000_000], &particles)
        );
    }

    #[test]
    fn composite_estimand_variance_preserves_cross_destination_covariance() {
        let particles = vec![500_000, 500_000];
        let anticorrelated = [vec![-1_000_000, 1_000_000], vec![1_000_000, -1_000_000]];
        let single_state_variance = weighted_variance_q(&anticorrelated[0], &particles);

        let composite_variance =
            weighted_operator_variance_q(&anticorrelated, &particles, &[1_000, 1_000]);
        let rescaled_composite_variance =
            weighted_operator_variance_q(&anticorrelated, &particles, &[500, 500]);

        assert!(single_state_variance > 0);
        assert_eq!(composite_variance, 0);
        assert_eq!(rescaled_composite_variance, composite_variance);

        let correlated = [vec![-1_000_000, 1_000_000], vec![-1_000_000, 1_000_000]];
        let correlated_composite =
            weighted_operator_variance_q(&correlated, &particles, &[1_000, 1_000]);
        assert_eq!(correlated_composite, single_state_variance);
    }

    #[test]
    fn particle_update_is_bayesian_and_depleted_ensemble_is_not_committed() {
        let analysis = analysis();
        let informative = candidate("informative", 1);
        let mut model = response_model(&analysis, &informative, true);
        let weights = uniform_particle_weights(analysis.bootstrap_draws.len());
        let updated =
            update_lineage_propagation_particle_weights(&weights, &model, "high").unwrap();
        assert_ne!(updated, weights);
        assert_eq!(
            updated.iter().map(|weight| u64::from(*weight)).sum::<u64>(),
            1_000_000
        );

        let mut policy = LineagePropagationAcquisitionPolicy::new(
            analysis.clone(),
            std::slice::from_ref(&informative),
            std::slice::from_ref(&model),
            &[],
            2,
        )
        .unwrap();
        let mut extreme = vec![0; analysis.bootstrap_draws.len()];
        extreme[0] = 1_000;
        let mut opposite = vec![1_000; analysis.bootstrap_draws.len()];
        opposite[0] = 0;
        model.outcomes[0].likelihood_milli_by_bootstrap_draw = extreme;
        model.outcomes[1].likelihood_milli_by_bootstrap_draw = opposite;
        policy.response_models.insert("informative".into(), model);
        let before = policy.particle_weights_million().to_vec();
        assert!(matches!(
            policy.assimilate_outcome("informative", "high"),
            Err(LineagePropagationAcquisitionError::LowEffectiveSampleSize { .. })
        ));
        assert_eq!(policy.particle_weights_million(), before);
    }

    #[test]
    fn joint_pair_likelihood_prevents_double_counting_correlated_assay_noise() {
        let response = |action_id: &str| LineagePropagationAssayResponseModel {
            action_id: action_id.into(),
            analysis_digest: ContentHash::of_bytes(b"pair-analysis"),
            model_version: "marginal-v1".into(),
            calibration_source_digest: ContentHash::of_bytes(action_id.as_bytes()),
            calibration_unit_count: 24,
            outcomes: vec![
                LineagePropagationOutcomeLikelihood {
                    outcome_id: "high".into(),
                    likelihood_milli_by_bootstrap_draw: vec![800, 200],
                },
                LineagePropagationOutcomeLikelihood {
                    outcome_id: "low".into(),
                    likelihood_milli_by_bootstrap_draw: vec![200, 800],
                },
            ],
        };
        let first = response("assay-a");
        let second = response("assay-b");
        let joint = LineagePropagationJointAssayResponseModel {
            first_action_id: "assay-a".into(),
            second_action_id: "assay-b".into(),
            analysis_digest: first.analysis_digest.clone(),
            model_version: "shared-noise-v1".into(),
            calibration_source_digest: ContentHash::of_bytes(b"joint-calibration"),
            calibration_unit_count: 24,
            outcomes: vec![
                LineagePropagationJointOutcomeLikelihood {
                    first_outcome_id: "high".into(),
                    second_outcome_id: "high".into(),
                    likelihood_milli_by_bootstrap_draw: vec![800, 200],
                },
                LineagePropagationJointOutcomeLikelihood {
                    first_outcome_id: "high".into(),
                    second_outcome_id: "low".into(),
                    likelihood_milli_by_bootstrap_draw: vec![0, 0],
                },
                LineagePropagationJointOutcomeLikelihood {
                    first_outcome_id: "low".into(),
                    second_outcome_id: "high".into(),
                    likelihood_milli_by_bootstrap_draw: vec![0, 0],
                },
                LineagePropagationJointOutcomeLikelihood {
                    first_outcome_id: "low".into(),
                    second_outcome_id: "low".into(),
                    likelihood_milli_by_bootstrap_draw: vec![200, 800],
                },
            ],
        };
        let response_models = BTreeMap::from([
            (first.action_id.clone(), first.clone()),
            (second.action_id.clone(), second.clone()),
        ]);
        let joint_models = BTreeMap::from([(
            joint_pair_key(&joint.first_action_id, &joint.second_action_id),
            joint,
        )]);
        let prior = vec![500_000, 500_000];
        let after_first =
            update_lineage_propagation_particle_weights(&prior, &first, "high").unwrap();
        let conditioned_second = conditional_response_model(
            &LineagePropagationObservedAssay {
                action_id: "assay-a".into(),
                outcome_id: "high".into(),
            },
            "assay-b",
            &response_models,
            &joint_models,
        )
        .unwrap();
        assert_eq!(
            conditioned_second.outcomes[0].likelihood_milli_by_bootstrap_draw,
            vec![1_000, 1_000]
        );
        assert_eq!(
            conditioned_second.outcomes[1].likelihood_milli_by_bootstrap_draw,
            vec![0, 0]
        );
        let after_correlated_second =
            update_lineage_propagation_particle_weights(&after_first, &conditioned_second, "high")
                .unwrap();
        let incorrectly_independent_second =
            update_lineage_propagation_particle_weights(&after_first, &second, "high").unwrap();
        assert_eq!(after_correlated_second, after_first);
        assert_ne!(incorrectly_independent_second, after_first);
    }

    #[test]
    fn joint_policy_conditions_live_scores_and_replays_prior_history() {
        let analysis = analysis();
        let first_candidate = candidate("assay-a", 1);
        let second_candidate = candidate("assay-b", 1);
        let candidates = vec![first_candidate.clone(), second_candidate.clone()];
        let response_models = vec![
            response_model(&analysis, &first_candidate, true),
            response_model(&analysis, &second_candidate, true),
        ];
        let joint_model =
            perfectly_correlated_pair_model(&analysis, &response_models[0], &response_models[1]);
        let target = LineagePropagationAcquisitionTarget {
            destination_state_order: analysis.state_order.clone(),
            destination_state_weights_milli: vec![1_000, 0],
        };
        let mut policy =
            LineagePropagationAcquisitionPolicy::new_with_target_and_joint_response_models(
                analysis.clone(),
                &candidates,
                &response_models,
                std::slice::from_ref(&joint_model),
                target.clone(),
                &[],
                &[],
                3,
            )
            .unwrap();
        let initial_plan = policy.plan(&candidates).unwrap();
        assert_eq!(
            initial_plan.response_dependence,
            LineagePropagationResponseDependence::FirstOrderPairCalibration
        );
        assert_eq!(initial_plan.conditioned_on, None);
        assert!(initial_plan.scores[0].expected_variance_reduction_milli > 0);

        policy.assimilate_outcome("assay-a", "high").unwrap();
        let conditional_score = policy.score(&second_candidate).unwrap();
        assert_eq!(conditional_score.expected_variance_reduction_milli, 0);
        let conditioned_plan = policy.plan(&candidates).unwrap();
        assert_eq!(
            conditioned_plan.conditioned_on,
            Some(LineagePropagationObservedAssay {
                action_id: "assay-a".into(),
                outcome_id: "high".into(),
            })
        );

        let replay =
            LineagePropagationAcquisitionPolicy::new_with_target_and_joint_response_models(
                analysis.clone(),
                &candidates,
                &response_models,
                std::slice::from_ref(&joint_model),
                target.clone(),
                policy.response_history(),
                policy.particle_weights_million(),
                3,
            )
            .unwrap();
        assert_eq!(
            replay.particle_weights_million(),
            policy.particle_weights_million()
        );
        assert_eq!(replay.score(&second_candidate).unwrap(), conditional_score);

        let independent_policy = LineagePropagationAcquisitionPolicy::new_with_target(
            analysis.clone(),
            &candidates,
            &response_models,
            target.clone(),
            policy.particle_weights_million(),
            3,
        )
        .unwrap();
        assert!(
            independent_policy
                .score(&second_candidate)
                .unwrap()
                .expected_variance_reduction_milli
                > conditional_score.expected_variance_reduction_milli
        );

        let mut invalid_marginal = joint_model;
        invalid_marginal.outcomes[0].likelihood_milli_by_bootstrap_draw[0] += 1;
        invalid_marginal.outcomes[3].likelihood_milli_by_bootstrap_draw[0] -= 1;
        assert!(
            LineagePropagationAcquisitionPolicy::new_with_target_and_joint_response_models(
                analysis,
                &candidates,
                &response_models,
                &[invalid_marginal],
                target,
                &[],
                &[],
                3,
            )
            .is_err()
        );
    }

    #[test]
    fn bounded_adaptive_rollout_values_complementarity_and_respects_budget() {
        let analysis = analysis();
        let first = candidate("assay-a", 1);
        let second = candidate("assay-b", 1);
        let candidates = vec![first.clone(), second.clone()];
        let models = vec![
            response_model(&analysis, &first, true),
            response_model(&analysis, &second, true),
        ];
        let target = LineagePropagationAcquisitionTarget {
            destination_state_order: analysis.state_order.clone(),
            destination_state_weights_milli: vec![1_000, 0],
        };
        let independent = LineagePropagationAcquisitionPolicy::new_with_target(
            analysis.clone(),
            &candidates,
            &models,
            target.clone(),
            &[],
            2,
        )
        .unwrap();
        let paired_model = perfectly_correlated_pair_model(&analysis, &models[0], &models[1]);
        let paired =
            LineagePropagationAcquisitionPolicy::new_with_target_and_joint_response_models(
                analysis.clone(),
                &candidates,
                &models,
                std::slice::from_ref(&paired_model),
                target,
                &[],
                &[],
                2,
            )
            .unwrap();

        let independent_scores = independent
            .candidate_specific_priorities_with_lookahead(&candidates, 2, 2)
            .unwrap();
        let paired_scores = paired
            .candidate_specific_priorities_with_lookahead(&candidates, 2, 2)
            .unwrap();
        let independent_first = independent_scores[&("npc_like".into(), "assay-a".into())];
        let paired_first = paired_scores[&("npc_like".into(), "assay-a".into())];
        assert_eq!(independent_first.horizon_assays, 2);
        assert_eq!(paired_first.horizon_assays, 1);
        assert!(
            independent_first.expected_variance_reduction_milli
                > paired_first.expected_variance_reduction_milli
        );

        let one_assay_budget = independent
            .candidate_specific_priorities_with_lookahead(&candidates, 1, 2)
            .unwrap();
        assert!(one_assay_budget
            .values()
            .all(|priority| priority.horizon_assays == 1));

        let one_round_left = independent
            .candidate_specific_priorities_with_lookahead(&candidates, 2, 1)
            .unwrap();
        assert!(one_round_left
            .values()
            .all(|priority| priority.horizon_assays == 1));

        let oversized = (0..=MAX_ROLLOUT_CANDIDATES)
            .map(|index| candidate(&format!("assay-{index:03}"), 1))
            .collect::<Vec<_>>();
        assert!(matches!(
            independent.candidate_specific_priorities_with_lookahead(&oversized, 1, 2),
            Err(LineagePropagationAcquisitionError::InvalidRequest(_))
        ));
    }

    #[test]
    fn adaptive_rollout_can_choose_a_followup_from_a_different_glioma_state() {
        let analysis = analysis();
        let first = candidate("assay-a", 1);
        let mut second = candidate("assay-b", 1);
        second.stratum_id = "mes_like".into();
        let candidates = vec![first.clone(), second.clone()];
        let models = vec![
            response_model(&analysis, &first, true),
            response_model(&analysis, &second, true),
        ];
        let policy = LineagePropagationAcquisitionPolicy::new_with_target(
            analysis.clone(),
            &candidates,
            &models,
            LineagePropagationAcquisitionTarget {
                destination_state_order: analysis.state_order.clone(),
                destination_state_weights_milli: vec![1_000, 0],
            },
            &[],
            2,
        )
        .unwrap();

        let priorities = policy
            .candidate_specific_priorities_with_lookahead(&candidates, 2, 2)
            .unwrap();
        let reversed = candidates.iter().rev().cloned().collect::<Vec<_>>();
        let reversed_priorities = policy
            .candidate_specific_priorities_with_lookahead(&reversed, 2, 2)
            .unwrap();

        assert_eq!(priorities, reversed_priorities);
        assert_eq!(
            priorities[&("npc_like".into(), "assay-a".into())].horizon_assays,
            2
        );
        assert_eq!(
            priorities[&("mes_like".into(), "assay-b".into())].horizon_assays,
            2
        );
    }
}

//! Transition-uncertainty-guided adaptive assay selection for preclinical glioma studies.
//!
//! The P10 state-transition analysis is descriptive, not causal. This bridge uses its arm- and
//! state-specific transition counts to find rows where either support is weak or a plausible
//! treatment/control shift remains. It increases sampling priority for those states, then hands
//! off to P06-F19's independent state posteriors and local assay executor. No cell states are
//! pooled and no transition priority is interpreted as efficacy or a treatment recommendation.

use super::state_stratified_campaign::{
    execute_glioma_state_stratified_campaign, GliomaResearchStratum,
    GliomaStateStratifiedAssayExecutor, StateStratifiedCampaign, StateStratifiedCampaignError,
    StateStratifiedCampaignRequest, StratifiedAssayCandidate, StratifiedAssayObservation,
};
use crate::glioma::programs::p10_interpretation_replication::state_transition::{
    analyze_glioma_state_transitions, StateTransitionAnalysis, StateTransitionObservation,
    StateTransitionRequest,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = super::adaptive_information_campaign::FEATURE_ID;
pub const OUTPUT_SCHEMA: &str = "GliomaTransitionGuidedStateCampaign1@3";
pub const WORKFLOW_OUTPUT_SCHEMA: &str = "GliomaStatePlasticityWorkflow1@3";
pub const SCORE_SCALE: u64 = 1_000;
const POSTERIOR_MASS_SCALE: u64 = 1_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionGuidanceRequest {
    /// Minimum independent source units in each arm before a state receives no support-gap bonus.
    pub min_source_units_per_arm: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionGuidedStatePriority {
    pub state_id: String,
    pub investigator_priority_milli: u16,
    pub control_transition_count: u32,
    pub treatment_transition_count: u32,
    pub control_source_unit_count: u32,
    pub treatment_source_unit_count: u32,
    pub support_gap_milli: u16,
    /// Largest observed change in a treatment/control transition contrast after omitting a batch.
    pub batch_sensitivity_distance_milli: u16,
    /// Fraction of contributing batches whose omission could not be evaluated due to support,
    /// expressed on the same 0..=1,000 scale. This is an evidence-coverage gap, not an effect.
    pub batch_robustness_gap_milli: u16,
    /// Combined batch-specific follow-up priority, kept separate from transition-posterior
    /// attention so untestable batch folds cannot mask state-level transition uncertainty.
    pub batch_followup_attention_milli: u16,
    /// Total-variation distance between the two posterior mean transition rows.
    pub posterior_mean_total_variation_milli: u16,
    /// Expected whole-row L2 distance, including observed shift and posterior uncertainty.
    pub posterior_expected_transition_distance_milli: u16,
    /// Uncertainty-only component of the whole-row L2 distance.
    pub transition_uncertainty_distance_milli: u16,
    /// Between-specimen transition-row disagreement, reported separately from mean shift.
    pub between_unit_heterogeneity_distance_milli: u16,
    pub transition_attention_milli: u16,
    pub adjusted_priority_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionGuidedStateCampaignRequest {
    pub campaign: StateStratifiedCampaignRequest,
    pub transition_guidance: TransitionGuidanceRequest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionGuidedStateCampaign {
    pub feature_id: String,
    pub output_schema: String,
    pub transition_analysis_digest: ContentHash,
    pub priorities: Vec<TransitionGuidedStatePriority>,
    pub campaign: StateStratifiedCampaign,
    pub digest: ContentHash,
}

/// Inputs for the closed-loop preclinical cell-state plasticity workflow. The longitudinal
/// observation set is analyzed first; its state-specific transition uncertainty then prioritizes
/// adaptive assays from the declared local candidate set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaStatePlasticityWorkflowRequest {
    pub transition_analysis: StateTransitionRequest,
    pub assay_campaign: TransitionGuidedStateCampaignRequest,
}

/// Result of a complete transition-analysis → state-prioritization → adaptive-assay workflow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaStatePlasticityResearchRun {
    pub feature_id: String,
    pub output_schema: String,
    pub transition_analysis: StateTransitionAnalysis,
    pub assay_campaign: TransitionGuidedStateCampaign,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TransitionGuidedStateCampaignError {
    #[error("transition-guided campaign request is invalid: {0}")]
    InvalidRequest(String),
    #[error("transition-guided campaign input is invalid: {0}")]
    InvalidInput(String),
    #[error("transition-guided campaign output is invalid: {0}")]
    InvalidOutput(String),
    #[error("state-stratified campaign failed: {0}")]
    Campaign(String),
    #[error("transition-guided campaign digest failed: {0}")]
    Digest(String),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GliomaStatePlasticityWorkflowError {
    #[error("glioma state-plasticity workflow input is invalid: {0}")]
    InvalidInput(String),
    #[error("glioma state-plasticity workflow output is invalid: {0}")]
    InvalidOutput(String),
    #[error("glioma state-plasticity assay campaign failed: {0}")]
    Campaign(String),
    #[error("glioma state-plasticity workflow digest failed: {0}")]
    Digest(String),
}

fn digest_input(run: &TransitionGuidedStateCampaign) -> serde_json::Value {
    serde_json::json!({
        "feature_id": run.feature_id,
        "output_schema": run.output_schema,
        "transition_analysis_digest": run.transition_analysis_digest,
        "priorities": run.priorities,
        "campaign_digest": run.campaign.digest,
    })
}

fn workflow_digest_input(run: &GliomaStatePlasticityResearchRun) -> serde_json::Value {
    serde_json::json!({
        "feature_id": run.feature_id,
        "output_schema": run.output_schema,
        "transition_analysis_digest": run.transition_analysis.digest,
        "assay_campaign_digest": run.assay_campaign.digest,
    })
}

fn integer_sqrt(value: u64) -> u64 {
    if value < 2 {
        return value;
    }
    let mut estimate = value / 2;
    loop {
        let next = (estimate + value / estimate) / 2;
        if next >= estimate {
            return estimate;
        }
        estimate = next;
    }
}

fn transition_count(analysis: &StateTransitionAnalysis, arm_id: &str, from_state: &str) -> u32 {
    analysis
        .cells
        .iter()
        .find(|cell| cell.arm_id == arm_id && cell.from_state == from_state)
        .map_or(0, |cell| cell.source_transition_count)
}

fn source_unit_count(analysis: &StateTransitionAnalysis, arm_id: &str, from_state: &str) -> u32 {
    analysis
        .cells
        .iter()
        .find(|cell| cell.arm_id == arm_id && cell.from_state == from_state)
        .map_or(0, |cell| cell.source_unit_count)
}

fn unit_transition_probabilities_milli(
    profile: &crate::glioma::programs::p10_interpretation_replication::state_transition::StateTransitionUnitProfile,
    state_order: &[String],
) -> Vec<u64> {
    let denominator = u64::from(profile.source_transition_count);
    if denominator == 0 {
        return vec![0; state_order.len()];
    }
    let mut probabilities = vec![0_u64; state_order.len()];
    let mut remainders = Vec::with_capacity(state_order.len());
    let mut assigned = 0_u64;
    for (index, destination) in state_order.iter().enumerate() {
        let count = profile
            .destination_counts
            .iter()
            .find(|item| item.to_state == *destination)
            .map_or(0_u64, |item| u64::from(item.transition_count));
        let scaled = count * SCORE_SCALE;
        let share = scaled / denominator;
        probabilities[index] = share;
        assigned += share;
        remainders.push((index, scaled % denominator));
    }
    remainders.sort_by(
        |(left_index, left_remainder), (right_index, right_remainder)| {
            right_remainder
                .cmp(left_remainder)
                .then_with(|| left_index.cmp(right_index))
        },
    );
    for (index, _) in remainders
        .into_iter()
        .take(SCORE_SCALE.saturating_sub(assigned) as usize)
    {
        probabilities[index] = probabilities[index].saturating_add(1);
    }
    probabilities
}

/// Give each independent source unit exactly one normalized transition row in the posterior.
/// Integer largest-remainder apportionment preserves each unit's row mass and makes ties stable;
/// repeated windows can refine that unit's row proportions but cannot increase its population
/// weight. This is an equal-unit Dirichlet approximation, not a random-effects model.
fn independent_unit_effective_counts(
    analysis: &StateTransitionAnalysis,
    arm_id: &str,
    from_state: &str,
) -> Vec<u64> {
    let profiles = analysis
        .unit_profiles
        .iter()
        .filter(|profile| profile.arm_id == arm_id && profile.from_state == from_state);
    let mut effective = vec![0_u64; analysis.state_order.len()];
    for profile in profiles {
        for (total, unit_probability) in
            effective
                .iter_mut()
                .zip(unit_transition_probabilities_milli(
                    profile,
                    &analysis.state_order,
                ))
        {
            *total = total.saturating_add(unit_probability);
        }
    }
    effective
}

/// Exact marginal variance of the equal-unit mean under Dirichlet(1) Bayesian-bootstrap weights.
/// This is reported as a biological-unit disagreement component, not folded into the treatment
/// effect or described as a causal/effect posterior.
fn between_unit_variance_milli_squared(
    analysis: &StateTransitionAnalysis,
    arm_id: &str,
    from_state: &str,
    to_state: &str,
) -> u64 {
    let destination_index = match analysis
        .state_order
        .iter()
        .position(|state| state == to_state)
    {
        Some(index) => index,
        None => return 0,
    };
    let unit_rows = analysis
        .unit_profiles
        .iter()
        .filter(|profile| profile.arm_id == arm_id && profile.from_state == from_state)
        .map(|profile| {
            unit_transition_probabilities_milli(profile, &analysis.state_order)[destination_index]
        })
        .collect::<Vec<_>>();
    let unit_count = unit_rows.len() as u128;
    if unit_count < 2 {
        return 0;
    }
    let row_sum = unit_rows
        .iter()
        .map(|value| u128::from(*value))
        .sum::<u128>();
    let centered_sum_squares = unit_rows
        .iter()
        .map(|value| {
            // Centered unit values can be negative when a specimen's transition probability is
            // below the arm mean, so retain the sign until after centering.
            let centered = i128::from(*value) * unit_count as i128 - row_sum as i128;
            let magnitude = centered.unsigned_abs();
            magnitude * magnitude
        })
        .sum::<u128>();
    // Convert Σ(n*x_i - Σx)^2 / [n^3(n+1)] to milli-probability squared.
    let denominator = unit_count
        .saturating_mul(unit_count)
        .saturating_mul(unit_count)
        .saturating_mul(unit_count.saturating_add(1));
    ((centered_sum_squares + denominator / 2) / denominator.max(1)).min(u128::from(u64::MAX)) as u64
}

fn posterior_mean_and_variance_milli(
    analysis: &StateTransitionAnalysis,
    arm_id: &str,
    from_state: &str,
    to_state: &str,
) -> (u64, u64) {
    let counts = independent_unit_effective_counts(analysis, arm_id, from_state);
    let total = counts.iter().copied().sum::<u64>();
    let alpha = counts
        .iter()
        .zip(&analysis.state_order)
        .find(|(_, destination)| destination.as_str() == to_state)
        .map_or(POSTERIOR_MASS_SCALE, |(count, _)| {
            count.saturating_add(POSTERIOR_MASS_SCALE)
        });
    let concentration = total
        .saturating_add((analysis.state_order.len() as u64).saturating_mul(POSTERIOR_MASS_SCALE));
    let mean_milli = alpha
        .saturating_mul(SCORE_SCALE)
        .saturating_add(concentration / 2)
        / concentration.max(1);
    let numerator = u128::from(alpha)
        .saturating_mul(u128::from(concentration.saturating_sub(alpha)))
        .saturating_mul(1_000_000)
        .saturating_mul(u128::from(POSTERIOR_MASS_SCALE));
    let denominator = u128::from(concentration)
        .saturating_mul(u128::from(concentration))
        .saturating_mul(u128::from(
            concentration.saturating_add(POSTERIOR_MASS_SCALE),
        ))
        .max(1);
    let variance_milli_squared =
        (numerator.saturating_add(denominator / 2) / denominator).min(u128::from(u64::MAX)) as u64;
    (mean_milli.min(SCORE_SCALE), variance_milli_squared)
}

fn normalize_priorities(raw: &BTreeMap<String, u64>) -> BTreeMap<String, u16> {
    let state_count = raw.len() as u64;
    let distributable = SCORE_SCALE.saturating_sub(state_count);
    let total = raw.values().copied().sum::<u64>().max(1);
    let mut output = BTreeMap::new();
    let mut remainders = Vec::with_capacity(raw.len());
    for (state_id, score) in raw {
        let scaled = score.saturating_mul(distributable);
        let share = scaled / total;
        output.insert(state_id.clone(), 1_u16.saturating_add(share as u16));
        remainders.push((state_id.clone(), scaled % total));
    }
    let assigned = output.values().map(|value| u64::from(*value)).sum::<u64>();
    let remaining = SCORE_SCALE.saturating_sub(assigned) as usize;
    remainders.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    for (state_id, _) in remainders.into_iter().take(remaining) {
        if let Some(weight) = output.get_mut(&state_id) {
            *weight = weight.saturating_add(1);
        }
    }
    output
}

/// Turn a validated P10 transition matrix into transparent per-state sampling priorities.
/// A symmetric unit Dirichlet prior stabilizes empty rows. Posterior concentration is capped at
/// independent source-unit support and averages normalized rows equally across specimens. A
/// separate Bayesian-bootstrap disagreement term rewards follow-up on states with heterogeneous
/// unit-level trajectories; it is a sampling-design signal, not an efficacy probability.
pub fn plan_glioma_transition_guided_state_priorities(
    request: &TransitionGuidanceRequest,
    analysis: &StateTransitionAnalysis,
    strata: &[GliomaResearchStratum],
) -> Result<Vec<TransitionGuidedStatePriority>, TransitionGuidedStateCampaignError> {
    analysis
        .validate()
        .map_err(|error| TransitionGuidedStateCampaignError::InvalidInput(error.to_string()))?;
    if request.min_source_units_per_arm == 0 || strata.is_empty() || strata.len() > 128 {
        return Err(TransitionGuidedStateCampaignError::InvalidRequest(
            "a positive independent-source-unit floor and 1..=128 strata are required".into(),
        ));
    }
    let stratum_ids = strata
        .iter()
        .map(|stratum| stratum.stratum_id.as_str())
        .collect::<BTreeSet<_>>();
    let weight_sum = strata
        .iter()
        .map(|stratum| u32::from(stratum.priority_weight_milli))
        .sum::<u32>();
    if stratum_ids.len() != strata.len()
        || strata.iter().any(|stratum| {
            stratum.stratum_id.trim().is_empty() || stratum.priority_weight_milli == 0
        })
        || weight_sum != SCORE_SCALE as u32
        || analysis
            .state_order
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>()
            != stratum_ids
    {
        return Err(TransitionGuidedStateCampaignError::InvalidInput(
            "strata must uniquely and exactly match the transition analysis states, with positive priorities summing to 1,000".into(),
        ));
    }

    let mut priorities = Vec::with_capacity(strata.len());
    let mut raw_scores = BTreeMap::new();
    for stratum in strata {
        let state_id = stratum.stratum_id.as_str();
        let control_transition_count = transition_count(analysis, &analysis.control_arm, state_id);
        let treatment_transition_count =
            transition_count(analysis, &analysis.treatment_arm, state_id);
        let control_source_unit_count =
            source_unit_count(analysis, &analysis.control_arm, state_id);
        let treatment_source_unit_count =
            source_unit_count(analysis, &analysis.treatment_arm, state_id);
        let min_support = control_source_unit_count.min(treatment_source_unit_count);
        let support_gap_milli = if min_support >= request.min_source_units_per_arm {
            0
        } else {
            (u64::from(request.min_source_units_per_arm - min_support) * SCORE_SCALE
                / u64::from(request.min_source_units_per_arm)) as u16
        };

        let mut absolute_mean_shift_sum_milli = 0_u64;
        let mut expected_squared_distance_milli = 0_u64;
        let mut uncertainty_squared_distance_milli = 0_u64;
        let mut between_unit_heterogeneity_squared_distance_milli = 0_u64;
        for destination in &analysis.state_order {
            let (control_mean, control_variance) = posterior_mean_and_variance_milli(
                analysis,
                &analysis.control_arm,
                state_id,
                destination,
            );
            let (treatment_mean, treatment_variance) = posterior_mean_and_variance_milli(
                analysis,
                &analysis.treatment_arm,
                state_id,
                destination,
            );
            let shift = control_mean.abs_diff(treatment_mean);
            let variance = control_variance.saturating_add(treatment_variance);
            let unit_heterogeneity = between_unit_variance_milli_squared(
                analysis,
                &analysis.control_arm,
                state_id,
                destination,
            )
            .saturating_add(between_unit_variance_milli_squared(
                analysis,
                &analysis.treatment_arm,
                state_id,
                destination,
            ));
            absolute_mean_shift_sum_milli = absolute_mean_shift_sum_milli.saturating_add(shift);
            expected_squared_distance_milli = expected_squared_distance_milli
                .saturating_add(shift.saturating_mul(shift))
                .saturating_add(variance);
            uncertainty_squared_distance_milli =
                uncertainty_squared_distance_milli.saturating_add(variance);
            between_unit_heterogeneity_squared_distance_milli =
                between_unit_heterogeneity_squared_distance_milli
                    .saturating_add(unit_heterogeneity);
        }
        // For two probability vectors, squared L2 distance is at most 2. Dividing the
        // posterior expected squared distance by 2 before the integer square root yields a
        // stable 0..=1,000 design-attention scale. Exact Dirichlet marginal variances account
        // for uncertainty without Monte Carlo noise or a max-over-destinations penalty.
        let posterior_expected_distance_milli =
            integer_sqrt(expected_squared_distance_milli / 2).min(SCORE_SCALE);
        let uncertainty_distance_milli =
            integer_sqrt(uncertainty_squared_distance_milli / 2).min(SCORE_SCALE);
        let between_unit_heterogeneity_distance_milli =
            integer_sqrt(between_unit_heterogeneity_squared_distance_milli / 2).min(SCORE_SCALE);
        let mean_total_variation_milli = (absolute_mean_shift_sum_milli / 2).min(SCORE_SCALE);
        let design_attention_distance_milli = integer_sqrt(
            expected_squared_distance_milli
                .saturating_add(between_unit_heterogeneity_squared_distance_milli)
                / 2,
        )
        .min(SCORE_SCALE);
        let batch_sensitivity = analysis
            .batch_sensitivity
            .iter()
            .find(|sensitivity| sensitivity.from_state == state_id)
            .expect("validated transition analysis has one batch sensitivity row per state");
        let batch_sensitivity_distance_milli =
            batch_sensitivity.maximum_leave_one_batch_out_deviation_milli;
        let batch_robustness_gap_milli = if batch_sensitivity.batch_count == 0 {
            SCORE_SCALE as u16
        } else {
            ((u64::from(
                batch_sensitivity
                    .batch_count
                    .saturating_sub(batch_sensitivity.evaluated_batch_count),
            ) * SCORE_SCALE)
                / u64::from(batch_sensitivity.batch_count)) as u16
        };
        let batch_followup_attention_milli =
            u64::from(batch_sensitivity_distance_milli).max(u64::from(batch_robustness_gap_milli));
        let attention = u64::from(support_gap_milli).max(design_attention_distance_milli);
        // Compose independently reported signals so a maximal batch-coverage gap cannot mask
        // measured transition uncertainty or specimen-level heterogeneity.
        let raw_score = u64::from(stratum.priority_weight_milli)
            .saturating_mul(SCORE_SCALE.saturating_add(attention))
            .saturating_mul(SCORE_SCALE.saturating_add(batch_followup_attention_milli))
            / SCORE_SCALE;
        raw_scores.insert(state_id.to_owned(), raw_score.max(1));
        priorities.push(TransitionGuidedStatePriority {
            state_id: state_id.to_owned(),
            investigator_priority_milli: stratum.priority_weight_milli,
            control_transition_count,
            treatment_transition_count,
            control_source_unit_count,
            treatment_source_unit_count,
            support_gap_milli,
            batch_sensitivity_distance_milli,
            batch_robustness_gap_milli,
            batch_followup_attention_milli: batch_followup_attention_milli as u16,
            posterior_mean_total_variation_milli: mean_total_variation_milli as u16,
            posterior_expected_transition_distance_milli: posterior_expected_distance_milli as u16,
            transition_uncertainty_distance_milli: uncertainty_distance_milli as u16,
            between_unit_heterogeneity_distance_milli: between_unit_heterogeneity_distance_milli
                as u16,
            transition_attention_milli: attention as u16,
            adjusted_priority_milli: 0,
        });
    }
    let normalized = normalize_priorities(&raw_scores);
    for priority in &mut priorities {
        priority.adjusted_priority_milli = normalized[&priority.state_id];
    }
    priorities.sort_by(|left, right| left.state_id.cmp(&right.state_id));
    Ok(priorities)
}

impl TransitionGuidedStateCampaign {
    pub fn validate(&self) -> Result<(), TransitionGuidedStateCampaignError> {
        self.campaign.validate().map_err(|error| {
            TransitionGuidedStateCampaignError::InvalidOutput(error.to_string())
        })?;
        let adjusted_sum = self
            .priorities
            .iter()
            .map(|priority| u32::from(priority.adjusted_priority_milli))
            .sum::<u32>();
        let priority_ids = self
            .priorities
            .iter()
            .map(|priority| priority.state_id.clone())
            .collect::<Vec<_>>();
        let campaign_priorities = self
            .campaign
            .final_posteriors
            .iter()
            .map(|posterior| {
                (
                    posterior.stratum_id.as_str(),
                    posterior.priority_weight_milli,
                )
            })
            .collect::<BTreeMap<_, _>>();
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.priorities.is_empty()
            || self.transition_analysis_digest.as_str().len() != 64
            || campaign_priorities.len() != self.priorities.len()
            || adjusted_sum != SCORE_SCALE as u32
            || priority_ids.windows(2).any(|pair| pair[0] >= pair[1])
            || self.priorities.iter().any(|priority| {
                priority.state_id.trim().is_empty()
                    || priority.investigator_priority_milli > SCORE_SCALE as u16
                    || priority.adjusted_priority_milli == 0
                    || priority.adjusted_priority_milli > SCORE_SCALE as u16
                    || priority.support_gap_milli > SCORE_SCALE as u16
                    || priority.batch_sensitivity_distance_milli > SCORE_SCALE as u16
                    || priority.batch_robustness_gap_milli > SCORE_SCALE as u16
                    || priority.batch_followup_attention_milli
                        != priority
                            .batch_sensitivity_distance_milli
                            .max(priority.batch_robustness_gap_milli)
                    || priority.transition_attention_milli > SCORE_SCALE as u16
                    || priority.posterior_mean_total_variation_milli > SCORE_SCALE as u16
                    || priority.posterior_expected_transition_distance_milli > SCORE_SCALE as u16
                    || priority.transition_uncertainty_distance_milli > SCORE_SCALE as u16
                    || priority.between_unit_heterogeneity_distance_milli > SCORE_SCALE as u16
                    || campaign_priorities.get(priority.state_id.as_str())
                        != Some(&priority.adjusted_priority_milli)
            })
        {
            return Err(TransitionGuidedStateCampaignError::InvalidOutput(
                "priority ordering, exact weight normalization, campaign alignment, or identity is invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| TransitionGuidedStateCampaignError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(TransitionGuidedStateCampaignError::InvalidOutput(
                "transition-guided campaign digest mismatch".into(),
            ));
        }
        Ok(())
    }
}

/// Prioritize and execute one local state-stratified assay at a time, using P10 longitudinal
/// transition uncertainty to direct P06 sampling effort while preserving separate mechanism models.
pub fn execute_glioma_transition_guided_state_campaign<E: GliomaStateStratifiedAssayExecutor>(
    request: &TransitionGuidedStateCampaignRequest,
    analysis: &StateTransitionAnalysis,
    strata: &[GliomaResearchStratum],
    candidates: &[StratifiedAssayCandidate],
    initial_observations: &[StratifiedAssayObservation],
    executor: &mut E,
) -> Result<TransitionGuidedStateCampaign, TransitionGuidedStateCampaignError> {
    if analysis.model_system != request.campaign.campaign.model_system {
        return Err(TransitionGuidedStateCampaignError::InvalidInput(
            "transition analysis and assay campaign must target the same preclinical model system"
                .into(),
        ));
    }
    let priorities = plan_glioma_transition_guided_state_priorities(
        &request.transition_guidance,
        analysis,
        strata,
    )?;
    let weights = priorities
        .iter()
        .map(|priority| (priority.state_id.as_str(), priority.adjusted_priority_milli))
        .collect::<BTreeMap<_, _>>();
    let guided_strata = strata
        .iter()
        .cloned()
        .map(|mut stratum| {
            stratum.priority_weight_milli = weights[stratum.stratum_id.as_str()];
            stratum
        })
        .collect::<Vec<_>>();
    let campaign = execute_glioma_state_stratified_campaign(
        &request.campaign,
        &guided_strata,
        candidates,
        initial_observations,
        executor,
    )
    .map_err(|error: StateStratifiedCampaignError| {
        TransitionGuidedStateCampaignError::Campaign(error.to_string())
    })?;
    let mut output = TransitionGuidedStateCampaign {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        transition_analysis_digest: analysis.digest.clone(),
        priorities,
        campaign,
        digest: ContentHash::of_bytes(b"unsealed-transition-guided-state-campaign"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| TransitionGuidedStateCampaignError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

impl GliomaStatePlasticityResearchRun {
    pub fn validate(&self) -> Result<(), GliomaStatePlasticityWorkflowError> {
        self.transition_analysis.validate().map_err(|error| {
            GliomaStatePlasticityWorkflowError::InvalidOutput(error.to_string())
        })?;
        self.assay_campaign.validate().map_err(|error| {
            GliomaStatePlasticityWorkflowError::InvalidOutput(error.to_string())
        })?;
        if self.feature_id != FEATURE_ID
            || self.output_schema != WORKFLOW_OUTPUT_SCHEMA
            || self.transition_analysis.model_system != self.assay_campaign.campaign.model_system
            || self.assay_campaign.transition_analysis_digest != self.transition_analysis.digest
        {
            return Err(GliomaStatePlasticityWorkflowError::InvalidOutput(
                "workflow identity or transition-analysis-to-campaign linkage is invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&workflow_digest_input(self))
            .map_err(|error| GliomaStatePlasticityWorkflowError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(GliomaStatePlasticityWorkflowError::InvalidOutput(
                "state-plasticity workflow digest mismatch".into(),
            ));
        }
        Ok(())
    }
}

/// Analyze longitudinal preclinical glioma observations, allocate next assays by transition-row
/// uncertainty, then execute the bounded state-stratified campaign one outcome at a time.
///
/// This is a bounded autonomy surface: the caller supplies the eligible assay candidates and a
/// local executor. It does not open instruments, infer missing outcomes, or make clinical claims.
pub fn execute_glioma_state_plasticity_research_workflow<E: GliomaStateStratifiedAssayExecutor>(
    request: &GliomaStatePlasticityWorkflowRequest,
    longitudinal_observations: &[StateTransitionObservation],
    strata: &[GliomaResearchStratum],
    assay_candidates: &[StratifiedAssayCandidate],
    initial_assay_observations: &[StratifiedAssayObservation],
    executor: &mut E,
) -> Result<GliomaStatePlasticityResearchRun, GliomaStatePlasticityWorkflowError> {
    if request.transition_analysis.model_system
        != request.assay_campaign.campaign.campaign.model_system
    {
        return Err(GliomaStatePlasticityWorkflowError::InvalidInput(
            "longitudinal transition analysis and adaptive assay campaign must target the same preclinical model system".into(),
        ));
    }
    let transition_analysis =
        analyze_glioma_state_transitions(&request.transition_analysis, longitudinal_observations)
            .map_err(|error| GliomaStatePlasticityWorkflowError::InvalidInput(error.to_string()))?;
    let assay_campaign = execute_glioma_transition_guided_state_campaign(
        &request.assay_campaign,
        &transition_analysis,
        strata,
        assay_candidates,
        initial_assay_observations,
        executor,
    )
    .map_err(|error| GliomaStatePlasticityWorkflowError::Campaign(error.to_string()))?;
    let mut run = GliomaStatePlasticityResearchRun {
        feature_id: FEATURE_ID.into(),
        output_schema: WORKFLOW_OUTPUT_SCHEMA.into(),
        transition_analysis,
        assay_campaign,
        digest: ContentHash::of_bytes(b"unsealed-glioma-state-plasticity-research-run"),
    };
    run.digest = ContentHash::of_value(&workflow_digest_input(&run))
        .map_err(|error| GliomaStatePlasticityWorkflowError::Digest(error.to_string()))?;
    run.validate()?;
    Ok(run)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p06_experiment_design::adaptive_information_campaign::{
        AdaptiveInformationCampaignRequest, AdaptiveInformationObservation,
    };
    use crate::glioma::programs::p06_experiment_design::information_design::{
        DesignAction, DesignMechanism, DesignOutcome,
    };
    use crate::glioma::programs::p06_experiment_design::state_stratified_campaign::{
        StateStratifiedCampaignRequest, StratifiedAssayCandidate,
    };
    use crate::glioma::programs::p10_interpretation_replication::state_transition::{
        analyze_glioma_state_transitions, StateTransitionObservation, StateTransitionRequest,
        StateTransitionUnitDestinationCount,
    };
    use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
    use std::collections::VecDeque;

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: id.into(),
            content_hash: ContentHash::of_bytes(id.as_bytes()),
            content_type: "application/vnd.aurora.preclinical-assay+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn transition_request() -> StateTransitionRequest {
        StateTransitionRequest {
            objective: "characterize preclinical glioma state transitions".into(),
            control_arm: "control".into(),
            treatment_arm: "treatment".into(),
            model_system: GliomaModelSystem::Organoid,
            state_order: vec!["mes".into(), "npc".into()],
            min_units_per_arm: 1,
            min_transitions_per_arm: 1,
            max_timepoint_gap: 1,
            min_contrast_milli: 100,
        }
    }

    fn transition_observations() -> Vec<StateTransitionObservation> {
        let mut observations = Vec::new();
        let units = [
            ("control", "mes-c1", "mes", "mes"),
            ("control", "mes-c2", "mes", "mes"),
            ("control", "npc-c1", "npc", "npc"),
            ("treatment", "mes-t1", "mes", "mes"),
            ("treatment", "mes-t2", "mes", "mes"),
            ("treatment", "npc-t1", "npc", "mes"),
        ];
        for (arm, unit, from, to) in units {
            observations.push(StateTransitionObservation {
                observation_id: format!("{unit}-before"),
                unit_id: unit.into(),
                arm_id: arm.into(),
                model_system: GliomaModelSystem::Organoid,
                batch_id: format!("{arm}-batch"),
                timepoint: 0,
                state_id: from.into(),
                state_score_milli: if from == "mes" { 800 } else { 300 },
                artifact: artifact(&format!("{unit}-before")),
            });
            observations.push(StateTransitionObservation {
                observation_id: format!("{unit}-after"),
                unit_id: unit.into(),
                arm_id: arm.into(),
                model_system: GliomaModelSystem::Organoid,
                batch_id: format!("{arm}-batch"),
                timepoint: 1,
                state_id: to.into(),
                state_score_milli: if to == "mes" { 800 } else { 300 },
                artifact: artifact(&format!("{unit}-after")),
            });
        }
        observations
    }

    fn transition_analysis() -> StateTransitionAnalysis {
        analyze_glioma_state_transitions(&transition_request(), &transition_observations()).unwrap()
    }

    fn transition_analysis_with_unit_heterogeneity(heterogeneous: bool) -> StateTransitionAnalysis {
        let mut observations = Vec::new();
        for arm in ["control", "treatment"] {
            let mes_sequences: [(&str, &[&str]); 2] = if heterogeneous {
                [
                    ("mes-stable", &["mes", "mes", "mes"]),
                    ("mes-switching", &["mes", "npc", "mes", "npc"]),
                ]
            } else {
                [
                    ("mes-balanced-a", &["mes", "mes", "npc", "mes"]),
                    ("mes-balanced-b", &["mes", "npc", "mes", "mes"]),
                ]
            };
            for (unit_id, states) in mes_sequences
                .into_iter()
                .chain([("npc-source", &["npc", "mes"][..])])
            {
                for (timepoint, state_id) in states.iter().enumerate() {
                    let observation_id = format!("{arm}-{unit_id}-{timepoint}");
                    observations.push(StateTransitionObservation {
                        observation_id: observation_id.clone(),
                        unit_id: format!("{arm}-{unit_id}"),
                        arm_id: arm.into(),
                        model_system: GliomaModelSystem::Organoid,
                        batch_id: format!("{arm}-batch"),
                        timepoint: timepoint as u32,
                        state_id: (*state_id).into(),
                        state_score_milli: if *state_id == "mes" { 800 } else { 300 },
                        artifact: artifact(&observation_id),
                    });
                }
            }
        }
        analyze_glioma_state_transitions(&transition_request(), &observations).unwrap()
    }

    fn stratum(state_id: &str) -> GliomaResearchStratum {
        GliomaResearchStratum {
            stratum_id: state_id.into(),
            label: format!("{state_id} glioma state"),
            priority_weight_milli: 500,
            mechanism_priors: vec![
                DesignMechanism {
                    mechanism_id: "m-a".into(),
                    prior_milli: 500,
                },
                DesignMechanism {
                    mechanism_id: "m-b".into(),
                    prior_milli: 500,
                },
            ],
        }
    }

    fn assay(state_id: &str) -> StratifiedAssayCandidate {
        let outcome = |id: &str, a: u16, b: u16| DesignOutcome {
            outcome_id: id.into(),
            label: id.into(),
            probability_milli_by_mechanism: [("m-a".into(), a), ("m-b".into(), b)].into(),
        };
        StratifiedAssayCandidate {
            stratum_id: state_id.into(),
            action: DesignAction {
                action_id: format!("{state_id}-orthogonal-assay"),
                feature_id: "GAF-GLIOMA-P06-F19".into(),
                label: format!("{state_id} orthogonal assay"),
                outcomes: vec![outcome("null", 900, 100), outcome("signal", 100, 900)],
                feasibility_milli: 1_000,
                risk_milli: 0,
                cost_units: 1,
                max_replicates: 1,
            },
            negative_outcome_order: vec!["null".into()],
        }
    }

    fn campaign_request() -> TransitionGuidedStateCampaignRequest {
        TransitionGuidedStateCampaignRequest {
            campaign: StateStratifiedCampaignRequest {
                campaign: AdaptiveInformationCampaignRequest {
                    objective: "validate state-specific glioma mechanisms".into(),
                    model_system: GliomaModelSystem::Organoid,
                    max_rounds: 1,
                    max_actions_per_round: 1,
                    budget_units: 2,
                    min_information_gain_milli: 1,
                    information_weight_milli: 1_000,
                    feasibility_weight_milli: 0,
                    risk_penalty_milli: 0,
                    cost_penalty_milli: 0,
                    risk_ceiling_milli: 1_000,
                    stop_concentration_milli: 1_000,
                },
                min_assays_per_stratum: 1,
            },
            transition_guidance: TransitionGuidanceRequest {
                min_source_units_per_arm: 3,
            },
        }
    }

    struct OneOutcome {
        outcome: VecDeque<String>,
    }

    impl GliomaStateStratifiedAssayExecutor for OneOutcome {
        fn execute_action(
            &mut self,
            _: &str,
            action: &DesignAction,
            _: u16,
        ) -> Result<
            AdaptiveInformationObservation,
            super::super::state_stratified_campaign::StateStratifiedExecutionFailure,
        > {
            Ok(AdaptiveInformationObservation {
                action_id: action.action_id.clone(),
                outcome_id: self.outcome.pop_front().unwrap_or_else(|| "null".into()),
                replicate_index: 1,
                artifact: artifact(&action.action_id),
            })
        }
    }

    #[test]
    fn posterior_transition_uncertainty_prioritizes_the_underobserved_state_and_executes_it_first()
    {
        let analysis = transition_analysis();
        let strata = vec![stratum("mes"), stratum("npc")];
        let priorities = plan_glioma_transition_guided_state_priorities(
            &campaign_request().transition_guidance,
            &analysis,
            &strata,
        )
        .unwrap();
        let mes = priorities
            .iter()
            .find(|item| item.state_id == "mes")
            .unwrap();
        let npc = priorities
            .iter()
            .find(|item| item.state_id == "npc")
            .unwrap();
        assert_eq!(mes.control_source_unit_count, 2);
        assert_eq!(mes.treatment_source_unit_count, 2);
        assert_eq!(mes.control_transition_count, 2);
        assert_eq!(mes.treatment_transition_count, 2);
        assert_eq!(npc.control_source_unit_count, 1);
        assert_eq!(npc.treatment_source_unit_count, 1);
        assert!(npc.support_gap_milli > mes.support_gap_milli);
        assert_eq!(npc.batch_sensitivity_distance_milli, 0);
        assert_eq!(npc.batch_robustness_gap_milli, 1_000);
        assert_eq!(npc.batch_followup_attention_milli, 1_000);
        assert!(npc.adjusted_priority_milli > mes.adjusted_priority_milli);
        assert_eq!(
            priorities
                .iter()
                .map(|item| u32::from(item.adjusted_priority_milli))
                .sum::<u32>(),
            1_000
        );

        let mut executor = OneOutcome {
            outcome: ["null".into()].into(),
        };
        let run = execute_glioma_transition_guided_state_campaign(
            &campaign_request(),
            &analysis,
            &strata,
            &[assay("mes"), assay("npc")],
            &[],
            &mut executor,
        )
        .unwrap();
        assert_eq!(run.campaign.rounds[0].stratum_id, "npc");
        run.validate().unwrap();
    }

    #[test]
    fn measured_batch_fragility_reprioritizes_and_executes_the_affected_state_first() {
        let mut balanced_batch_data = transition_observations();
        for observation in &mut balanced_batch_data {
            observation.batch_id = if observation.unit_id.ends_with('1') {
                "batch-a".into()
            } else {
                "batch-b".into()
            };
        }
        // The NPC state has opposite treatment/control patterns in the two batches; the MES
        // state remains stable in both. Each omission still leaves one independent unit per arm.
        for (arm, unit, destination, batch) in [
            ("control", "npc-c2", "mes", "batch-b"),
            ("treatment", "npc-t2", "npc", "batch-b"),
        ] {
            for (timepoint, state_id) in [(0, "npc"), (1, destination)] {
                let observation_id = format!("{unit}-{timepoint}");
                balanced_batch_data.push(StateTransitionObservation {
                    observation_id: observation_id.clone(),
                    unit_id: unit.into(),
                    arm_id: arm.into(),
                    model_system: GliomaModelSystem::Organoid,
                    batch_id: batch.into(),
                    timepoint,
                    state_id: state_id.into(),
                    state_score_milli: if state_id == "mes" { 800 } else { 300 },
                    artifact: artifact(&observation_id),
                });
            }
        }
        let fragile_analysis =
            analyze_glioma_state_transitions(&transition_request(), &balanced_batch_data).unwrap();
        let mut unstratified_batch_data = balanced_batch_data.clone();
        for observation in &mut unstratified_batch_data {
            observation.batch_id = "single-batch".into();
        }
        let unstratified_analysis =
            analyze_glioma_state_transitions(&transition_request(), &unstratified_batch_data)
                .unwrap();

        let strata = vec![stratum("mes"), stratum("npc")];
        let request = campaign_request();
        let fragile_priorities = plan_glioma_transition_guided_state_priorities(
            &request.transition_guidance,
            &fragile_analysis,
            &strata,
        )
        .unwrap();
        let unstratified_priorities = plan_glioma_transition_guided_state_priorities(
            &request.transition_guidance,
            &unstratified_analysis,
            &strata,
        )
        .unwrap();
        fn priority<'a>(
            items: &'a [TransitionGuidedStatePriority],
            state: &str,
        ) -> &'a TransitionGuidedStatePriority {
            items.iter().find(|item| item.state_id == state).unwrap()
        }
        assert_eq!(
            priority(&fragile_priorities, "mes").batch_followup_attention_milli,
            0
        );
        assert_eq!(
            priority(&fragile_priorities, "npc").batch_sensitivity_distance_milli,
            1_000
        );
        assert_eq!(
            priority(&fragile_priorities, "npc").batch_robustness_gap_milli,
            0
        );
        assert!(
            priority(&fragile_priorities, "npc").adjusted_priority_milli
                > priority(&unstratified_priorities, "npc").adjusted_priority_milli
        );

        let mut executor = OneOutcome {
            outcome: ["null".into()].into(),
        };
        let run = execute_glioma_transition_guided_state_campaign(
            &request,
            &fragile_analysis,
            &strata,
            &[assay("mes"), assay("npc")],
            &[],
            &mut executor,
        )
        .unwrap();
        assert_eq!(run.campaign.rounds[0].stratum_id, "npc");
        run.validate().unwrap();
    }

    #[test]
    fn dirichlet_transition_posterior_exposes_mean_shift_and_uncertainty_separately() {
        let analysis = transition_analysis();
        let repeated = transition_analysis_with_repeated_windows();
        let mes_control = posterior_mean_and_variance_milli(&analysis, "control", "mes", "mes");
        let mes_treatment = posterior_mean_and_variance_milli(&analysis, "treatment", "mes", "mes");
        let mes_destination = posterior_mean_and_variance_milli(&analysis, "control", "mes", "npc");

        assert_eq!(mes_control, (750, 37_500));
        assert_eq!(mes_treatment, mes_control);
        assert_eq!(mes_destination, (250, 37_500));
        for to_state in ["mes", "npc"] {
            assert_eq!(
                posterior_mean_and_variance_milli(&analysis, "control", "mes", to_state),
                posterior_mean_and_variance_milli(&repeated, "control", "mes", to_state),
                "repeated windows must not increase precision for control:mes->{to_state}"
            );
        }
        assert!(repeated.cells.iter().any(|cell| {
            cell.arm_id == "control"
                && cell.from_state == "mes"
                && cell.source_transition_count == 4
                && cell.source_unit_count == 2
        }));

        let priorities = plan_glioma_transition_guided_state_priorities(
            &campaign_request().transition_guidance,
            &analysis,
            &[stratum("mes"), stratum("npc")],
        )
        .unwrap();
        let mes = priorities
            .iter()
            .find(|item| item.state_id == "mes")
            .unwrap();
        assert_eq!(mes.posterior_mean_total_variation_milli, 0);
        assert!(mes.transition_uncertainty_distance_milli > 0);
        assert!(
            mes.posterior_expected_transition_distance_milli
                >= mes.transition_uncertainty_distance_milli
        );
    }

    #[test]
    fn between_specimen_disagreement_increases_sampling_attention_without_changing_the_mean() {
        let homogeneous = transition_analysis_with_unit_heterogeneity(false);
        let heterogeneous = transition_analysis_with_unit_heterogeneity(true);
        let strata = [stratum("mes"), stratum("npc")];
        let request = TransitionGuidanceRequest {
            min_source_units_per_arm: 1,
        };
        let homogeneous_priorities =
            plan_glioma_transition_guided_state_priorities(&request, &homogeneous, &strata)
                .unwrap();
        let heterogeneous_priorities =
            plan_glioma_transition_guided_state_priorities(&request, &heterogeneous, &strata)
                .unwrap();
        let homogeneous_mes = homogeneous_priorities
            .iter()
            .find(|priority| priority.state_id == "mes")
            .unwrap();
        let heterogeneous_mes = heterogeneous_priorities
            .iter()
            .find(|priority| priority.state_id == "mes")
            .unwrap();

        assert_eq!(
            homogeneous_mes.control_source_unit_count,
            heterogeneous_mes.control_source_unit_count
        );
        assert_eq!(
            homogeneous_mes.posterior_mean_total_variation_milli,
            heterogeneous_mes.posterior_mean_total_variation_milli
        );
        assert_eq!(
            homogeneous_mes.posterior_expected_transition_distance_milli,
            heterogeneous_mes.posterior_expected_transition_distance_milli
        );
        assert_eq!(homogeneous_mes.between_unit_heterogeneity_distance_milli, 0);
        assert!(heterogeneous_mes.between_unit_heterogeneity_distance_milli > 0);
        assert!(
            heterogeneous_mes.transition_attention_milli
                > homogeneous_mes.transition_attention_milli
        );
    }

    #[test]
    fn independent_unit_precision_cap_preserves_fractional_transition_proportions() {
        let mut analysis = transition_analysis();
        let profile = analysis
            .unit_profiles
            .iter_mut()
            .find(|profile| {
                profile.arm_id == "control"
                    && profile.unit_id == "mes-c1"
                    && profile.from_state == "mes"
            })
            .unwrap();
        profile.source_transition_count = 3;
        profile.destination_counts = vec![
            StateTransitionUnitDestinationCount {
                to_state: "mes".into(),
                transition_count: 1,
            },
            StateTransitionUnitDestinationCount {
                to_state: "npc".into(),
                transition_count: 2,
            },
        ];
        analysis.unit_profiles.retain(|profile| {
            profile.arm_id != "control"
                || profile.from_state != "mes"
                || profile.unit_id == "mes-c1"
        });
        assert_eq!(
            independent_unit_effective_counts(&analysis, "control", "mes"),
            vec![333, 667]
        );
        assert_eq!(
            posterior_mean_and_variance_milli(&analysis, "control", "mes", "mes").0,
            444
        );
        assert_eq!(
            posterior_mean_and_variance_milli(&analysis, "control", "mes", "npc").0,
            556
        );
    }

    fn transition_analysis_with_repeated_windows() -> StateTransitionAnalysis {
        let mut observations = transition_observations();
        let followup = observations
            .iter()
            .filter(|observation| {
                observation.timepoint == 1
                    && observation.arm_id == "control"
                    && observation.unit_id.starts_with("mes-c")
            })
            .cloned()
            .map(|mut observation| {
                observation.observation_id = format!("{}-repeat", observation.observation_id);
                observation.timepoint = 2;
                observation.artifact = artifact(&observation.observation_id);
                observation
            })
            .collect::<Vec<_>>();
        observations.extend(followup);
        analyze_glioma_state_transitions(&transition_request(), &observations).unwrap()
    }

    #[test]
    fn state_plasticity_workflow_runs_from_longitudinal_data_through_adaptive_assay_execution() {
        let request = GliomaStatePlasticityWorkflowRequest {
            transition_analysis: transition_request(),
            assay_campaign: campaign_request(),
        };
        let mut executor = OneOutcome {
            outcome: ["null".into()].into(),
        };
        let run = execute_glioma_state_plasticity_research_workflow(
            &request,
            &transition_observations(),
            &[stratum("mes"), stratum("npc")],
            &[assay("mes"), assay("npc")],
            &[],
            &mut executor,
        )
        .unwrap();

        assert_eq!(run.transition_analysis.unit_order.len(), 6);
        assert_eq!(run.assay_campaign.campaign.rounds[0].stratum_id, "npc");
        assert_eq!(run.assay_campaign.campaign.rounds[0].outcome_id, "null");
        run.validate().unwrap();
    }

    #[test]
    fn priority_plan_is_order_invariant_and_rejects_state_mismatch() {
        let analysis = transition_analysis();
        let request = campaign_request();
        let first = plan_glioma_transition_guided_state_priorities(
            &request.transition_guidance,
            &analysis,
            &[stratum("mes"), stratum("npc")],
        )
        .unwrap();
        let second = plan_glioma_transition_guided_state_priorities(
            &request.transition_guidance,
            &analysis,
            &[stratum("npc"), stratum("mes")],
        )
        .unwrap();
        assert_eq!(first, second);
        assert!(plan_glioma_transition_guided_state_priorities(
            &request.transition_guidance,
            &analysis,
            &[stratum("mes"), stratum("wrong-state")],
        )
        .is_err());
    }
}

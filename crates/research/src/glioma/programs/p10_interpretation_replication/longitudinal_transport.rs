//! Longitudinal transport of independent preclinical replication studies.
//!
//! The P10 roadmap names longitudinal replication transport as a next wave, but the configured
//! blueprint distribution contains no detailed module contract for it. The completeness, weighting,
//! and promotion rules here are therefore explicitly repository-defined. They preserve a common
//! eligible study set across the requested time grid, retain source artifact references, and never
//! impute a missing interval or make a clinical claim.

use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = super::lineage_dynamics::FEATURE_ID;
pub const OUTPUT_SCHEMA: &str = "GliomaLongitudinalTransport1@1";
pub const MAX_STUDIES: usize = 4_096;
pub const MAX_OBSERVATIONS: usize = 16_384;
pub const MAX_TIMEPOINTS: usize = 256;
pub const MAX_SIGNATURE_DIMENSIONS: usize = 256;
pub const MAX_UNCERTAINTY_MILLI: u64 = 1_000_000;
pub const MAX_EFFECT_ABS_MILLI: u64 = 1_000_000_000;
const WEIGHT_SCALE: u128 = 1_000_000_000_000;
const MAX_CONTRIBUTION_WEIGHT: u64 = 1_000_000_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LongitudinalTransportRequest {
    pub objective: String,
    pub estimand_id: String,
    pub effect_unit: String,
    pub target_model_system: GliomaModelSystem,
    pub target_signature: Vec<i64>,
    pub timepoint_order: Vec<u32>,
    pub min_studies: usize,
    pub min_replicates_per_timepoint: u16,
    pub min_quality_milli: u16,
    pub distance_scale_milli: u32,
    pub max_transport_gap_milli: u16,
    pub max_heterogeneity_milli: u16,
    pub min_direction_concordance_milli: u16,
    pub max_leave_one_out_shift_milli: u64,
    pub effect_threshold_milli_per_tick: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LongitudinalTransportObservation {
    pub study_id: String,
    pub estimand_id: String,
    pub effect_unit: String,
    pub timepoint: u32,
    pub model_system: GliomaModelSystem,
    pub population_signature: Vec<i64>,
    pub effect_milli: i64,
    pub uncertainty_milli: u64,
    pub replicate_count: u16,
    pub quality_milli: u16,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LongitudinalStudyExclusion {
    pub study_id: String,
    pub reason_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LongitudinalTransportContribution {
    pub study_id: String,
    pub source_model_system: GliomaModelSystem,
    pub distance_milli: u32,
    pub similarity_milli: u16,
    pub model_system_gap_milli: u16,
    pub weight: u64,
    pub effect_milli: i64,
    pub uncertainty_milli: u64,
    pub replicate_count: u16,
    pub quality_milli: u16,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LongitudinalTimepointEstimate {
    pub timepoint: u32,
    pub contributions: Vec<LongitudinalTransportContribution>,
    pub pooled_effect_milli: Option<i64>,
    pub pooled_uncertainty_milli: Option<u64>,
    pub transport_gap_milli: Option<u16>,
    pub heterogeneity_milli: Option<u16>,
    pub max_leave_one_out_shift_milli: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LongitudinalStudyTrend {
    pub study_id: String,
    pub slope_milli_per_tick: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LongitudinalTransportDisposition {
    Qualified,
    Negative,
    Heterogeneous,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LongitudinalTransportAnalysis {
    pub feature_id: String,
    pub output_schema: String,
    pub request: LongitudinalTransportRequest,
    /// Commitment to the exact request and source observation sequence.
    pub input_digest: ContentHash,
    pub study_order: Vec<String>,
    pub included_order: Vec<String>,
    pub exclusions: Vec<LongitudinalStudyExclusion>,
    pub timepoint_order: Vec<u32>,
    pub timepoints: Vec<LongitudinalTimepointEstimate>,
    pub study_trends: Vec<LongitudinalStudyTrend>,
    pub pooled_trend_milli_per_tick: Option<i64>,
    pub pooled_trend_uncertainty_milli_per_tick: Option<u64>,
    pub direction_concordance_milli: u16,
    pub max_transport_gap_milli: u16,
    pub max_heterogeneity_milli: u16,
    pub max_leave_one_out_shift_milli: u64,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: LongitudinalTransportDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LongitudinalTransportError {
    #[error("longitudinal transport request is invalid: {0}")]
    InvalidRequest(String),
    #[error("longitudinal transport observations are invalid: {0}")]
    InvalidObservation(String),
    #[error("longitudinal transport output is invalid: {0}")]
    InvalidOutput(String),
    #[error("longitudinal transport digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn valid_request(request: &LongitudinalTransportRequest) -> Result<(), LongitudinalTransportError> {
    if request.objective.trim().is_empty()
        || request.estimand_id.trim().is_empty()
        || request.effect_unit.trim().is_empty()
        || request.target_signature.is_empty()
        || request.target_signature.len() > MAX_SIGNATURE_DIMENSIONS
        || request.timepoint_order.len() < 2
        || request.timepoint_order.len() > MAX_TIMEPOINTS
        || !request
            .timepoint_order
            .windows(2)
            .all(|pair| pair[0] < pair[1])
        || !(2..=MAX_STUDIES).contains(&request.min_studies)
        || request.min_replicates_per_timepoint == 0
        || request.min_quality_milli > 1_000
        || request.distance_scale_milli == 0
        || request.max_transport_gap_milli > 1_000
        || request.max_heterogeneity_milli > 1_000
        || request.min_direction_concordance_milli > 1_000
        || request.effect_threshold_milli_per_tick == 0
    {
        return Err(LongitudinalTransportError::InvalidRequest(
            "objective, target signature, increasing time grid, study quorum, replicate/QC floors, and bounded promotion thresholds are required".into(),
        ));
    }
    Ok(())
}

fn validate_inputs<'a>(
    request: &LongitudinalTransportRequest,
    observations: &'a [LongitudinalTransportObservation],
) -> Result<
    BTreeMap<String, BTreeMap<u32, &'a LongitudinalTransportObservation>>,
    LongitudinalTransportError,
> {
    valid_request(request)?;
    if observations.len() > MAX_OBSERVATIONS {
        return Err(LongitudinalTransportError::InvalidObservation(
            "observation count exceeds the declared bound".into(),
        ));
    }
    let declared_timepoints = request
        .timepoint_order
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut grouped = BTreeMap::<String, BTreeMap<u32, &LongitudinalTransportObservation>>::new();
    let mut metadata = BTreeMap::<String, (GliomaModelSystem, Vec<i64>)>::new();
    for observation in observations {
        if observation.study_id.trim().is_empty()
            || observation.estimand_id != request.estimand_id
            || observation.effect_unit != request.effect_unit
            || !declared_timepoints.contains(&observation.timepoint)
            || observation.population_signature.len() != request.target_signature.len()
            || observation.effect_milli.unsigned_abs() > MAX_EFFECT_ABS_MILLI
            || observation.uncertainty_milli > MAX_UNCERTAINTY_MILLI
            || observation.replicate_count == 0
            || observation.quality_milli > 1_000
            || observation.artifact.validate().is_err()
            || !observation.artifact.local_only
            || observation.artifact.contains_human_data
            || observation.artifact.contains_direct_identifiers
        {
            return Err(LongitudinalTransportError::InvalidObservation(
                "each source must match the declared estimand and effect unit and provide a declared timepoint, matching signature dimensions, bounded effect/QC/uncertainty, replicates, and a local de-identified artifact".into(),
            ));
        }
        let current_metadata = (
            observation.model_system,
            observation.population_signature.clone(),
        );
        if metadata
            .insert(observation.study_id.clone(), current_metadata.clone())
            .is_some_and(|prior| prior != current_metadata)
        {
            return Err(LongitudinalTransportError::InvalidObservation(
                "a study's model system and population signature must remain stable across timepoints".into(),
            ));
        }
        let study = grouped.entry(observation.study_id.clone()).or_default();
        if study.insert(observation.timepoint, observation).is_some() {
            return Err(LongitudinalTransportError::InvalidObservation(
                "a study may contribute at most one effect observation per timepoint".into(),
            ));
        }
    }
    if grouped.len() > MAX_STUDIES {
        return Err(LongitudinalTransportError::InvalidObservation(
            "study count exceeds the declared bound".into(),
        ));
    }
    Ok(grouped)
}

fn input_digest(
    request: &LongitudinalTransportRequest,
    observations: &[LongitudinalTransportObservation],
) -> Result<ContentHash, LongitudinalTransportError> {
    #[derive(Serialize)]
    struct Input<'a> {
        input_schema: &'static str,
        request: &'a LongitudinalTransportRequest,
        observations: &'a [LongitudinalTransportObservation],
    }
    ContentHash::of_serializable(&Input {
        input_schema: "GliomaLongitudinalTransportInput1@1",
        request,
        observations,
    })
    .map_err(|error| LongitudinalTransportError::Digest(error.to_string()))
}

fn distance(left: &[i64], right: &[i64]) -> u32 {
    left.iter()
        .zip(right)
        .map(|(left, right)| (i128::from(*left) - i128::from(*right)).unsigned_abs())
        .fold(0_u128, u128::saturating_add)
        .min(u128::from(u32::MAX)) as u32
}

fn similarity(distance_milli: u32, scale_milli: u32) -> u16 {
    let scaled = u128::from(distance_milli)
        .saturating_mul(1_000)
        .checked_div(u128::from(scale_milli))
        .unwrap_or(1_000)
        .min(1_000) as u16;
    1_000_u16.saturating_sub(scaled)
}

fn model_system_gap(source: GliomaModelSystem, target: GliomaModelSystem) -> u16 {
    if source == target { 0 } else { 1_000 }
}

fn weight(observation: &LongitudinalTransportObservation, similarity_milli: u16) -> u64 {
    let numerator = u128::from(similarity_milli)
        .saturating_mul(u128::from(observation.quality_milli))
        .saturating_mul(u128::from(observation.replicate_count))
        .saturating_mul(WEIGHT_SCALE);
    let uncertainty = u128::from(observation.uncertainty_milli.max(1));
    let value = numerator
        .checked_div(uncertainty.saturating_mul(uncertainty))
        .unwrap_or(0)
        .max(1)
        .min(u128::from(MAX_CONTRIBUTION_WEIGHT));
    value as u64
}

fn median_u64(values: &mut [u64]) -> u64 {
    values.sort_unstable();
    values[(values.len() - 1) / 2]
}

fn weighted_effect_from(weighted_sum: i128, total_weight: u128) -> Option<i64> {
    if total_weight == 0 {
        return None;
    }
    Some(
        (weighted_sum / total_weight as i128).clamp(i128::from(i64::MIN), i128::from(i64::MAX))
            as i64,
    )
}

fn weighted_effect(contributions: &[LongitudinalTransportContribution]) -> Option<i64> {
    let total_weight = contributions
        .iter()
        .map(|contribution| u128::from(contribution.weight))
        .sum::<u128>();
    let weighted_sum = contributions.iter().fold(0_i128, |total, contribution| {
        total + i128::from(contribution.effect_milli) * i128::from(contribution.weight)
    });
    weighted_effect_from(weighted_sum, total_weight)
}

fn heterogeneity(effects: &[i64], pooled_effect_milli: i64) -> u16 {
    if effects.is_empty() {
        return 0;
    }
    let mut deviations = effects
        .iter()
        .map(|effect| {
            (i128::from(*effect) - i128::from(pooled_effect_milli))
                .unsigned_abs()
                .min(u128::from(u64::MAX)) as u64
        })
        .collect::<Vec<_>>();
    let mad = u128::from(median_u64(&mut deviations));
    let scale = i128::from(pooled_effect_milli).unsigned_abs().max(1);
    mad.saturating_mul(1_000)
        .checked_div(scale.saturating_add(mad))
        .unwrap_or(1_000)
        .min(1_000) as u16
}

fn leave_one_out_shift(contributions: &[LongitudinalTransportContribution], pooled: i64) -> u64 {
    if contributions.len() < 2 {
        return 0;
    }
    let total_weight = contributions
        .iter()
        .map(|contribution| u128::from(contribution.weight))
        .sum::<u128>();
    let weighted_sum = contributions.iter().fold(0_i128, |total, contribution| {
        total + i128::from(contribution.effect_milli) * i128::from(contribution.weight)
    });
    contributions
        .iter()
        .filter_map(|excluded| {
            let remaining_weight = total_weight - u128::from(excluded.weight);
            let remaining_sum =
                weighted_sum - i128::from(excluded.effect_milli) * i128::from(excluded.weight);
            weighted_effect_from(remaining_sum, remaining_weight).map(|effect| {
                (i128::from(effect) - i128::from(pooled))
                    .unsigned_abs()
                    .min(u128::from(u64::MAX)) as u64
            })
        })
        .max()
        .unwrap_or(0)
}

fn slope(timepoints: &[u32], effects: &[i64]) -> Option<i64> {
    if timepoints.len() < 2 || timepoints.len() != effects.len() {
        return None;
    }
    let xs = timepoints
        .iter()
        .map(|time| i128::from(*time))
        .collect::<Vec<_>>();
    let ys = effects
        .iter()
        .map(|effect| i128::from(*effect))
        .collect::<Vec<_>>();
    let n = xs.len() as i128;
    let sum_x = xs.iter().sum::<i128>();
    let sum_y = ys.iter().sum::<i128>();
    let sum_xx = xs.iter().map(|value| value * value).sum::<i128>();
    let sum_xy = xs.iter().zip(&ys).map(|(x, y)| x * y).sum::<i128>();
    let denominator = n * sum_xx - sum_x * sum_x;
    if denominator == 0 {
        return None;
    }
    let numerator = n * sum_xy - sum_x * sum_y;
    Some((numerator / denominator).clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64)
}

fn estimate_timepoint(
    timepoint: u32,
    contributions: Vec<LongitudinalTransportContribution>,
) -> LongitudinalTimepointEstimate {
    let pooled_effect = weighted_effect(&contributions);
    let (pooled_uncertainty, gap, heterogeneity_milli, leave_one_out) =
        if let Some(pooled) = pooled_effect {
            let total_weight = contributions
                .iter()
                .map(|contribution| u128::from(contribution.weight))
                .sum::<u128>();
            let uncertainty_numerator = contributions.iter().fold(0_u128, |total, contribution| {
                total.saturating_add(
                    u128::from(contribution.uncertainty_milli)
                        .saturating_mul(u128::from(contribution.weight)),
                )
            });
            let source_uncertainty = uncertainty_numerator
                .checked_div(total_weight.max(1))
                .unwrap_or(0)
                .min(u128::from(u64::MAX)) as u64;
            let effects = contributions
                .iter()
                .map(|contribution| contribution.effect_milli)
                .collect::<Vec<_>>();
            let spread = effects
                .iter()
                .map(|effect| {
                    (i128::from(*effect) - i128::from(pooled))
                        .unsigned_abs()
                        .min(u128::from(u64::MAX)) as u64
                })
                .collect::<Vec<_>>();
            let mut spread = spread;
            let mad = median_u64(&mut spread);
            let max_gap = contributions
                .iter()
                .map(|contribution| {
                    1_000_u16
                        .saturating_sub(contribution.similarity_milli)
                        .max(contribution.model_system_gap_milli)
                })
                .max()
                .unwrap_or(0);
            (
                Some(source_uncertainty.saturating_add(mad)),
                Some(max_gap),
                Some(heterogeneity(&effects, pooled)),
                Some(leave_one_out_shift(&contributions, pooled)),
            )
        } else {
            (None, None, None, None)
        };
    LongitudinalTimepointEstimate {
        timepoint,
        contributions,
        pooled_effect_milli: pooled_effect,
        pooled_uncertainty_milli: pooled_uncertainty,
        transport_gap_milli: gap,
        heterogeneity_milli,
        max_leave_one_out_shift_milli: leave_one_out,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TransportGateMetrics {
    pooled_trend_milli_per_tick: Option<i64>,
    direction_concordance_milli: u16,
    max_transport_gap_milli: u16,
    max_heterogeneity_milli: u16,
    max_leave_one_out_shift_milli: u64,
}

fn derive_disposition(
    request: &LongitudinalTransportRequest,
    included_count: usize,
    timepoints: &[LongitudinalTimepointEstimate],
    metrics: TransportGateMetrics,
) -> LongitudinalTransportDisposition {
    if included_count < request.min_studies
        || metrics.pooled_trend_milli_per_tick.is_none()
        || timepoints
            .iter()
            .any(|timepoint| timepoint.pooled_effect_milli.is_none())
        || metrics.max_transport_gap_milli > request.max_transport_gap_milli
    {
        LongitudinalTransportDisposition::Unresolved
    } else if metrics.max_heterogeneity_milli > request.max_heterogeneity_milli
        || metrics.max_leave_one_out_shift_milli > request.max_leave_one_out_shift_milli
        || metrics.direction_concordance_milli < request.min_direction_concordance_milli
    {
        LongitudinalTransportDisposition::Heterogeneous
    } else if metrics
        .pooled_trend_milli_per_tick
        .is_some_and(|value| value.unsigned_abs() < request.effect_threshold_milli_per_tick)
    {
        LongitudinalTransportDisposition::Negative
    } else {
        LongitudinalTransportDisposition::Qualified
    }
}

#[derive(Serialize)]
struct OutputDigest<'a> {
    feature_id: &'a str,
    output_schema: &'a str,
    request: &'a LongitudinalTransportRequest,
    input_digest: &'a ContentHash,
    study_order: &'a [String],
    included_order: &'a [String],
    exclusions: &'a [LongitudinalStudyExclusion],
    timepoint_order: &'a [u32],
    timepoints: &'a [LongitudinalTimepointEstimate],
    study_trends: &'a [LongitudinalStudyTrend],
    pooled_trend_milli_per_tick: Option<i64>,
    pooled_trend_uncertainty_milli_per_tick: Option<u64>,
    direction_concordance_milli: u16,
    max_transport_gap_milli: u16,
    max_heterogeneity_milli: u16,
    max_leave_one_out_shift_milli: u64,
    negative_evidence: &'a [String],
    uncertainty: &'a [String],
    disposition: LongitudinalTransportDisposition,
}

fn output_digest(
    output: &LongitudinalTransportAnalysis,
) -> Result<ContentHash, LongitudinalTransportError> {
    ContentHash::of_serializable(&OutputDigest {
        feature_id: &output.feature_id,
        output_schema: &output.output_schema,
        request: &output.request,
        input_digest: &output.input_digest,
        study_order: &output.study_order,
        included_order: &output.included_order,
        exclusions: &output.exclusions,
        timepoint_order: &output.timepoint_order,
        timepoints: &output.timepoints,
        study_trends: &output.study_trends,
        pooled_trend_milli_per_tick: output.pooled_trend_milli_per_tick,
        pooled_trend_uncertainty_milli_per_tick: output.pooled_trend_uncertainty_milli_per_tick,
        direction_concordance_milli: output.direction_concordance_milli,
        max_transport_gap_milli: output.max_transport_gap_milli,
        max_heterogeneity_milli: output.max_heterogeneity_milli,
        max_leave_one_out_shift_milli: output.max_leave_one_out_shift_milli,
        negative_evidence: &output.negative_evidence,
        uncertainty: &output.uncertainty,
        disposition: output.disposition,
    })
    .map_err(|error| LongitudinalTransportError::Digest(error.to_string()))
}

impl LongitudinalTransportAnalysis {
    pub fn validate(&self) -> Result<(), LongitudinalTransportError> {
        valid_request(&self.request)?;
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.input_digest.as_str().len() != 64
            || self.timepoint_order != self.request.timepoint_order
            || self.timepoints.len() != self.timepoint_order.len()
            || self
                .timepoints
                .iter()
                .zip(&self.timepoint_order)
                .any(|(estimate, timepoint)| estimate.timepoint != *timepoint)
            || !canonical(&self.study_order)
            || !canonical(&self.included_order)
            || !canonical(
                &self
                    .exclusions
                    .iter()
                    .map(|exclusion| exclusion.study_id.clone())
                    .collect::<Vec<_>>(),
            )
            || !canonical(
                &self
                    .study_trends
                    .iter()
                    .map(|trend| trend.study_id.clone())
                    .collect::<Vec<_>>(),
            )
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.direction_concordance_milli > 1_000
            || self.max_transport_gap_milli > 1_000
            || self.max_heterogeneity_milli > 1_000
        {
            return Err(LongitudinalTransportError::InvalidOutput(
                "identity, request, source partition, time grid, or canonical ordering is invalid"
                    .into(),
            ));
        }
        let included = self.included_order.iter().cloned().collect::<BTreeSet<_>>();
        let excluded = self
            .exclusions
            .iter()
            .map(|exclusion| exclusion.study_id.clone())
            .collect::<BTreeSet<_>>();
        let all = self.study_order.iter().cloned().collect::<BTreeSet<_>>();
        if included.intersection(&excluded).next().is_some()
            || included.union(&excluded).cloned().collect::<BTreeSet<_>>() != all
            || self.exclusions.iter().any(|exclusion| {
                exclusion.reason_order.is_empty() || !canonical(&exclusion.reason_order)
            })
            || self
                .study_trends
                .iter()
                .map(|trend| trend.study_id.clone())
                .collect::<BTreeSet<_>>()
                != included
        {
            return Err(LongitudinalTransportError::InvalidOutput(
                "included and excluded studies must partition every source and retain a trend for each included study".into(),
            ));
        }
        for estimate in &self.timepoints {
            let ids = estimate
                .contributions
                .iter()
                .map(|contribution| contribution.study_id.clone())
                .collect::<Vec<_>>();
            if ids != self.included_order
                || estimate.contributions.iter().any(|contribution| {
                    contribution.similarity_milli == 0
                        || contribution.similarity_milli > 1_000
                        || contribution.model_system_gap_milli > 1_000
                        || contribution.model_system_gap_milli
                            != model_system_gap(
                                contribution.source_model_system,
                                self.request.target_model_system,
                            )
                        || contribution.weight == 0
                        || contribution.weight > MAX_CONTRIBUTION_WEIGHT
                        || contribution.effect_milli.unsigned_abs() > MAX_EFFECT_ABS_MILLI
                        || contribution.uncertainty_milli > MAX_UNCERTAINTY_MILLI
                        || contribution.quality_milli > 1_000
                        || contribution.replicate_count == 0
                        || contribution.artifact.validate().is_err()
                        || !contribution.artifact.local_only
                        || contribution.artifact.contains_human_data
                        || contribution.artifact.contains_direct_identifiers
                })
                || estimate.pooled_effect_milli.is_some() != !estimate.contributions.is_empty()
                || estimate.pooled_uncertainty_milli.is_some()
                    != estimate.pooled_effect_milli.is_some()
                || estimate.transport_gap_milli.is_some() != estimate.pooled_effect_milli.is_some()
                || estimate.heterogeneity_milli.is_some() != estimate.pooled_effect_milli.is_some()
                || estimate.max_leave_one_out_shift_milli.is_some()
                    != estimate.pooled_effect_milli.is_some()
            {
                return Err(LongitudinalTransportError::InvalidOutput(
                    "timepoint contributions must preserve the common eligible cohort and valid local artifacts".into(),
                ));
            }
        }
        let expected_gap = self
            .timepoints
            .iter()
            .filter_map(|timepoint| timepoint.transport_gap_milli)
            .max()
            .unwrap_or(0);
        let expected_heterogeneity = self
            .timepoints
            .iter()
            .filter_map(|timepoint| timepoint.heterogeneity_milli)
            .max()
            .unwrap_or(0);
        let expected_influence = self
            .timepoints
            .iter()
            .filter_map(|timepoint| timepoint.max_leave_one_out_shift_milli)
            .max()
            .unwrap_or(0);
        if self.max_transport_gap_milli != expected_gap
            || self.max_heterogeneity_milli != expected_heterogeneity
            || self.max_leave_one_out_shift_milli != expected_influence
        {
            return Err(LongitudinalTransportError::InvalidOutput(
                "aggregate transport, heterogeneity, and influence summaries must derive from timepoint estimates".into(),
            ));
        }
        let expected_disposition = derive_disposition(
            &self.request,
            self.included_order.len(),
            &self.timepoints,
            TransportGateMetrics {
                pooled_trend_milli_per_tick: self.pooled_trend_milli_per_tick,
                direction_concordance_milli: self.direction_concordance_milli,
                max_transport_gap_milli: self.max_transport_gap_milli,
                max_heterogeneity_milli: self.max_heterogeneity_milli,
                max_leave_one_out_shift_milli: self.max_leave_one_out_shift_milli,
            },
        );
        if self.disposition != expected_disposition
            || self.disposition == LongitudinalTransportDisposition::Negative
                && self.negative_evidence.is_empty()
            || self.pooled_trend_milli_per_tick.is_some()
                != self.pooled_trend_uncertainty_milli_per_tick.is_some()
        {
            return Err(LongitudinalTransportError::InvalidOutput(
                "disposition, negative evidence, or trend uncertainty does not derive from the declared gates".into(),
            ));
        }
        if output_digest(self)? != self.digest {
            return Err(LongitudinalTransportError::InvalidOutput(
                "longitudinal transport digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }

    /// Verify this result against the exact source request and observations retained by the caller.
    pub fn validate_against(
        &self,
        request: &LongitudinalTransportRequest,
        observations: &[LongitudinalTransportObservation],
    ) -> Result<(), LongitudinalTransportError> {
        self.validate()?;
        validate_inputs(request, observations)?;
        if &self.request != request || input_digest(request, observations)? != self.input_digest {
            return Err(LongitudinalTransportError::InvalidOutput(
                "longitudinal transport input digest does not match the supplied request and sources".into(),
            ));
        }
        let expected = compile(request, observations)?;
        if self != &expected {
            return Err(LongitudinalTransportError::InvalidOutput(
                "longitudinal transport estimates or provenance do not derive from the supplied observations".into(),
            ));
        }
        Ok(())
    }
}

fn compile(
    request: &LongitudinalTransportRequest,
    observations: &[LongitudinalTransportObservation],
) -> Result<LongitudinalTransportAnalysis, LongitudinalTransportError> {
    let grouped = validate_inputs(request, observations)?;
    let mut included = Vec::new();
    let mut exclusions = Vec::new();
    let mut source_distances = BTreeMap::new();
    for (study_id, rows) in &grouped {
        let first = rows.values().next();
        let signature = first
            .map(|row| row.population_signature.as_slice())
            .unwrap_or(&[]);
        let model_system = first
            .map(|row| row.model_system)
            .unwrap_or(request.target_model_system);
        let distance_milli = distance(signature, &request.target_signature);
        let similarity_milli = similarity(distance_milli, request.distance_scale_milli);
        let model_gap_milli = model_system_gap(model_system, request.target_model_system);
        source_distances.insert(
            study_id.clone(),
            (
                distance_milli,
                similarity_milli,
                model_system,
                model_gap_milli,
            ),
        );
        let mut reasons = BTreeSet::new();
        if request
            .timepoint_order
            .iter()
            .any(|timepoint| !rows.contains_key(timepoint))
        {
            reasons.insert("required-time-grid-incomplete".to_string());
        }
        if request
            .timepoint_order
            .iter()
            .filter_map(|timepoint| rows.get(timepoint))
            .any(|row| row.replicate_count < request.min_replicates_per_timepoint)
        {
            reasons.insert("replicate-floor-not-met-at-every-timepoint".to_string());
        }
        if request
            .timepoint_order
            .iter()
            .filter_map(|timepoint| rows.get(timepoint))
            .any(|row| row.quality_milli < request.min_quality_milli)
        {
            reasons.insert("quality-floor-not-met-at-every-timepoint".to_string());
        }
        if similarity_milli == 0 {
            reasons.insert("population-signature-outside-transport-scale".to_string());
        }
        if 1_000_u16.saturating_sub(similarity_milli) > request.max_transport_gap_milli {
            reasons.insert("population-signature-gap-exceeds-tolerance".to_string());
        }
        if model_gap_milli > request.max_transport_gap_milli {
            reasons.insert("source-model-system-gap-exceeds-tolerance".to_string());
        }
        if reasons.is_empty() {
            included.push(study_id.clone());
        } else {
            exclusions.push(LongitudinalStudyExclusion {
                study_id: study_id.clone(),
                reason_order: reasons.into_iter().collect(),
            });
        }
    }
    let mut timepoints = Vec::with_capacity(request.timepoint_order.len());
    for timepoint in &request.timepoint_order {
        let contributions = included
            .iter()
            .filter_map(|study_id| {
                let observation = grouped.get(study_id)?.get(timepoint)?;
                let (distance_milli, similarity_milli, model_system, model_gap_milli) =
                    source_distances[study_id];
                Some(LongitudinalTransportContribution {
                    study_id: study_id.clone(),
                    source_model_system: model_system,
                    distance_milli,
                    similarity_milli,
                    model_system_gap_milli: model_gap_milli,
                    weight: weight(observation, similarity_milli),
                    effect_milli: observation.effect_milli,
                    uncertainty_milli: observation.uncertainty_milli,
                    replicate_count: observation.replicate_count,
                    quality_milli: observation.quality_milli,
                    artifact: observation.artifact.clone(),
                })
            })
            .collect::<Vec<_>>();
        timepoints.push(estimate_timepoint(*timepoint, contributions));
    }
    let study_trends = included
        .iter()
        .filter_map(|study_id| {
            let rows = grouped.get(study_id)?;
            let effects = request
                .timepoint_order
                .iter()
                .map(|timepoint| rows.get(timepoint).map(|row| row.effect_milli))
                .collect::<Option<Vec<_>>>()?;
            Some(LongitudinalStudyTrend {
                study_id: study_id.clone(),
                slope_milli_per_tick: slope(&request.timepoint_order, &effects)?,
            })
        })
        .collect::<Vec<_>>();
    let pooled_effects = timepoints
        .iter()
        .map(|timepoint| timepoint.pooled_effect_milli)
        .collect::<Option<Vec<_>>>();
    let pooled_trend = pooled_effects
        .as_ref()
        .and_then(|effects| slope(&request.timepoint_order, effects));
    let direction_concordance =
        if let (false, Some(pooled_trend)) = (study_trends.is_empty(), pooled_trend) {
            let pooled_sign = pooled_trend.signum();
            let matching = study_trends
                .iter()
                .filter(|trend| trend.slope_milli_per_tick.signum() == pooled_sign)
                .count();
            ((matching * 1_000) / study_trends.len()) as u16
        } else {
            0
        };
    let trend_uncertainty =
        if let (Some(first), Some(last)) = (timepoints.first(), timepoints.last()) {
            first
                .pooled_uncertainty_milli
                .zip(last.pooled_uncertainty_milli)
                .map(|(first, last)| {
                    first.saturating_add(last)
                        / u64::from(
                            request.timepoint_order.last().unwrap() - request.timepoint_order[0],
                        )
                })
        } else {
            None
        };
    let max_transport_gap = timepoints
        .iter()
        .filter_map(|timepoint| timepoint.transport_gap_milli)
        .max()
        .unwrap_or(0);
    let max_heterogeneity = timepoints
        .iter()
        .filter_map(|timepoint| timepoint.heterogeneity_milli)
        .max()
        .unwrap_or(0);
    let max_leave_one_out = timepoints
        .iter()
        .filter_map(|timepoint| timepoint.max_leave_one_out_shift_milli)
        .max()
        .unwrap_or(0);
    let disposition = derive_disposition(
        request,
        included.len(),
        &timepoints,
        TransportGateMetrics {
            pooled_trend_milli_per_tick: pooled_trend,
            direction_concordance_milli: direction_concordance,
            max_transport_gap_milli: max_transport_gap,
            max_heterogeneity_milli: max_heterogeneity,
            max_leave_one_out_shift_milli: max_leave_one_out,
        },
    );
    let mut uncertainty = BTreeSet::new();
    if included.len() < request.min_studies {
        uncertainty.insert("independent-study-quorum-not-met".to_string());
    }
    if timepoints
        .iter()
        .any(|timepoint| timepoint.pooled_effect_milli.is_none())
    {
        uncertainty.insert("one-or-more-timepoints-have-no-eligible-effect".to_string());
    }
    if max_transport_gap > request.max_transport_gap_milli {
        uncertainty.insert("target-model-transport-gap-exceeds-tolerance".to_string());
    }
    if timepoints.iter().any(|timepoint| {
        timepoint
            .contributions
            .iter()
            .any(|contribution| contribution.model_system_gap_milli > 0)
    }) {
        uncertainty.insert("different-source-model-system-included-under-maximum-gap".to_string());
    }
    if max_heterogeneity > request.max_heterogeneity_milli {
        uncertainty.insert("between-study-heterogeneity-exceeds-tolerance".to_string());
    }
    if max_leave_one_out > request.max_leave_one_out_shift_milli {
        uncertainty.insert("leave-one-study-out-influence-exceeds-tolerance".to_string());
    }
    if direction_concordance < request.min_direction_concordance_milli {
        uncertainty.insert("study-trend-direction-concordance-below-floor".to_string());
    }
    let negative_evidence = if disposition == LongitudinalTransportDisposition::Negative {
        vec!["stable-longitudinal-effect-below-declared-threshold".to_string()]
    } else {
        Vec::new()
    };
    let mut output = LongitudinalTransportAnalysis {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        request: request.clone(),
        input_digest: input_digest(request, observations)?,
        study_order: grouped.keys().cloned().collect(),
        included_order: included,
        exclusions,
        timepoint_order: request.timepoint_order.clone(),
        timepoints,
        study_trends,
        pooled_trend_milli_per_tick: pooled_trend,
        pooled_trend_uncertainty_milli_per_tick: trend_uncertainty,
        direction_concordance_milli: direction_concordance,
        max_transport_gap_milli: max_transport_gap,
        max_heterogeneity_milli: max_heterogeneity,
        max_leave_one_out_shift_milli: max_leave_one_out,
        negative_evidence,
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-longitudinal-transport"),
    };
    output.digest = output_digest(&output)?;
    output.validate()?;
    Ok(output)
}

/// Analyze longitudinal study effects against one target model and a predeclared time grid.
pub fn analyze_glioma_longitudinal_transport(
    request: &LongitudinalTransportRequest,
    observations: &[LongitudinalTransportObservation],
) -> Result<LongitudinalTransportAnalysis, LongitudinalTransportError> {
    compile(request, observations)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_bytes(label.as_bytes())
    }

    fn request() -> LongitudinalTransportRequest {
        LongitudinalTransportRequest {
            objective: "longitudinal transport fixture".into(),
            estimand_id: "viability-change-per-timepoint".into(),
            effect_unit: "normalized-signal-milli".into(),
            target_model_system: GliomaModelSystem::Organoid,
            target_signature: vec![100, 200],
            timepoint_order: vec![0, 10, 20],
            min_studies: 2,
            min_replicates_per_timepoint: 3,
            min_quality_milli: 800,
            distance_scale_milli: 500,
            max_transport_gap_milli: 500,
            max_heterogeneity_milli: 500,
            min_direction_concordance_milli: 1_000,
            max_leave_one_out_shift_milli: 100,
            effect_threshold_milli_per_tick: 5,
        }
    }

    fn observations() -> Vec<LongitudinalTransportObservation> {
        let mut rows = Vec::new();
        for (study, offset) in [("study-a", 0_i64), ("study-b", 2_i64), ("study-c", -1_i64)] {
            for (index, timepoint) in [0_u32, 10, 20].into_iter().enumerate() {
                rows.push(LongitudinalTransportObservation {
                    study_id: study.into(),
                    estimand_id: "viability-change-per-timepoint".into(),
                    effect_unit: "normalized-signal-milli".into(),
                    timepoint,
                    model_system: GliomaModelSystem::Organoid,
                    population_signature: vec![110, 210],
                    effect_milli: offset + index as i64 * 100,
                    uncertainty_milli: 10,
                    replicate_count: 4,
                    quality_milli: 950,
                    artifact: LocalArtifactRef {
                        artifact_id: format!("{study}-{timepoint}"),
                        content_hash: hash(&format!("{study}-{timepoint}")),
                        content_type: "application/vnd.aurora.glioma-study-summary+json".into(),
                        local_only: true,
                        contains_human_data: false,
                        contains_direct_identifiers: false,
                    },
                });
            }
        }
        rows
    }

    #[test]
    fn aligned_complete_studies_qualify_with_replayable_trend_and_sources() {
        let request = request();
        let observations = observations();
        let output = analyze_glioma_longitudinal_transport(&request, &observations).unwrap();
        assert_eq!(
            output.disposition,
            LongitudinalTransportDisposition::Qualified
        );
        assert_eq!(output.included_order, vec!["study-a", "study-b", "study-c"]);
        assert_eq!(output.pooled_trend_milli_per_tick, Some(10));
        assert_eq!(output.direction_concordance_milli, 1_000);
        assert_eq!(output.timepoints[1].contributions.len(), 3);
        assert_eq!(
            output.timepoints[1].contributions[0].artifact.artifact_id,
            "study-a-10"
        );
        output.validate_against(&request, &observations).unwrap();
    }

    #[test]
    fn different_model_system_requires_and_reports_the_maximum_gap() {
        let mut request = request();
        let mut observations = observations();
        for row in &mut observations {
            row.model_system = GliomaModelSystem::MouseModel;
        }

        let blocked = analyze_glioma_longitudinal_transport(&request, &observations).unwrap();
        assert_eq!(
            blocked.disposition,
            LongitudinalTransportDisposition::Unresolved
        );
        assert!(blocked.included_order.is_empty());
        assert!(blocked.exclusions.iter().all(|exclusion| {
            exclusion
                .reason_order
                .contains(&"source-model-system-gap-exceeds-tolerance".into())
        }));

        request.max_transport_gap_milli = 1_000;
        let allowed = analyze_glioma_longitudinal_transport(&request, &observations).unwrap();
        assert_eq!(
            allowed.disposition,
            LongitudinalTransportDisposition::Qualified
        );
        assert_eq!(allowed.max_transport_gap_milli, 1_000);
        assert!(allowed.timepoints.iter().all(|timepoint| {
            timepoint
                .contributions
                .iter()
                .all(|contribution| contribution.model_system_gap_milli == 1_000)
        }));
        assert!(
            allowed
                .uncertainty
                .contains(&"different-source-model-system-included-under-maximum-gap".into())
        );
        allowed.validate_against(&request, &observations).unwrap();
    }

    #[test]
    fn incomplete_study_is_excluded_from_every_timepoint_instead_of_imputed() {
        let request = request();
        let mut observations = observations();
        observations.retain(|row| !(row.study_id == "study-c" && row.timepoint == 10));
        let output = analyze_glioma_longitudinal_transport(&request, &observations).unwrap();
        assert_eq!(output.included_order, vec!["study-a", "study-b"]);
        assert_eq!(output.exclusions[0].study_id, "study-c");
        assert!(
            output.exclusions[0]
                .reason_order
                .contains(&"required-time-grid-incomplete".into())
        );
        assert!(output.timepoints.iter().all(|timepoint| {
            timepoint
                .contributions
                .iter()
                .map(|item| item.study_id.as_str())
                .collect::<Vec<_>>()
                == ["study-a", "study-b"]
        }));
    }

    #[test]
    fn opposite_study_trends_remain_heterogeneous_not_averaged_into_a_pass() {
        let request = request();
        let mut observations = observations();
        for row in observations
            .iter_mut()
            .filter(|row| row.study_id == "study-c")
        {
            row.effect_milli = 400 - (row.timepoint as i64 * 10);
        }
        let output = analyze_glioma_longitudinal_transport(&request, &observations).unwrap();
        assert_eq!(
            output.disposition,
            LongitudinalTransportDisposition::Heterogeneous
        );
        assert!(output.direction_concordance_milli < 1_000);
        assert!(
            output
                .uncertainty
                .contains(&"study-trend-direction-concordance-below-floor".into())
        );
    }

    #[test]
    fn empty_sources_are_explicitly_unresolved_not_a_zero_effect() {
        let request = request();
        let output = analyze_glioma_longitudinal_transport(&request, &[]).unwrap();
        assert_eq!(
            output.disposition,
            LongitudinalTransportDisposition::Unresolved
        );
        assert_eq!(output.pooled_trend_milli_per_tick, None);
        assert!(
            output
                .uncertainty
                .contains(&"independent-study-quorum-not-met".into())
        );
        assert!(
            output
                .timepoints
                .iter()
                .all(|timepoint| timepoint.pooled_effect_milli.is_none())
        );
    }

    #[test]
    fn source_rebinding_and_duplicate_timepoints_are_refused() {
        let request = request();
        let observations = observations();
        let output = analyze_glioma_longitudinal_transport(&request, &observations).unwrap();
        let mut rebound = observations.clone();
        rebound[0].study_id = "study-b".into();
        assert!(output.validate_against(&request, &rebound).is_err());

        let mut duplicate = observations;
        duplicate.push(duplicate[0].clone());
        assert!(matches!(
            analyze_glioma_longitudinal_transport(&request, &duplicate),
            Err(LongitudinalTransportError::InvalidObservation(_))
        ));
    }

    #[test]
    fn effect_magnitude_is_bounded_before_fixed_point_pooling() {
        let request = request();
        let mut observations = observations();
        observations[0].effect_milli = i64::MAX;
        assert!(matches!(
            analyze_glioma_longitudinal_transport(&request, &observations),
            Err(LongitudinalTransportError::InvalidObservation(_))
        ));
    }
}

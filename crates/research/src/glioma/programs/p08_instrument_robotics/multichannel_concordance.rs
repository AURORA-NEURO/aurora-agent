//! Local multichannel temporal alignment and concordance for preclinical glioma instruments.
//!
//! Instrument channels often share a biological pulse but arrive with clock offsets, missing
//! points, or a drifting connector. This feature searches a bounded integer lag, computes exact
//! fixed-point Pearson concordance and median absolute residuals, and admits only channels that
//! clear overlap, quality, correlation, and residual gates. Raw traces never appear in the
//! returned artifact and the route never executes hardware or makes a clinical decision.

use crate::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P08-F05";
pub const OUTPUT_SCHEMA: &str = "GliomaInstrumentMultichannelConcordance1@1";
pub const MAX_CHANNELS: usize = 256;
pub const MAX_POINTS_PER_CHANNEL: usize = 8_192;
pub const MAX_LAG_TICKS: i64 = 1_024;
pub const MAX_VALUE_MILLI: i64 = 1_000_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultichannelConcordanceRequest {
    pub objective: String,
    pub instrument_id: String,
    pub model_system: GliomaModelSystem,
    pub reference_channel_id: String,
    pub max_lag_ticks: i64,
    pub min_overlap_points: usize,
    pub min_correlation_milli: i16,
    pub max_residual_milli: u64,
    pub min_quality_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultichannelPoint {
    pub point_id: String,
    pub sequence_index: u32,
    pub time_ticks: i64,
    pub value_milli: i64,
    pub quality_milli: u16,
    pub local_only: bool,
    pub contains_human_data: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultichannelInput {
    pub channel_id: String,
    pub quality_milli: u16,
    pub points: Vec<MultichannelPoint>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelConcordanceDisposition {
    Reference,
    Qualified,
    QualityBlocked,
    LowOverlap,
    WeakCorrelation,
    ResidualBlocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelConcordance {
    pub channel_id: String,
    pub best_lag_ticks: i64,
    pub overlap_points: usize,
    pub correlation_milli: i16,
    pub residual_mad_milli: u64,
    pub quality_milli: u16,
    pub disposition: ChannelConcordanceDisposition,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultichannelConcordanceDisposition {
    Qualified,
    Partial,
    NoReference,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentMultichannelConcordance {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub instrument_id: String,
    pub model_system: GliomaModelSystem,
    pub reference_channel_id: String,
    pub channel_order: Vec<String>,
    pub alignments: Vec<ChannelConcordance>,
    pub qualified_channel_order: Vec<String>,
    pub blocked_channel_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: MultichannelConcordanceDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MultichannelConcordanceError {
    #[error("multichannel concordance request is invalid: {0}")]
    InvalidRequest(String),
    #[error("multichannel concordance input is invalid: {0}")]
    InvalidInput(String),
    #[error("multichannel concordance output is invalid: {0}")]
    InvalidOutput(String),
    #[error("multichannel concordance digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn integer_sqrt(value: u128) -> u128 {
    if value == 0 {
        return 0;
    }
    let mut low = 0_u128;
    let mut high = value.saturating_add(1);
    while low + 1 < high {
        let mid = low + (high - low) / 2;
        if mid <= value / mid {
            low = mid;
        } else {
            high = mid;
        }
    }
    low
}

fn median(values: &mut [u64]) -> u64 {
    values.sort_unstable();
    values[values.len() / 2]
}

fn digest_input(output: &InstrumentMultichannelConcordance) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "instrument_id": output.instrument_id,
        "model_system": output.model_system,
        "reference_channel_id": output.reference_channel_id,
        "channel_order": output.channel_order,
        "alignments": output.alignments,
        "qualified_channel_order": output.qualified_channel_order,
        "blocked_channel_order": output.blocked_channel_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

impl ChannelConcordance {
    pub fn validate(&self) -> Result<(), MultichannelConcordanceError> {
        if self.channel_id.trim().is_empty()
            || self.best_lag_ticks.abs() > MAX_LAG_TICKS
            || self.correlation_milli < -1_000
            || self.correlation_milli > 1_000
            || self.quality_milli > 1_000
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
        {
            return Err(MultichannelConcordanceError::InvalidOutput(
                "channel identity, lag, correlation, quality, or ordering is invalid".into(),
            ));
        }
        Ok(())
    }
}

impl InstrumentMultichannelConcordance {
    pub fn validate(&self) -> Result<(), MultichannelConcordanceError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.instrument_id.trim().is_empty()
            || self.reference_channel_id.trim().is_empty()
            || !canonical(&self.channel_order)
            || !canonical(&self.qualified_channel_order)
            || !canonical(&self.blocked_channel_order)
            || self
                .alignments
                .windows(2)
                .any(|pair| pair[0].channel_id >= pair[1].channel_id)
            || self.alignments.iter().any(|item| item.validate().is_err())
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
        {
            return Err(MultichannelConcordanceError::InvalidOutput(
                "identity, channel ordering, alignment, or limitation state is invalid".into(),
            ));
        }
        let channel_ids = self.channel_order.iter().cloned().collect::<BTreeSet<_>>();
        let alignment_ids = self
            .alignments
            .iter()
            .map(|alignment| alignment.channel_id.clone())
            .collect::<BTreeSet<_>>();
        let qualified_ids = self
            .qualified_channel_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let blocked_ids = self
            .blocked_channel_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        if channel_ids.len() != self.channel_order.len()
            || channel_ids != alignment_ids
            || qualified_ids.intersection(&blocked_ids).next().is_some()
            || qualified_ids
                .union(&blocked_ids)
                .any(|id| !channel_ids.contains(id))
            || !channel_ids.contains(&self.reference_channel_id)
        {
            return Err(MultichannelConcordanceError::InvalidOutput(
                "channel and disposition partitions do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| MultichannelConcordanceError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(MultichannelConcordanceError::InvalidOutput(
                "digest is not bound to multichannel concordance".into(),
            ));
        }
        Ok(())
    }
}

fn paired_statistics(
    reference: &[MultichannelPoint],
    channel: &[MultichannelPoint],
    lag: i64,
) -> Option<(usize, i16, u64)> {
    let reference_by_time = reference
        .iter()
        .map(|point| (point.time_ticks, point.value_milli))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut pairs = channel
        .iter()
        .filter_map(|point| {
            reference_by_time
                .get(&point.time_ticks.saturating_add(lag))
                .map(|reference| (*reference, point.value_milli))
        })
        .collect::<Vec<_>>();
    if pairs.is_empty() {
        return None;
    }
    let count = pairs.len() as i128;
    let reference_sum = pairs
        .iter()
        .map(|(reference, _)| i128::from(*reference))
        .sum::<i128>();
    let channel_sum = pairs
        .iter()
        .map(|(_, channel)| i128::from(*channel))
        .sum::<i128>();
    let reference_mean = reference_sum / count;
    let channel_mean = channel_sum / count;
    let mut covariance = 0_i128;
    let mut reference_variance = 0_i128;
    let mut channel_variance = 0_i128;
    let mut residuals = Vec::with_capacity(pairs.len());
    for (reference, channel) in pairs.drain(..) {
        let reference_delta = i128::from(reference) - reference_mean;
        let channel_delta = i128::from(channel) - channel_mean;
        covariance = covariance.saturating_add(reference_delta.saturating_mul(channel_delta));
        reference_variance =
            reference_variance.saturating_add(reference_delta.saturating_mul(reference_delta));
        channel_variance =
            channel_variance.saturating_add(channel_delta.saturating_mul(channel_delta));
        residuals.push(reference.abs_diff(channel));
    }
    let denominator = integer_sqrt(
        reference_variance
            .max(0)
            .unsigned_abs()
            .saturating_mul(channel_variance.max(0).unsigned_abs()),
    );
    let correlation = if denominator == 0 {
        0
    } else {
        (covariance.saturating_mul(1_000) / denominator as i128).clamp(-1_000, 1_000) as i16
    };
    Some((count as usize, correlation, median(&mut residuals)))
}

fn align_channel(
    reference: &MultichannelInput,
    channel: &MultichannelInput,
    request: &MultichannelConcordanceRequest,
) -> ChannelConcordance {
    let mut best: Option<(i64, usize, i16, u64)> = None;
    for lag in -request.max_lag_ticks..=request.max_lag_ticks {
        if let Some((overlap, correlation, residual)) =
            paired_statistics(&reference.points, &channel.points, lag)
        {
            let candidate = (lag, overlap, correlation, residual);
            let better = best.as_ref().is_none_or(|current| {
                candidate.1 > current.1
                    || (candidate.1 == current.1 && candidate.2 > current.2)
                    || (candidate.1 == current.1
                        && candidate.2 == current.2
                        && candidate.3 < current.3)
                    || (candidate.1 == current.1
                        && candidate.2 == current.2
                        && candidate.3 == current.3
                        && candidate.0.abs() < current.0.abs())
            });
            if better {
                best = Some(candidate);
            }
        }
    }
    let (best_lag_ticks, overlap_points, correlation_milli, residual_mad_milli) =
        best.unwrap_or((0, 0, 0, 0));
    let mut negative_evidence = Vec::new();
    let mut uncertainty = Vec::new();
    let disposition = if channel.quality_milli < request.min_quality_milli {
        negative_evidence.push("channel quality is below the alignment floor".into());
        ChannelConcordanceDisposition::QualityBlocked
    } else if best.is_none() || overlap_points < request.min_overlap_points {
        negative_evidence.push("channel has insufficient time-aligned overlap".into());
        ChannelConcordanceDisposition::LowOverlap
    } else if correlation_milli < request.min_correlation_milli {
        negative_evidence.push("channel correlation is below the concordance floor".into());
        ChannelConcordanceDisposition::WeakCorrelation
    } else if residual_mad_milli > request.max_residual_milli {
        negative_evidence.push("channel residual MAD exceeds the concordance ceiling".into());
        ChannelConcordanceDisposition::ResidualBlocked
    } else {
        ChannelConcordanceDisposition::Qualified
    };
    if overlap_points < request.min_overlap_points.saturating_mul(2) {
        uncertainty.push("alignment is supported by a narrow overlap window".into());
    }
    negative_evidence.sort();
    uncertainty.sort();
    ChannelConcordance {
        channel_id: channel.channel_id.clone(),
        best_lag_ticks,
        overlap_points,
        correlation_milli,
        residual_mad_milli,
        quality_milli: channel.quality_milli,
        disposition,
        negative_evidence,
        uncertainty,
    }
}

pub fn analyze_glioma_instrument_multichannel_concordance(
    request: &MultichannelConcordanceRequest,
    channels: &[MultichannelInput],
) -> Result<InstrumentMultichannelConcordance, MultichannelConcordanceError> {
    if request.objective.trim().is_empty()
        || request.instrument_id.trim().is_empty()
        || request.reference_channel_id.trim().is_empty()
        || request.max_lag_ticks < 0
        || request.max_lag_ticks > MAX_LAG_TICKS
        || request.min_overlap_points == 0
        || request.min_quality_milli > 1_000
        || request.min_correlation_milli < -1_000
        || request.min_correlation_milli > 1_000
        || request.max_residual_milli > MAX_VALUE_MILLI as u64
    {
        return Err(MultichannelConcordanceError::InvalidRequest(
            "objective, instrument, lag, overlap, quality, correlation, and residual gates are invalid"
                .into(),
        ));
    }
    if channels.len() < 2 || channels.len() > MAX_CHANNELS {
        return Err(MultichannelConcordanceError::InvalidInput(
            "a bounded multi-channel input is required".into(),
        ));
    }
    let mut channel_ids = BTreeSet::new();
    let mut reference = None;
    for channel in channels {
        if !channel_ids.insert(channel.channel_id.clone())
            || channel.channel_id.trim().is_empty()
            || channel.points.is_empty()
            || channel.points.len() > MAX_POINTS_PER_CHANNEL
            || channel.quality_milli > 1_000
            || channel.points.windows(2).any(|pair| {
                pair[0].sequence_index >= pair[1].sequence_index
                    || pair[0].time_ticks >= pair[1].time_ticks
            })
            || channel.points.iter().any(|point| {
                point.point_id.trim().is_empty()
                    || point.value_milli.abs() > MAX_VALUE_MILLI
                    || point.quality_milli > 1_000
                    || !point.local_only
                    || point.contains_human_data
            })
        {
            return Err(MultichannelConcordanceError::InvalidInput(
                "channel identity, ordering, value bounds, locality, or privacy is invalid".into(),
            ));
        }
        if channel.channel_id == request.reference_channel_id {
            reference = Some(channel);
        }
    }
    let reference = reference.ok_or_else(|| {
        MultichannelConcordanceError::InvalidInput(
            "declared reference channel is not present".into(),
        )
    })?;
    let mut alignments = channels
        .iter()
        .map(|channel| {
            if channel.channel_id == reference.channel_id {
                ChannelConcordance {
                    channel_id: channel.channel_id.clone(),
                    best_lag_ticks: 0,
                    overlap_points: channel.points.len(),
                    correlation_milli: 1_000,
                    residual_mad_milli: 0,
                    quality_milli: channel.quality_milli,
                    disposition: ChannelConcordanceDisposition::Reference,
                    negative_evidence: Vec::new(),
                    uncertainty: Vec::new(),
                }
            } else {
                align_channel(reference, channel, request)
            }
        })
        .collect::<Vec<_>>();
    alignments.sort_by(|left, right| left.channel_id.cmp(&right.channel_id));
    let channel_order = alignments
        .iter()
        .map(|alignment| alignment.channel_id.clone())
        .collect::<Vec<_>>();
    let qualified_channel_order = alignments
        .iter()
        .filter(|alignment| {
            matches!(
                alignment.disposition,
                ChannelConcordanceDisposition::Qualified | ChannelConcordanceDisposition::Reference
            )
        })
        .map(|alignment| alignment.channel_id.clone())
        .collect::<Vec<_>>();
    let blocked_channel_order = alignments
        .iter()
        .filter(|alignment| !qualified_channel_order.contains(&alignment.channel_id))
        .map(|alignment| alignment.channel_id.clone())
        .collect::<Vec<_>>();
    let mut negative_evidence = alignments
        .iter()
        .flat_map(|alignment| alignment.negative_evidence.iter().cloned())
        .collect::<Vec<_>>();
    let mut uncertainty = alignments
        .iter()
        .flat_map(|alignment| alignment.uncertainty.iter().cloned())
        .collect::<Vec<_>>();
    negative_evidence.sort();
    negative_evidence.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    let disposition = if qualified_channel_order.len() == channel_order.len() {
        MultichannelConcordanceDisposition::Qualified
    } else if qualified_channel_order.len() > 1 {
        MultichannelConcordanceDisposition::Partial
    } else {
        MultichannelConcordanceDisposition::Unresolved
    };
    let mut output = InstrumentMultichannelConcordance {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        instrument_id: request.instrument_id.clone(),
        model_system: request.model_system,
        reference_channel_id: request.reference_channel_id.clone(),
        channel_order,
        alignments,
        qualified_channel_order,
        blocked_channel_order,
        negative_evidence,
        uncertainty,
        disposition,
        digest: ContentHash::of_value(&serde_json::Value::Null)
            .map_err(|error| MultichannelConcordanceError::Digest(error.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| MultichannelConcordanceError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(channel: &str, index: u32, time: i64, value: i64) -> MultichannelPoint {
        MultichannelPoint {
            point_id: format!("{channel}-{index}"),
            sequence_index: index,
            time_ticks: time,
            value_milli: value,
            quality_milli: 950,
            local_only: true,
            contains_human_data: false,
        }
    }

    fn request() -> MultichannelConcordanceRequest {
        MultichannelConcordanceRequest {
            objective: "align glioma imaging channels".into(),
            instrument_id: "microscope-01".into(),
            model_system: GliomaModelSystem::Organoid,
            reference_channel_id: "reference".into(),
            max_lag_ticks: 8,
            min_overlap_points: 3,
            min_correlation_milli: 900,
            max_residual_milli: 5,
            min_quality_milli: 800,
        }
    }

    #[test]
    fn recovers_shift_and_qualifies_concordant_channel() {
        let reference = MultichannelInput {
            channel_id: "reference".into(),
            quality_milli: 950,
            points: vec![
                point("reference", 0, 0, 10),
                point("reference", 1, 1, 20),
                point("reference", 2, 2, 30),
                point("reference", 3, 3, 40),
            ],
        };
        let shifted = MultichannelInput {
            channel_id: "shifted".into(),
            quality_milli: 940,
            points: vec![
                point("shifted", 0, 3, 10),
                point("shifted", 1, 4, 20),
                point("shifted", 2, 5, 30),
                point("shifted", 3, 6, 40),
            ],
        };
        let output =
            analyze_glioma_instrument_multichannel_concordance(&request(), &[reference, shifted])
                .expect("alignment");
        let result = output
            .alignments
            .iter()
            .find(|alignment| alignment.channel_id == "shifted")
            .expect("shifted result");
        assert_eq!(result.best_lag_ticks, -3);
        assert_eq!(result.disposition, ChannelConcordanceDisposition::Qualified);
        output.validate().expect("digest validates");
    }

    #[test]
    fn negative_concordance_remains_blocked() {
        let reference = MultichannelInput {
            channel_id: "reference".into(),
            quality_milli: 950,
            points: vec![
                point("reference", 0, 0, 10),
                point("reference", 1, 1, 20),
                point("reference", 2, 2, 30),
            ],
        };
        let inverse = MultichannelInput {
            channel_id: "inverse".into(),
            quality_milli: 940,
            points: vec![
                point("inverse", 0, 0, -10),
                point("inverse", 1, 1, -20),
                point("inverse", 2, 2, -30),
            ],
        };
        let output =
            analyze_glioma_instrument_multichannel_concordance(&request(), &[reference, inverse])
                .expect("alignment");
        assert_eq!(
            output.disposition,
            MultichannelConcordanceDisposition::Unresolved
        );
        assert!(output
            .negative_evidence
            .iter()
            .any(|item| item.contains("correlation")));
    }
}

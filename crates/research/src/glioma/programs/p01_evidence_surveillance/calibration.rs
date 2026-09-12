//! Source-family calibration for autonomous preclinical glioma evidence surveillance.
//!
//! Evidence scores are useful only when their relationship to later verification is measurable.
//! This feature calibrates source-family support scores against resolved local outcomes using
//! quality-weighted isotonic regression.  It keeps unknown and negative outcomes visible, emits
//! review actions for under-observed or miscalibrated families, and produces deterministic
//! fixed-point values suitable for P02/P04/P07 planning.  It never fetches literature, infers a
//! contradiction from text, or converts calibration into a clinical decision.

use crate::glioma::evidence::EvidenceState;
use crate::glioma_engine::LocalArtifactRef;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P01-F11";
pub const OUTPUT_SCHEMA: &str = "GliomaEvidenceCalibration1@1";
pub const MAX_OBSERVATIONS: usize = 100_000;
pub const MAX_FAMILIES: usize = 4_096;
pub const MAX_BINS: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceCalibrationRequest {
    pub objective: String,
    pub bin_count: usize,
    pub min_observations_per_family: usize,
    pub min_resolved_per_bin: usize,
    pub max_expected_calibration_error_milli: u16,
}

/// A score/outcome pair from a local validation or replication loop.  The artifact reference is
/// metadata-only; raw source bytes remain in the institution-local store.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceCalibrationObservation {
    pub observation_id: String,
    pub source_family: String,
    pub predicted_support_milli: u16,
    pub outcome: EvidenceState,
    pub quality_milli: u16,
    pub independent_group: String,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CalibrationBinDisposition {
    Qualified,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalibrationBin {
    pub source_family: String,
    pub bin_index: usize,
    pub lower_bound_milli: u16,
    pub upper_bound_milli: u16,
    pub observation_order: Vec<String>,
    pub resolved_count: usize,
    pub unknown_count: usize,
    pub predicted_support_milli: u16,
    pub observed_support_milli: u16,
    pub calibrated_support_milli: u16,
    pub brier_milli: u16,
    pub disposition: CalibrationBinDisposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceCalibrationDisposition {
    Qualified,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceCalibration {
    pub source_family: String,
    pub observation_count: usize,
    pub resolved_count: usize,
    pub independent_group_count: usize,
    pub brier_milli: u16,
    pub expected_calibration_error_milli: u16,
    pub calibrated_reliability_milli: u16,
    pub disposition: SourceCalibrationDisposition,
    pub uncertainty: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceCalibrationDisposition {
    Qualified,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceCalibrationAnalysis {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub source_family_order: Vec<String>,
    pub bins: Vec<CalibrationBin>,
    pub families: Vec<SourceCalibration>,
    pub review_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: EvidenceCalibrationDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EvidenceCalibrationError {
    #[error("evidence calibration request is invalid: {0}")]
    InvalidRequest(String),
    #[error("evidence calibration observation is invalid: {0}")]
    InvalidObservation(String),
    #[error("evidence calibration output is invalid: {0}")]
    InvalidOutput(String),
    #[error("evidence calibration digest failed: {0}")]
    Digest(String),
}

#[derive(Debug, Clone)]
struct BinAggregate {
    observation_order: Vec<String>,
    resolved_count: usize,
    unknown_count: usize,
    predicted_weighted_sum: u64,
    quality_sum: u64,
    resolved_quality_sum: u64,
    observed_weighted_sum: u64,
    brier_weighted_sum: u64,
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn resolved(outcome: EvidenceState) -> Option<u16> {
    match outcome {
        EvidenceState::Supported => Some(1_000),
        EvidenceState::Negative | EvidenceState::Contradicted => Some(0),
        EvidenceState::Unknown | EvidenceState::Stale | EvidenceState::Unmeasured => None,
    }
}

fn digest_input(output: &EvidenceCalibrationAnalysis) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "source_family_order": output.source_family_order,
        "bins": output.bins,
        "families": output.families,
        "review_order": output.review_order,
        "negative_evidence_order": output.negative_evidence_order,
        "unknown_order": output.unknown_order,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn bin_index(score_milli: u16, bin_count: usize) -> usize {
    (usize::from(score_milli) * bin_count / 1_001).min(bin_count.saturating_sub(1))
}

fn bin_bounds(index: usize, bin_count: usize) -> (u16, u16) {
    let lower = (index * 1_001 / bin_count).min(1_000) as u16;
    let upper = (((index + 1) * 1_001 / bin_count).saturating_sub(1)).min(1_000) as u16;
    (lower, upper)
}

fn isotonic_rates(aggregates: &[BinAggregate]) -> Vec<u16> {
    #[derive(Debug, Clone)]
    struct Block {
        start: usize,
        end: usize,
        successes: u64,
        weight: u64,
    }
    let active = aggregates
        .iter()
        .enumerate()
        .filter(|(_, aggregate)| aggregate.resolved_quality_sum > 0)
        .collect::<Vec<_>>();
    let mut rates = vec![0_u16; aggregates.len()];
    if active.is_empty() {
        return rates;
    }
    let mut blocks = Vec::new();
    for (active_index, (_, aggregate)) in active.iter().enumerate() {
        let successes = aggregate.observed_weighted_sum;
        let weight = aggregate.resolved_quality_sum;
        blocks.push(Block {
            start: active_index,
            end: active_index,
            successes,
            weight,
        });
        while blocks.len() >= 2 {
            let right = blocks.len() - 1;
            let left = right - 1;
            let left_rate = blocks[left].successes * 1_000_000 / blocks[left].weight;
            let right_rate = blocks[right].successes * 1_000_000 / blocks[right].weight;
            if left_rate <= right_rate {
                break;
            }
            let right_block = blocks.pop().expect("right block exists");
            let left_block = blocks.pop().expect("left block exists");
            blocks.push(Block {
                start: left_block.start,
                end: right_block.end,
                successes: left_block.successes + right_block.successes,
                weight: left_block.weight + right_block.weight,
            });
        }
    }
    for block in blocks {
        let rate = (block.successes * 1_000 / block.weight).min(1_000) as u16;
        for active_index in block.start..=block.end {
            rates[active[active_index].0] = rate;
        }
    }
    let mut last_rate = rates[active[0].0];
    for index in 0..rates.len() {
        if aggregates[index].resolved_quality_sum == 0 {
            rates[index] = last_rate;
        } else {
            last_rate = rates[index];
        }
    }
    let mut next_rate = rates[active[active.len() - 1].0];
    for index in (0..rates.len()).rev() {
        if aggregates[index].resolved_quality_sum == 0 {
            rates[index] = next_rate;
        } else {
            next_rate = rates[index];
        }
    }
    rates
}

impl EvidenceCalibrationAnalysis {
    pub fn validate(&self) -> Result<(), EvidenceCalibrationError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || !canonical(&self.source_family_order)
            || !canonical(&self.review_order)
            || !canonical(&self.negative_evidence_order)
            || !canonical(&self.unknown_order)
            || !canonical(&self.uncertainty)
            || self.families.len() != self.source_family_order.len()
            || self
                .families
                .iter()
                .map(|family| family.source_family.clone())
                .collect::<Vec<_>>()
                != self.source_family_order
            || self
                .families
                .windows(2)
                .any(|pair| pair[0].source_family >= pair[1].source_family)
            || self.bins.windows(2).any(|pair| {
                (pair[0].source_family.as_str(), pair[0].bin_index)
                    >= (pair[1].source_family.as_str(), pair[1].bin_index)
            })
            || self.bins.iter().any(|bin| {
                bin.source_family.trim().is_empty()
                    || bin
                        .observation_order
                        .windows(2)
                        .any(|pair| pair[0] >= pair[1])
                    || bin.predicted_support_milli > 1_000
                    || bin.observed_support_milli > 1_000
                    || bin.calibrated_support_milli > 1_000
                    || bin.brier_milli > 1_000
            })
            || self.families.iter().any(|family| {
                family.source_family.trim().is_empty()
                    || family.brier_milli > 1_000
                    || family.expected_calibration_error_milli > 1_000
                    || family.calibrated_reliability_milli > 1_000
                    || !canonical(&family.uncertainty)
            })
        {
            return Err(EvidenceCalibrationError::InvalidOutput(
                "identity, ordering, family/bin alignment, or calibration bounds are invalid"
                    .into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| EvidenceCalibrationError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(EvidenceCalibrationError::InvalidOutput(
                "evidence calibration digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Calibrate source-family support scores with quality-weighted isotonic regression.
pub fn calibrate_glioma_evidence(
    request: &EvidenceCalibrationRequest,
    observations: &[EvidenceCalibrationObservation],
) -> Result<EvidenceCalibrationAnalysis, EvidenceCalibrationError> {
    if request.objective.trim().is_empty()
        || request.bin_count < 2
        || request.bin_count > MAX_BINS
        || request.min_observations_per_family == 0
        || request.min_resolved_per_bin == 0
        || request.max_expected_calibration_error_milli > 1_000
        || observations.is_empty()
        || observations.len() > MAX_OBSERVATIONS
    {
        return Err(EvidenceCalibrationError::InvalidRequest(
            "objective, bounded bins, observation floors, calibration threshold, and observations are required".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    let mut groups = BTreeMap::<String, Vec<&EvidenceCalibrationObservation>>::new();
    let mut negative = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    for observation in observations {
        observation
            .artifact
            .validate()
            .map_err(|error| EvidenceCalibrationError::InvalidObservation(error.to_string()))?;
        if observation.observation_id.trim().is_empty()
            || observation.source_family.trim().is_empty()
            || observation.independent_group.trim().is_empty()
            || observation.predicted_support_milli > 1_000
            || observation.quality_milli > 1_000
            || observation.quality_milli == 0
            || !ids.insert(observation.observation_id.clone())
        {
            return Err(EvidenceCalibrationError::InvalidObservation(
                "observation identity, source family, independent group, score, quality, and uniqueness are required".into(),
            ));
        }
        match observation.outcome {
            EvidenceState::Negative | EvidenceState::Contradicted => {
                negative.insert(observation.observation_id.clone());
            }
            EvidenceState::Unknown | EvidenceState::Stale | EvidenceState::Unmeasured => {
                unknown.insert(observation.observation_id.clone());
            }
            EvidenceState::Supported => {}
        }
        groups
            .entry(observation.source_family.clone())
            .or_default()
            .push(observation);
    }
    if groups.len() > MAX_FAMILIES {
        return Err(EvidenceCalibrationError::InvalidObservation(
            "source-family bound exceeded".into(),
        ));
    }
    let source_family_order = groups.keys().cloned().collect::<Vec<_>>();
    let mut bins = Vec::new();
    let mut families = Vec::new();
    let mut review_order = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for source_family in &source_family_order {
        let mut family_observations = groups.remove(source_family).expect("source family exists");
        family_observations.sort_by(|left, right| {
            left.predicted_support_milli
                .cmp(&right.predicted_support_milli)
                .then_with(|| left.observation_id.cmp(&right.observation_id))
        });
        let mut aggregates = (0..request.bin_count)
            .map(|_| BinAggregate {
                observation_order: Vec::new(),
                resolved_count: 0,
                unknown_count: 0,
                predicted_weighted_sum: 0,
                quality_sum: 0,
                resolved_quality_sum: 0,
                observed_weighted_sum: 0,
                brier_weighted_sum: 0,
            })
            .collect::<Vec<_>>();
        for observation in family_observations {
            let index = bin_index(observation.predicted_support_milli, request.bin_count);
            let aggregate = &mut aggregates[index];
            aggregate
                .observation_order
                .push(observation.observation_id.clone());
            aggregate.quality_sum += u64::from(observation.quality_milli);
            aggregate.predicted_weighted_sum += u64::from(observation.predicted_support_milli)
                * u64::from(observation.quality_milli);
            if let Some(outcome) = resolved(observation.outcome) {
                aggregate.resolved_count += 1;
                aggregate.resolved_quality_sum += u64::from(observation.quality_milli);
                aggregate.observed_weighted_sum +=
                    u64::from(outcome) * u64::from(observation.quality_milli);
                let error = i64::from(observation.predicted_support_milli) - i64::from(outcome);
                aggregate.brier_weighted_sum +=
                    (error.unsigned_abs() * error.unsigned_abs()) / 1_000;
            } else {
                aggregate.unknown_count += 1;
            }
        }
        let isotonic = isotonic_rates(&aggregates);
        let mut family_brier_sum = 0_u64;
        let mut family_brier_weight = 0_u64;
        let mut family_predicted_sum = 0_u64;
        let mut family_resolved_weight = 0_u64;
        let mut family_observed_sum = 0_u64;
        let mut independent_groups = BTreeSet::new();
        // The family observations have been moved into aggregates above; derive independent-group
        // coverage from the original slice without retaining raw records in the output.
        for observation in observations
            .iter()
            .filter(|observation| observation.source_family == *source_family)
        {
            independent_groups.insert(observation.independent_group.clone());
        }
        let mut family_uncertainty = BTreeSet::new();
        for (index, aggregate) in aggregates.iter().enumerate() {
            let (lower, upper) = bin_bounds(index, request.bin_count);
            let predicted = if aggregate.quality_sum == 0 {
                0
            } else {
                aggregate
                    .predicted_weighted_sum
                    .checked_div(aggregate.quality_sum)
                    .unwrap_or(0)
                    .min(1_000) as u16
            };
            let observed = if aggregate.resolved_quality_sum == 0 {
                0
            } else {
                aggregate
                    .observed_weighted_sum
                    .checked_div(aggregate.resolved_quality_sum)
                    .unwrap_or(0)
                    .min(1_000) as u16
            };
            let disposition = if aggregate.resolved_count >= request.min_resolved_per_bin
                && aggregate.observation_order.len() >= request.min_observations_per_family
            {
                CalibrationBinDisposition::Qualified
            } else if aggregate.resolved_count > 0 {
                CalibrationBinDisposition::Partial
            } else {
                CalibrationBinDisposition::Unresolved
            };
            if disposition != CalibrationBinDisposition::Qualified {
                family_uncertainty.insert(format!(
                    "{source_family}:bin-{index}-insufficient-resolved-outcomes"
                ));
            }
            family_brier_sum += aggregate.brier_weighted_sum;
            family_brier_weight += aggregate.resolved_quality_sum;
            family_predicted_sum += aggregate.predicted_weighted_sum;
            family_resolved_weight += aggregate.resolved_quality_sum;
            family_observed_sum += aggregate.observed_weighted_sum;
            bins.push(CalibrationBin {
                source_family: source_family.clone(),
                bin_index: index,
                lower_bound_milli: lower,
                upper_bound_milli: upper,
                observation_order: aggregate.observation_order.clone(),
                resolved_count: aggregate.resolved_count,
                unknown_count: aggregate.unknown_count,
                predicted_support_milli: predicted,
                observed_support_milli: observed,
                calibrated_support_milli: isotonic[index],
                brier_milli: if aggregate.resolved_quality_sum == 0 {
                    0
                } else {
                    aggregate
                        .brier_weighted_sum
                        .checked_div(aggregate.resolved_quality_sum)
                        .unwrap_or(0)
                        .min(1_000) as u16
                },
                disposition,
            });
        }
        let ece = if family_resolved_weight == 0 {
            1_000
        } else {
            aggregates
                .iter()
                .zip(isotonic.iter())
                .map(|(aggregate, calibrated)| {
                    let predicted = if aggregate.resolved_quality_sum == 0 {
                        0
                    } else {
                        aggregate
                            .predicted_weighted_sum
                            .checked_div(aggregate.resolved_quality_sum)
                            .unwrap_or(0)
                    };
                    predicted.abs_diff(u64::from(*calibrated)) * aggregate.resolved_quality_sum
                })
                .sum::<u64>()
                .saturating_div(family_resolved_weight)
                .min(1_000) as u16
        };
        let brier = if family_brier_weight == 0 {
            1_000
        } else {
            family_brier_sum
                .checked_div(family_brier_weight)
                .unwrap_or(0)
                .min(1_000) as u16
        };
        let resolved_count = aggregates
            .iter()
            .map(|aggregate| aggregate.resolved_count)
            .sum::<usize>();
        let observation_count = aggregates
            .iter()
            .map(|aggregate| aggregate.observation_order.len())
            .sum::<usize>();
        let disposition = if resolved_count < request.min_observations_per_family {
            review_order.insert(source_family.clone());
            family_uncertainty.insert(format!(
                "{source_family}:insufficient-resolved-observations"
            ));
            SourceCalibrationDisposition::Unresolved
        } else if ece > request.max_expected_calibration_error_milli {
            review_order.insert(source_family.clone());
            family_uncertainty.insert(format!("{source_family}:calibration-error-exceeds-gate"));
            SourceCalibrationDisposition::Partial
        } else if family_uncertainty.is_empty() {
            SourceCalibrationDisposition::Qualified
        } else {
            SourceCalibrationDisposition::Partial
        };
        let reliability = if resolved_count == 0 {
            0
        } else {
            (1_000_u16.saturating_sub(ece))
                .saturating_mul(resolved_count.min(1_000) as u16)
                .saturating_div(observation_count.clamp(1, 1_000) as u16)
        };
        families.push(SourceCalibration {
            source_family: source_family.clone(),
            observation_count,
            resolved_count,
            independent_group_count: independent_groups.len(),
            brier_milli: brier,
            expected_calibration_error_milli: ece,
            calibrated_reliability_milli: reliability,
            disposition,
            uncertainty: family_uncertainty.into_iter().collect(),
        });
        if family_predicted_sum == 0 && family_observed_sum == 0 {
            uncertainty.insert(format!("{source_family}:no-resolved-score-mass"));
        }
    }
    let disposition = if families.is_empty()
        || families
            .iter()
            .all(|family| family.disposition == SourceCalibrationDisposition::Unresolved)
    {
        EvidenceCalibrationDisposition::Unresolved
    } else if families
        .iter()
        .all(|family| family.disposition == SourceCalibrationDisposition::Qualified)
    {
        EvidenceCalibrationDisposition::Qualified
    } else {
        EvidenceCalibrationDisposition::Partial
    };
    let mut output = EvidenceCalibrationAnalysis {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        source_family_order,
        bins,
        families,
        review_order: review_order.into_iter().collect(),
        negative_evidence_order: negative.into_iter().collect(),
        unknown_order: unknown.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-evidence-calibration"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| EvidenceCalibrationError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: format!("artifact-{id}"),
            content_hash: ContentHash::of_bytes(id.as_bytes()),
            content_type: "application/vnd.aurora.glioma-calibration+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn observation(
        id: &str,
        family: &str,
        predicted: u16,
        outcome: EvidenceState,
    ) -> EvidenceCalibrationObservation {
        EvidenceCalibrationObservation {
            observation_id: id.into(),
            source_family: family.into(),
            predicted_support_milli: predicted,
            outcome,
            quality_milli: 900,
            independent_group: format!("group-{id}"),
            artifact: artifact(id),
        }
    }

    fn request() -> EvidenceCalibrationRequest {
        EvidenceCalibrationRequest {
            objective: "calibrate glioma evidence families".into(),
            bin_count: 4,
            min_observations_per_family: 1,
            min_resolved_per_bin: 1,
            max_expected_calibration_error_milli: 800,
        }
    }

    #[test]
    fn isotonic_calibration_pools_non_monotonic_outcomes() {
        let output = calibrate_glioma_evidence(
            &request(),
            &[
                observation("low", "assay", 100, EvidenceState::Supported),
                observation("high", "assay", 900, EvidenceState::Contradicted),
            ],
        )
        .unwrap();
        let bins = output
            .bins
            .iter()
            .filter(|bin| bin.source_family == "assay")
            .collect::<Vec<_>>();
        assert!(bins
            .windows(2)
            .all(|pair| pair[0].calibrated_support_milli <= pair[1].calibrated_support_milli));
        assert_eq!(output.negative_evidence_order, vec!["high"]);
        output.validate().unwrap();
    }

    #[test]
    fn unknown_and_underobserved_family_remain_unresolved() {
        let mut request = request();
        request.min_observations_per_family = 2;
        let output = calibrate_glioma_evidence(
            &request,
            &[observation(
                "unknown",
                "literature",
                700,
                EvidenceState::Unknown,
            )],
        )
        .unwrap();
        assert_eq!(
            output.disposition,
            EvidenceCalibrationDisposition::Unresolved
        );
        assert_eq!(output.unknown_order, vec!["unknown"]);
        assert!(output.review_order.contains(&"literature".to_string()));
    }

    #[test]
    fn calibration_replays_byte_stably() {
        let observations = vec![
            observation("a", "assay", 600, EvidenceState::Supported),
            observation("b", "assay", 400, EvidenceState::Negative),
        ];
        let first = calibrate_glioma_evidence(&request(), &observations).unwrap();
        let second = calibrate_glioma_evidence(&request(), &observations).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.digest, second.digest);
    }
}

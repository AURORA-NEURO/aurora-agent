//! Missingness-mechanism audit for preclinical glioma multimodal studies.
//!
//! A missing modality is not a single state: an assay may be absent, partially observed, or
//! present but below its declared quality floor. This feature materializes the bounded
//! sample-by-modality matrix, detects correlated dropout that can bias a downstream endpoint,
//! and returns a deterministic reacquisition order. It never imputes a value or treats an
//! unmeasured modality as negative biological evidence.

use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P03-F05";
pub const OUTPUT_SCHEMA: &str = "GliomaMultimodalMissingnessAudit1@1";
pub const MAX_SAMPLES: usize = 4096;
pub const MAX_MODALITIES: usize = 64;
pub const MAX_OBSERVATIONS: usize = 262_144;
pub const MAX_PATTERNS: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissingnessState {
    Complete,
    Partial,
    Missing,
    QualityFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissingnessPatternDisposition {
    Complete,
    Partial,
    QualityBlocked,
    MissingRequired,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissingnessAuditDisposition {
    Ready,
    Conditional,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissingnessObservation {
    pub sample_id: String,
    pub modality: GliomaModality,
    pub expected_feature_count: u32,
    pub observed_feature_count: u32,
    pub quality_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissingnessAuditRequest {
    pub objective: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub sample_ids: Vec<String>,
    pub required_modalities: Vec<GliomaModality>,
    pub observations: Vec<MissingnessObservation>,
    pub min_quality_milli: u16,
    pub min_complete_fraction_milli: u16,
    pub max_missing_fraction_milli: u16,
    pub min_samples_per_pattern: u32,
    pub max_patterns: usize,
    pub max_pairwise_dropout_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissingnessModalitySummary {
    pub modality: GliomaModality,
    pub sample_count: u32,
    pub complete_count: u32,
    pub partial_count: u32,
    pub missing_count: u32,
    pub quality_failed_count: u32,
    pub observed_feature_coverage_milli: u16,
    pub incomplete_fraction_milli: u16,
    pub acquisition_priority_milli: u16,
    pub recommended_action: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissingnessPattern {
    pub pattern_id: String,
    pub sample_id_order: Vec<String>,
    pub complete_modality_order: Vec<GliomaModality>,
    pub partial_modality_order: Vec<GliomaModality>,
    pub missing_modality_order: Vec<GliomaModality>,
    pub quality_failed_modality_order: Vec<GliomaModality>,
    pub sample_count: u32,
    pub fraction_milli: u16,
    pub disposition: MissingnessPatternDisposition,
    pub next_action: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissingnessPairSummary {
    pub left_modality: GliomaModality,
    pub right_modality: GliomaModality,
    pub left_incomplete_count: u32,
    pub right_incomplete_count: u32,
    pub co_dropout_count: u32,
    pub co_dropout_fraction_milli: u16,
    pub disposition: MissingnessPatternDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissingnessAudit {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub sample_order: Vec<String>,
    pub modality_order: Vec<GliomaModality>,
    pub modality_summaries: Vec<MissingnessModalitySummary>,
    pub pattern_order: Vec<String>,
    pub patterns: Vec<MissingnessPattern>,
    pub pair_order: Vec<String>,
    pub pairs: Vec<MissingnessPairSummary>,
    pub complete_sample_fraction_milli: u16,
    pub acquisition_order: Vec<GliomaModality>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: MissingnessAuditDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MissingnessAuditError {
    #[error("missingness audit request is invalid: {0}")]
    InvalidRequest(String),
    #[error("missingness audit input is invalid: {0}")]
    InvalidInput(String),
    #[error("missingness audit output is invalid: {0}")]
    InvalidOutput(String),
    #[error("missingness audit digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn milli_fraction(numerator: u64, denominator: u64) -> u16 {
    if denominator == 0 {
        return 0;
    }
    ((numerator.saturating_mul(1_000) / denominator).min(1_000)) as u16
}

fn digest_input(output: &MissingnessAudit) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "study_id": output.study_id,
        "model_system": output.model_system,
        "sample_order": output.sample_order,
        "modality_order": output.modality_order,
        "modality_summaries": output.modality_summaries,
        "pattern_order": output.pattern_order,
        "patterns": output.patterns,
        "pair_order": output.pair_order,
        "pairs": output.pairs,
        "complete_sample_fraction_milli": output.complete_sample_fraction_milli,
        "acquisition_order": output.acquisition_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn validate_request(request: &MissingnessAuditRequest) -> Result<(), MissingnessAuditError> {
    if request.objective.trim().is_empty()
        || request.study_id.trim().is_empty()
        || request.sample_ids.is_empty()
        || request.sample_ids.len() > MAX_SAMPLES
        || request.required_modalities.is_empty()
        || request.required_modalities.len() > MAX_MODALITIES
        || request.observations.len() > MAX_OBSERVATIONS
        || request.min_quality_milli > 1_000
        || request.min_complete_fraction_milli > 1_000
        || request.max_missing_fraction_milli > 1_000
        || request.min_samples_per_pattern == 0
        || request.max_patterns == 0
        || request.max_patterns > MAX_PATTERNS
        || request.max_pairwise_dropout_milli > 1_000
    {
        return Err(MissingnessAuditError::InvalidRequest(
            "objective, study, bounded samples/modalities/observations, and quality/dropout gates are required".into(),
        ));
    }
    let samples = request.sample_ids.iter().collect::<BTreeSet<_>>();
    if samples.len() != request.sample_ids.len()
        || !canonical(&request.sample_ids)
        || !canonical(&request.required_modalities)
    {
        return Err(MissingnessAuditError::InvalidRequest(
            "sample and modality identifiers must be unique and canonical".into(),
        ));
    }
    if request.max_missing_fraction_milli < 1_000 - request.min_complete_fraction_milli {
        return Err(MissingnessAuditError::InvalidRequest(
            "missingness and complete-case gates are inconsistent".into(),
        ));
    }
    let modalities = request
        .required_modalities
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    for observation in &request.observations {
        if !samples.contains(&observation.sample_id)
            || !modalities.contains(&observation.modality)
            || observation.expected_feature_count == 0
            || observation.observed_feature_count > observation.expected_feature_count
            || observation.quality_milli > 1_000
            || !seen.insert((observation.sample_id.clone(), observation.modality))
        {
            return Err(MissingnessAuditError::InvalidInput(
                "observations require known samples/modalities, bounded feature counts, finite quality, and unique sample-modality keys".into(),
            ));
        }
    }
    Ok(())
}

fn validate_output(output: &MissingnessAudit) -> Result<(), MissingnessAuditError> {
    if output.feature_id != FEATURE_ID
        || output.output_schema != OUTPUT_SCHEMA
        || output.objective.trim().is_empty()
        || output.study_id.trim().is_empty()
        || !canonical(&output.sample_order)
        || !canonical(&output.modality_order)
        || !canonical(&output.pattern_order)
        || !canonical(&output.pair_order)
        || !canonical(&output.acquisition_order)
        || !canonical(&output.negative_evidence)
        || !canonical(&output.uncertainty)
        || output.complete_sample_fraction_milli > 1_000
        || output.modality_summaries.len() != output.modality_order.len()
        || output.patterns.len() != output.pattern_order.len()
        || output.pairs.len() != output.pair_order.len()
        || output
            .patterns
            .windows(2)
            .any(|pair| pair[0].pattern_id >= pair[1].pattern_id)
        || output.pairs.windows(2).any(|pair| {
            (pair[0].left_modality, pair[0].right_modality)
                >= (pair[1].left_modality, pair[1].right_modality)
        })
        || output.modality_summaries.iter().any(|summary| {
            summary.sample_count == 0
                || summary.complete_count
                    + summary.partial_count
                    + summary.missing_count
                    + summary.quality_failed_count
                    != summary.sample_count
                || summary.observed_feature_coverage_milli > 1_000
                || summary.incomplete_fraction_milli > 1_000
                || summary.acquisition_priority_milli > 1_000
                || summary.recommended_action.trim().is_empty()
        })
        || output.patterns.iter().any(|pattern| {
            pattern.sample_id_order.is_empty()
                || !canonical(&pattern.sample_id_order)
                || !canonical(&pattern.complete_modality_order)
                || !canonical(&pattern.partial_modality_order)
                || !canonical(&pattern.missing_modality_order)
                || !canonical(&pattern.quality_failed_modality_order)
                || pattern.sample_count != pattern.sample_id_order.len() as u32
                || pattern.fraction_milli > 1_000
                || pattern.next_action.trim().is_empty()
        })
        || output.pairs.iter().any(|pair| {
            pair.left_modality >= pair.right_modality
                || pair.co_dropout_count > output.sample_order.len() as u32
                || pair.co_dropout_fraction_milli > 1_000
        })
    {
        return Err(MissingnessAuditError::InvalidOutput(
            "identity, ordering, counts, fractions, or action invariants are invalid".into(),
        ));
    }
    let pattern_ids = output
        .pattern_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let pattern_sample_ids = output
        .patterns
        .iter()
        .flat_map(|pattern| pattern.sample_id_order.iter().cloned())
        .collect::<BTreeSet<_>>();
    if pattern_ids.len() != output.patterns.len()
        || pattern_sample_ids != output.sample_order.iter().cloned().collect()
    {
        return Err(MissingnessAuditError::InvalidOutput(
            "pattern identifiers or sample partition do not reconcile".into(),
        ));
    }
    let pair_ids = output
        .pairs
        .iter()
        .map(|pair| format!("{:?}::{:?}", pair.left_modality, pair.right_modality))
        .collect::<Vec<_>>();
    if pair_ids != output.pair_order {
        return Err(MissingnessAuditError::InvalidOutput(
            "pair order does not reconcile with pair summaries".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(output))
        .map_err(|error| MissingnessAuditError::Digest(error.to_string()))?;
    if expected != output.digest {
        return Err(MissingnessAuditError::InvalidOutput(
            "digest is not bound to missingness-audit output".into(),
        ));
    }
    Ok(())
}

impl MissingnessAudit {
    pub fn validate(&self) -> Result<(), MissingnessAuditError> {
        validate_output(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Cell {
    state: MissingnessState,
    observed_feature_count: u32,
    expected_feature_count: u32,
}

fn classify(observation: Option<&MissingnessObservation>, min_quality_milli: u16) -> Cell {
    let Some(observation) = observation else {
        return Cell {
            state: MissingnessState::Missing,
            observed_feature_count: 0,
            expected_feature_count: 1,
        };
    };
    let state = if observation.quality_milli < min_quality_milli {
        MissingnessState::QualityFailed
    } else if observation.observed_feature_count == 0 {
        MissingnessState::Missing
    } else if observation.observed_feature_count < observation.expected_feature_count {
        MissingnessState::Partial
    } else {
        MissingnessState::Complete
    };
    Cell {
        state,
        observed_feature_count: observation.observed_feature_count,
        expected_feature_count: observation.expected_feature_count,
    }
}

fn incomplete(state: MissingnessState) -> bool {
    !matches!(state, MissingnessState::Complete)
}

/// Audit the sample-by-modality missingness topology for a local preclinical glioma study.
pub fn analyze_glioma_multimodal_missingness(
    request: &MissingnessAuditRequest,
) -> Result<MissingnessAudit, MissingnessAuditError> {
    validate_request(request)?;
    let observation_map = request
        .observations
        .iter()
        .map(|observation| {
            (
                (observation.sample_id.clone(), observation.modality),
                observation,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let sample_order = request.sample_ids.clone();
    let modality_order = request.required_modalities.clone();
    let mut cells = BTreeMap::new();
    for sample_id in &sample_order {
        for modality in &modality_order {
            cells.insert(
                (sample_id.clone(), *modality),
                classify(
                    observation_map
                        .get(&(sample_id.clone(), *modality))
                        .copied(),
                    request.min_quality_milli,
                ),
            );
        }
    }

    let mut summaries = Vec::new();
    let mut acquisition_scores = BTreeMap::new();
    let mut negative_evidence = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for modality in &modality_order {
        let modality_cells = sample_order
            .iter()
            .map(|sample_id| cells[&(sample_id.clone(), *modality)])
            .collect::<Vec<_>>();
        let complete_count = modality_cells
            .iter()
            .filter(|cell| cell.state == MissingnessState::Complete)
            .count() as u32;
        let partial_count = modality_cells
            .iter()
            .filter(|cell| cell.state == MissingnessState::Partial)
            .count() as u32;
        let missing_count = modality_cells
            .iter()
            .filter(|cell| cell.state == MissingnessState::Missing)
            .count() as u32;
        let quality_failed_count = modality_cells
            .iter()
            .filter(|cell| cell.state == MissingnessState::QualityFailed)
            .count() as u32;
        let coverage_numerator = modality_cells
            .iter()
            .map(|cell| {
                u64::from(cell.observed_feature_count)
                    .saturating_mul(1_000)
                    .checked_div(u64::from(cell.expected_feature_count))
                    .unwrap_or(0)
            })
            .sum::<u64>();
        let observed_feature_coverage_milli =
            milli_fraction(coverage_numerator, sample_order.len() as u64);
        let incomplete_count = partial_count + missing_count + quality_failed_count;
        let incomplete_fraction_milli =
            milli_fraction(u64::from(incomplete_count), sample_order.len() as u64);
        let acquisition_priority_milli = milli_fraction(
            u64::from(missing_count + quality_failed_count) * 700 + u64::from(partial_count) * 300,
            sample_order.len() as u64,
        );
        let recommended_action = if missing_count > 0 {
            format!(
                "acquire {} for {} missing samples",
                format!("{:?}", modality),
                missing_count
            )
        } else if quality_failed_count > 0 {
            format!(
                "repeat QC for {} quality-failed samples",
                format!("{:?}", modality)
            )
        } else if partial_count > 0 {
            format!(
                "complete {} for {} partial samples",
                format!("{:?}", modality),
                partial_count
            )
        } else {
            format!("{:?} is complete at the declared quality floor", modality)
        };
        if missing_count > 0 {
            negative_evidence.insert(format!("{:?}:missing-cells={missing_count}", modality));
        }
        if quality_failed_count > 0 {
            uncertainty.insert(format!(
                "{:?}:quality-failed-cells={quality_failed_count}",
                modality
            ));
        }
        acquisition_scores.insert(
            *modality,
            (
                acquisition_priority_milli,
                missing_count + quality_failed_count,
                partial_count,
            ),
        );
        summaries.push(MissingnessModalitySummary {
            modality: *modality,
            sample_count: sample_order.len() as u32,
            complete_count,
            partial_count,
            missing_count,
            quality_failed_count,
            observed_feature_coverage_milli,
            incomplete_fraction_milli,
            acquisition_priority_milli,
            recommended_action,
        });
    }

    let mut grouped_patterns = BTreeMap::<Vec<MissingnessState>, Vec<String>>::new();
    for sample_id in &sample_order {
        let signature = modality_order
            .iter()
            .map(|modality| cells[&(sample_id.clone(), *modality)].state)
            .collect::<Vec<_>>();
        grouped_patterns
            .entry(signature)
            .or_default()
            .push(sample_id.clone());
    }
    if grouped_patterns.len() > request.max_patterns {
        return Err(MissingnessAuditError::InvalidInput(
            "missingness topology exceeds the declared pattern bound".into(),
        ));
    }
    let mut patterns = Vec::new();
    for (index, (signature, mut sample_ids)) in grouped_patterns.into_iter().enumerate() {
        sample_ids.sort();
        let mut complete = Vec::new();
        let mut partial = Vec::new();
        let mut missing = Vec::new();
        let mut quality_failed = Vec::new();
        for (modality, state) in modality_order.iter().zip(signature.iter()) {
            match state {
                MissingnessState::Complete => complete.push(*modality),
                MissingnessState::Partial => partial.push(*modality),
                MissingnessState::Missing => missing.push(*modality),
                MissingnessState::QualityFailed => quality_failed.push(*modality),
            }
        }
        let fraction_milli = milli_fraction(sample_ids.len() as u64, sample_order.len() as u64);
        let rare = sample_ids.len() < request.min_samples_per_pattern as usize;
        let disposition = if rare {
            uncertainty.insert(format!("pattern-{index:03}:rare-pattern"));
            MissingnessPatternDisposition::Unresolved
        } else if !missing.is_empty() {
            MissingnessPatternDisposition::MissingRequired
        } else if !quality_failed.is_empty() {
            MissingnessPatternDisposition::QualityBlocked
        } else if !partial.is_empty() {
            MissingnessPatternDisposition::Partial
        } else {
            MissingnessPatternDisposition::Complete
        };
        if !missing.is_empty() {
            negative_evidence.insert(format!("pattern-{index:03}:missing-required-modalities"));
        }
        let next_action = match disposition {
            MissingnessPatternDisposition::Complete => {
                "pattern is admissible for downstream multimodal analysis".into()
            }
            MissingnessPatternDisposition::Partial => {
                "complete partial modalities before high-confidence interpretation".into()
            }
            MissingnessPatternDisposition::QualityBlocked => {
                "repeat quality-failed modalities before interpretation".into()
            }
            MissingnessPatternDisposition::MissingRequired => {
                "acquire missing required modalities before interpretation".into()
            }
            MissingnessPatternDisposition::Unresolved => {
                "increase pattern support or review the rare missingness mechanism".into()
            }
        };
        let sample_count = sample_ids.len() as u32;
        patterns.push(MissingnessPattern {
            pattern_id: format!("pattern-{index:03}"),
            sample_id_order: sample_ids,
            complete_modality_order: complete,
            partial_modality_order: partial,
            missing_modality_order: missing,
            quality_failed_modality_order: quality_failed,
            sample_count,
            fraction_milli,
            disposition,
            next_action,
        });
    }

    let mut pairs = Vec::new();
    for (left_index, left) in modality_order.iter().enumerate() {
        for right in modality_order.iter().skip(left_index + 1) {
            let mut left_incomplete_count = 0_u32;
            let mut right_incomplete_count = 0_u32;
            let mut co_dropout_count = 0_u32;
            for sample_id in &sample_order {
                let left_state = cells[&(sample_id.clone(), *left)].state;
                let right_state = cells[&(sample_id.clone(), *right)].state;
                if incomplete(left_state) {
                    left_incomplete_count += 1;
                }
                if incomplete(right_state) {
                    right_incomplete_count += 1;
                }
                if incomplete(left_state) && incomplete(right_state) {
                    co_dropout_count += 1;
                }
            }
            let co_dropout_fraction_milli =
                milli_fraction(u64::from(co_dropout_count), sample_order.len() as u64);
            let disposition = if co_dropout_fraction_milli > request.max_pairwise_dropout_milli {
                negative_evidence.insert(format!(
                    "{:?}::{:?}:correlated-dropout={co_dropout_fraction_milli}",
                    left, right
                ));
                MissingnessPatternDisposition::MissingRequired
            } else if co_dropout_count > 0 {
                uncertainty.insert(format!(
                    "{:?}::{:?}:co-dropout-observed={co_dropout_count}",
                    left, right
                ));
                MissingnessPatternDisposition::Partial
            } else {
                MissingnessPatternDisposition::Complete
            };
            pairs.push(MissingnessPairSummary {
                left_modality: *left,
                right_modality: *right,
                left_incomplete_count,
                right_incomplete_count,
                co_dropout_count,
                co_dropout_fraction_milli,
                disposition,
            });
        }
    }
    pairs.sort_by_key(|pair| (pair.left_modality, pair.right_modality));
    let pair_order = pairs
        .iter()
        .map(|pair| format!("{:?}::{:?}", pair.left_modality, pair.right_modality))
        .collect::<Vec<_>>();

    let complete_sample_count = sample_order
        .iter()
        .filter(|sample_id| {
            modality_order.iter().all(|modality| {
                cells[&((*sample_id).clone(), *modality)].state == MissingnessState::Complete
            })
        })
        .count() as u64;
    let complete_sample_fraction_milli =
        milli_fraction(complete_sample_count, sample_order.len() as u64);
    let mut acquisition_order = modality_order
        .iter()
        .copied()
        .filter(|modality| acquisition_scores[modality].0 > 0)
        .collect::<Vec<_>>();
    acquisition_order.sort_by(|left, right| {
        acquisition_scores[right]
            .cmp(&acquisition_scores[left])
            .then_with(|| left.cmp(right))
    });
    let correlated_dropout = pairs
        .iter()
        .any(|pair| pair.co_dropout_fraction_milli > request.max_pairwise_dropout_milli);
    let has_missing = summaries
        .iter()
        .any(|summary| summary.missing_count > 0 || summary.quality_failed_count > 0);
    let over_missing_gate = summaries
        .iter()
        .any(|summary| summary.incomplete_fraction_milli > request.max_missing_fraction_milli);
    let has_unresolved = patterns
        .iter()
        .any(|pattern| pattern.disposition == MissingnessPatternDisposition::Unresolved);
    let disposition = if has_unresolved {
        MissingnessAuditDisposition::Unresolved
    } else if over_missing_gate || correlated_dropout {
        MissingnessAuditDisposition::Blocked
    } else if has_missing || complete_sample_fraction_milli < request.min_complete_fraction_milli {
        MissingnessAuditDisposition::Conditional
    } else {
        MissingnessAuditDisposition::Ready
    };
    if complete_sample_fraction_milli < request.min_complete_fraction_milli {
        uncertainty.insert(format!(
            "complete-sample-fraction-below-gate={complete_sample_fraction_milli}"
        ));
    }
    let mut output = MissingnessAudit {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        study_id: request.study_id.clone(),
        model_system: request.model_system,
        sample_order,
        modality_order,
        modality_summaries: summaries,
        pattern_order: patterns
            .iter()
            .map(|pattern| pattern.pattern_id.clone())
            .collect(),
        patterns,
        pair_order,
        pairs,
        complete_sample_fraction_milli,
        acquisition_order,
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-multimodal-missingness-audit"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| MissingnessAuditError::Digest(error.to_string()))?;
    validate_output(&output)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(observations: Vec<MissingnessObservation>) -> MissingnessAuditRequest {
        MissingnessAuditRequest {
            objective: "audit multimodal missingness before invasion analysis".into(),
            study_id: "study-1".into(),
            model_system: GliomaModelSystem::Organoid,
            sample_ids: vec!["sample-a".into(), "sample-b".into(), "sample-c".into()],
            required_modalities: vec![GliomaModality::Genomics, GliomaModality::Imaging],
            observations,
            min_quality_milli: 700,
            min_complete_fraction_milli: 500,
            max_missing_fraction_milli: 700,
            min_samples_per_pattern: 1,
            max_patterns: 8,
            max_pairwise_dropout_milli: 300,
        }
    }

    #[test]
    fn missingness_audit_distinguishes_partial_and_quality_failed_cells() {
        let output = analyze_glioma_multimodal_missingness(&request(vec![
            MissingnessObservation {
                sample_id: "sample-a".into(),
                modality: GliomaModality::Genomics,
                expected_feature_count: 100,
                observed_feature_count: 100,
                quality_milli: 900,
            },
            MissingnessObservation {
                sample_id: "sample-a".into(),
                modality: GliomaModality::Imaging,
                expected_feature_count: 100,
                observed_feature_count: 50,
                quality_milli: 900,
            },
            MissingnessObservation {
                sample_id: "sample-b".into(),
                modality: GliomaModality::Genomics,
                expected_feature_count: 100,
                observed_feature_count: 100,
                quality_milli: 600,
            },
            MissingnessObservation {
                sample_id: "sample-b".into(),
                modality: GliomaModality::Imaging,
                expected_feature_count: 100,
                observed_feature_count: 100,
                quality_milli: 900,
            },
            MissingnessObservation {
                sample_id: "sample-c".into(),
                modality: GliomaModality::Genomics,
                expected_feature_count: 100,
                observed_feature_count: 100,
                quality_milli: 900,
            },
            MissingnessObservation {
                sample_id: "sample-c".into(),
                modality: GliomaModality::Imaging,
                expected_feature_count: 100,
                observed_feature_count: 100,
                quality_milli: 900,
            },
        ]))
        .unwrap();
        let imaging = output
            .modality_summaries
            .iter()
            .find(|summary| summary.modality == GliomaModality::Imaging)
            .unwrap();
        assert_eq!(imaging.partial_count, 1);
        let genomics = output
            .modality_summaries
            .iter()
            .find(|summary| summary.modality == GliomaModality::Genomics)
            .unwrap();
        assert_eq!(genomics.quality_failed_count, 1);
        assert_eq!(output.disposition, MissingnessAuditDisposition::Conditional);
        output.validate().unwrap();
    }

    #[test]
    fn missingness_audit_blocks_correlated_dropout() {
        let output = analyze_glioma_multimodal_missingness(&request(vec![
            MissingnessObservation {
                sample_id: "sample-a".into(),
                modality: GliomaModality::Genomics,
                expected_feature_count: 100,
                observed_feature_count: 0,
                quality_milli: 900,
            },
            MissingnessObservation {
                sample_id: "sample-a".into(),
                modality: GliomaModality::Imaging,
                expected_feature_count: 100,
                observed_feature_count: 0,
                quality_milli: 900,
            },
            MissingnessObservation {
                sample_id: "sample-b".into(),
                modality: GliomaModality::Genomics,
                expected_feature_count: 100,
                observed_feature_count: 100,
                quality_milli: 900,
            },
            MissingnessObservation {
                sample_id: "sample-b".into(),
                modality: GliomaModality::Imaging,
                expected_feature_count: 100,
                observed_feature_count: 100,
                quality_milli: 900,
            },
            MissingnessObservation {
                sample_id: "sample-c".into(),
                modality: GliomaModality::Genomics,
                expected_feature_count: 100,
                observed_feature_count: 100,
                quality_milli: 900,
            },
            MissingnessObservation {
                sample_id: "sample-c".into(),
                modality: GliomaModality::Imaging,
                expected_feature_count: 100,
                observed_feature_count: 100,
                quality_milli: 900,
            },
        ]))
        .unwrap();
        assert_eq!(output.disposition, MissingnessAuditDisposition::Blocked);
        assert_eq!(
            output.acquisition_order,
            vec![GliomaModality::Genomics, GliomaModality::Imaging]
        );
        assert!(output
            .negative_evidence
            .iter()
            .any(|item| item.contains("correlated-dropout")));
    }
}

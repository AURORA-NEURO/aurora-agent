//! Uncertainty-aware preclinical outcome analysis.
//!
//! This is a small, auditable analysis kernel rather than a model zoo.  It computes a declared
//! two-arm estimand, an exact permutation tail probability for bounded datasets, and explicit
//! uncertainty when the dataset is too large or has batch overlap.  A provider can replace it
//! with a richer local statistical backend while preserving the same input/output contract.

use super::super::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P10-F01";
pub const OUTPUT_SCHEMA: &str = "GliomaAnalysisResult1@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisRow {
    pub row_id: String,
    pub arm_id: String,
    pub model_system: GliomaModelSystem,
    pub batch_id: String,
    pub outcome_milli: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisDataset {
    pub dataset_id: String,
    pub artifact: LocalArtifactRef,
    pub rows: Vec<AnalysisRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisRequest {
    pub objective: String,
    pub control_arm: String,
    pub treatment_arm: String,
    pub model_system: GliomaModelSystem,
    pub min_replicates_per_arm: usize,
    pub effect_threshold_milli: u64,
    pub alpha_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArmSummary {
    pub arm_id: String,
    pub count: usize,
    pub mean_milli: i64,
    pub min_milli: i64,
    pub max_milli: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisDisposition {
    Qualified,
    Negative,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub estimand: String,
    pub summaries: Vec<ArmSummary>,
    pub effect_milli: i64,
    pub interval_low_milli: i64,
    pub interval_high_milli: i64,
    pub uncertainty_milli: u64,
    pub permutation_p_milli: Option<u16>,
    pub batch_overlap_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: AnalysisDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AnalysisError {
    #[error("analysis request is invalid: {0}")]
    InvalidRequest(String),
    #[error("analysis dataset is invalid: {0}")]
    InvalidDataset(String),
    #[error("analysis result is invalid: {0}")]
    InvalidOutput(String),
    #[error("analysis digest failed: {0}")]
    Digest(String),
}

fn mean(values: &[i64]) -> i64 {
    values.iter().sum::<i64>() / values.len() as i64
}

fn integer_sqrt(value: u64) -> u64 {
    let mut root = 0;
    while (root + 1) * (root + 1) <= value {
        root += 1;
    }
    root
}

fn combinations_with_extreme(
    values: &[i64],
    treatment_count: usize,
    observed_abs: i128,
) -> (u64, u64) {
    if values.len() > 18 {
        return (0, 0);
    }
    let total_masks = 1_u64 << values.len();
    let mut total = 0;
    let mut extreme = 0;
    let total_sum = values.iter().map(|value| *value as i128).sum::<i128>();
    for mask in 0..total_masks {
        if mask.count_ones() as usize != treatment_count {
            continue;
        }
        let treatment_sum = values
            .iter()
            .enumerate()
            .filter(|(index, _)| mask & (1_u64 << index) != 0)
            .map(|(_, value)| *value as i128)
            .sum::<i128>();
        let control_count = values.len() - treatment_count;
        let diff_numerator = treatment_sum * control_count as i128
            - (total_sum - treatment_sum) * treatment_count as i128;
        let diff_abs = diff_numerator.abs();
        total += 1;
        if diff_abs >= observed_abs {
            extreme += 1;
        }
    }
    (extreme, total)
}

fn digest_input(result: &AnalysisResult) -> serde_json::Value {
    serde_json::json!({
        "feature_id": result.feature_id,
        "output_schema": result.output_schema,
        "objective": result.objective,
        "estimand": result.estimand,
        "summaries": result.summaries,
        "effect_milli": result.effect_milli,
        "interval_low_milli": result.interval_low_milli,
        "interval_high_milli": result.interval_high_milli,
        "uncertainty_milli": result.uncertainty_milli,
        "permutation_p_milli": result.permutation_p_milli,
        "batch_overlap_order": result.batch_overlap_order,
        "negative_evidence": result.negative_evidence,
        "uncertainty": result.uncertainty,
        "disposition": result.disposition,
    })
}

impl AnalysisResult {
    pub fn validate(&self) -> Result<(), AnalysisError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.estimand.trim().is_empty()
            || self.summaries.len() != 2
            || self
                .summaries
                .iter()
                .any(|summary| summary.count == 0 || summary.min_milli > summary.max_milli)
            || self
                .summaries
                .windows(2)
                .any(|pair| pair[0].arm_id > pair[1].arm_id)
            || self
                .batch_overlap_order
                .windows(2)
                .any(|pair| pair[0] > pair[1])
            || self
                .negative_evidence
                .windows(2)
                .any(|pair| pair[0] > pair[1])
            || self.uncertainty.windows(2).any(|pair| pair[0] > pair[1])
            || self.permutation_p_milli.is_some_and(|value| value > 1_000)
        {
            return Err(AnalysisError::InvalidOutput(
                "identity, summaries, bounds, or ordering is invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|e| AnalysisError::Digest(e.to_string()))?;
        if expected != self.digest {
            return Err(AnalysisError::InvalidOutput(
                "digest is not bound to the analysis result".into(),
            ));
        }
        Ok(())
    }
}

pub fn analyze_preclinical_outcomes(
    request: &AnalysisRequest,
    dataset: &AnalysisDataset,
) -> Result<AnalysisResult, AnalysisError> {
    if request.objective.trim().is_empty()
        || request.control_arm.trim().is_empty()
        || request.treatment_arm.trim().is_empty()
        || request.control_arm == request.treatment_arm
        || request.min_replicates_per_arm == 0
        || request.alpha_milli == 0
        || request.alpha_milli > 100
    {
        return Err(AnalysisError::InvalidRequest(
            "objective, arm identity, replicate floor, or alpha is invalid".into(),
        ));
    }
    if dataset.dataset_id.trim().is_empty() || dataset.rows.is_empty() {
        return Err(AnalysisError::InvalidDataset(
            "dataset identity and rows are required".into(),
        ));
    }
    dataset
        .artifact
        .validate()
        .map_err(|e| AnalysisError::InvalidDataset(e.to_string()))?;
    let mut rows = BTreeMap::<String, Vec<&AnalysisRow>>::new();
    let mut row_ids = BTreeSet::new();
    for row in &dataset.rows {
        if row.row_id.trim().is_empty()
            || row.arm_id != request.control_arm && row.arm_id != request.treatment_arm
            || row.batch_id.trim().is_empty()
            || row.model_system != request.model_system
            || !row_ids.insert(row.row_id.clone())
        {
            return Err(AnalysisError::InvalidDataset(
                "row identity, arm binding, model system, batch, or uniqueness is invalid".into(),
            ));
        }
        rows.entry(row.arm_id.clone()).or_default().push(row);
    }
    let control = rows.get(&request.control_arm).cloned().unwrap_or_default();
    let treatment = rows
        .get(&request.treatment_arm)
        .cloned()
        .unwrap_or_default();
    let mut negative = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    if control.len() < request.min_replicates_per_arm {
        negative.insert("control-replicate-floor-not-met".into());
    }
    if treatment.len() < request.min_replicates_per_arm {
        negative.insert("treatment-replicate-floor-not-met".into());
    }
    if control.is_empty() || treatment.is_empty() {
        negative.insert("one-or-more-analysis-arms-empty".into());
    }
    let mut batches = BTreeMap::<String, BTreeSet<String>>::new();
    for row in control.iter().chain(treatment.iter()) {
        batches
            .entry(row.batch_id.clone())
            .or_default()
            .insert(row.arm_id.clone());
    }
    let batch_overlap_order = batches
        .iter()
        .filter(|(_, arms)| arms.len() > 1)
        .map(|(batch, _)| batch.clone())
        .collect::<Vec<_>>();
    if !batch_overlap_order.is_empty() {
        uncertainty.insert("batch-overlap-may-confound-arm-effect".into());
    }
    let control_values = control
        .iter()
        .map(|row| row.outcome_milli)
        .collect::<Vec<_>>();
    let treatment_values = treatment
        .iter()
        .map(|row| row.outcome_milli)
        .collect::<Vec<_>>();
    let mut summaries = vec![
        summary(&request.control_arm, &control_values),
        summary(&request.treatment_arm, &treatment_values),
    ];
    summaries.sort_by(|left, right| left.arm_id.cmp(&right.arm_id));
    if control_values.is_empty() || treatment_values.is_empty() {
        let result = unresolved_result(
            request,
            summaries,
            batch_overlap_order,
            negative,
            uncertainty,
        )?;
        result.validate()?;
        return Ok(result);
    }
    let control_mean = mean(&control_values);
    let treatment_mean = mean(&treatment_values);
    let effect_milli = treatment_mean - control_mean;
    let max_range = control_values
        .iter()
        .max()
        .unwrap()
        .saturating_sub(*control_values.iter().min().unwrap())
        .unsigned_abs()
        .max(
            treatment_values
                .iter()
                .max()
                .unwrap()
                .saturating_sub(*treatment_values.iter().min().unwrap())
                .unsigned_abs(),
        );
    let uncertainty_milli =
        max_range / integer_sqrt(control_values.len().min(treatment_values.len()) as u64).max(1);
    let interval_low_milli =
        effect_milli.saturating_sub(uncertainty_milli.min(i64::MAX as u64) as i64);
    let interval_high_milli =
        effect_milli.saturating_add(uncertainty_milli.min(i64::MAX as u64) as i64);
    let combined = control_values
        .iter()
        .chain(treatment_values.iter())
        .copied()
        .collect::<Vec<_>>();
    let observed_numerator = (treatment_mean as i128 * control_values.len() as i128
        - control_mean as i128 * treatment_values.len() as i128)
        .abs();
    let (extreme, total) =
        combinations_with_extreme(&combined, treatment_values.len(), observed_numerator);
    let permutation_p_milli = if total == 0 {
        uncertainty.insert("exact-permutation-test-not-computed-over-size-bound".into());
        None
    } else {
        Some(((extreme * 1_000).div_ceil(total)).min(1_000) as u16)
    };
    let effect_meets_threshold = effect_milli.unsigned_abs() >= request.effect_threshold_milli;
    let disposition = if control.len() < request.min_replicates_per_arm
        || treatment.len() < request.min_replicates_per_arm
    {
        AnalysisDisposition::Unresolved
    } else if effect_meets_threshold
        && permutation_p_milli.is_some_and(|p| p <= request.alpha_milli * 10)
        && batch_overlap_order.is_empty()
    {
        AnalysisDisposition::Qualified
    } else if !effect_meets_threshold {
        negative.insert("effect-below-declared-threshold".into());
        AnalysisDisposition::Negative
    } else {
        AnalysisDisposition::Unresolved
    };
    let mut result = AnalysisResult {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        estimand: format!(
            "mean({}) - mean({})",
            request.treatment_arm, request.control_arm
        ),
        summaries,
        effect_milli,
        interval_low_milli,
        interval_high_milli,
        uncertainty_milli,
        permutation_p_milli,
        batch_overlap_order,
        negative_evidence: negative.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_value(&serde_json::json!({}))
            .map_err(|e| AnalysisError::Digest(e.to_string()))?,
    };
    result.digest = ContentHash::of_value(&digest_input(&result))
        .map_err(|e| AnalysisError::Digest(e.to_string()))?;
    result.validate()?;
    Ok(result)
}

fn summary(arm_id: &str, values: &[i64]) -> ArmSummary {
    ArmSummary {
        arm_id: arm_id.into(),
        count: values.len(),
        mean_milli: if values.is_empty() { 0 } else { mean(values) },
        min_milli: values.iter().copied().min().unwrap_or(0),
        max_milli: values.iter().copied().max().unwrap_or(0),
    }
}

fn unresolved_result(
    request: &AnalysisRequest,
    summaries: Vec<ArmSummary>,
    batch_overlap_order: Vec<String>,
    negative: BTreeSet<String>,
    uncertainty: BTreeSet<String>,
) -> Result<AnalysisResult, AnalysisError> {
    let mut result = AnalysisResult {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        estimand: format!(
            "mean({}) - mean({})",
            request.treatment_arm, request.control_arm
        ),
        summaries,
        effect_milli: 0,
        interval_low_milli: 0,
        interval_high_milli: 0,
        uncertainty_milli: 0,
        permutation_p_milli: None,
        batch_overlap_order,
        negative_evidence: negative.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition: AnalysisDisposition::Unresolved,
        digest: ContentHash::of_value(&serde_json::json!({}))
            .map_err(|e| AnalysisError::Digest(e.to_string()))?,
    };
    result.digest = ContentHash::of_value(&digest_input(&result))
        .map_err(|e| AnalysisError::Digest(e.to_string()))?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dataset(values: &[(String, String, i64)]) -> AnalysisDataset {
        AnalysisDataset {
            dataset_id: "dataset-1".into(),
            artifact: LocalArtifactRef {
                artifact_id: "artifact-dataset-1".into(),
                content_hash: ContentHash::of_value(&serde_json::json!({"dataset": "1"})).unwrap(),
                content_type: "application/vnd.aurora.glioma-analysis+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
            rows: values
                .iter()
                .enumerate()
                .map(|(index, (arm, batch, outcome))| AnalysisRow {
                    row_id: format!("row-{index}"),
                    arm_id: arm.clone(),
                    model_system: GliomaModelSystem::Organoid,
                    batch_id: batch.clone(),
                    outcome_milli: *outcome,
                })
                .collect(),
        }
    }

    fn request() -> AnalysisRequest {
        AnalysisRequest {
            objective: "estimate perturbation effect".into(),
            control_arm: "control".into(),
            treatment_arm: "treated".into(),
            model_system: GliomaModelSystem::Organoid,
            min_replicates_per_arm: 3,
            effect_threshold_milli: 100,
            alpha_milli: 50,
        }
    }

    #[test]
    fn analysis_returns_effect_uncertainty_and_replay_stable_digest() {
        let data = dataset(&[
            ("control".into(), "batch-a".into(), 100),
            ("control".into(), "batch-b".into(), 105),
            ("control".into(), "batch-c".into(), 95),
            ("treated".into(), "batch-d".into(), 300),
            ("treated".into(), "batch-e".into(), 305),
            ("treated".into(), "batch-f".into(), 295),
        ]);
        let first = analyze_preclinical_outcomes(&request(), &data).unwrap();
        let second = analyze_preclinical_outcomes(&request(), &data).unwrap();
        assert_eq!(first, second);
        assert!(first.effect_milli > 0);
        assert!(first.permutation_p_milli.is_some());
        first.validate().unwrap();
    }

    #[test]
    fn null_effect_is_published_as_negative_not_as_failure() {
        let data = dataset(&[
            ("control".into(), "batch-a".into(), 100),
            ("control".into(), "batch-b".into(), 101),
            ("control".into(), "batch-c".into(), 99),
            ("treated".into(), "batch-d".into(), 101),
            ("treated".into(), "batch-e".into(), 100),
            ("treated".into(), "batch-f".into(), 100),
        ]);
        let result = analyze_preclinical_outcomes(&request(), &data).unwrap();
        assert_eq!(result.disposition, AnalysisDisposition::Negative);
        assert!(result
            .negative_evidence
            .iter()
            .any(|item| item.contains("below")));
    }
}

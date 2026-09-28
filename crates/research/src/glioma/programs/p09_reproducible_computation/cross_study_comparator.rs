//! Cross-study computational result comparison for preclinical glioma research.
//!
//! A numerical result is not comparable merely because two files contain the same column name.
//! Unit changes, semantic-schema changes, normalization revisions, and repeated samples can all
//! create apparent biological disagreement.  This feature aligns an explicit feature contract,
//! excludes incomparable fields without coercion, aggregates independent experimental groups with
//! equal weight, uses inverse-uncertainty weighting within each group, and decomposes the remaining
//! variation into descriptive within-pipeline and between-pipeline components.  It is a comparator
//! and audit surface, not a causal estimator and not a clinical classifier.

use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P09-F18";
pub const OUTPUT_SCHEMA: &str = "GliomaCrossStudyComputationComparison1@1";
pub const MAX_STUDIES: usize = 256;
pub const MAX_FEATURES: usize = 4_096;
pub const MAX_VALUE_MILLI: i64 = 1_000_000_000_000;

/// A feature's semantic contract.  Values use the declared unit and fixed-point milli-scale;
/// unit conversion is intentionally not attempted by the comparator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationFeatureSpec {
    pub feature_id: String,
    pub label: String,
    pub unit: String,
    pub semantic_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationFeatureObservation {
    pub feature_id: String,
    pub value_milli: i64,
    pub uncertainty_milli: u64,
    pub measured: bool,
    pub missing_reason: Option<String>,
}

/// One locally held computation result.  The artifact reference is a value-only handle; raw rows
/// remain at the institution and are never moved by this comparator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossStudyComputationStudy {
    pub study_id: String,
    pub independence_group: String,
    pub model_system: GliomaModelSystem,
    pub pipeline_id: String,
    pub pipeline_version: String,
    pub normalization_id: String,
    pub environment_digest: ContentHash,
    pub feature_schema_digest: ContentHash,
    pub feature_specs: Vec<ComputationFeatureSpec>,
    pub observations: Vec<ComputationFeatureObservation>,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossStudyComputationComparatorRequest {
    pub objective: String,
    pub feature_order: Vec<String>,
    pub studies: Vec<CrossStudyComputationStudy>,
    pub minimum_studies: usize,
    pub minimum_independence_groups: usize,
    pub maximum_total_range_milli: u64,
    pub maximum_pipeline_shift_milli: u64,
    pub maximum_leave_one_group_out_shift_milli: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureComparisonDisposition {
    Comparable,
    BiologicalVariation,
    ProcessingShift,
    NonComparable,
    Insufficient,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossStudyFeatureComparison {
    pub feature_id: String,
    pub label: String,
    pub unit: String,
    pub included_study_order: Vec<String>,
    pub included_independence_group_order: Vec<String>,
    pub excluded_study_order: Vec<String>,
    pub pooled_mean_milli: Option<i64>,
    pub pooled_uncertainty_milli: Option<u64>,
    pub total_range_milli: Option<u64>,
    pub total_mad_milli: Option<u64>,
    pub within_pipeline_mad_milli: Option<u64>,
    pub between_pipeline_mad_milli: Option<u64>,
    pub leave_one_group_out_max_shift_milli: Option<u64>,
    pub agreement_score_milli: Option<u16>,
    pub disposition: FeatureComparisonDisposition,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrossStudyComputationComparisonDisposition {
    Qualified,
    Partial,
    NonComparable,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossStudyComputationComparison {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub study_order: Vec<String>,
    pub pipeline_order: Vec<String>,
    pub model_system_order: Vec<GliomaModelSystem>,
    pub comparisons: Vec<CrossStudyFeatureComparison>,
    pub non_comparable_feature_order: Vec<String>,
    pub processing_shift_feature_order: Vec<String>,
    pub biological_variation_feature_order: Vec<String>,
    pub excluded_study_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: CrossStudyComputationComparisonDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CrossStudyComputationComparisonError {
    #[error("cross-study comparator request is invalid: {0}")]
    InvalidRequest(String),
    #[error("cross-study comparator output is invalid: {0}")]
    InvalidOutput(String),
    #[error("cross-study comparator digest failed: {0}")]
    Digest(String),
}

#[derive(Debug, Clone)]
struct ValidValue {
    study_id: String,
    independence_group: String,
    pipeline_key: String,
    value_milli: i64,
    uncertainty_milli: u64,
}

fn canonical_strings(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn canonical_models(values: &[GliomaModelSystem]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique_strings(values: &[String]) -> bool {
    let mut seen = BTreeSet::new();
    values
        .iter()
        .all(|value| !value.trim().is_empty() && seen.insert(value))
}

fn sorted_unique(values: impl IntoIterator<Item = String>) -> Vec<String> {
    values
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn mean_i64(values: &[i64]) -> i64 {
    if values.is_empty() {
        return 0;
    }
    let sum = values.iter().map(|value| i128::from(*value)).sum::<i128>();
    (sum / i128::from(values.len() as i64)) as i64
}

fn mean_u64(values: &[u64]) -> u64 {
    if values.is_empty() {
        return 0;
    }
    let sum = values.iter().map(|value| u128::from(*value)).sum::<u128>();
    (sum / u128::from(values.len() as u64)) as u64
}

fn abs_difference(left: i64, right: i64) -> u64 {
    (i128::from(left) - i128::from(right)).unsigned_abs() as u64
}

fn mad(values: &[i64], center: i64) -> u64 {
    mean_u64(
        &values
            .iter()
            .map(|value| abs_difference(*value, center))
            .collect::<Vec<_>>(),
    )
}

/// Convert an uncertainty value into a bounded integer precision weight. Zero uncertainty is
/// treated as the most precise declared value, while a very noisy observation retains a minimum
/// weight of one so it remains visible rather than being silently dropped.
fn precision_weight(uncertainty_milli: u64) -> u128 {
    const MAX_PRECISION: u128 = 1_000_000_000;
    if uncertainty_milli == 0 {
        MAX_PRECISION
    } else {
        MAX_PRECISION
            .checked_div(u128::from(uncertainty_milli))
            .unwrap_or(1)
            .max(1)
    }
}

fn weighted_mean(values: &[&ValidValue]) -> i64 {
    if values.is_empty() {
        return 0;
    }
    let (weighted_sum, total_weight) = values.iter().fold((0_i128, 0_u128), |state, value| {
        let weight = precision_weight(value.uncertainty_milli);
        (
            state
                .0
                .saturating_add(i128::from(value.value_milli).saturating_mul(weight as i128)),
            state.1.saturating_add(weight),
        )
    });
    if total_weight == 0 {
        0
    } else {
        (weighted_sum / total_weight as i128) as i64
    }
}

fn weighted_mad(values: &[&ValidValue], center: i64) -> u64 {
    if values.is_empty() {
        return 0;
    }
    let (weighted_deviation, total_weight) =
        values.iter().fold((0_u128, 0_u128), |state, value| {
            let weight = precision_weight(value.uncertainty_milli);
            (
                state.0.saturating_add(
                    u128::from(abs_difference(value.value_milli, center)).saturating_mul(weight),
                ),
                state.1.saturating_add(weight),
            )
        });
    if total_weight == 0 {
        0
    } else {
        (weighted_deviation / total_weight).min(u128::from(u64::MAX)) as u64
    }
}

fn digest_input(output: &CrossStudyComputationComparison) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "study_order": output.study_order,
        "pipeline_order": output.pipeline_order,
        "model_system_order": output.model_system_order,
        "comparisons": output.comparisons,
        "non_comparable_feature_order": output.non_comparable_feature_order,
        "processing_shift_feature_order": output.processing_shift_feature_order,
        "biological_variation_feature_order": output.biological_variation_feature_order,
        "excluded_study_order": output.excluded_study_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn validate_study(
    study: &CrossStudyComputationStudy,
    feature_order: &[String],
) -> Result<(), CrossStudyComputationComparisonError> {
    if study.study_id.trim().is_empty()
        || study.independence_group.trim().is_empty()
        || study.pipeline_id.trim().is_empty()
        || study.pipeline_version.trim().is_empty()
        || study.normalization_id.trim().is_empty()
        || study.environment_digest.as_str().len() != 64
        || study.feature_schema_digest.as_str().len() != 64
        || study.feature_specs.is_empty()
        || study.feature_specs.len() > MAX_FEATURES
        || study.observations.len() > MAX_FEATURES
        || study.artifact.validate().is_err()
        || !study
            .feature_specs
            .windows(2)
            .all(|pair| pair[0].feature_id < pair[1].feature_id)
        || !study
            .observations
            .windows(2)
            .all(|pair| pair[0].feature_id < pair[1].feature_id)
    {
        return Err(CrossStudyComputationComparisonError::InvalidRequest(
            "study identity, pipeline/environment/schema identities, local artifact, and canonical feature records are required".into(),
        ));
    }
    let mut spec_ids = BTreeSet::new();
    for spec in &study.feature_specs {
        if spec.feature_id.trim().is_empty()
            || spec.label.trim().is_empty()
            || spec.unit.trim().is_empty()
            || spec.semantic_version.trim().is_empty()
            || !spec_ids.insert(spec.feature_id.clone())
        {
            return Err(CrossStudyComputationComparisonError::InvalidRequest(
                "feature specs must have unique identities, labels, units, and semantic versions"
                    .into(),
            ));
        }
    }
    let mut observation_ids = BTreeSet::new();
    for observation in &study.observations {
        if !observation_ids.insert(observation.feature_id.clone())
            || observation.feature_id.trim().is_empty()
            || observation.value_milli.abs() > MAX_VALUE_MILLI
            || observation.uncertainty_milli > MAX_VALUE_MILLI as u64
            || observation.measured && observation.missing_reason.is_some()
            || !observation.measured
                && observation
                    .missing_reason
                    .as_deref()
                    .is_none_or(str::is_empty)
        {
            return Err(CrossStudyComputationComparisonError::InvalidRequest(
                "observations must be unique, bounded, and explicit about measured versus missing state".into(),
            ));
        }
    }
    if feature_order
        .iter()
        .any(|feature_id| !spec_ids.contains(feature_id))
    {
        return Err(CrossStudyComputationComparisonError::InvalidRequest(
            "every requested feature must have a semantic spec in every study".into(),
        ));
    }
    Ok(())
}

fn validate_request(
    request: &CrossStudyComputationComparatorRequest,
) -> Result<(), CrossStudyComputationComparisonError> {
    if request.objective.trim().is_empty()
        || request.feature_order.is_empty()
        || request.feature_order.len() > MAX_FEATURES
        || !canonical_strings(&request.feature_order)
        || !unique_strings(&request.feature_order)
        || request.studies.len() < 2
        || request.studies.len() > MAX_STUDIES
        || request.minimum_studies < 2
        || request.minimum_studies > request.studies.len()
        || request.minimum_independence_groups < 2
        || request.maximum_total_range_milli == 0
        || request.maximum_pipeline_shift_milli == 0
        || request.maximum_leave_one_group_out_shift_milli == 0
    {
        return Err(CrossStudyComputationComparisonError::InvalidRequest(
            "objective, canonical feature order, at least two bounded studies, independent-group floor, and positive comparison gates are required".into(),
        ));
    }
    let mut study_ids = BTreeSet::new();
    for study in &request.studies {
        if !study_ids.insert(study.study_id.clone()) {
            return Err(CrossStudyComputationComparisonError::InvalidRequest(
                "study identities must be unique".into(),
            ));
        }
        validate_study(study, &request.feature_order)?;
    }
    Ok(())
}

impl CrossStudyComputationComparison {
    pub fn validate(&self) -> Result<(), CrossStudyComputationComparisonError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || !canonical_strings(&self.study_order)
            || !canonical_strings(&self.pipeline_order)
            || !canonical_models(&self.model_system_order)
            || self
                .comparisons
                .windows(2)
                .any(|pair| pair[0].feature_id >= pair[1].feature_id)
            || !canonical_strings(&self.non_comparable_feature_order)
            || !canonical_strings(&self.processing_shift_feature_order)
            || !canonical_strings(&self.biological_variation_feature_order)
            || !canonical_strings(&self.excluded_study_order)
            || !canonical_strings(&self.negative_evidence)
            || !canonical_strings(&self.uncertainty)
            || self.comparisons.iter().any(|comparison| {
                comparison.feature_id.trim().is_empty()
                    || comparison.label.trim().is_empty()
                    || comparison.unit.trim().is_empty()
                    || !canonical_strings(&comparison.included_study_order)
                    || !canonical_strings(&comparison.included_independence_group_order)
                    || !canonical_strings(&comparison.excluded_study_order)
                    || !canonical_strings(&comparison.reasons)
                    || comparison
                        .agreement_score_milli
                        .is_some_and(|score| score > 1_000)
            })
        {
            return Err(CrossStudyComputationComparisonError::InvalidOutput(
                "comparison identity, ordering, score, and evidence invariants are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| CrossStudyComputationComparisonError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(CrossStudyComputationComparisonError::InvalidOutput(
                "cross-study comparison digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Compare typed computational results while keeping unit/schema/missingness failures explicit.
pub fn compare_glioma_cross_study_computation(
    request: &CrossStudyComputationComparatorRequest,
) -> Result<CrossStudyComputationComparison, CrossStudyComputationComparisonError> {
    validate_request(request)?;
    let mut studies = request.studies.clone();
    studies.sort_by(|left, right| left.study_id.cmp(&right.study_id));
    let study_order = studies
        .iter()
        .map(|study| study.study_id.clone())
        .collect::<Vec<_>>();
    let pipeline_order = sorted_unique(studies.iter().map(|study| {
        format!(
            "{}@{}:{}",
            study.pipeline_id, study.pipeline_version, study.normalization_id
        )
    }));
    let model_system_order = studies
        .iter()
        .map(|study| study.model_system)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut comparisons = Vec::new();
    let mut excluded_study_order = BTreeSet::new();
    let mut negative_evidence = Vec::new();
    let mut uncertainty = Vec::new();

    for feature_id in &request.feature_order {
        let reference_spec = studies
            .iter()
            .find_map(|study| {
                study
                    .feature_specs
                    .iter()
                    .find(|spec| spec.feature_id == *feature_id)
            })
            .expect("validated feature spec coverage");
        let mut values = Vec::<ValidValue>::new();
        let mut excluded = Vec::new();
        let mut reasons = Vec::new();
        for study in &studies {
            let spec = study
                .feature_specs
                .iter()
                .find(|spec| spec.feature_id == *feature_id)
                .expect("validated feature spec coverage");
            if spec.unit != reference_spec.unit
                || spec.semantic_version != reference_spec.semantic_version
            {
                excluded.push(study.study_id.clone());
                excluded_study_order.insert(study.study_id.clone());
                reasons.push(format!(
                    "{}:feature-contract-mismatch:{}:{}",
                    study.study_id, spec.unit, spec.semantic_version
                ));
                continue;
            }
            let Some(observation) = study
                .observations
                .iter()
                .find(|observation| observation.feature_id == *feature_id)
            else {
                excluded.push(study.study_id.clone());
                excluded_study_order.insert(study.study_id.clone());
                reasons.push(format!("{}:observation-absent", study.study_id));
                continue;
            };
            if !observation.measured {
                excluded.push(study.study_id.clone());
                excluded_study_order.insert(study.study_id.clone());
                reasons.push(format!(
                    "{}:not-measured:{}",
                    study.study_id,
                    observation
                        .missing_reason
                        .as_deref()
                        .unwrap_or("unspecified")
                ));
                continue;
            }
            values.push(ValidValue {
                study_id: study.study_id.clone(),
                independence_group: study.independence_group.clone(),
                pipeline_key: format!(
                    "{}@{}:{}",
                    study.pipeline_id, study.pipeline_version, study.normalization_id
                ),
                value_milli: observation.value_milli,
                uncertainty_milli: observation.uncertainty_milli,
            });
        }
        excluded.sort();
        reasons.sort();
        let mut groups = BTreeMap::<String, Vec<&ValidValue>>::new();
        for value in &values {
            groups
                .entry(value.independence_group.clone())
                .or_default()
                .push(value);
        }
        let included_independence_group_order = groups.keys().cloned().collect::<Vec<_>>();
        let included_study_order = values
            .iter()
            .map(|value| value.study_id.clone())
            .collect::<Vec<_>>();
        let label = reference_spec.label.clone();
        let unit = reference_spec.unit.clone();
        let mut comparison = CrossStudyFeatureComparison {
            feature_id: feature_id.clone(),
            label,
            unit,
            included_study_order,
            included_independence_group_order,
            excluded_study_order: excluded,
            pooled_mean_milli: None,
            pooled_uncertainty_milli: None,
            total_range_milli: None,
            total_mad_milli: None,
            within_pipeline_mad_milli: None,
            between_pipeline_mad_milli: None,
            leave_one_group_out_max_shift_milli: None,
            agreement_score_milli: None,
            disposition: FeatureComparisonDisposition::Insufficient,
            reasons,
        };
        if values.len() < request.minimum_studies
            || groups.len() < request.minimum_independence_groups
        {
            comparison.reasons.push(format!(
                "insufficient-comparable-support:studies={}:groups={}",
                values.len(),
                groups.len()
            ));
            comparison.reasons.sort();
            comparison.disposition = if values.is_empty() {
                FeatureComparisonDisposition::NonComparable
            } else {
                FeatureComparisonDisposition::Insufficient
            };
            comparisons.push(comparison);
            continue;
        }

        let group_means = groups
            .values()
            .map(|group| weighted_mean(group))
            .collect::<Vec<_>>();
        let pooled_mean = mean_i64(&group_means);
        let all_values = values
            .iter()
            .map(|value| value.value_milli)
            .collect::<Vec<_>>();
        let min_value = *all_values.iter().min().expect("non-empty values");
        let max_value = *all_values.iter().max().expect("non-empty values");
        let total_range = abs_difference(max_value, min_value);
        let value_refs = values.iter().collect::<Vec<_>>();
        let total_mad = weighted_mad(&value_refs, pooled_mean);
        let pooled_uncertainty = mean_u64(
            &values
                .iter()
                .map(|value| value.uncertainty_milli)
                .collect::<Vec<_>>(),
        );
        let mut pipeline_groups = BTreeMap::<String, Vec<&ValidValue>>::new();
        for value in &values {
            pipeline_groups
                .entry(value.pipeline_key.clone())
                .or_default()
                .push(value);
        }
        let pipeline_means = pipeline_groups
            .values()
            .map(|pipeline_values| weighted_mean(pipeline_values))
            .collect::<Vec<_>>();
        let within_pipeline = mean_u64(
            &pipeline_groups
                .values()
                .map(|pipeline_values| {
                    weighted_mad(pipeline_values, weighted_mean(pipeline_values))
                })
                .collect::<Vec<_>>(),
        );
        let between_pipeline = mad(&pipeline_means, pooled_mean);
        let mut leave_one_out_max_shift = 0_u64;
        for omitted_group in groups.keys() {
            let remaining_means = groups
                .iter()
                .filter(|(group, _)| *group != omitted_group)
                .map(|(_, group)| weighted_mean(group))
                .collect::<Vec<_>>();
            if !remaining_means.is_empty() {
                leave_one_out_max_shift = leave_one_out_max_shift
                    .max(abs_difference(pooled_mean, mean_i64(&remaining_means)));
            }
        }
        let agreement_score = if request.maximum_total_range_milli == 0 {
            0
        } else {
            1_000_u64.saturating_sub(
                total_range
                    .saturating_mul(1_000)
                    .checked_div(request.maximum_total_range_milli)
                    .unwrap_or(1_000),
            ) as u16
        };
        comparison.pooled_mean_milli = Some(pooled_mean);
        comparison.pooled_uncertainty_milli = Some(pooled_uncertainty);
        comparison.total_range_milli = Some(total_range);
        comparison.total_mad_milli = Some(total_mad);
        comparison.within_pipeline_mad_milli = Some(within_pipeline);
        comparison.between_pipeline_mad_milli = Some(between_pipeline);
        comparison.leave_one_group_out_max_shift_milli = Some(leave_one_out_max_shift);
        comparison.agreement_score_milli = Some(agreement_score);
        if total_range <= request.maximum_total_range_milli
            && between_pipeline <= request.maximum_pipeline_shift_milli
            && leave_one_out_max_shift <= request.maximum_leave_one_group_out_shift_milli
        {
            comparison.disposition = FeatureComparisonDisposition::Comparable;
        } else if between_pipeline > within_pipeline
            && between_pipeline > request.maximum_pipeline_shift_milli
        {
            comparison.disposition = FeatureComparisonDisposition::ProcessingShift;
            comparison
                .reasons
                .push("between-pipeline variation dominates within-pipeline variation".into());
        } else {
            comparison.disposition = FeatureComparisonDisposition::BiologicalVariation;
            comparison.reasons.push("variation remains after pipeline grouping and is not attributable to a dominant processing shift".into());
        }
        comparison.reasons.sort();
        if comparison.disposition != FeatureComparisonDisposition::Comparable {
            uncertainty.push(format!("{}:{:?}", feature_id, comparison.disposition));
        }
        comparisons.push(comparison);
    }

    let non_comparable_feature_order = comparisons
        .iter()
        .filter(|comparison| {
            matches!(
                comparison.disposition,
                FeatureComparisonDisposition::NonComparable
                    | FeatureComparisonDisposition::Insufficient
            )
        })
        .map(|comparison| comparison.feature_id.clone())
        .collect::<Vec<_>>();
    let processing_shift_feature_order = comparisons
        .iter()
        .filter(|comparison| {
            comparison.disposition == FeatureComparisonDisposition::ProcessingShift
        })
        .map(|comparison| comparison.feature_id.clone())
        .collect::<Vec<_>>();
    let biological_variation_feature_order = comparisons
        .iter()
        .filter(|comparison| {
            comparison.disposition == FeatureComparisonDisposition::BiologicalVariation
        })
        .map(|comparison| comparison.feature_id.clone())
        .collect::<Vec<_>>();
    let comparable_count = comparisons
        .iter()
        .filter(|comparison| comparison.disposition == FeatureComparisonDisposition::Comparable)
        .count();
    let disposition = if comparable_count == request.feature_order.len() {
        CrossStudyComputationComparisonDisposition::Qualified
    } else if comparable_count == 0 && !comparisons.is_empty() {
        if comparisons
            .iter()
            .all(|comparison| comparison.disposition == FeatureComparisonDisposition::NonComparable)
        {
            CrossStudyComputationComparisonDisposition::NonComparable
        } else {
            CrossStudyComputationComparisonDisposition::Unresolved
        }
    } else {
        CrossStudyComputationComparisonDisposition::Partial
    };
    if !non_comparable_feature_order.is_empty() {
        negative_evidence
            .push("incomparable-or-underpowered-features-excluded-from-pooling".into());
    }
    if !processing_shift_feature_order.is_empty() {
        negative_evidence.push("pipeline-processing-shift-requires-method-review".into());
    }
    let mut output = CrossStudyComputationComparison {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        study_order,
        pipeline_order,
        model_system_order,
        comparisons,
        non_comparable_feature_order,
        processing_shift_feature_order,
        biological_variation_feature_order,
        excluded_study_order: excluded_study_order.into_iter().collect(),
        negative_evidence: sorted_unique(negative_evidence),
        uncertainty: sorted_unique(uncertainty),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-cross-study-comparison"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| CrossStudyComputationComparisonError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(study_id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: format!("artifact-{study_id}"),
            content_hash: ContentHash::of_bytes(study_id.as_bytes()),
            content_type: "application/vnd.aurora.glioma.computation+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn study(
        study_id: &str,
        group: &str,
        pipeline: &str,
        value: i64,
    ) -> CrossStudyComputationStudy {
        CrossStudyComputationStudy {
            study_id: study_id.into(),
            independence_group: group.into(),
            model_system: GliomaModelSystem::Organoid,
            pipeline_id: pipeline.into(),
            pipeline_version: "1".into(),
            normalization_id: "counts-per-cell".into(),
            environment_digest: ContentHash::of_bytes(format!("env-{pipeline}").as_bytes()),
            feature_schema_digest: ContentHash::of_bytes(b"feature-schema-v1"),
            feature_specs: vec![ComputationFeatureSpec {
                feature_id: "invasion_score".into(),
                label: "invasion score".into(),
                unit: "milli_score".into(),
                semantic_version: "1".into(),
            }],
            observations: vec![ComputationFeatureObservation {
                feature_id: "invasion_score".into(),
                value_milli: value,
                uncertainty_milli: 10,
                measured: true,
                missing_reason: None,
            }],
            artifact: artifact(study_id),
        }
    }

    fn request(studies: Vec<CrossStudyComputationStudy>) -> CrossStudyComputationComparatorRequest {
        CrossStudyComputationComparatorRequest {
            objective: "compare glioma invasion computation across studies".into(),
            feature_order: vec!["invasion_score".into()],
            studies,
            minimum_studies: 2,
            minimum_independence_groups: 2,
            maximum_total_range_milli: 50,
            maximum_pipeline_shift_milli: 20,
            maximum_leave_one_group_out_shift_milli: 25,
        }
    }

    #[test]
    fn equal_weighted_cross_study_comparison_is_replay_stable() {
        let first_request = request(vec![
            study("study-b", "group-b", "pipeline-a", 102),
            study("study-a", "group-a", "pipeline-a", 100),
        ]);
        let first = compare_glioma_cross_study_computation(&first_request).unwrap();
        let second = compare_glioma_cross_study_computation(&request(vec![
            study("study-a", "group-a", "pipeline-a", 100),
            study("study-b", "group-b", "pipeline-a", 102),
        ]))
        .unwrap();
        assert_eq!(first, second);
        assert_eq!(
            first.disposition,
            CrossStudyComputationComparisonDisposition::Qualified
        );
        assert_eq!(first.comparisons[0].pooled_mean_milli, Some(101));
        assert_eq!(first.comparisons[0].total_range_milli, Some(2));
    }

    #[test]
    fn noisy_replicate_is_downweighted_within_an_independence_group() {
        let mut noisy = study("study-b", "group-a", "pipeline-a", 300);
        noisy.observations[0].uncertainty_milli = 1_000;
        let output = compare_glioma_cross_study_computation(&request(vec![
            study("study-a", "group-a", "pipeline-a", 100),
            noisy,
            study("study-c", "group-b", "pipeline-a", 100),
        ]))
        .unwrap();
        let comparison = &output.comparisons[0];
        assert_eq!(comparison.pooled_mean_milli, Some(100));
        assert!(comparison.total_mad_milli.unwrap() < 5);
        output.validate().unwrap();
    }

    #[test]
    fn unit_or_semantic_shift_is_excluded_not_silently_converted() {
        let mut shifted = study("study-b", "group-b", "pipeline-a", 100);
        shifted.feature_specs[0].unit = "microns".into();
        let output = compare_glioma_cross_study_computation(&request(vec![
            study("study-a", "group-a", "pipeline-a", 100),
            shifted,
        ]))
        .unwrap();
        assert_eq!(
            output.disposition,
            CrossStudyComputationComparisonDisposition::Unresolved
        );
        assert_eq!(
            output.comparisons[0].disposition,
            FeatureComparisonDisposition::Insufficient
        );
        assert!(output.comparisons[0]
            .reasons
            .iter()
            .any(|reason| reason.contains("feature-contract-mismatch")));
    }

    #[test]
    fn pipeline_shift_is_reported_separately_from_biological_variation() {
        let output = compare_glioma_cross_study_computation(&request(vec![
            study("study-a", "group-a", "pipeline-a", 100),
            study("study-b", "group-b", "pipeline-a", 110),
            study("study-c", "group-c", "pipeline-b", 300),
            study("study-d", "group-d", "pipeline-b", 310),
        ]))
        .unwrap();
        let comparison = &output.comparisons[0];
        assert_eq!(
            comparison.disposition,
            FeatureComparisonDisposition::ProcessingShift
        );
        assert!(
            comparison.between_pipeline_mad_milli.unwrap()
                > comparison.within_pipeline_mad_milli.unwrap()
        );
        assert!(output
            .processing_shift_feature_order
            .contains(&"invasion_score".into()));
    }

    #[test]
    fn repeated_independence_group_does_not_create_false_support_floor() {
        let output = compare_glioma_cross_study_computation(&request(vec![
            study("study-a", "group-a", "pipeline-a", 100),
            study("study-b", "group-a", "pipeline-a", 102),
        ]))
        .unwrap();
        assert_eq!(
            output.disposition,
            CrossStudyComputationComparisonDisposition::Unresolved
        );
        assert_eq!(
            output.comparisons[0].disposition,
            FeatureComparisonDisposition::Insufficient
        );
        assert!(output.comparisons[0]
            .reasons
            .iter()
            .any(|reason| reason.starts_with("insufficient-comparable-support")));
    }
}

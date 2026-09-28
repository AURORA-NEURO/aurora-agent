//! Sensitivity envelope for missing registered preclinical outcome results.
//!
//! The detailed P10-F05 source blueprint is not configured in this checkout. This
//! repository-defined analysis evaluates how caller-declared effect and uncertainty bounds for
//! unavailable registered outcomes could change independent-group directional support. It keeps
//! observed effects separate from hypothetical scenarios, never pools studies, and does not infer
//! why an outcome is missing or claim publication bias.

use super::outcome_record::FEATURE_ID;
pub use super::outcome_record::{OutcomeAvailability, OutcomeSensitivityStudy};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const OUTPUT_SCHEMA: &str = "GliomaOutcomeMissingnessSensitivity1@1";
pub const MAX_STUDIES: usize = 512;
pub const MAX_EFFECT_ABS_MILLI: u64 = 1_000_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeSensitivityDirection {
    Positive,
    Negative,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutcomeSensitivityRequest {
    pub objective: String,
    pub outcome_id: String,
    pub estimand_id: String,
    pub effect_unit: String,
    pub expected_direction: OutcomeSensitivityDirection,
    pub registry_report_order: Vec<ContentHash>,
    pub result_report_order: Vec<ContentHash>,
    pub min_studies: usize,
    pub min_reported_independent_groups: usize,
    pub min_supporting_groups: usize,
    pub min_quality_milli: u16,
    pub effect_threshold_milli: u64,
    /// Explicit caller-owned range used only for missing-result scenarios.
    pub missing_effect_min_milli: i64,
    pub missing_effect_max_milli: i64,
    pub missing_uncertainty_max_milli: u64,
    /// Assumed scenario quality; missing results never inherit observed-study quality.
    pub missing_scenario_quality_milli: u16,
    pub min_direction_concordance_milli: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservedDirection {
    SupportsExpected,
    RefutesExpected,
    Neutral,
    Unobserved,
    NotDue,
    ExcludedLowQuality,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutcomeSensitivityRow {
    pub study_id: String,
    pub independence_group: String,
    pub availability: OutcomeAvailability,
    pub observed_direction: ObservedDirection,
    pub eligible_observed_result: bool,
    pub scenario_eligible: bool,
    pub missing_effect_can_support: bool,
    pub missing_effect_can_refute: bool,
    pub missing_effect_can_be_neutral: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeSensitivityDisposition {
    InsufficientEvidence,
    ObservedDiscordance,
    RobustSupport,
    SensitivityDependent,
    NotSupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutcomeMissingnessSensitivity {
    pub feature_id: String,
    pub output_schema: String,
    pub request: OutcomeSensitivityRequest,
    pub studies: Vec<OutcomeSensitivityStudy>,
    pub input_digest: ContentHash,
    pub study_order: Vec<String>,
    pub rows: Vec<OutcomeSensitivityRow>,
    pub observed_support_order: Vec<String>,
    pub observed_refutation_order: Vec<String>,
    pub observed_neutral_order: Vec<String>,
    pub missing_scenario_order: Vec<String>,
    pub missing_can_support_order: Vec<String>,
    pub missing_can_refute_order: Vec<String>,
    pub missing_can_be_neutral_order: Vec<String>,
    pub low_quality_order: Vec<String>,
    pub not_due_order: Vec<String>,
    pub observed_result_count: usize,
    pub missing_scenario_count: usize,
    pub minimum_supporting_groups: usize,
    pub maximum_supporting_groups: usize,
    pub minimum_direction_concordance_milli: u16,
    pub maximum_direction_concordance_milli: u16,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: OutcomeSensitivityDisposition,
    /// Sensitivity analysis only. No study result is imputed and no research action is dispatched.
    pub dispatch: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum OutcomeSensitivityError {
    #[error("outcome-sensitivity request is invalid: {0}")]
    InvalidRequest(String),
    #[error("outcome-sensitivity study rows are invalid: {0}")]
    InvalidStudy(String),
    #[error("outcome-sensitivity output is invalid: {0}")]
    InvalidOutput(String),
    #[error("outcome-sensitivity digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn valid_hash(hash: &ContentHash) -> bool {
    hash.as_str().len() == 64
}

fn validate_request(request: &OutcomeSensitivityRequest) -> Result<(), OutcomeSensitivityError> {
    if request.objective.trim().is_empty()
        || request.objective.len() > 4_096
        || request.outcome_id.trim().is_empty()
        || request.outcome_id.len() > 128
        || request.estimand_id.trim().is_empty()
        || request.estimand_id.len() > 128
        || request.effect_unit.trim().is_empty()
        || request.effect_unit.len() > 128
        || !(2..=MAX_STUDIES).contains(&request.min_studies)
        || !(2..=request.min_studies).contains(&request.min_reported_independent_groups)
        || !(1..=request.min_reported_independent_groups).contains(&request.min_supporting_groups)
        || request.registry_report_order.len() > MAX_STUDIES
        || !canonical(&request.registry_report_order)
        || request
            .registry_report_order
            .iter()
            .any(|digest| !valid_hash(digest))
        || request.result_report_order.len() > MAX_STUDIES
        || !canonical(&request.result_report_order)
        || request
            .result_report_order
            .iter()
            .any(|digest| !valid_hash(digest))
        || request.effect_threshold_milli == 0
        || request.effect_threshold_milli > MAX_EFFECT_ABS_MILLI
        || request.missing_effect_min_milli > request.missing_effect_max_milli
        || request.missing_effect_min_milli.unsigned_abs() > MAX_EFFECT_ABS_MILLI
        || request.missing_effect_max_milli.unsigned_abs() > MAX_EFFECT_ABS_MILLI
        || request.missing_uncertainty_max_milli > MAX_EFFECT_ABS_MILLI
        || request.missing_scenario_quality_milli > 1_000
        || request.min_quality_milli > 1_000
        || request.min_direction_concordance_milli == 0
        || request.min_direction_concordance_milli > 1_000
    {
        return Err(OutcomeSensitivityError::InvalidRequest(
            "outcome/estimand/unit, canonical report digests, independent-group floors, observed-quality floor, and bounded caller-declared missing-result scenarios are required".into(),
        ));
    }
    Ok(())
}

fn validate_studies(
    request: &OutcomeSensitivityRequest,
    studies: &[OutcomeSensitivityStudy],
) -> Result<(), OutcomeSensitivityError> {
    if studies.len() > MAX_STUDIES {
        return Err(OutcomeSensitivityError::InvalidStudy(
            "study count exceeds the declared bound".into(),
        ));
    }
    let registry_reports = request
        .registry_report_order
        .iter()
        .collect::<BTreeSet<_>>();
    let result_reports = request.result_report_order.iter().collect::<BTreeSet<_>>();
    let mut used_registry_reports = BTreeSet::new();
    let mut used_result_reports = BTreeSet::new();
    let mut study_ids = BTreeSet::new();
    let mut groups = BTreeSet::new();
    for study in studies {
        let observation_shape_valid = match study.availability {
            OutcomeAvailability::Estimate | OutcomeAvailability::Null => {
                study.result_report_digest.is_some()
                    && study.result_artifact.is_some()
                    && study.effect_milli.is_some()
                    && study.uncertainty_milli.is_some()
                    && study.quality_milli.is_some()
            }
            OutcomeAvailability::Incomplete | OutcomeAvailability::Ambiguous => {
                study.result_report_digest.is_some()
                    && study.result_artifact.is_some()
                    && study.effect_milli.is_none()
                    && study.uncertainty_milli.is_none()
                    && study.quality_milli.is_none()
            }
            OutcomeAvailability::Missing => {
                study.effect_milli.is_none()
                    && study.uncertainty_milli.is_none()
                    && study.quality_milli.is_none()
                    && study.result_artifact.is_some() == study.result_report_digest.is_some()
            }
            OutcomeAvailability::NotDue => {
                study.result_report_digest.is_none()
                    && study.result_artifact.is_none()
                    && study.effect_milli.is_none()
                    && study.uncertainty_milli.is_none()
                    && study.quality_milli.is_none()
            }
            OutcomeAvailability::Unresolved => {
                study.effect_milli.is_none()
                    && study.uncertainty_milli.is_none()
                    && study.quality_milli.is_none()
                    && study.result_artifact.is_some() == study.result_report_digest.is_some()
            }
        };
        let null_shape_valid = study.availability != OutcomeAvailability::Null
            || study.effect_milli.zip(study.uncertainty_milli).is_some_and(
                |(effect, uncertainty)| {
                    let low = i128::from(effect) - i128::from(uncertainty);
                    let high = i128::from(effect) + i128::from(uncertainty);
                    low <= i128::from(request.effect_threshold_milli)
                        && high >= -i128::from(request.effect_threshold_milli)
                },
            );
        let registry_valid = valid_hash(&study.registry_report_digest)
            && registry_reports.contains(&&study.registry_report_digest)
            && used_registry_reports.insert(&study.registry_report_digest);
        let result_valid = match (&study.result_report_digest, &study.result_artifact) {
            (Some(digest), Some(artifact)) => {
                valid_hash(digest)
                    && result_reports.contains(&digest)
                    && used_result_reports.insert(digest)
                    && artifact.validate().is_ok()
                    && artifact.local_only
                    && !artifact.contains_human_data
                    && !artifact.contains_direct_identifiers
            }
            (None, None) => true,
            _ => false,
        };
        if study.study_id.trim().is_empty()
            || study.study_id.len() > 128
            || study.independence_group.trim().is_empty()
            || study.independence_group.len() > 128
            || !study_ids.insert(study.study_id.as_str())
            || !groups.insert(study.independence_group.as_str())
            || !registry_valid
            || study.registry_artifact.validate().is_err()
            || !study.registry_artifact.local_only
            || study.registry_artifact.contains_human_data
            || study.registry_artifact.contains_direct_identifiers
            || study.outcome_id != request.outcome_id
            || study.estimand_id != request.estimand_id
            || study.effect_unit != request.effect_unit
            || !observation_shape_valid
            || !null_shape_valid
            || !result_valid
            || study
                .effect_milli
                .is_some_and(|effect| effect.unsigned_abs() > MAX_EFFECT_ABS_MILLI)
            || study
                .uncertainty_milli
                .is_some_and(|uncertainty| uncertainty > MAX_EFFECT_ABS_MILLI)
            || study.quality_milli.is_some_and(|quality| quality > 1_000)
        {
            return Err(OutcomeSensitivityError::InvalidStudy(
                "each row must bind a unique independent group, exact registered outcome/estimand/unit, valid status-specific effect data, and local de-identified source artifacts".into(),
            ));
        }
    }
    if used_registry_reports != registry_reports || used_result_reports != result_reports {
        return Err(OutcomeSensitivityError::InvalidStudy(
            "registry/result report digest sets must exactly cover the supplied study rows".into(),
        ));
    }
    Ok(())
}

fn interval(effect: i64, uncertainty: u64) -> (i128, i128) {
    (
        i128::from(effect) - i128::from(uncertainty),
        i128::from(effect) + i128::from(uncertainty),
    )
}

fn observed_direction(
    request: &OutcomeSensitivityRequest,
    effect: i64,
    uncertainty: u64,
) -> ObservedDirection {
    let (low, high) = interval(effect, uncertainty);
    let threshold = i128::from(request.effect_threshold_milli);
    let (supports, refutes) = match request.expected_direction {
        OutcomeSensitivityDirection::Positive => (low > threshold, high < -threshold),
        OutcomeSensitivityDirection::Negative => (high < -threshold, low > threshold),
    };
    if supports {
        ObservedDirection::SupportsExpected
    } else if refutes {
        ObservedDirection::RefutesExpected
    } else {
        ObservedDirection::Neutral
    }
}

fn missing_direction_possibilities(request: &OutcomeSensitivityRequest) -> (bool, bool, bool) {
    if request.missing_scenario_quality_milli < request.min_quality_milli {
        return (false, false, false);
    }
    let uncertainty = i128::from(request.missing_uncertainty_max_milli);
    let min = i128::from(request.missing_effect_min_milli);
    let max = i128::from(request.missing_effect_max_milli);
    let threshold = i128::from(request.effect_threshold_milli);
    let low = min - uncertainty;
    let high = max + uncertainty;
    let (can_support, can_refute) = match request.expected_direction {
        OutcomeSensitivityDirection::Positive => (
            max - uncertainty > threshold,
            min + uncertainty < -threshold,
        ),
        OutcomeSensitivityDirection::Negative => (
            min + uncertainty < -threshold,
            max - uncertainty > threshold,
        ),
    };
    let can_be_neutral = low <= threshold && high >= -threshold;
    (can_support, can_refute, can_be_neutral)
}

fn input_digest(
    request: &OutcomeSensitivityRequest,
    studies: &[OutcomeSensitivityStudy],
) -> Result<ContentHash, OutcomeSensitivityError> {
    #[derive(Serialize)]
    struct Input<'a> {
        schema: &'static str,
        request: &'a OutcomeSensitivityRequest,
        studies: &'a [OutcomeSensitivityStudy],
    }
    let mut ordered = studies.to_vec();
    ordered.sort_by(|left, right| left.study_id.cmp(&right.study_id));
    ContentHash::of_serializable(&Input {
        schema: "GliomaOutcomeMissingnessSensitivityInput1@1",
        request,
        studies: &ordered,
    })
    .map_err(|error| OutcomeSensitivityError::Digest(error.to_string()))
}

#[derive(Serialize)]
struct OutputDigest<'a> {
    feature_id: &'a str,
    output_schema: &'a str,
    request: &'a OutcomeSensitivityRequest,
    input_digest: &'a ContentHash,
    study_order: &'a [String],
    rows: &'a [OutcomeSensitivityRow],
    observed_support_order: &'a [String],
    observed_refutation_order: &'a [String],
    observed_neutral_order: &'a [String],
    missing_scenario_order: &'a [String],
    missing_can_support_order: &'a [String],
    missing_can_refute_order: &'a [String],
    missing_can_be_neutral_order: &'a [String],
    low_quality_order: &'a [String],
    not_due_order: &'a [String],
    observed_result_count: usize,
    missing_scenario_count: usize,
    minimum_supporting_groups: usize,
    maximum_supporting_groups: usize,
    minimum_direction_concordance_milli: u16,
    maximum_direction_concordance_milli: u16,
    negative_evidence: &'a [String],
    uncertainty: &'a [String],
    disposition: OutcomeSensitivityDisposition,
    dispatch: &'a str,
}

fn output_digest(
    output: &OutcomeMissingnessSensitivity,
) -> Result<ContentHash, OutcomeSensitivityError> {
    ContentHash::of_serializable(&OutputDigest {
        feature_id: &output.feature_id,
        output_schema: &output.output_schema,
        request: &output.request,
        input_digest: &output.input_digest,
        study_order: &output.study_order,
        rows: &output.rows,
        observed_support_order: &output.observed_support_order,
        observed_refutation_order: &output.observed_refutation_order,
        observed_neutral_order: &output.observed_neutral_order,
        missing_scenario_order: &output.missing_scenario_order,
        missing_can_support_order: &output.missing_can_support_order,
        missing_can_refute_order: &output.missing_can_refute_order,
        missing_can_be_neutral_order: &output.missing_can_be_neutral_order,
        low_quality_order: &output.low_quality_order,
        not_due_order: &output.not_due_order,
        observed_result_count: output.observed_result_count,
        missing_scenario_count: output.missing_scenario_count,
        minimum_supporting_groups: output.minimum_supporting_groups,
        maximum_supporting_groups: output.maximum_supporting_groups,
        minimum_direction_concordance_milli: output.minimum_direction_concordance_milli,
        maximum_direction_concordance_milli: output.maximum_direction_concordance_milli,
        negative_evidence: &output.negative_evidence,
        uncertainty: &output.uncertainty,
        disposition: output.disposition,
        dispatch: &output.dispatch,
    })
    .map_err(|error| OutcomeSensitivityError::Digest(error.to_string()))
}

fn concordance_milli(support: usize, refute: usize) -> u16 {
    let decisive = support + refute;
    if decisive == 0 {
        0
    } else {
        ((support as u128 * 1_000) / decisive as u128) as u16
    }
}

struct SensitivityCounts {
    study_count: usize,
    observed_result_count: usize,
    support_count: usize,
    refutation_count: usize,
    maximum_support_count: usize,
    minimum_concordance: u16,
    maximum_concordance: u16,
}

fn derive_disposition(
    request: &OutcomeSensitivityRequest,
    counts: SensitivityCounts,
) -> OutcomeSensitivityDisposition {
    if counts.study_count < request.min_studies
        || counts.observed_result_count < request.min_reported_independent_groups
    {
        OutcomeSensitivityDisposition::InsufficientEvidence
    } else if counts.support_count > 0 && counts.refutation_count > 0 {
        OutcomeSensitivityDisposition::ObservedDiscordance
    } else if counts.support_count >= request.min_supporting_groups
        && counts.minimum_concordance >= request.min_direction_concordance_milli
    {
        OutcomeSensitivityDisposition::RobustSupport
    } else if counts.maximum_support_count >= request.min_supporting_groups
        && counts.maximum_concordance >= request.min_direction_concordance_milli
    {
        OutcomeSensitivityDisposition::SensitivityDependent
    } else {
        OutcomeSensitivityDisposition::NotSupported
    }
}

fn compile(
    request: &OutcomeSensitivityRequest,
    studies: &[OutcomeSensitivityStudy],
) -> Result<OutcomeMissingnessSensitivity, OutcomeSensitivityError> {
    let mut ordered = studies.to_vec();
    ordered.sort_by(|left, right| left.study_id.cmp(&right.study_id));
    let study_order = ordered
        .iter()
        .map(|study| study.study_id.clone())
        .collect::<Vec<_>>();
    let (missing_support_possible, missing_refute_possible, missing_neutral_possible) =
        missing_direction_possibilities(request);
    let mut rows = Vec::new();
    let mut observed_support_order = Vec::new();
    let mut observed_refutation_order = Vec::new();
    let mut observed_neutral_order = Vec::new();
    let mut missing_scenario_order = Vec::new();
    let mut missing_can_support_order = Vec::new();
    let mut missing_can_refute_order = Vec::new();
    let mut missing_can_be_neutral_order = Vec::new();
    let mut low_quality_order = Vec::new();
    let mut not_due_order = Vec::new();
    let mut observed_result_count = 0usize;
    let mut potential_refute_count = 0usize;
    let mut scenario_count = 0usize;
    for study in &ordered {
        let is_observed = matches!(
            study.availability,
            OutcomeAvailability::Estimate | OutcomeAvailability::Null
        ) && study
            .quality_milli
            .is_some_and(|quality| quality >= request.min_quality_milli);
        let direction = match study.availability {
            OutcomeAvailability::NotDue => ObservedDirection::NotDue,
            OutcomeAvailability::Estimate | OutcomeAvailability::Null
                if !study
                    .quality_milli
                    .is_some_and(|quality| quality >= request.min_quality_milli) =>
            {
                low_quality_order.push(study.study_id.clone());
                ObservedDirection::ExcludedLowQuality
            }
            OutcomeAvailability::Estimate | OutcomeAvailability::Null => {
                observed_result_count += 1;
                let value = observed_direction(
                    request,
                    study
                        .effect_milli
                        .expect("validated observed outcome effect"),
                    study
                        .uncertainty_milli
                        .expect("validated observed outcome uncertainty"),
                );
                match value {
                    ObservedDirection::SupportsExpected => {
                        observed_support_order.push(study.study_id.clone());
                    }
                    ObservedDirection::RefutesExpected => {
                        observed_refutation_order.push(study.study_id.clone());
                    }
                    ObservedDirection::Neutral => {
                        observed_neutral_order.push(study.study_id.clone());
                    }
                    _ => {}
                }
                value
            }
            OutcomeAvailability::Missing
            | OutcomeAvailability::Incomplete
            | OutcomeAvailability::Ambiguous
            | OutcomeAvailability::Unresolved => {
                scenario_count += 1;
                missing_scenario_order.push(study.study_id.clone());
                if missing_support_possible {
                    missing_can_support_order.push(study.study_id.clone());
                }
                if missing_refute_possible {
                    potential_refute_count += 1;
                    missing_can_refute_order.push(study.study_id.clone());
                }
                if missing_neutral_possible {
                    missing_can_be_neutral_order.push(study.study_id.clone());
                }
                ObservedDirection::Unobserved
            }
        };
        if study.availability == OutcomeAvailability::NotDue {
            not_due_order.push(study.study_id.clone());
        }
        let scenario_eligible = matches!(
            study.availability,
            OutcomeAvailability::Missing
                | OutcomeAvailability::Incomplete
                | OutcomeAvailability::Ambiguous
                | OutcomeAvailability::Unresolved
        ) && request.missing_scenario_quality_milli
            >= request.min_quality_milli;
        rows.push(OutcomeSensitivityRow {
            study_id: study.study_id.clone(),
            independence_group: study.independence_group.clone(),
            availability: study.availability,
            observed_direction: direction,
            eligible_observed_result: is_observed,
            scenario_eligible,
            missing_effect_can_support: scenario_eligible && missing_support_possible,
            missing_effect_can_refute: scenario_eligible && missing_refute_possible,
            missing_effect_can_be_neutral: scenario_eligible && missing_neutral_possible,
        });
    }
    let support_count = observed_support_order.len();
    let refutation_count = observed_refutation_order.len();
    let scenario_support_count = missing_can_support_order
        .iter()
        .filter(|study_id| {
            rows.iter().any(|row| {
                &row.study_id == *study_id
                    && row.scenario_eligible
                    && row.missing_effect_can_support
            })
        })
        .count();
    let scenario_refute_count = missing_can_refute_order
        .iter()
        .filter(|study_id| {
            rows.iter().any(|row| {
                &row.study_id == *study_id && row.scenario_eligible && row.missing_effect_can_refute
            })
        })
        .count();
    let maximum_supporting_groups = support_count + scenario_support_count;
    let minimum_direction_concordance_milli =
        concordance_milli(support_count, refutation_count + scenario_refute_count);
    let maximum_direction_concordance_milli =
        concordance_milli(maximum_supporting_groups, refutation_count);
    let disposition = derive_disposition(
        request,
        SensitivityCounts {
            study_count: ordered.len(),
            observed_result_count,
            support_count,
            refutation_count,
            maximum_support_count: maximum_supporting_groups,
            minimum_concordance: minimum_direction_concordance_milli,
            maximum_concordance: maximum_direction_concordance_milli,
        },
    );
    let mut negative_evidence = Vec::new();
    match disposition {
        OutcomeSensitivityDisposition::InsufficientEvidence => {
            negative_evidence.push("reported_independent_group_floor_not_met".into());
        }
        OutcomeSensitivityDisposition::ObservedDiscordance => {
            negative_evidence.push("reported_independent_groups_include_opposed_directions".into());
        }
        OutcomeSensitivityDisposition::RobustSupport => {}
        OutcomeSensitivityDisposition::SensitivityDependent => {
            negative_evidence.push("support_threshold_depends_on_missing_outcome_scenario".into());
        }
        OutcomeSensitivityDisposition::NotSupported => {
            negative_evidence.push("declared_scenarios_do_not_establish_the_support_floor".into());
        }
    }
    let mut uncertainty = Vec::new();
    if scenario_count > 0 {
        uncertainty.push(
            "missing_outcome_effect_bounds_are_caller_declared_scenarios_not_imputed_results"
                .into(),
        );
    }
    if !low_quality_order.is_empty() {
        uncertainty.push(
            "low_quality_reported_effects_are_retained_but_excluded_from_support_counts".into(),
        );
    }
    if !not_due_order.is_empty() {
        uncertainty.push("not_due_outcomes_are_excluded_from_missing_result_scenarios".into());
    }
    if potential_refute_count > 0 {
        uncertainty.push(
            "some_missing_outcomes_could_refute_the_expected_direction_within_declared_bounds"
                .into(),
        );
    }
    uncertainty.sort();
    let mut output = OutcomeMissingnessSensitivity {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        request: request.clone(),
        studies: ordered,
        input_digest: input_digest(request, studies)?,
        study_order,
        rows,
        observed_support_order,
        observed_refutation_order,
        observed_neutral_order,
        missing_scenario_order,
        missing_can_support_order,
        missing_can_refute_order,
        missing_can_be_neutral_order,
        low_quality_order,
        not_due_order,
        observed_result_count,
        missing_scenario_count: scenario_count,
        minimum_supporting_groups: support_count,
        maximum_supporting_groups,
        minimum_direction_concordance_milli,
        maximum_direction_concordance_milli,
        negative_evidence,
        uncertainty,
        disposition,
        dispatch: "not_dispatched_sensitivity_only".into(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-outcome-missingness-sensitivity"),
    };
    output.digest = output_digest(&output)?;
    Ok(output)
}

impl OutcomeMissingnessSensitivity {
    pub fn validate(&self) -> Result<(), OutcomeSensitivityError> {
        self.validate_against(&self.request, &self.studies)
    }

    pub fn validate_against(
        &self,
        request: &OutcomeSensitivityRequest,
        studies: &[OutcomeSensitivityStudy],
    ) -> Result<(), OutcomeSensitivityError> {
        validate_request(request)?;
        validate_studies(request, studies)?;
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.dispatch != "not_dispatched_sensitivity_only"
            || &self.request != request
            || input_digest(request, studies)? != self.input_digest
        {
            return Err(OutcomeSensitivityError::InvalidOutput(
                "identity, input binding, or no-dispatch boundary is invalid".into(),
            ));
        }
        let expected = compile(request, studies)?;
        if self != &expected {
            return Err(OutcomeSensitivityError::InvalidOutput(
                "observed and hypothetical direction bounds do not replay from the supplied reports and scenario assumptions".into(),
            ));
        }
        Ok(())
    }
}

pub fn analyze_glioma_outcome_missingness_sensitivity(
    request: &OutcomeSensitivityRequest,
    studies: &[OutcomeSensitivityStudy],
) -> Result<OutcomeMissingnessSensitivity, OutcomeSensitivityError> {
    validate_request(request)?;
    validate_studies(request, studies)?;
    let output = compile(request, studies)?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma_engine::LocalArtifactRef;

    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }

    fn artifact(value: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: value.into(),
            content_hash: hash(value),
            content_type: "application/vnd.aurora.local-study-outcome+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn request() -> OutcomeSensitivityRequest {
        let mut registry_report_order = vec![hash("registry-a"), hash("registry-b")];
        registry_report_order.sort();
        let mut result_report_order = vec![hash("result-a"), hash("result-b")];
        result_report_order.sort();
        OutcomeSensitivityRequest {
            objective: "stress expected-direction support under missing registered results".into(),
            outcome_id: "primary-viability".into(),
            estimand_id: "viability-change-at-24h".into(),
            effect_unit: "normalized-signal-milli".into(),
            expected_direction: OutcomeSensitivityDirection::Positive,
            registry_report_order,
            result_report_order,
            min_studies: 2,
            min_reported_independent_groups: 2,
            min_supporting_groups: 2,
            min_quality_milli: 800,
            effect_threshold_milli: 50,
            missing_effect_min_milli: -300,
            missing_effect_max_milli: 300,
            missing_uncertainty_max_milli: 20,
            missing_scenario_quality_milli: 900,
            min_direction_concordance_milli: 800,
        }
    }

    fn study(
        id: &str,
        availability: OutcomeAvailability,
        effect: Option<i64>,
    ) -> OutcomeSensitivityStudy {
        let reported = matches!(
            availability,
            OutcomeAvailability::Estimate
                | OutcomeAvailability::Null
                | OutcomeAvailability::Incomplete
                | OutcomeAvailability::Ambiguous
        );
        OutcomeSensitivityStudy {
            study_id: id.into(),
            independence_group: format!("group-{id}"),
            registry_report_digest: hash(&format!("registry-{id}")),
            registry_artifact: artifact(&format!("registry-artifact-{id}")),
            result_report_digest: reported.then(|| hash(&format!("result-{id}"))),
            result_artifact: reported.then(|| artifact(&format!("result-artifact-{id}"))),
            outcome_id: "primary-viability".into(),
            estimand_id: "viability-change-at-24h".into(),
            effect_unit: "normalized-signal-milli".into(),
            availability,
            effect_milli: effect,
            uncertainty_milli: effect.map(|_| 20),
            quality_milli: effect.map(|_| 950),
        }
    }

    fn bind_request_reports(
        request: &mut OutcomeSensitivityRequest,
        studies: &[OutcomeSensitivityStudy],
    ) {
        request.registry_report_order = studies
            .iter()
            .map(|study| study.registry_report_digest.clone())
            .collect();
        request.registry_report_order.sort();
        request.result_report_order = studies
            .iter()
            .filter_map(|study| study.result_report_digest.clone())
            .collect();
        request.result_report_order.sort();
    }

    #[test]
    fn complete_observed_support_is_robust_and_replays_without_pooling() {
        let studies = vec![
            study("a", OutcomeAvailability::Estimate, Some(200)),
            study("b", OutcomeAvailability::Estimate, Some(250)),
        ];
        let mut request = request();
        bind_request_reports(&mut request, &studies);
        let result = analyze_glioma_outcome_missingness_sensitivity(&request, &studies).unwrap();
        assert_eq!(
            result.disposition,
            OutcomeSensitivityDisposition::RobustSupport
        );
        assert_eq!(result.minimum_supporting_groups, 2);
        assert_eq!(result.maximum_supporting_groups, 2);
        assert_eq!(result.minimum_direction_concordance_milli, 1_000);
        assert_eq!(result.dispatch, "not_dispatched_sensitivity_only");
        result.validate_against(&request, &studies).unwrap();
    }

    #[test]
    fn missing_result_scenarios_show_when_support_depends_on_unobserved_values() {
        let studies = vec![
            study("a", OutcomeAvailability::Estimate, Some(200)),
            study("b", OutcomeAvailability::Null, Some(0)),
            study("c", OutcomeAvailability::Missing, None),
        ];
        let mut request = request();
        request.min_studies = 3;
        request.min_supporting_groups = 2;
        bind_request_reports(&mut request, &studies);
        let result = analyze_glioma_outcome_missingness_sensitivity(&request, &studies).unwrap();
        assert_eq!(
            result.disposition,
            OutcomeSensitivityDisposition::SensitivityDependent
        );
        assert_eq!(result.minimum_supporting_groups, 1);
        assert_eq!(result.maximum_supporting_groups, 2);
        assert_eq!(result.minimum_direction_concordance_milli, 500);
        assert_eq!(result.maximum_direction_concordance_milli, 1_000);
        assert_eq!(result.missing_can_support_order, vec!["c"]);
        assert_eq!(result.missing_can_refute_order, vec!["c"]);
        assert!(
            result
                .uncertainty
                .iter()
                .any(|item| item.contains("caller_declared_scenarios"))
        );
        result.validate().unwrap();
    }

    #[test]
    fn missing_scenario_quality_below_floor_cannot_create_support() {
        let studies = vec![
            study("a", OutcomeAvailability::Estimate, Some(200)),
            study("b", OutcomeAvailability::Null, Some(0)),
            study("c", OutcomeAvailability::Missing, None),
        ];
        let mut request = request();
        request.min_studies = 3;
        request.min_supporting_groups = 2;
        request.missing_scenario_quality_milli = 700;
        bind_request_reports(&mut request, &studies);
        let result = analyze_glioma_outcome_missingness_sensitivity(&request, &studies).unwrap();
        assert_eq!(
            result.disposition,
            OutcomeSensitivityDisposition::NotSupported
        );
        assert_eq!(result.maximum_supporting_groups, 1);
        assert!(result.missing_can_support_order.is_empty());
    }

    #[test]
    fn not_due_and_low_quality_rows_never_enter_the_support_scenario() {
        let studies = vec![
            study("a", OutcomeAvailability::Estimate, Some(200)),
            study("b", OutcomeAvailability::Estimate, Some(250)),
            study("c", OutcomeAvailability::NotDue, None),
        ];
        let mut request = request();
        request.min_studies = 3;
        request.min_reported_independent_groups = 2;
        request.min_supporting_groups = 1;
        bind_request_reports(&mut request, &studies);
        let result = analyze_glioma_outcome_missingness_sensitivity(&request, &studies).unwrap();
        assert_eq!(result.not_due_order, vec!["c"]);
        assert!(result.missing_scenario_order.is_empty());
        assert_eq!(result.maximum_supporting_groups, 2);
    }

    #[test]
    fn duplicate_groups_and_tampered_sensitivity_bounds_are_rejected() {
        let mut studies = vec![
            study("a", OutcomeAvailability::Estimate, Some(200)),
            study("b", OutcomeAvailability::Estimate, Some(250)),
        ];
        studies[1].independence_group = studies[0].independence_group.clone();
        let mut request = request();
        bind_request_reports(&mut request, &studies);
        assert!(matches!(
            analyze_glioma_outcome_missingness_sensitivity(&request, &studies),
            Err(OutcomeSensitivityError::InvalidStudy(_))
        ));
        studies[1].independence_group = "group-b".into();
        let mut result =
            analyze_glioma_outcome_missingness_sensitivity(&request, &studies).unwrap();
        result.maximum_supporting_groups = 512;
        result.digest = output_digest(&result).unwrap();
        assert!(matches!(
            result.validate(),
            Err(OutcomeSensitivityError::InvalidOutput(_))
        ));
    }

    #[test]
    fn empty_inputs_are_explicitly_insufficient_and_unknown_effect_fields_are_rejected() {
        let mut request = request();
        request.registry_report_order.clear();
        request.result_report_order.clear();
        let output = analyze_glioma_outcome_missingness_sensitivity(&request, &[]).unwrap();
        assert_eq!(
            output.disposition,
            OutcomeSensitivityDisposition::InsufficientEvidence
        );
        let parsed: Result<OutcomeSensitivityStudy, _> =
            serde_json::from_value(serde_json::json!({
                "study_id":"study-a","independence_group":"group-a",
                "registry_report_digest":hash("registry").as_str(),
                "registry_artifact":artifact("registry-artifact"),
                "result_report_digest":hash("result").as_str(),
                "result_artifact":artifact("result-artifact"),
                "outcome_id":"primary-viability","estimand_id":"viability-change-at-24h",
                "effect_unit":"normalized-signal-milli","availability":"missing",
                "p_value":0.01
            }));
        assert!(parsed.is_err());
    }
}

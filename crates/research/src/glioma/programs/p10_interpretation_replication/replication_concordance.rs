//! Pairwise concordance for independent, estimand-aligned preclinical glioma studies.
//!
//! The detailed P10-F02 source blueprint is not configured in this checkout. This
//! repository-defined analysis compares study-level effect intervals without pooling them. It
//! requires exact estimand and effect-unit binding, counts independent groups once, stratifies
//! comparisons by model system, and reports cross-model comparisons as incomparable rather than
//! implying transportability. It is a local research interpretation and never dispatches work.

use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P10-F02";
pub const OUTPUT_SCHEMA: &str = "GliomaMultiStudyConcordance1@1";
pub const MAX_STUDIES: usize = 512;
pub const MAX_PAIR_ROWS: usize = MAX_STUDIES * (MAX_STUDIES - 1) / 2;
pub const MAX_EFFECT_ABS_MILLI: u64 = 1_000_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpectedEffectDirection {
    Positive,
    Negative,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiStudyConcordanceRequest {
    pub objective: String,
    pub estimand_id: String,
    pub effect_unit: String,
    pub expected_direction: ExpectedEffectDirection,
    pub source_report_order: Vec<ContentHash>,
    pub min_studies: usize,
    pub min_independent_groups: usize,
    pub min_supporting_groups: usize,
    pub min_quality_milli: u16,
    pub effect_threshold_milli: u64,
    pub max_interval_gap_milli: u64,
    pub min_direction_concordance_milli: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultiStudyEffectStatus {
    Estimate,
    Null,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiStudyEffect {
    pub study_id: String,
    pub site_id: String,
    /// Globally meaningful independence identifier. One row per group prevents pseudo-replication.
    pub independence_group: String,
    pub source_report_digest: ContentHash,
    pub estimand_id: String,
    pub effect_unit: String,
    pub model_system: GliomaModelSystem,
    pub status: MultiStudyEffectStatus,
    pub effect_milli: Option<i64>,
    pub uncertainty_milli: Option<u64>,
    pub replicate_count: u16,
    pub quality_milli: u16,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiStudyEffectSummary {
    pub study_id: String,
    pub site_id: String,
    pub independence_group: String,
    pub source_report_digest: ContentHash,
    pub estimand_id: String,
    pub effect_unit: String,
    pub model_system: GliomaModelSystem,
    pub status: MultiStudyEffectStatus,
    pub effect_milli: Option<i64>,
    pub uncertainty_milli: Option<u64>,
    pub replicate_count: u16,
    pub quality_milli: u16,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiStudyExclusion {
    pub study_id: String,
    pub reason_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct MultiStudyPair {
    pub first_study_id: String,
    pub second_study_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultiStudyPairRelation {
    Concordant,
    Discordant,
    Heterogeneous,
    CrossModelIncomparable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiStudyPairwiseComparison {
    pub pair: MultiStudyPair,
    pub first_model_system: GliomaModelSystem,
    pub second_model_system: GliomaModelSystem,
    pub effect_difference_milli: u64,
    /// Minimum gap between the two closed uncertainty intervals; zero means they overlap.
    pub interval_gap_milli: u64,
    pub relation: MultiStudyPairRelation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultiStudyConcordanceDisposition {
    InsufficientEvidence,
    CrossModelOnly,
    Negative,
    Heterogeneous,
    Discordant,
    Concordant,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiStudyConcordance {
    pub feature_id: String,
    pub output_schema: String,
    pub request: MultiStudyConcordanceRequest,
    pub input_digest: ContentHash,
    pub study_order: Vec<String>,
    pub eligible_order: Vec<String>,
    pub exclusions: Vec<MultiStudyExclusion>,
    pub study_summaries: Vec<MultiStudyEffectSummary>,
    pub same_model_pair_order: Vec<MultiStudyPair>,
    pub cross_model_pair_order: Vec<MultiStudyPair>,
    pub concordant_pair_order: Vec<MultiStudyPair>,
    pub discordant_pair_order: Vec<MultiStudyPair>,
    pub heterogeneous_pair_order: Vec<MultiStudyPair>,
    pub pairwise_comparisons: Vec<MultiStudyPairwiseComparison>,
    pub supporting_order: Vec<String>,
    pub refuting_order: Vec<String>,
    pub neutral_order: Vec<String>,
    pub null_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub low_quality_order: Vec<String>,
    pub independent_group_count: usize,
    pub direction_concordance_milli: u16,
    pub max_interval_gap_milli: u64,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: MultiStudyConcordanceDisposition,
    /// Analysis only. No study, assay, provider, or federation action is dispatched.
    pub dispatch: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MultiStudyConcordanceError {
    #[error("multi-study concordance request is invalid: {0}")]
    InvalidRequest(String),
    #[error("multi-study effect rows are invalid: {0}")]
    InvalidStudy(String),
    #[error("multi-study concordance output is invalid: {0}")]
    InvalidOutput(String),
    #[error("multi-study concordance digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn valid_hash(hash: &ContentHash) -> bool {
    hash.as_str().len() == 64
}

fn validate_request(
    request: &MultiStudyConcordanceRequest,
) -> Result<(), MultiStudyConcordanceError> {
    if request.objective.trim().is_empty()
        || request.estimand_id.trim().is_empty()
        || request.effect_unit.trim().is_empty()
        || !(2..=MAX_STUDIES).contains(&request.min_studies)
        || !(2..=request.min_studies).contains(&request.min_independent_groups)
        || !(1..=request.min_independent_groups).contains(&request.min_supporting_groups)
        || request.source_report_order.is_empty()
        || request.source_report_order.len() > MAX_STUDIES
        || !canonical(&request.source_report_order)
        || request
            .source_report_order
            .iter()
            .any(|digest| !valid_hash(digest))
        || request.min_quality_milli > 1_000
        || request.effect_threshold_milli == 0
        || request.effect_threshold_milli > MAX_EFFECT_ABS_MILLI
        || request.max_interval_gap_milli > MAX_EFFECT_ABS_MILLI.saturating_mul(2)
        || request.min_direction_concordance_milli > 1_000
    {
        return Err(MultiStudyConcordanceError::InvalidRequest(
            "objective, estimand/unit, source reports, study and independence floors, and bounded concordance thresholds are required".into(),
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

fn validate_studies(
    request: &MultiStudyConcordanceRequest,
    studies: &[MultiStudyEffect],
) -> Result<(), MultiStudyConcordanceError> {
    if studies.len() > MAX_STUDIES {
        return Err(MultiStudyConcordanceError::InvalidStudy(
            "study count exceeds the declared bound".into(),
        ));
    }
    let source_reports = request.source_report_order.iter().collect::<BTreeSet<_>>();
    let mut study_ids = BTreeSet::new();
    let mut independence_groups = BTreeSet::new();
    for study in studies {
        let effects_valid = match study.status {
            MultiStudyEffectStatus::Estimate => {
                study.effect_milli.is_some() && study.uncertainty_milli.is_some()
            }
            MultiStudyEffectStatus::Null => match (study.effect_milli, study.uncertainty_milli) {
                (Some(effect), Some(uncertainty)) => {
                    let (low, high) = interval(effect, uncertainty);
                    low <= i128::from(request.effect_threshold_milli)
                        && high >= -i128::from(request.effect_threshold_milli)
                }
                _ => false,
            },
            MultiStudyEffectStatus::Unresolved => {
                study.effect_milli.is_none() && study.uncertainty_milli.is_none()
            }
        };
        if study.study_id.trim().is_empty()
            || study.site_id.trim().is_empty()
            || study.independence_group.trim().is_empty()
            || !study_ids.insert(study.study_id.as_str())
            || !independence_groups.insert(study.independence_group.as_str())
            || !valid_hash(&study.source_report_digest)
            || !source_reports.contains(&&study.source_report_digest)
            || study.estimand_id != request.estimand_id
            || study.effect_unit != request.effect_unit
            || !effects_valid
            || study
                .effect_milli
                .is_some_and(|effect| effect.unsigned_abs() > MAX_EFFECT_ABS_MILLI)
            || study
                .uncertainty_milli
                .is_some_and(|uncertainty| uncertainty > MAX_EFFECT_ABS_MILLI)
            || study.replicate_count == 0
            || study.quality_milli > 1_000
            || study.artifact.validate().is_err()
            || !study.artifact.local_only
            || study.artifact.contains_human_data
            || study.artifact.contains_direct_identifiers
        {
            return Err(MultiStudyConcordanceError::InvalidStudy(
                "each study must bind a unique independent group, exact estimand/unit, declared report, valid status-specific effect interval, quality, replicates, and local de-identified artifact".into(),
            ));
        }
    }
    Ok(())
}

fn input_digest(
    request: &MultiStudyConcordanceRequest,
    studies: &[MultiStudyEffect],
) -> Result<ContentHash, MultiStudyConcordanceError> {
    #[derive(Serialize)]
    struct Input<'a> {
        input_schema: &'static str,
        request: &'a MultiStudyConcordanceRequest,
        studies: &'a [MultiStudyEffect],
    }
    let mut ordered = studies.to_vec();
    ordered.sort_by(|left, right| left.study_id.cmp(&right.study_id));
    ContentHash::of_serializable(&Input {
        input_schema: "GliomaMultiStudyConcordanceInput1@1",
        request,
        studies: &ordered,
    })
    .map_err(|error| MultiStudyConcordanceError::Digest(error.to_string()))
}

#[derive(Serialize)]
struct OutputDigest<'a> {
    feature_id: &'a str,
    output_schema: &'a str,
    request: &'a MultiStudyConcordanceRequest,
    input_digest: &'a ContentHash,
    study_order: &'a [String],
    eligible_order: &'a [String],
    exclusions: &'a [MultiStudyExclusion],
    study_summaries: &'a [MultiStudyEffectSummary],
    same_model_pair_order: &'a [MultiStudyPair],
    cross_model_pair_order: &'a [MultiStudyPair],
    concordant_pair_order: &'a [MultiStudyPair],
    discordant_pair_order: &'a [MultiStudyPair],
    heterogeneous_pair_order: &'a [MultiStudyPair],
    pairwise_comparisons: &'a [MultiStudyPairwiseComparison],
    supporting_order: &'a [String],
    refuting_order: &'a [String],
    neutral_order: &'a [String],
    null_order: &'a [String],
    unresolved_order: &'a [String],
    low_quality_order: &'a [String],
    independent_group_count: usize,
    direction_concordance_milli: u16,
    max_interval_gap_milli: u64,
    negative_evidence: &'a [String],
    uncertainty: &'a [String],
    disposition: MultiStudyConcordanceDisposition,
    dispatch: &'a str,
}

fn output_digest(
    output: &MultiStudyConcordance,
) -> Result<ContentHash, MultiStudyConcordanceError> {
    ContentHash::of_serializable(&OutputDigest {
        feature_id: &output.feature_id,
        output_schema: &output.output_schema,
        request: &output.request,
        input_digest: &output.input_digest,
        study_order: &output.study_order,
        eligible_order: &output.eligible_order,
        exclusions: &output.exclusions,
        study_summaries: &output.study_summaries,
        same_model_pair_order: &output.same_model_pair_order,
        cross_model_pair_order: &output.cross_model_pair_order,
        concordant_pair_order: &output.concordant_pair_order,
        discordant_pair_order: &output.discordant_pair_order,
        heterogeneous_pair_order: &output.heterogeneous_pair_order,
        pairwise_comparisons: &output.pairwise_comparisons,
        supporting_order: &output.supporting_order,
        refuting_order: &output.refuting_order,
        neutral_order: &output.neutral_order,
        null_order: &output.null_order,
        unresolved_order: &output.unresolved_order,
        low_quality_order: &output.low_quality_order,
        independent_group_count: output.independent_group_count,
        direction_concordance_milli: output.direction_concordance_milli,
        max_interval_gap_milli: output.max_interval_gap_milli,
        negative_evidence: &output.negative_evidence,
        uncertainty: &output.uncertainty,
        disposition: output.disposition,
        dispatch: &output.dispatch,
    })
    .map_err(|error| MultiStudyConcordanceError::Digest(error.to_string()))
}

fn expected_support_and_refutation(
    request: &MultiStudyConcordanceRequest,
    effect: i64,
    uncertainty: u64,
) -> (bool, bool) {
    let (low, high) = interval(effect, uncertainty);
    let threshold = i128::from(request.effect_threshold_milli);
    match request.expected_direction {
        ExpectedEffectDirection::Positive => (low > threshold, high < -threshold),
        ExpectedEffectDirection::Negative => (high < -threshold, low > threshold),
    }
}

fn comparison(
    request: &MultiStudyConcordanceRequest,
    first: &MultiStudyEffectSummary,
    second: &MultiStudyEffectSummary,
) -> MultiStudyPairwiseComparison {
    let first_effect = first
        .effect_milli
        .expect("eligible effect has a point estimate");
    let second_effect = second
        .effect_milli
        .expect("eligible effect has a point estimate");
    let first_uncertainty = first
        .uncertainty_milli
        .expect("eligible effect has uncertainty");
    let second_uncertainty = second
        .uncertainty_milli
        .expect("eligible effect has uncertainty");
    let distance = (i128::from(first_effect) - i128::from(second_effect)).unsigned_abs();
    let gap = distance
        .saturating_sub(u128::from(first_uncertainty))
        .saturating_sub(u128::from(second_uncertainty))
        .min(u128::from(u64::MAX)) as u64;
    let opposite = interval(first_effect, first_uncertainty).0
        > i128::from(request.effect_threshold_milli)
        && interval(second_effect, second_uncertainty).1
            < -i128::from(request.effect_threshold_milli)
        || interval(second_effect, second_uncertainty).0
            > i128::from(request.effect_threshold_milli)
            && interval(first_effect, first_uncertainty).1
                < -i128::from(request.effect_threshold_milli);
    let relation = if first.model_system != second.model_system {
        MultiStudyPairRelation::CrossModelIncomparable
    } else if opposite {
        MultiStudyPairRelation::Discordant
    } else if gap <= request.max_interval_gap_milli {
        MultiStudyPairRelation::Concordant
    } else {
        MultiStudyPairRelation::Heterogeneous
    };
    MultiStudyPairwiseComparison {
        pair: MultiStudyPair {
            first_study_id: first.study_id.clone(),
            second_study_id: second.study_id.clone(),
        },
        first_model_system: first.model_system,
        second_model_system: second.model_system,
        effect_difference_milli: distance.min(u128::from(u64::MAX)) as u64,
        interval_gap_milli: gap,
        relation,
    }
}

struct ConcordanceMetrics {
    eligible_count: usize,
    same_model_pair_count: usize,
    cross_model_pair_count: usize,
    support_count: usize,
    refute_count: usize,
    concordance_milli: u16,
    heterogeneous_count: usize,
    discordant_count: usize,
}

fn derive_disposition(
    request: &MultiStudyConcordanceRequest,
    metrics: ConcordanceMetrics,
) -> MultiStudyConcordanceDisposition {
    if metrics.eligible_count < request.min_studies
        || metrics.eligible_count < request.min_independent_groups
    {
        MultiStudyConcordanceDisposition::InsufficientEvidence
    } else if metrics.same_model_pair_count == 0 && metrics.cross_model_pair_count > 0 {
        MultiStudyConcordanceDisposition::CrossModelOnly
    } else if metrics.same_model_pair_count == 0 {
        MultiStudyConcordanceDisposition::InsufficientEvidence
    } else if metrics.discordant_count > 0 || metrics.support_count > 0 && metrics.refute_count > 0
    {
        MultiStudyConcordanceDisposition::Discordant
    } else if metrics.support_count < request.min_supporting_groups {
        MultiStudyConcordanceDisposition::Negative
    } else if metrics.heterogeneous_count > 0
        || metrics.concordance_milli < request.min_direction_concordance_milli
    {
        MultiStudyConcordanceDisposition::Heterogeneous
    } else {
        MultiStudyConcordanceDisposition::Concordant
    }
}

fn compile(
    request: &MultiStudyConcordanceRequest,
    studies: &[MultiStudyEffect],
) -> Result<MultiStudyConcordance, MultiStudyConcordanceError> {
    let mut ordered = studies.to_vec();
    ordered.sort_by(|left, right| left.study_id.cmp(&right.study_id));
    let study_order = ordered
        .iter()
        .map(|study| study.study_id.clone())
        .collect::<Vec<_>>();
    let study_summaries = ordered
        .iter()
        .map(|study| MultiStudyEffectSummary {
            study_id: study.study_id.clone(),
            site_id: study.site_id.clone(),
            independence_group: study.independence_group.clone(),
            source_report_digest: study.source_report_digest.clone(),
            estimand_id: study.estimand_id.clone(),
            effect_unit: study.effect_unit.clone(),
            model_system: study.model_system,
            status: study.status,
            effect_milli: study.effect_milli,
            uncertainty_milli: study.uncertainty_milli,
            replicate_count: study.replicate_count,
            quality_milli: study.quality_milli,
            artifact: study.artifact.clone(),
        })
        .collect::<Vec<_>>();
    let mut eligible_order = Vec::new();
    let mut exclusions = Vec::new();
    let mut supporting_order = Vec::new();
    let mut refuting_order = Vec::new();
    let mut neutral_order = Vec::new();
    let mut null_order = Vec::new();
    let mut unresolved_order = Vec::new();
    let mut low_quality_order = Vec::new();
    let mut eligible = Vec::<&MultiStudyEffectSummary>::new();
    for study in &study_summaries {
        let mut reason_order = Vec::new();
        if study.quality_milli < request.min_quality_milli {
            reason_order.push("below_minimum_quality".into());
            low_quality_order.push(study.study_id.clone());
        }
        match study.status {
            MultiStudyEffectStatus::Estimate
                if study.quality_milli >= request.min_quality_milli =>
            {
                let (support, refute) = expected_support_and_refutation(
                    request,
                    study.effect_milli.expect("validated estimated effect"),
                    study
                        .uncertainty_milli
                        .expect("validated estimated uncertainty"),
                );
                if support {
                    supporting_order.push(study.study_id.clone());
                } else if refute {
                    refuting_order.push(study.study_id.clone());
                } else {
                    neutral_order.push(study.study_id.clone());
                }
            }
            MultiStudyEffectStatus::Estimate => {}
            MultiStudyEffectStatus::Null => null_order.push(study.study_id.clone()),
            MultiStudyEffectStatus::Unresolved => {
                unresolved_order.push(study.study_id.clone());
                reason_order.push("effect_unresolved".into());
            }
        }
        if reason_order.is_empty() {
            eligible_order.push(study.study_id.clone());
            eligible.push(study);
        } else {
            reason_order.sort();
            exclusions.push(MultiStudyExclusion {
                study_id: study.study_id.clone(),
                reason_order,
            });
        }
    }
    let mut pairwise_comparisons = Vec::new();
    for first_index in 0..eligible.len() {
        for second_index in (first_index + 1)..eligible.len() {
            pairwise_comparisons.push(comparison(
                request,
                eligible[first_index],
                eligible[second_index],
            ));
        }
    }
    let same_model_pair_order = pairwise_comparisons
        .iter()
        .filter(|pair| pair.relation != MultiStudyPairRelation::CrossModelIncomparable)
        .map(|pair| pair.pair.clone())
        .collect::<Vec<_>>();
    let cross_model_pair_order = pairwise_comparisons
        .iter()
        .filter(|pair| pair.relation == MultiStudyPairRelation::CrossModelIncomparable)
        .map(|pair| pair.pair.clone())
        .collect::<Vec<_>>();
    let concordant_pair_order = pairwise_comparisons
        .iter()
        .filter(|pair| pair.relation == MultiStudyPairRelation::Concordant)
        .map(|pair| pair.pair.clone())
        .collect::<Vec<_>>();
    let discordant_pair_order = pairwise_comparisons
        .iter()
        .filter(|pair| pair.relation == MultiStudyPairRelation::Discordant)
        .map(|pair| pair.pair.clone())
        .collect::<Vec<_>>();
    let heterogeneous_pair_order = pairwise_comparisons
        .iter()
        .filter(|pair| pair.relation == MultiStudyPairRelation::Heterogeneous)
        .map(|pair| pair.pair.clone())
        .collect::<Vec<_>>();
    let decisive_count = supporting_order.len() + refuting_order.len();
    let direction_concordance_milli = if decisive_count == 0 {
        0
    } else {
        ((supporting_order.len() as u128 * 1_000) / decisive_count as u128) as u16
    };
    let max_interval_gap_milli = pairwise_comparisons
        .iter()
        .filter(|pair| pair.relation != MultiStudyPairRelation::CrossModelIncomparable)
        .map(|pair| pair.interval_gap_milli)
        .max()
        .unwrap_or(0);
    let disposition = derive_disposition(
        request,
        ConcordanceMetrics {
            eligible_count: eligible.len(),
            same_model_pair_count: same_model_pair_order.len(),
            cross_model_pair_count: cross_model_pair_order.len(),
            support_count: supporting_order.len(),
            refute_count: refuting_order.len(),
            concordance_milli: direction_concordance_milli,
            heterogeneous_count: heterogeneous_pair_order.len(),
            discordant_count: discordant_pair_order.len(),
        },
    );
    let independent_group_count = eligible.len();
    let mut negative_evidence = Vec::new();
    match disposition {
        MultiStudyConcordanceDisposition::InsufficientEvidence => {
            negative_evidence.push("independent_same_model_study_floor_not_met".into());
        }
        MultiStudyConcordanceDisposition::CrossModelOnly => {
            negative_evidence.push("eligible_studies_have_no_same_model_pair".into());
        }
        MultiStudyConcordanceDisposition::Negative => {
            negative_evidence
                .push("no_eligible_study_confidently_supports_the_expected_direction".into());
        }
        MultiStudyConcordanceDisposition::Heterogeneous => {
            negative_evidence
                .push("same_model_effect_intervals_or_directions_are_not_concordant".into());
        }
        MultiStudyConcordanceDisposition::Discordant => {
            negative_evidence
                .push("independent_studies_include_confidently_opposed_effects".into());
        }
        MultiStudyConcordanceDisposition::Concordant => {}
    }
    let mut uncertainty = Vec::new();
    if !cross_model_pair_order.is_empty() {
        uncertainty.push("cross_model_pairs_are_reported_as_incomparable".into());
    }
    if !low_quality_order.is_empty() {
        uncertainty.push("low_quality_studies_are_retained_but_excluded_from_comparison".into());
    }
    if !unresolved_order.is_empty() {
        uncertainty.push("unresolved_study_effects_are_retained_without_imputation".into());
    }
    if !null_order.is_empty() {
        uncertainty.push(
            "explicit_null_studies_remain_visible_and_do_not_count_as_directional_support".into(),
        );
    }
    uncertainty.sort();
    let mut output = MultiStudyConcordance {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        request: request.clone(),
        input_digest: input_digest(request, studies)?,
        study_order,
        eligible_order,
        exclusions,
        study_summaries,
        same_model_pair_order,
        cross_model_pair_order,
        concordant_pair_order,
        discordant_pair_order,
        heterogeneous_pair_order,
        pairwise_comparisons,
        supporting_order,
        refuting_order,
        neutral_order,
        null_order,
        unresolved_order,
        low_quality_order,
        independent_group_count,
        direction_concordance_milli,
        max_interval_gap_milli,
        negative_evidence,
        uncertainty,
        disposition,
        dispatch: "not_dispatched_analysis_only".into(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-multistudy-concordance"),
    };
    output.digest = output_digest(&output)?;
    Ok(output)
}

impl MultiStudyConcordance {
    pub fn validate(&self) -> Result<(), MultiStudyConcordanceError> {
        validate_request(&self.request)?;
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !valid_hash(&self.input_digest)
            || self.study_summaries.len() > MAX_STUDIES
            || !canonical(&self.study_order)
            || !canonical(&self.eligible_order)
            || !canonical(
                &self
                    .exclusions
                    .iter()
                    .map(|item| item.study_id.clone())
                    .collect::<Vec<_>>(),
            )
            || !canonical(&self.supporting_order)
            || !canonical(&self.refuting_order)
            || !canonical(&self.neutral_order)
            || !canonical(&self.null_order)
            || !canonical(&self.unresolved_order)
            || !canonical(&self.low_quality_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.direction_concordance_milli > 1_000
            || self.dispatch != "not_dispatched_analysis_only"
            || self
                .study_summaries
                .iter()
                .map(|item| item.study_id.clone())
                .collect::<Vec<_>>()
                != self.study_order
        {
            return Err(MultiStudyConcordanceError::InvalidOutput(
                "identity, canonical order, dispatch boundary, or study ledger is invalid".into(),
            ));
        }
        let mut expected_eligible = Vec::new();
        let mut expected_exclusions = Vec::new();
        let mut expected_support = Vec::new();
        let mut expected_refute = Vec::new();
        let mut expected_neutral = Vec::new();
        let mut expected_null = Vec::new();
        let mut expected_unresolved = Vec::new();
        let mut expected_low_quality = Vec::new();
        let mut eligible = Vec::new();
        let mut groups = BTreeSet::new();
        for study in &self.study_summaries {
            if study.estimand_id != self.request.estimand_id
                || study.effect_unit != self.request.effect_unit
                || study.study_id.trim().is_empty()
                || study.site_id.trim().is_empty()
                || !valid_hash(&study.source_report_digest)
                || !self
                    .request
                    .source_report_order
                    .contains(&study.source_report_digest)
                || study.independence_group.trim().is_empty()
                || !groups.insert(study.independence_group.as_str())
                || study.replicate_count == 0
                || study.quality_milli > 1_000
                || study
                    .effect_milli
                    .is_some_and(|effect| effect.unsigned_abs() > MAX_EFFECT_ABS_MILLI)
                || study
                    .uncertainty_milli
                    .is_some_and(|uncertainty| uncertainty > MAX_EFFECT_ABS_MILLI)
                || !study.artifact.local_only
                || study.artifact.contains_human_data
                || study.artifact.contains_direct_identifiers
                || match study.status {
                    MultiStudyEffectStatus::Estimate => {
                        study.effect_milli.is_none() || study.uncertainty_milli.is_none()
                    }
                    MultiStudyEffectStatus::Null => {
                        match (study.effect_milli, study.uncertainty_milli) {
                            (Some(effect), Some(uncertainty)) => {
                                let (low, high) = interval(effect, uncertainty);
                                low > i128::from(self.request.effect_threshold_milli)
                                    || high < -i128::from(self.request.effect_threshold_milli)
                            }
                            _ => true,
                        }
                    }
                    MultiStudyEffectStatus::Unresolved => {
                        study.effect_milli.is_some() || study.uncertainty_milli.is_some()
                    }
                }
                || study.artifact.validate().is_err()
            {
                return Err(MultiStudyConcordanceError::InvalidOutput(
                    "study summary has invalid lineage, independence, or local artifact".into(),
                ));
            }
            let mut reasons = Vec::new();
            if study.quality_milli < self.request.min_quality_milli {
                reasons.push("below_minimum_quality".to_string());
                expected_low_quality.push(study.study_id.clone());
            }
            match study.status {
                MultiStudyEffectStatus::Estimate
                    if study.quality_milli >= self.request.min_quality_milli =>
                {
                    let (support, refute) = expected_support_and_refutation(
                        &self.request,
                        study.effect_milli.ok_or_else(|| {
                            MultiStudyConcordanceError::InvalidOutput(
                                "estimated row lacks an effect".into(),
                            )
                        })?,
                        study.uncertainty_milli.ok_or_else(|| {
                            MultiStudyConcordanceError::InvalidOutput(
                                "estimated row lacks uncertainty".into(),
                            )
                        })?,
                    );
                    if support {
                        expected_support.push(study.study_id.clone());
                    } else if refute {
                        expected_refute.push(study.study_id.clone());
                    } else {
                        expected_neutral.push(study.study_id.clone());
                    }
                }
                MultiStudyEffectStatus::Estimate => {}
                MultiStudyEffectStatus::Null => expected_null.push(study.study_id.clone()),
                MultiStudyEffectStatus::Unresolved => {
                    expected_unresolved.push(study.study_id.clone());
                    reasons.push("effect_unresolved".to_string());
                }
            }
            if reasons.is_empty() {
                expected_eligible.push(study.study_id.clone());
                eligible.push(study);
            } else {
                reasons.sort();
                expected_exclusions.push(MultiStudyExclusion {
                    study_id: study.study_id.clone(),
                    reason_order: reasons,
                });
            }
        }
        if expected_eligible != self.eligible_order
            || expected_exclusions != self.exclusions
            || expected_support != self.supporting_order
            || expected_refute != self.refuting_order
            || expected_neutral != self.neutral_order
            || expected_null != self.null_order
            || expected_unresolved != self.unresolved_order
            || expected_low_quality != self.low_quality_order
            || self.independent_group_count != eligible.len()
        {
            return Err(MultiStudyConcordanceError::InvalidOutput(
                "eligibility, evidence classification, or independent-group count does not derive from the study ledger".into(),
            ));
        }
        let mut expected_pairs = Vec::new();
        for first_index in 0..eligible.len() {
            for second_index in (first_index + 1)..eligible.len() {
                expected_pairs.push(comparison(
                    &self.request,
                    eligible[first_index],
                    eligible[second_index],
                ));
            }
        }
        if expected_pairs != self.pairwise_comparisons || expected_pairs.len() > MAX_PAIR_ROWS {
            return Err(MultiStudyConcordanceError::InvalidOutput(
                "pairwise comparison rows are incomplete, out of order, or inconsistent".into(),
            ));
        }
        let same = expected_pairs
            .iter()
            .filter(|item| item.relation != MultiStudyPairRelation::CrossModelIncomparable)
            .map(|item| item.pair.clone())
            .collect::<Vec<_>>();
        let cross = expected_pairs
            .iter()
            .filter(|item| item.relation == MultiStudyPairRelation::CrossModelIncomparable)
            .map(|item| item.pair.clone())
            .collect::<Vec<_>>();
        let concordant = expected_pairs
            .iter()
            .filter(|item| item.relation == MultiStudyPairRelation::Concordant)
            .map(|item| item.pair.clone())
            .collect::<Vec<_>>();
        let discordant = expected_pairs
            .iter()
            .filter(|item| item.relation == MultiStudyPairRelation::Discordant)
            .map(|item| item.pair.clone())
            .collect::<Vec<_>>();
        let heterogeneous = expected_pairs
            .iter()
            .filter(|item| item.relation == MultiStudyPairRelation::Heterogeneous)
            .map(|item| item.pair.clone())
            .collect::<Vec<_>>();
        let decisive = expected_support.len() + expected_refute.len();
        let direction_concordance = if decisive == 0 {
            0
        } else {
            ((expected_support.len() as u128 * 1_000) / decisive as u128) as u16
        };
        let maximum_gap = expected_pairs
            .iter()
            .filter(|item| item.relation != MultiStudyPairRelation::CrossModelIncomparable)
            .map(|item| item.interval_gap_milli)
            .max()
            .unwrap_or(0);
        let disposition = derive_disposition(
            &self.request,
            ConcordanceMetrics {
                eligible_count: eligible.len(),
                same_model_pair_count: same.len(),
                cross_model_pair_count: cross.len(),
                support_count: expected_support.len(),
                refute_count: expected_refute.len(),
                concordance_milli: direction_concordance,
                heterogeneous_count: heterogeneous.len(),
                discordant_count: discordant.len(),
            },
        );
        let expected_negative = match disposition {
            MultiStudyConcordanceDisposition::InsufficientEvidence => {
                vec!["independent_same_model_study_floor_not_met".to_string()]
            }
            MultiStudyConcordanceDisposition::CrossModelOnly => {
                vec!["eligible_studies_have_no_same_model_pair".to_string()]
            }
            MultiStudyConcordanceDisposition::Negative => {
                vec!["no_eligible_study_confidently_supports_the_expected_direction".to_string()]
            }
            MultiStudyConcordanceDisposition::Heterogeneous => {
                vec!["same_model_effect_intervals_or_directions_are_not_concordant".to_string()]
            }
            MultiStudyConcordanceDisposition::Discordant => {
                vec!["independent_studies_include_confidently_opposed_effects".to_string()]
            }
            MultiStudyConcordanceDisposition::Concordant => vec![],
        };
        let mut expected_uncertainty = Vec::new();
        if !cross.is_empty() {
            expected_uncertainty.push("cross_model_pairs_are_reported_as_incomparable".to_string());
        }
        if !expected_low_quality.is_empty() {
            expected_uncertainty
                .push("low_quality_studies_are_retained_but_excluded_from_comparison".to_string());
        }
        if !expected_unresolved.is_empty() {
            expected_uncertainty
                .push("unresolved_study_effects_are_retained_without_imputation".to_string());
        }
        if !expected_null.is_empty() {
            expected_uncertainty.push(
                "explicit_null_studies_remain_visible_and_do_not_count_as_directional_support"
                    .to_string(),
            );
        }
        expected_uncertainty.sort();
        if self.same_model_pair_order != same
            || self.cross_model_pair_order != cross
            || self.concordant_pair_order != concordant
            || self.discordant_pair_order != discordant
            || self.heterogeneous_pair_order != heterogeneous
            || self.direction_concordance_milli != direction_concordance
            || self.max_interval_gap_milli != maximum_gap
            || self.disposition != disposition
            || self.negative_evidence != expected_negative
            || self.uncertainty != expected_uncertainty
            || output_digest(self)? != self.digest
        {
            return Err(MultiStudyConcordanceError::InvalidOutput(
                "pair partitions, aggregate metrics, disposition, uncertainty, or output digest is inconsistent".into(),
            ));
        }
        Ok(())
    }

    pub fn validate_against(
        &self,
        request: &MultiStudyConcordanceRequest,
        studies: &[MultiStudyEffect],
    ) -> Result<(), MultiStudyConcordanceError> {
        self.validate()?;
        validate_request(request)?;
        validate_studies(request, studies)?;
        if &self.request != request || input_digest(request, studies)? != self.input_digest {
            return Err(MultiStudyConcordanceError::InvalidOutput(
                "concordance input digest does not match the supplied request and studies".into(),
            ));
        }
        let expected = compile(request, studies)?;
        if self != &expected {
            return Err(MultiStudyConcordanceError::InvalidOutput(
                "study classification, pair comparisons, or provenance do not derive from supplied sources".into(),
            ));
        }
        Ok(())
    }
}

pub fn analyze_glioma_multistudy_concordance(
    request: &MultiStudyConcordanceRequest,
    studies: &[MultiStudyEffect],
) -> Result<MultiStudyConcordance, MultiStudyConcordanceError> {
    validate_request(request)?;
    validate_studies(request, studies)?;
    let output = compile(request, studies)?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }

    fn artifact(value: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: value.into(),
            content_hash: hash(value),
            content_type: "application/vnd.aurora.local-study-effect+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn request() -> MultiStudyConcordanceRequest {
        MultiStudyConcordanceRequest {
            objective: "check replication of a preclinical response direction".into(),
            estimand_id: "viability-change-at-24h".into(),
            effect_unit: "normalized-signal-milli".into(),
            expected_direction: ExpectedEffectDirection::Positive,
            source_report_order: vec![hash("report")],
            min_studies: 2,
            min_independent_groups: 2,
            min_supporting_groups: 2,
            min_quality_milli: 800,
            effect_threshold_milli: 50,
            max_interval_gap_milli: 20,
            min_direction_concordance_milli: 900,
        }
    }

    fn study(
        id: &str,
        group: &str,
        model_system: GliomaModelSystem,
        status: MultiStudyEffectStatus,
        effect_milli: Option<i64>,
        uncertainty_milli: Option<u64>,
        quality_milli: u16,
    ) -> MultiStudyEffect {
        let request = request();
        MultiStudyEffect {
            study_id: id.into(),
            site_id: format!("site-{id}"),
            independence_group: group.into(),
            source_report_digest: hash("report"),
            estimand_id: request.estimand_id,
            effect_unit: request.effect_unit,
            model_system,
            status,
            effect_milli,
            uncertainty_milli,
            replicate_count: 4,
            quality_milli,
            artifact: artifact(id),
        }
    }

    fn estimate(id: &str, group: &str, effect: i64, uncertainty: u64) -> MultiStudyEffect {
        study(
            id,
            group,
            GliomaModelSystem::Organoid,
            MultiStudyEffectStatus::Estimate,
            Some(effect),
            Some(uncertainty),
            950,
        )
    }

    #[test]
    fn same_model_overlapping_intervals_produce_replayable_concordance() {
        let request = request();
        let studies = vec![
            estimate("study-a", "group-a", 250, 20),
            estimate("study-b", "group-b", 300, 20),
        ];
        let result = analyze_glioma_multistudy_concordance(&request, &studies).unwrap();
        assert_eq!(
            result.disposition,
            MultiStudyConcordanceDisposition::Concordant
        );
        assert_eq!(result.concordant_pair_order.len(), 1);
        assert_eq!(result.direction_concordance_milli, 1_000);
        assert_eq!(result.dispatch, "not_dispatched_analysis_only");
        result.validate_against(&request, &studies).unwrap();
        let replay = analyze_glioma_multistudy_concordance(
            &request,
            &[studies[1].clone(), studies[0].clone()],
        )
        .unwrap();
        assert_eq!(result, replay);
    }

    #[test]
    fn opposed_confident_intervals_remain_a_discordant_result() {
        let request = request();
        let studies = vec![
            estimate("study-a", "group-a", 250, 20),
            estimate("study-b", "group-b", -250, 20),
        ];
        let result = analyze_glioma_multistudy_concordance(&request, &studies).unwrap();
        assert_eq!(
            result.disposition,
            MultiStudyConcordanceDisposition::Discordant
        );
        assert_eq!(result.supporting_order, vec!["study-a"]);
        assert_eq!(result.refuting_order, vec!["study-b"]);
        assert_eq!(result.discordant_pair_order.len(), 1);
    }

    #[test]
    fn cross_model_pairs_are_retained_as_incomparable() {
        let request = request();
        let studies = vec![
            estimate("study-a", "group-a", 250, 20),
            study(
                "study-b",
                "group-b",
                GliomaModelSystem::CellLine,
                MultiStudyEffectStatus::Estimate,
                Some(260),
                Some(20),
                950,
            ),
        ];
        let result = analyze_glioma_multistudy_concordance(&request, &studies).unwrap();
        assert_eq!(
            result.disposition,
            MultiStudyConcordanceDisposition::CrossModelOnly
        );
        assert_eq!(result.cross_model_pair_order.len(), 1);
        assert!(result.same_model_pair_order.is_empty());
        assert!(result.uncertainty[0].contains("cross_model"));
    }

    #[test]
    fn low_quality_null_and_unresolved_rows_are_explicit_but_not_promoted() {
        let mut request = request();
        request.min_studies = 3;
        let studies = vec![
            estimate("study-a", "group-a", 250, 20),
            study(
                "study-b",
                "group-b",
                GliomaModelSystem::Organoid,
                MultiStudyEffectStatus::Estimate,
                Some(300),
                Some(20),
                200,
            ),
            study(
                "study-c",
                "group-c",
                GliomaModelSystem::Organoid,
                MultiStudyEffectStatus::Null,
                Some(0),
                Some(40),
                950,
            ),
            study(
                "study-d",
                "group-d",
                GliomaModelSystem::Organoid,
                MultiStudyEffectStatus::Unresolved,
                None,
                None,
                950,
            ),
        ];
        let result = analyze_glioma_multistudy_concordance(&request, &studies).unwrap();
        assert_eq!(
            result.disposition,
            MultiStudyConcordanceDisposition::InsufficientEvidence
        );
        assert_eq!(result.low_quality_order, vec!["study-b"]);
        assert_eq!(result.null_order, vec!["study-c"]);
        assert_eq!(result.unresolved_order, vec!["study-d"]);
        assert_eq!(result.independent_group_count, 2);
        assert_eq!(result.supporting_order, vec!["study-a"]);
        result.validate_against(&request, &studies).unwrap();
    }

    #[test]
    fn a_null_directional_result_cannot_claim_a_confident_non_null_interval() {
        let request = request();
        let studies = vec![
            estimate("study-a", "group-a", 250, 20),
            study(
                "study-b",
                "group-b",
                GliomaModelSystem::Organoid,
                MultiStudyEffectStatus::Null,
                Some(400),
                Some(20),
                950,
            ),
        ];
        assert!(matches!(
            analyze_glioma_multistudy_concordance(&request, &studies),
            Err(MultiStudyConcordanceError::InvalidStudy(_))
        ));
    }

    #[test]
    fn duplicate_independence_groups_and_tampered_pair_ledgers_are_rejected() {
        let request = request();
        let mut studies = vec![
            estimate("study-a", "same-group", 250, 20),
            estimate("study-b", "same-group", 300, 20),
        ];
        assert!(matches!(
            analyze_glioma_multistudy_concordance(&request, &studies),
            Err(MultiStudyConcordanceError::InvalidStudy(_))
        ));
        studies[1].independence_group = "group-b".into();
        let mut result = analyze_glioma_multistudy_concordance(&request, &studies).unwrap();
        result.pairwise_comparisons.clear();
        assert!(result.validate().is_err());
        assert!(result.validate_against(&request, &studies).is_err());
    }

    #[test]
    fn restamped_output_cannot_weaken_local_artifact_privacy_flags() {
        let request = request();
        let studies = vec![
            estimate("study-a", "group-a", 250, 20),
            estimate("study-b", "group-b", 300, 20),
        ];
        let mut result = analyze_glioma_multistudy_concordance(&request, &studies).unwrap();
        result.study_summaries[0]
            .artifact
            .contains_direct_identifiers = true;
        result.digest = output_digest(&result).unwrap();
        assert!(matches!(
            result.validate(),
            Err(MultiStudyConcordanceError::InvalidOutput(_))
        ));
    }
}

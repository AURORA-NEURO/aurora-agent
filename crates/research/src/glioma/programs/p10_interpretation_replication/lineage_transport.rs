//! Compare lineage-resolved glioma perturbation contrasts across preclinical model systems.
//!
//! Each input is a validated P10-F02 study analysis. Biological units remain nested within
//! studies; studies are equally weighted within model-system strata; represented systems are
//! equally weighted. The included systems are fixed, not a random sample of all possible models.
//! Implements `GAF-GLIOMA-P10-F03`.

use crate::glioma::programs::p10_interpretation_replication::lineage_propagation::{
    EffectDisposition, LineagePropagationAnalysis, LineagePropagationDisposition, PpmInterval,
};
use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P10-F03";
pub const OUTPUT_SCHEMA: &str = "GliomaLineageTransportAudit1@1";
const MAX_STUDIES: usize = 64;
const MAX_SYSTEMS: usize = 8;
const MIN_BOOTSTRAP: usize = 99;
const MAX_BOOTSTRAP: usize = 999;
const MAX_WORK: u128 = 50_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineageTransportRequest {
    pub objective: String,
    pub minimum_model_systems: usize,
    pub minimum_studies_per_model_system: usize,
    pub minimum_effect_ppm: u64,
    pub maximum_model_system_range_ppm: u64,
    pub bootstrap_replicates: usize,
    pub bootstrap_seed: ContentHash,
    pub confidence_level_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineageTransportStudy {
    pub study_id: String,
    pub analysis: LineagePropagationAnalysis,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineageTransportStudyExclusionReason {
    PredictionFailure,
    PartiallyIdentified,
    UnresolvedAnalysis,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineageTransportStudyExclusion {
    pub study_id: String,
    pub model_system: String,
    pub reason: LineageTransportStudyExclusionReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineageTransportFollowUpKind {
    AdditionalIndependentStudy,
    ReplicateDiscordantSystem,
    NoTargetedFollowUp,
    ResolveUnmeasuredContrast,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineageTransportSystemContrast {
    pub model_system: String,
    pub source_study_count: u32,
    pub contributing_study_ids: Vec<String>,
    pub omitted_study_ids: Vec<String>,
    pub mean_effect_ppm: Option<i64>,
    pub follow_up: LineageTransportFollowUpKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineageTransportContrastDisposition {
    Qualified,
    ModelDependent,
    Negative,
    Partial,
    Inconclusive,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineageTransportContrast {
    pub from_state: String,
    pub to_state: String,
    pub pooled_effect_ppm: Option<i64>,
    pub pooled_interval_ppm: Option<PpmInterval>,
    pub model_system_range_ppm: Option<u64>,
    pub maximum_leave_one_system_out_shift_ppm: Option<u64>,
    pub material_direction_reversal_count: u32,
    pub systems: Vec<LineageTransportSystemContrast>,
    pub disposition: LineageTransportContrastDisposition,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineageTransportDisposition {
    Qualified,
    ModelDependent,
    Negative,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineageTransportAnalysis {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub state_order: Vec<String>,
    pub control_arm: String,
    pub treatment_arm: String,
    pub interval_days: u32,
    pub study_order: Vec<String>,
    pub included_study_order: Vec<String>,
    pub excluded_studies: Vec<LineageTransportStudyExclusion>,
    pub model_system_order: Vec<String>,
    pub minimum_model_systems: usize,
    pub minimum_studies_per_model_system: usize,
    pub minimum_effect_ppm: u64,
    pub maximum_model_system_range_ppm: u64,
    pub bootstrap_replicates: usize,
    pub confidence_level_milli: u16,
    pub contrasts: Vec<LineageTransportContrast>,
    pub disposition: LineageTransportDisposition,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub limitations: Vec<String>,
    pub input_digest: ContentHash,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LineageTransportError {
    #[error("lineage transport request is invalid: {0}")]
    InvalidRequest(String),
    #[error("lineage transport study is invalid: {0}")]
    InvalidStudy(String),
    #[error("lineage transport output is invalid: {0}")]
    InvalidOutput(String),
    #[error("lineage transport digest failed: {0}")]
    Digest(String),
}

#[derive(Debug)]
struct StudyEffect<'a> {
    study: &'a LineageTransportStudy,
    point_ppm: i64,
    bootstrap_ppm: Vec<i64>,
}

#[derive(Debug, Default)]
struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        value ^ (value >> 31)
    }

    fn index(&mut self, bound: usize) -> usize {
        let bound = bound as u64;
        let ceiling = u64::MAX - u64::MAX % bound;
        loop {
            let value = self.next();
            if value < ceiling {
                return (value % bound) as usize;
            }
        }
    }
}

fn model_system_name(model: GliomaModelSystem) -> &'static str {
    match model {
        GliomaModelSystem::CellLine => "cell_line",
        GliomaModelSystem::Organoid => "organoid",
        GliomaModelSystem::PatientDerivedXenograft => "patient_derived_xenograft",
        GliomaModelSystem::MouseModel => "mouse_model",
        GliomaModelSystem::ZebrafishModel => "zebrafish_model",
        GliomaModelSystem::InSilico => "in_silico",
    }
}

fn round_mean(values: impl Iterator<Item = i64>) -> Option<i64> {
    let values = values.collect::<Vec<_>>();
    if values.is_empty() {
        return None;
    }
    let sum = values.iter().map(|value| i128::from(*value)).sum::<i128>();
    let count = values.len() as i128;
    let adjustment = if sum < 0 { -(count / 2) } else { count / 2 };
    Some(((sum + adjustment) / count) as i64)
}

fn validate_request(request: &LineageTransportRequest) -> Result<(), LineageTransportError> {
    if request.objective.trim().is_empty()
        || !(2..=MAX_SYSTEMS).contains(&request.minimum_model_systems)
        || !(1..=MAX_STUDIES).contains(&request.minimum_studies_per_model_system)
        || request.minimum_effect_ppm == 0
        || request.maximum_model_system_range_ppm == 0
        || !(MIN_BOOTSTRAP..=MAX_BOOTSTRAP).contains(&request.bootstrap_replicates)
        || !(800..=999).contains(&request.confidence_level_milli)
    {
        return Err(LineageTransportError::InvalidRequest(
            "objective, system/study floors, effect/range margins, bounded bootstrap, or confidence level is invalid".into(),
        ));
    }
    Ok(())
}

fn validate_studies<'a>(
    request: &LineageTransportRequest,
    studies: &'a [LineageTransportStudy],
) -> Result<(Vec<&'a LineageTransportStudy>, Vec<String>), LineageTransportError> {
    if !(2..=MAX_STUDIES).contains(&studies.len()) {
        return Err(LineageTransportError::InvalidStudy(format!(
            "study set must contain 2..={MAX_STUDIES} studies"
        )));
    }
    let mut ordered = studies.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| left.study_id.cmp(&right.study_id));
    if ordered.iter().any(|study| study.study_id.trim().is_empty())
        || ordered
            .windows(2)
            .any(|pair| pair[0].study_id == pair[1].study_id)
    {
        return Err(LineageTransportError::InvalidStudy(
            "study IDs must be non-empty and unique".into(),
        ));
    }

    let first = ordered[0];
    let mut systems = BTreeSet::new();
    for study in &ordered {
        study
            .analysis
            .validate()
            .map_err(|error| LineageTransportError::InvalidStudy(error.to_string()))?;
        study
            .artifact
            .validate()
            .map_err(|error| LineageTransportError::InvalidStudy(error.to_string()))?;
        if !study.artifact.local_only
            || study.artifact.contains_human_data
            || study.artifact.contains_direct_identifiers
            || study.analysis.objective != request.objective
            || study.analysis.state_order != first.analysis.state_order
            || study.analysis.control_arm != first.analysis.control_arm
            || study.analysis.treatment_arm != first.analysis.treatment_arm
            || study.analysis.interval_days != first.analysis.interval_days
        {
            return Err(LineageTransportError::InvalidStudy(format!(
                "study {} differs in objective, state order, arms, interval, or local de-identified artifact boundary",
                study.study_id
            )));
        }
        systems.insert(model_system_name(study.analysis.model_system));
    }
    if systems.len() > MAX_SYSTEMS {
        return Err(LineageTransportError::InvalidStudy(format!(
            "model-system count exceeds {MAX_SYSTEMS}"
        )));
    }
    Ok((ordered, systems.into_iter().map(str::to_string).collect()))
}

fn contrast_draws(
    analysis: &LineagePropagationAnalysis,
    from: &str,
    to: &str,
) -> Option<(i64, Vec<i64>)> {
    let contrast = analysis
        .contrasts
        .iter()
        .find(|contrast| contrast.from_state == from && contrast.to_state == to)?;
    if contrast.disposition == EffectDisposition::Unresolved {
        return None;
    }
    let states = analysis.state_order.len();
    let from_index = analysis
        .state_order
        .iter()
        .position(|state| state == from)?;
    let to_index = analysis.state_order.iter().position(|state| state == to)?;
    let matrix_index = to_index * states + from_index;
    let draws = analysis
        .bootstrap_draws
        .iter()
        .map(|draw| {
            draw.treatment_coefficients_ppm[matrix_index] as i64
                - draw.control_coefficients_ppm[matrix_index] as i64
        })
        .collect();
    Some((contrast.treatment_minus_control_ppm, draws))
}

fn percentile_interval(values: &mut [i64], confidence_level_milli: u16) -> Option<PpmInterval> {
    if values.is_empty() {
        return None;
    }
    values.sort_unstable();
    let span = values.len() - 1;
    let tail_milli = (1_000 - u64::from(confidence_level_milli)) / 2;
    let lower_index = (span as u128 * u128::from(tail_milli) / 1_000) as usize;
    let upper_index = (span as u128 * u128::from(1_000 - tail_milli) / 1_000) as usize;
    Some(PpmInterval {
        lower: values[lower_index],
        upper: values[upper_index],
    })
}

fn hierarchical_bootstrap(
    groups: &[Vec<StudyEffect<'_>>],
    replicates: usize,
    rng: &mut SplitMix64,
) -> Vec<i64> {
    let mut pooled = Vec::with_capacity(replicates);
    for _ in 0..replicates {
        let mut system_means = Vec::new();
        for group in groups.iter().filter(|group| !group.is_empty()) {
            let mut study_values = Vec::with_capacity(group.len());
            for _ in 0..group.len() {
                let study = &group[rng.index(group.len())];
                study_values.push(study.bootstrap_ppm[rng.index(study.bootstrap_ppm.len())]);
            }
            if let Some(mean) = round_mean(study_values.into_iter()) {
                system_means.push(mean);
            }
        }
        if let Some(mean) = round_mean(system_means.into_iter()) {
            pooled.push(mean);
        }
    }
    pooled
}

fn follow_up_kind(
    system_effect: Option<i64>,
    pooled_effect: Option<i64>,
    study_count: usize,
    request: &LineageTransportRequest,
    system_range: Option<u64>,
) -> LineageTransportFollowUpKind {
    let Some(effect) = system_effect else {
        return LineageTransportFollowUpKind::ResolveUnmeasuredContrast;
    };
    if study_count < request.minimum_studies_per_model_system {
        return LineageTransportFollowUpKind::AdditionalIndependentStudy;
    }
    if let Some(pooled) = pooled_effect {
        if effect.unsigned_abs() >= request.minimum_effect_ppm
            && pooled.unsigned_abs() >= request.minimum_effect_ppm
            && effect.signum() != pooled.signum()
        {
            return LineageTransportFollowUpKind::ReplicateDiscordantSystem;
        }
    }
    if system_range.is_some_and(|range| range > request.maximum_model_system_range_ppm) {
        LineageTransportFollowUpKind::ReplicateDiscordantSystem
    } else {
        LineageTransportFollowUpKind::NoTargetedFollowUp
    }
}

fn summarize_contrast(
    request: &LineageTransportRequest,
    ordered_studies: &[&LineageTransportStudy],
    eligible_studies: &[&LineageTransportStudy],
    model_system_order: &[String],
    from_state: &str,
    to_state: &str,
    rng: &mut SplitMix64,
) -> LineageTransportContrast {
    let mut groups = Vec::<Vec<StudyEffect<'_>>>::new();
    let mut systems = Vec::with_capacity(model_system_order.len());
    for model_system in model_system_order {
        let source_studies = ordered_studies
            .iter()
            .filter(|study| model_system_name(study.analysis.model_system) == model_system)
            .count();
        let mut group = Vec::new();
        let mut omitted_study_ids = Vec::new();
        for study in eligible_studies
            .iter()
            .filter(|study| model_system_name(study.analysis.model_system) == model_system)
        {
            match contrast_draws(&study.analysis, from_state, to_state) {
                Some((point_ppm, bootstrap_ppm)) => group.push(StudyEffect {
                    study,
                    point_ppm,
                    bootstrap_ppm,
                }),
                None => omitted_study_ids.push(study.study_id.clone()),
            }
        }
        let contributing_study_ids = group
            .iter()
            .map(|effect| effect.study.study_id.clone())
            .collect::<Vec<_>>();
        let mean_effect_ppm = round_mean(group.iter().map(|effect| effect.point_ppm));
        systems.push(LineageTransportSystemContrast {
            model_system: model_system.clone(),
            source_study_count: source_studies as u32,
            contributing_study_ids,
            omitted_study_ids,
            mean_effect_ppm,
            follow_up: LineageTransportFollowUpKind::ResolveUnmeasuredContrast,
        });
        groups.push(group);
    }
    let system_effects = systems
        .iter()
        .filter_map(|system| system.mean_effect_ppm)
        .collect::<Vec<_>>();
    let pooled_effect_ppm = round_mean(system_effects.iter().copied());
    let system_range_ppm = if system_effects.len() >= 2 {
        Some(
            system_effects
                .iter()
                .min()
                .expect("two effects")
                .abs_diff(*system_effects.iter().max().expect("two effects")),
        )
    } else {
        None
    };
    let maximum_leave_one_system_out_shift_ppm = pooled_effect_ppm.and_then(|pooled| {
        (system_effects.len() >= 2).then(|| {
            system_effects
                .iter()
                .enumerate()
                .filter_map(|(omitted, _)| {
                    round_mean(
                        system_effects
                            .iter()
                            .enumerate()
                            .filter_map(|(index, effect)| (index != omitted).then_some(*effect)),
                    )
                    .map(|mean| pooled.abs_diff(mean))
                })
                .max()
                .unwrap_or(0)
        })
    });
    let material_direction_reversal_count = pooled_effect_ppm.map_or(0, |pooled| {
        if pooled.unsigned_abs() < request.minimum_effect_ppm {
            0
        } else {
            systems
                .iter()
                .filter_map(|system| system.mean_effect_ppm)
                .filter(|effect| {
                    effect.unsigned_abs() >= request.minimum_effect_ppm
                        && effect.signum() != pooled.signum()
                })
                .count() as u32
        }
    });
    let mut pooled_draws = hierarchical_bootstrap(&groups, request.bootstrap_replicates, rng);
    let pooled_interval_ppm =
        percentile_interval(&mut pooled_draws, request.confidence_level_milli);
    let represented_systems = systems
        .iter()
        .filter(|system| system.mean_effect_ppm.is_some())
        .count();
    let system_floors_met = systems.iter().all(|system| {
        system.mean_effect_ppm.is_some()
            && system.contributing_study_ids.len() >= request.minimum_studies_per_model_system
    }) && represented_systems >= request.minimum_model_systems;
    let disposition = if represented_systems < 2 {
        LineageTransportContrastDisposition::Unresolved
    } else if material_direction_reversal_count > 0
        || system_range_ppm.is_some_and(|range| range > request.maximum_model_system_range_ppm)
    {
        LineageTransportContrastDisposition::ModelDependent
    } else if !system_floors_met {
        LineageTransportContrastDisposition::Partial
    } else if pooled_interval_ppm.is_some_and(|interval| {
        interval.lower >= -(request.minimum_effect_ppm as i64)
            && interval.upper <= request.minimum_effect_ppm as i64
    }) {
        LineageTransportContrastDisposition::Negative
    } else if pooled_interval_ppm.is_some_and(|interval| {
        interval.lower > request.minimum_effect_ppm as i64
            || interval.upper < -(request.minimum_effect_ppm as i64)
    }) {
        LineageTransportContrastDisposition::Qualified
    } else {
        LineageTransportContrastDisposition::Inconclusive
    };
    for system in &mut systems {
        system.follow_up = follow_up_kind(
            system.mean_effect_ppm,
            pooled_effect_ppm,
            system.contributing_study_ids.len(),
            request,
            system_range_ppm,
        );
    }
    let rationale = match disposition {
        LineageTransportContrastDisposition::Qualified => "declared study/system floors were met, the pooled interval exceeded the practical-effect margin, and observed systems met the transport-gap threshold",
        LineageTransportContrastDisposition::ModelDependent => "a material system direction reversal or between-system effect range prevents transport qualification",
        LineageTransportContrastDisposition::Negative => "the pooled uncertainty interval lies within the prespecified practical-effect margin",
        LineageTransportContrastDisposition::Partial => "one or more represented model systems lack the prespecified number of independent studies or an estimable contrast",
        LineageTransportContrastDisposition::Inconclusive => "the pooled uncertainty interval overlaps the practical-effect boundary",
        LineageTransportContrastDisposition::Unresolved => "fewer than two model systems contribute an estimable contrast",
    };
    LineageTransportContrast {
        from_state: from_state.into(),
        to_state: to_state.into(),
        pooled_effect_ppm,
        pooled_interval_ppm,
        model_system_range_ppm: system_range_ppm,
        maximum_leave_one_system_out_shift_ppm,
        material_direction_reversal_count,
        systems,
        disposition,
        rationale: rationale.into(),
    }
}

fn root_disposition(
    contrasts: &[LineageTransportContrast],
    system_count: usize,
) -> LineageTransportDisposition {
    if system_count < 2
        || contrasts
            .iter()
            .all(|contrast| contrast.disposition == LineageTransportContrastDisposition::Unresolved)
    {
        LineageTransportDisposition::Unresolved
    } else if contrasts
        .iter()
        .any(|contrast| contrast.disposition == LineageTransportContrastDisposition::ModelDependent)
    {
        LineageTransportDisposition::ModelDependent
    } else if contrasts
        .iter()
        .all(|contrast| contrast.disposition == LineageTransportContrastDisposition::Negative)
    {
        LineageTransportDisposition::Negative
    } else if contrasts
        .iter()
        .all(|contrast| contrast.disposition == LineageTransportContrastDisposition::Qualified)
    {
        LineageTransportDisposition::Qualified
    } else {
        LineageTransportDisposition::Partial
    }
}

fn digest_for(output: &LineageTransportAnalysis) -> Result<ContentHash, LineageTransportError> {
    let mut input = output.clone();
    input.digest = ContentHash::of_bytes(b"glioma-lineage-transport-output-digest-placeholder");
    let value = serde_json::to_value(input)
        .map_err(|error| LineageTransportError::Digest(error.to_string()))?;
    ContentHash::of_value(&value).map_err(|error| LineageTransportError::Digest(error.to_string()))
}

impl LineageTransportAnalysis {
    pub fn validate(&self) -> Result<(), LineageTransportError> {
        let mut expected_pairs = self
            .state_order
            .iter()
            .flat_map(|from| self.state_order.iter().map(move |to| (from, to)))
            .collect::<Vec<_>>();
        expected_pairs.sort();
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || !(2..=8).contains(&self.state_order.len())
            || self.control_arm.trim().is_empty()
            || self.treatment_arm.trim().is_empty()
            || self.control_arm == self.treatment_arm
            || self.interval_days == 0
            || self.study_order.len() < 2
            || self.study_order.windows(2).any(|pair| pair[0] >= pair[1])
            || self
                .included_study_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self
                .model_system_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self.model_system_order.len() > MAX_SYSTEMS
            || !(2..=MAX_SYSTEMS).contains(&self.minimum_model_systems)
            || self.minimum_studies_per_model_system == 0
            || self.minimum_effect_ppm == 0
            || self.maximum_model_system_range_ppm == 0
            || !(MIN_BOOTSTRAP..=MAX_BOOTSTRAP).contains(&self.bootstrap_replicates)
            || !(800..=999).contains(&self.confidence_level_milli)
            || self.contrasts.len() != expected_pairs.len()
            || self
                .contrasts
                .iter()
                .map(|contrast| (&contrast.from_state, &contrast.to_state))
                .ne(expected_pairs.iter().copied())
            || self
                .excluded_studies
                .windows(2)
                .any(|pair| pair[0].study_id >= pair[1].study_id)
        {
            return Err(LineageTransportError::InvalidOutput(
                "feature/schema, analysis contract, stable order, or bounded dimensions are invalid".into(),
            ));
        }
        let partition = self
            .included_study_order
            .iter()
            .cloned()
            .chain(
                self.excluded_studies
                    .iter()
                    .map(|study| study.study_id.clone()),
            )
            .collect::<BTreeSet<_>>();
        if partition.len() != self.study_order.len()
            || partition.iter().ne(self.study_order.iter())
            || self
                .contrasts
                .iter()
                .any(|contrast| contrast.systems.len() != self.model_system_order.len())
        {
            return Err(LineageTransportError::InvalidOutput(
                "included/excluded studies or per-contrast model systems do not close".into(),
            ));
        }
        for contrast in &self.contrasts {
            if contrast
                .systems
                .iter()
                .map(|system| system.model_system.as_str())
                .ne(self.model_system_order.iter().map(String::as_str))
                || contrast.systems.iter().any(|system| {
                    system
                        .contributing_study_ids
                        .windows(2)
                        .any(|pair| pair[0] >= pair[1])
                        || system
                            .omitted_study_ids
                            .windows(2)
                            .any(|pair| pair[0] >= pair[1])
                        || (system.mean_effect_ppm.is_some()
                            != !system.contributing_study_ids.is_empty())
                        || system
                            .contributing_study_ids
                            .iter()
                            .any(|study| !self.included_study_order.contains(study))
                        || system
                            .omitted_study_ids
                            .iter()
                            .any(|study| !self.included_study_order.contains(study))
                })
                || (contrast.pooled_effect_ppm.is_some() != contrast.pooled_interval_ppm.is_some())
                || (contrast.model_system_range_ppm.is_some()
                    != (contrast
                        .systems
                        .iter()
                        .filter(|system| system.mean_effect_ppm.is_some())
                        .count()
                        >= 2))
            {
                return Err(LineageTransportError::InvalidOutput(
                    "contrast/system ordering or explicit estimate presence is invalid".into(),
                ));
            }
        }
        if root_disposition(&self.contrasts, self.model_system_order.len()) != self.disposition {
            return Err(LineageTransportError::InvalidOutput(
                "overall disposition disagrees with contrast evidence".into(),
            ));
        }
        if digest_for(self)? != self.digest {
            return Err(LineageTransportError::InvalidOutput(
                "lineage transport digest does not match output".into(),
            ));
        }
        Ok(())
    }
}

/// Audit whether harmonized lineage-propagation contrasts persist across observed preclinical systems.
pub fn analyze_glioma_lineage_transport(
    request: &LineageTransportRequest,
    studies: &[LineageTransportStudy],
) -> Result<LineageTransportAnalysis, LineageTransportError> {
    validate_request(request)?;
    let (ordered, model_system_order) = validate_studies(request, studies)?;
    let state_order = ordered[0].analysis.state_order.clone();
    let work = (request.bootstrap_replicates as u128)
        .saturating_mul(ordered.len() as u128)
        .saturating_mul(state_order.len().saturating_pow(2) as u128);
    if work > MAX_WORK {
        return Err(LineageTransportError::InvalidRequest(format!(
            "nested bootstrap requires {work} bounded study-contrast draws, above {MAX_WORK}"
        )));
    }

    let mut included_study_order = Vec::new();
    let mut excluded_studies = Vec::new();
    for study in &ordered {
        let reason = match study.analysis.disposition {
            LineagePropagationDisposition::Qualified => None,
            LineagePropagationDisposition::PartiallyIdentified => {
                Some(LineageTransportStudyExclusionReason::PartiallyIdentified)
            }
            LineagePropagationDisposition::PredictionFailure => {
                Some(LineageTransportStudyExclusionReason::PredictionFailure)
            }
            LineagePropagationDisposition::Unresolved => {
                Some(LineageTransportStudyExclusionReason::UnresolvedAnalysis)
            }
        };
        if let Some(reason) = reason {
            excluded_studies.push(LineageTransportStudyExclusion {
                study_id: study.study_id.clone(),
                model_system: model_system_name(study.analysis.model_system).into(),
                reason,
            });
        } else {
            included_study_order.push(study.study_id.clone());
        }
    }
    let eligible_studies = ordered
        .iter()
        .copied()
        .filter(|study| study.analysis.disposition == LineagePropagationDisposition::Qualified)
        .collect::<Vec<_>>();
    let input_value = serde_json::to_value((
        request,
        ordered
            .iter()
            .map(|study| (&study.study_id, &study.analysis.digest, &study.artifact))
            .collect::<Vec<_>>(),
    ))
    .map_err(|error| LineageTransportError::Digest(error.to_string()))?;
    let input_digest = ContentHash::of_value(&input_value)
        .map_err(|error| LineageTransportError::Digest(error.to_string()))?;
    let seed_value = serde_json::to_value((request.bootstrap_seed.as_str(), &input_digest))
        .map_err(|error| LineageTransportError::Digest(error.to_string()))?;
    let seed_hash = ContentHash::of_value(&seed_value)
        .map_err(|error| LineageTransportError::Digest(error.to_string()))?;
    let seed = u64::from_str_radix(&seed_hash.as_str()[..16], 16)
        .map_err(|error| LineageTransportError::Digest(error.to_string()))?;
    let mut rng = SplitMix64(seed);
    let mut contrasts = Vec::with_capacity(state_order.len().saturating_pow(2));
    for from in &state_order {
        for to in &state_order {
            contrasts.push(summarize_contrast(
                request,
                &ordered,
                &eligible_studies,
                &model_system_order,
                from,
                to,
                &mut rng,
            ));
        }
    }
    contrasts.sort_by(|left, right| {
        (&left.from_state, &left.to_state).cmp(&(&right.from_state, &right.to_state))
    });
    if !excluded_studies.is_empty() {
        for contrast in &mut contrasts {
            if matches!(
                contrast.disposition,
                LineageTransportContrastDisposition::Qualified
                    | LineageTransportContrastDisposition::Negative
            ) {
                contrast.disposition = LineageTransportContrastDisposition::Partial;
                contrast.rationale.push_str(
                    "; at least one source analysis was excluded, so the cross-system evidence set is partial",
                );
            }
        }
    }
    let disposition = root_disposition(&contrasts, model_system_order.len());
    let negative_evidence = contrasts
        .iter()
        .filter(|contrast| {
            matches!(
                contrast.disposition,
                LineageTransportContrastDisposition::Negative
                    | LineageTransportContrastDisposition::ModelDependent
            )
        })
        .map(|contrast| {
            format!(
                "{}→{}: {}",
                contrast.from_state, contrast.to_state, contrast.rationale
            )
        })
        .collect::<Vec<_>>();
    let uncertainty = contrasts
        .iter()
        .filter(|contrast| {
            matches!(
                contrast.disposition,
                LineageTransportContrastDisposition::Partial
                    | LineageTransportContrastDisposition::Inconclusive
                    | LineageTransportContrastDisposition::Unresolved
            )
        })
        .map(|contrast| {
            format!(
                "{}→{}: {}",
                contrast.from_state, contrast.to_state, contrast.rationale
            )
        })
        .chain(excluded_studies.iter().map(|study| {
            format!(
                "study {} from {} omitted because its P10-F02 result was {:?}",
                study.study_id, study.model_system, study.reason
            )
        }))
        .collect::<Vec<_>>();
    let mut output = LineageTransportAnalysis {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        state_order,
        control_arm: ordered[0].analysis.control_arm.clone(),
        treatment_arm: ordered[0].analysis.treatment_arm.clone(),
        interval_days: ordered[0].analysis.interval_days,
        study_order: ordered.iter().map(|study| study.study_id.clone()).collect(),
        included_study_order,
        excluded_studies,
        model_system_order,
        minimum_model_systems: request.minimum_model_systems,
        minimum_studies_per_model_system: request.minimum_studies_per_model_system,
        minimum_effect_ppm: request.minimum_effect_ppm,
        maximum_model_system_range_ppm: request.maximum_model_system_range_ppm,
        bootstrap_replicates: request.bootstrap_replicates,
        confidence_level_milli: request.confidence_level_milli,
        contrasts,
        disposition,
        negative_evidence,
        uncertainty,
        limitations: vec![
            "Model systems are fixed strata; results do not imply transport to unobserved systems.".into(),
            "Treatment-minus-control contrasts do not establish causal identification.".into(),
            "Only local, de-identified preclinical artifacts are accepted.".into(),
            "The bounded hierarchical bootstrap is empirical uncertainty, not a Bayesian posterior or a population-of-models estimator.".into(),
        ],
        input_digest,
        digest: ContentHash::of_bytes(b"glioma-lineage-transport-output-digest-placeholder"),
    };
    output.digest = digest_for(&output)?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p10_interpretation_replication::lineage_propagation::{
        analyze_glioma_lineage_propagation, LineagePropagationRequest, LineagePropagationSnapshot,
    };

    fn artifact(label: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: format!("local-{label}"),
            content_hash: ContentHash::of_bytes(label.as_bytes()),
            content_type: "application/glioma-lineage-study+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn study(
        study_id: &str,
        model_system: GliomaModelSystem,
        treatment_matrix: [[u32; 2]; 2],
    ) -> LineageTransportStudy {
        let objective = "compare preclinical glioma lineage propagation";
        let mut request = LineagePropagationRequest {
            objective: objective.into(),
            model_system,
            control_arm: "control".into(),
            treatment_arm: "perturbation".into(),
            state_order: vec!["npc_like".into(), "mes_like".into()],
            min_units_per_arm: 2,
            min_lineages_per_arm: 4,
            ridge_penalty_ppm: 1,
            max_coefficient_ppm: 5_000_000,
            max_prediction_error_ppm: 300_000,
            bootstrap_replicates: 99,
            bootstrap_seed: ContentHash::of_bytes(study_id.as_bytes()),
            confidence_level_milli: 900,
            minimum_effect_ppm: 25_000,
        };
        let control_matrix = [[8, 2], [1, 7]];
        let mut snapshots = Vec::new();
        for arm in ["control", "perturbation"] {
            for unit_index in 0..3 {
                let matrix = if arm == "control" {
                    control_matrix
                } else {
                    treatment_matrix
                };
                for source in 0..2 {
                    let unit = format!("{study_id}-{arm}-u{unit_index}");
                    let lineage = format!("{unit}-l{source}");
                    let mut counts = vec![0_u32; 2];
                    counts[source] = 100;
                    for (step, day) in [0_u32, 7, 14].into_iter().enumerate() {
                        snapshots.push(LineagePropagationSnapshot {
                            observation_id: format!("{lineage}-t{day}"),
                            experimental_unit_id: unit.clone(),
                            lineage_id: lineage.clone(),
                            arm_id: arm.into(),
                            model_system,
                            assay_batch_id: format!("{study_id}-batch-{unit_index}"),
                            timepoint_day: day,
                            state_counts: counts.clone(),
                            capture_fraction_ppm: 1_000_000,
                            artifact: artifact(&format!("{study_id}-{lineage}-t{day}")),
                        });
                        if step < 2 {
                            counts = (0..2)
                                .map(|to| {
                                    (0..2)
                                        .map(|from| matrix[to][from] * counts[from])
                                        .sum::<u32>()
                                        / 10
                                })
                                .collect();
                        }
                    }
                }
            }
        }
        request.model_system = model_system;
        let analysis = analyze_glioma_lineage_propagation(&request, &snapshots).unwrap();
        assert_eq!(
            analysis.disposition,
            LineagePropagationDisposition::Qualified
        );
        LineageTransportStudy {
            study_id: study_id.into(),
            analysis,
            artifact: artifact(study_id),
        }
    }

    fn request() -> LineageTransportRequest {
        LineageTransportRequest {
            objective: "compare preclinical glioma lineage propagation".into(),
            minimum_model_systems: 3,
            minimum_studies_per_model_system: 2,
            minimum_effect_ppm: 25_000,
            maximum_model_system_range_ppm: 100_000,
            bootstrap_replicates: 99,
            bootstrap_seed: ContentHash::of_bytes(b"lineage-transport-seed"),
            confidence_level_milli: 900,
        }
    }

    fn concordant_studies() -> Vec<LineageTransportStudy> {
        let matrix = [[6, 4], [3, 5]];
        [
            ("cell-a", GliomaModelSystem::CellLine),
            ("cell-b", GliomaModelSystem::CellLine),
            ("organoid-a", GliomaModelSystem::Organoid),
            ("organoid-b", GliomaModelSystem::Organoid),
            ("xeno-a", GliomaModelSystem::PatientDerivedXenograft),
            ("xeno-b", GliomaModelSystem::PatientDerivedXenograft),
        ]
        .into_iter()
        .map(|(id, system)| study(id, system, matrix))
        .collect()
    }

    #[test]
    fn concordant_study_contrasts_qualify_without_cell_or_system_pseudoreplication() {
        let result = analyze_glioma_lineage_transport(&request(), &concordant_studies()).unwrap();
        assert_eq!(result.disposition, LineageTransportDisposition::Qualified);
        assert_eq!(result.included_study_order.len(), 6);
        assert_eq!(result.model_system_order.len(), 3);
        assert!(result.contrasts.iter().all(|contrast| {
            contrast.disposition == LineageTransportContrastDisposition::Qualified
                && contrast.systems.iter().all(|system| {
                    system.source_study_count == 2
                        && system.contributing_study_ids.len() == 2
                        && system.follow_up == LineageTransportFollowUpKind::NoTargetedFollowUp
                })
        }));
        result.validate().unwrap();
    }

    #[test]
    fn reversed_model_system_is_retained_as_model_dependent_not_averaged_away() {
        let mut studies = concordant_studies();
        let reversal = [[8, 2], [0, 8]];
        studies[4] = study(
            "xeno-a",
            GliomaModelSystem::PatientDerivedXenograft,
            reversal,
        );
        studies[5] = study(
            "xeno-b",
            GliomaModelSystem::PatientDerivedXenograft,
            reversal,
        );
        let result = analyze_glioma_lineage_transport(&request(), &studies).unwrap();
        let npc_to_mes = result
            .contrasts
            .iter()
            .find(|contrast| contrast.from_state == "npc_like" && contrast.to_state == "mes_like")
            .unwrap();
        assert_eq!(
            npc_to_mes.disposition,
            LineageTransportContrastDisposition::ModelDependent
        );
        assert_eq!(npc_to_mes.material_direction_reversal_count, 1);
        assert!(npc_to_mes.systems.iter().any(|system| {
            system.model_system == "patient_derived_xenograft"
                && system.follow_up == LineageTransportFollowUpKind::ReplicateDiscordantSystem
        }));
        assert_eq!(
            result.disposition,
            LineageTransportDisposition::ModelDependent
        );
    }

    #[test]
    fn repeated_studies_in_one_model_system_cannot_fill_other_system_floors() {
        let matrix = [[6, 4], [3, 5]];
        let studies = vec![
            study("cell-a", GliomaModelSystem::CellLine, matrix),
            study("cell-b", GliomaModelSystem::CellLine, matrix),
            study("cell-c", GliomaModelSystem::CellLine, matrix),
            study("cell-d", GliomaModelSystem::CellLine, matrix),
            study("organoid-a", GliomaModelSystem::Organoid, matrix),
            study("xeno-a", GliomaModelSystem::PatientDerivedXenograft, matrix),
        ];
        let result = analyze_glioma_lineage_transport(&request(), &studies).unwrap();
        assert_eq!(result.disposition, LineageTransportDisposition::Partial);
        let contrast = &result.contrasts[0];
        let cell_system = contrast
            .systems
            .iter()
            .find(|system| system.model_system == "cell_line")
            .unwrap();
        let organoid = contrast
            .systems
            .iter()
            .find(|system| system.model_system == "organoid")
            .unwrap();
        assert_eq!(cell_system.contributing_study_ids.len(), 4);
        assert_eq!(
            organoid.follow_up,
            LineageTransportFollowUpKind::AdditionalIndependentStudy
        );
        assert_eq!(
            contrast.disposition,
            LineageTransportContrastDisposition::Partial
        );
    }

    #[test]
    fn shuffling_studies_preserves_the_complete_analysis_bytes_and_digest() {
        let studies = concordant_studies();
        let original = analyze_glioma_lineage_transport(&request(), &studies).unwrap();
        let mut shuffled = studies;
        shuffled.reverse();
        let replay = analyze_glioma_lineage_transport(&request(), &shuffled).unwrap();
        assert_eq!(original, replay);
        assert_eq!(original.digest, replay.digest);
    }

    #[test]
    fn protected_source_artifact_is_refused_and_output_tampering_is_detected() {
        let mut studies = concordant_studies();
        studies[0].artifact.contains_human_data = true;
        assert!(matches!(
            analyze_glioma_lineage_transport(&request(), &studies),
            Err(LineageTransportError::InvalidStudy(_))
        ));

        let mut output =
            analyze_glioma_lineage_transport(&request(), &concordant_studies()).unwrap();
        output.contrasts[0].pooled_effect_ppm = Some(0);
        assert!(matches!(
            output.validate(),
            Err(LineageTransportError::InvalidOutput(_))
        ));
    }
}

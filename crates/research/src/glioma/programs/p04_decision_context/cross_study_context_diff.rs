//! Field-level cross-study context comparison for `GAF-GLIOMA-P04-F18`.
//!
//! Glioma findings are not transportable merely because two studies use the same label. This
//! feature compares typed model, assay, material, timing, environment, intervention, and
//! missingness context fields; applies explicit value/unit harmonization; and preserves the
//! difference between a measured disagreement and an unmeasured or absent field. It emits
//! transport warnings and acquisition gaps for downstream experiment planning. It is metadata
//! only: no raw payload is opened and no clinical decision is produced.

use crate::glioma_engine::GliomaModality;
use bioprism_foundation::PRECLINICAL_BOUNDARY;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P04-F18";
pub const OUTPUT_SCHEMA: &str = "GliomaCrossStudyContextDifference1@1";
pub const MAX_STUDIES: usize = 128;
pub const MAX_FIELDS_PER_STUDY: usize = 512;
pub const MAX_RULES: usize = 512;
pub const MAX_COVERAGE: usize = 4_096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextFieldDomain {
    Model,
    Assay,
    Material,
    Timing,
    Environment,
    Intervention,
    Missingness,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StudyContextField {
    pub field_id: String,
    pub domain: ContextFieldDomain,
    pub value: Option<String>,
    pub unit: Option<String>,
    pub measured: bool,
    pub source_digest: Option<ContentHash>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StudyContextSpec {
    pub study_id: String,
    pub context_version: String,
    pub fields: Vec<StudyContextField>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextValueAlias {
    pub field_id: String,
    pub observed: String,
    pub canonical: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextUnitAlias {
    pub field_id: String,
    pub observed: String,
    pub canonical: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextHarmonizationRule {
    pub field_id: String,
    pub canonical_unit: Option<String>,
    pub value_aliases: Vec<ContextValueAlias>,
    pub unit_aliases: Vec<ContextUnitAlias>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextModalityCoverage {
    pub study_id: String,
    pub modality: GliomaModality,
    pub available: bool,
    pub quality_milli: u16,
    pub missing_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossStudyContextDifferenceRequest {
    pub objective: String,
    pub minimum_quality_milli: u16,
    pub studies: Vec<StudyContextSpec>,
    pub harmonization_rules: Vec<ContextHarmonizationRule>,
    pub modality_coverage: Vec<ContextModalityCoverage>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextDifferenceKind {
    Equal,
    HarmonizedEqual,
    ValueDifference,
    MissingLeft,
    MissingRight,
    UnmeasuredLeft,
    UnmeasuredRight,
    UnitMismatch,
    DomainMismatch,
    VersionMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextFieldDifference {
    pub field_id: String,
    pub domain: ContextFieldDomain,
    pub kind: ContextDifferenceKind,
    pub left_value: Option<String>,
    pub right_value: Option<String>,
    pub left_unit: Option<String>,
    pub right_unit: Option<String>,
    pub normalized_left_value: Option<String>,
    pub normalized_right_value: Option<String>,
    pub normalized_left_unit: Option<String>,
    pub normalized_right_unit: Option<String>,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextStudyPairDifference {
    pub pair_id: String,
    pub left_study_id: String,
    pub right_study_id: String,
    pub field_order: Vec<String>,
    pub comparable_field_order: Vec<String>,
    pub non_comparable_field_order: Vec<String>,
    pub differences: Vec<ContextFieldDifference>,
    pub transport_warning_order: Vec<String>,
    pub acquisition_gap_order: Vec<String>,
    pub comparable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrossStudyContextDifferenceDisposition {
    Comparable,
    Partial,
    NonComparable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossStudyContextDifferenceReport {
    pub feature_id: String,
    pub output_schema: String,
    pub boundary: String,
    pub objective: String,
    pub study_order: Vec<String>,
    pub pair_order: Vec<String>,
    pub pairs: Vec<ContextStudyPairDifference>,
    pub aligned_field_order: Vec<String>,
    pub non_comparable_field_order: Vec<String>,
    pub transport_warning_order: Vec<String>,
    pub acquisition_gap_order: Vec<String>,
    pub modality_gap_order: Vec<String>,
    pub disposition: CrossStudyContextDifferenceDisposition,
    pub next_action: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CrossStudyContextDifferenceError {
    #[error("cross-study context request is invalid: {0}")]
    InvalidRequest(String),
    #[error("cross-study context field is invalid: {0}")]
    InvalidField(String),
    #[error("cross-study context report is invalid: {0}")]
    InvalidOutput(String),
    #[error("cross-study context digest failed: {0}")]
    Digest(String),
}

fn bounded_text(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty() && trimmed.len() <= 512 && !trimmed.chars().any(char::is_control)
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|window| window[0] < window[1])
}

fn field_map(study: &StudyContextSpec) -> BTreeMap<String, StudyContextField> {
    study
        .fields
        .iter()
        .cloned()
        .map(|field| (field.field_id.clone(), field))
        .collect()
}

fn rule_map(rules: &[ContextHarmonizationRule]) -> BTreeMap<String, ContextHarmonizationRule> {
    rules
        .iter()
        .cloned()
        .map(|rule| (rule.field_id.clone(), rule))
        .collect()
}

fn normalize_value(
    field_id: &str,
    value: Option<&String>,
    rules: &BTreeMap<String, ContextHarmonizationRule>,
) -> Option<String> {
    let value = value?.clone();
    let Some(rule) = rules.get(field_id) else {
        return Some(value);
    };
    rule.value_aliases
        .iter()
        .find(|alias| alias.observed == value)
        .map(|alias| alias.canonical.clone())
        .or(Some(value))
}

fn normalize_unit(
    field_id: &str,
    unit: Option<&String>,
    rules: &BTreeMap<String, ContextHarmonizationRule>,
) -> Option<String> {
    let unit = unit?.clone();
    let Some(rule) = rules.get(field_id) else {
        return Some(unit);
    };
    rule.unit_aliases
        .iter()
        .find(|alias| alias.observed == unit)
        .map(|alias| alias.canonical.clone())
        .or(Some(unit))
}

fn digest_input(report: &CrossStudyContextDifferenceReport) -> serde_json::Value {
    serde_json::json!({
        "feature_id": report.feature_id,
        "output_schema": report.output_schema,
        "boundary": report.boundary,
        "objective": report.objective,
        "study_order": report.study_order,
        "pair_order": report.pair_order,
        "pairs": report.pairs,
        "aligned_field_order": report.aligned_field_order,
        "non_comparable_field_order": report.non_comparable_field_order,
        "transport_warning_order": report.transport_warning_order,
        "acquisition_gap_order": report.acquisition_gap_order,
        "modality_gap_order": report.modality_gap_order,
        "disposition": report.disposition,
        "next_action": report.next_action,
    })
}

impl CrossStudyContextDifferenceReport {
    pub fn validate(&self) -> Result<(), CrossStudyContextDifferenceError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.boundary != PRECLINICAL_BOUNDARY
            || !bounded_text(&self.objective)
            || self.study_order.len() < 2
            || !canonical(&self.study_order)
            || !canonical(&self.pair_order)
            || self.pair_order.len() != self.pairs.len()
            || self.pairs.iter().any(|pair| {
                !bounded_text(&pair.pair_id)
                    || pair.pair_id != format!("{}::{}", pair.left_study_id, pair.right_study_id)
                    || !canonical(&pair.field_order)
                    || !canonical(&pair.comparable_field_order)
                    || !canonical(&pair.non_comparable_field_order)
                    || !canonical(&pair.transport_warning_order)
                    || !canonical(&pair.acquisition_gap_order)
                    || pair
                        .differences
                        .iter()
                        .map(|difference| difference.field_id.clone())
                        .collect::<Vec<_>>()
                        != pair.field_order
            })
            || !canonical(&self.aligned_field_order)
            || !canonical(&self.non_comparable_field_order)
            || !canonical(&self.transport_warning_order)
            || !canonical(&self.acquisition_gap_order)
            || !canonical(&self.modality_gap_order)
            || self.digest.as_str().len() != 64
        {
            return Err(CrossStudyContextDifferenceError::InvalidOutput(
                "boundary, identity, pair ordering, field partitions, or digest is invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| CrossStudyContextDifferenceError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(CrossStudyContextDifferenceError::InvalidOutput(
                "cross-study difference digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &CrossStudyContextDifferenceRequest,
) -> Result<(), CrossStudyContextDifferenceError> {
    if !bounded_text(&request.objective)
        || request.studies.len() < 2
        || request.studies.len() > MAX_STUDIES
        || request.harmonization_rules.len() > MAX_RULES
        || request.modality_coverage.len() > MAX_COVERAGE
        || request.minimum_quality_milli > 1_000
    {
        return Err(CrossStudyContextDifferenceError::InvalidRequest(
            "objective, at least two bounded studies, and bounded harmonization/coverage inputs are required".into(),
        ));
    }
    let mut study_ids = BTreeSet::new();
    for study in &request.studies {
        if !bounded_text(&study.study_id)
            || !bounded_text(&study.context_version)
            || study.fields.len() > MAX_FIELDS_PER_STUDY
            || !study_ids.insert(study.study_id.clone())
        {
            return Err(CrossStudyContextDifferenceError::InvalidRequest(
                "study identifiers, context versions, field bounds, and uniqueness are required"
                    .into(),
            ));
        }
        let mut field_ids = BTreeSet::new();
        for field in &study.fields {
            if !bounded_text(&field.field_id)
                || field
                    .value
                    .as_ref()
                    .is_some_and(|value| !bounded_text(value))
                || field.unit.as_ref().is_some_and(|unit| !bounded_text(unit))
                || !field_ids.insert(field.field_id.clone())
                || (field.measured
                    && field
                        .value
                        .as_ref()
                        .is_none_or(|value| value.trim().is_empty()))
                || field
                    .source_digest
                    .as_ref()
                    .is_some_and(|digest| digest.as_str().len() != 64)
            {
                return Err(CrossStudyContextDifferenceError::InvalidField(format!(
                    "field {} in study {} is malformed or duplicated",
                    field.field_id, study.study_id
                )));
            }
        }
    }
    let mut rule_ids = BTreeSet::new();
    for rule in &request.harmonization_rules {
        if !bounded_text(&rule.field_id)
            || !rule_ids.insert(rule.field_id.clone())
            || rule
                .canonical_unit
                .as_ref()
                .is_some_and(|unit| !bounded_text(unit))
        {
            return Err(CrossStudyContextDifferenceError::InvalidRequest(
                "harmonization rule identifiers and canonical units must be unique and bounded"
                    .into(),
            ));
        }
        let mut aliases = BTreeSet::new();
        for alias in &rule.value_aliases {
            if alias.field_id != rule.field_id
                || !bounded_text(&alias.observed)
                || !bounded_text(&alias.canonical)
                || !aliases.insert((alias.observed.clone(), alias.canonical.clone()))
            {
                return Err(CrossStudyContextDifferenceError::InvalidRequest(
                    "value aliases must bind to their rule and be unique".into(),
                ));
            }
        }
        let mut unit_aliases = BTreeSet::new();
        for alias in &rule.unit_aliases {
            if alias.field_id != rule.field_id
                || !bounded_text(&alias.observed)
                || !bounded_text(&alias.canonical)
                || !unit_aliases.insert((alias.observed.clone(), alias.canonical.clone()))
            {
                return Err(CrossStudyContextDifferenceError::InvalidRequest(
                    "unit aliases must bind to their rule and be unique".into(),
                ));
            }
        }
    }
    let mut coverage_keys = BTreeSet::new();
    for coverage in &request.modality_coverage {
        if !study_ids.contains(&coverage.study_id)
            || coverage.quality_milli > 1_000
            || !coverage_keys.insert((coverage.study_id.clone(), coverage.modality))
            || coverage
                .missing_reason
                .as_ref()
                .is_some_and(|reason| !bounded_text(reason))
            || (!coverage.available && coverage.missing_reason.as_ref().is_none())
        {
            return Err(CrossStudyContextDifferenceError::InvalidRequest(
                "modality coverage must reference a study, be unique, bounded, and explain missingness".into(),
            ));
        }
    }
    Ok(())
}

fn compare_pair(
    left: &StudyContextSpec,
    right: &StudyContextSpec,
    rules: &BTreeMap<String, ContextHarmonizationRule>,
    coverage: &BTreeMap<(String, GliomaModality), ContextModalityCoverage>,
    minimum_quality_milli: u16,
) -> ContextStudyPairDifference {
    let left_fields = field_map(left);
    let right_fields = field_map(right);
    let mut field_ids = left_fields
        .keys()
        .chain(right_fields.keys())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if left.context_version != right.context_version {
        field_ids.push("@context_version".into());
        field_ids.sort();
    }
    let mut differences = Vec::new();
    let mut comparable_fields = Vec::new();
    let mut non_comparable_fields = Vec::new();
    let mut transport_warnings = Vec::new();
    let mut acquisition_gaps = Vec::new();
    for field_id in &field_ids {
        let left_field = left_fields.get(field_id);
        let right_field = right_fields.get(field_id);
        let domain = left_field
            .map(|field| field.domain)
            .or_else(|| right_field.map(|field| field.domain))
            .unwrap_or(ContextFieldDomain::Missingness);
        let (mut kind, mut rationale) = match (left_field, right_field) {
            (None, Some(_)) => {
                acquisition_gaps.push(format!("{}:missing-left", field_id));
                (
                    ContextDifferenceKind::MissingLeft,
                    "field is absent from the left study".into(),
                )
            }
            (Some(_), None) => {
                acquisition_gaps.push(format!("{}:missing-right", field_id));
                (
                    ContextDifferenceKind::MissingRight,
                    "field is absent from the right study".into(),
                )
            }
            (Some(left_field), Some(right_field)) if left_field.domain != right_field.domain => {
                transport_warnings.push(format!("{}:domain-mismatch", field_id));
                (
                    ContextDifferenceKind::DomainMismatch,
                    "the same field identifier has incompatible domains".into(),
                )
            }
            (Some(left_field), Some(_right_field)) if !left_field.measured => {
                transport_warnings.push(format!("{}:unmeasured-left", field_id));
                (
                    ContextDifferenceKind::UnmeasuredLeft,
                    "left field is explicitly unmeasured".into(),
                )
            }
            (Some(_left_field), Some(right_field)) if !right_field.measured => {
                transport_warnings.push(format!("{}:unmeasured-right", field_id));
                (
                    ContextDifferenceKind::UnmeasuredRight,
                    "right field is explicitly unmeasured".into(),
                )
            }
            (Some(left_field), Some(right_field)) => {
                let normalized_left_unit =
                    normalize_unit(field_id, left_field.unit.as_ref(), rules);
                let normalized_right_unit =
                    normalize_unit(field_id, right_field.unit.as_ref(), rules);
                if normalized_left_unit != normalized_right_unit {
                    transport_warnings.push(format!("{}:unit-mismatch", field_id));
                    (
                        ContextDifferenceKind::UnitMismatch,
                        "units are not equivalent under the declared harmonization rule".into(),
                    )
                } else {
                    let normalized_left =
                        normalize_value(field_id, left_field.value.as_ref(), rules);
                    let normalized_right =
                        normalize_value(field_id, right_field.value.as_ref(), rules);
                    if normalized_left == normalized_right && left_field.value == right_field.value
                    {
                        (
                            ContextDifferenceKind::Equal,
                            "measured values and units are identical".into(),
                        )
                    } else if normalized_left == normalized_right {
                        (
                            ContextDifferenceKind::HarmonizedEqual,
                            "observed values are equal after the declared alias rule".into(),
                        )
                    } else {
                        transport_warnings.push(format!("{}:value-difference", field_id));
                        (
                            ContextDifferenceKind::ValueDifference,
                            "measured values differ after harmonization".into(),
                        )
                    }
                }
            }
            (None, None) if field_id == "@context_version" => (
                ContextDifferenceKind::VersionMismatch,
                "context versions differ and were not silently treated as equivalent".into(),
            ),
            (None, None) => (
                ContextDifferenceKind::MissingLeft,
                "field was not present in either indexed context".into(),
            ),
        };
        if field_id == "@context_version" {
            kind = ContextDifferenceKind::VersionMismatch;
            rationale =
                "context versions differ and were not silently treated as equivalent".into();
            transport_warnings.push("@context_version:version-mismatch".into());
        }
        let mut left_value = left_field.and_then(|field| field.value.clone());
        let mut right_value = right_field.and_then(|field| field.value.clone());
        let left_unit = left_field.and_then(|field| field.unit.clone());
        let right_unit = right_field.and_then(|field| field.unit.clone());
        if field_id == "@context_version" {
            left_value = Some(left.context_version.clone());
            right_value = Some(right.context_version.clone());
        }
        let normalized_left_value = normalize_value(field_id, left_value.as_ref(), rules);
        let normalized_right_value = normalize_value(field_id, right_value.as_ref(), rules);
        let normalized_left_unit = normalize_unit(field_id, left_unit.as_ref(), rules);
        let normalized_right_unit = normalize_unit(field_id, right_unit.as_ref(), rules);
        if matches!(
            kind,
            ContextDifferenceKind::Equal
                | ContextDifferenceKind::HarmonizedEqual
                | ContextDifferenceKind::ValueDifference
        ) {
            comparable_fields.push(field_id.clone());
        } else {
            non_comparable_fields.push(field_id.clone());
        }
        differences.push(ContextFieldDifference {
            field_id: field_id.clone(),
            domain,
            kind,
            left_value,
            right_value,
            left_unit,
            right_unit,
            normalized_left_value,
            normalized_right_value,
            normalized_left_unit,
            normalized_right_unit,
            rationale,
        });
    }
    let modalities = coverage
        .keys()
        .filter(|(study_id, _)| study_id == &left.study_id || study_id == &right.study_id)
        .map(|(_, modality)| *modality)
        .collect::<BTreeSet<_>>();
    for modality in modalities {
        let left_coverage = coverage.get(&(left.study_id.clone(), modality));
        let right_coverage = coverage.get(&(right.study_id.clone(), modality));
        let left_good = left_coverage
            .is_some_and(|entry| entry.available && entry.quality_milli >= minimum_quality_milli);
        let right_good = right_coverage
            .is_some_and(|entry| entry.available && entry.quality_milli >= minimum_quality_milli);
        if left_good != right_good {
            let label = format!("modality:{modality:?}");
            acquisition_gaps.push(label.clone());
        }
    }
    let comparable = non_comparable_fields.is_empty();
    let pair_id = format!("{}::{}", left.study_id, right.study_id);
    transport_warnings.sort();
    transport_warnings.dedup();
    acquisition_gaps.sort();
    acquisition_gaps.dedup();
    ContextStudyPairDifference {
        pair_id,
        left_study_id: left.study_id.clone(),
        right_study_id: right.study_id.clone(),
        field_order: field_ids,
        comparable_field_order: comparable_fields,
        non_comparable_field_order: non_comparable_fields,
        differences,
        transport_warning_order: transport_warnings,
        acquisition_gap_order: acquisition_gaps,
        comparable,
    }
}

/// Compare bounded typed study contexts and produce transport/acquisition guidance.
pub fn analyze_glioma_cross_study_context_difference(
    request: &CrossStudyContextDifferenceRequest,
) -> Result<CrossStudyContextDifferenceReport, CrossStudyContextDifferenceError> {
    validate_request(request)?;
    let mut studies = request.studies.clone();
    studies.sort_by(|left, right| left.study_id.cmp(&right.study_id));
    let rules = rule_map(&request.harmonization_rules);
    let coverage = request
        .modality_coverage
        .iter()
        .cloned()
        .map(|entry| ((entry.study_id.clone(), entry.modality), entry))
        .collect::<BTreeMap<_, _>>();
    let mut pairs = Vec::new();
    for left_index in 0..studies.len() {
        for right_index in left_index + 1..studies.len() {
            pairs.push(compare_pair(
                &studies[left_index],
                &studies[right_index],
                &rules,
                &coverage,
                request.minimum_quality_milli,
            ));
        }
    }
    pairs.sort_by(|left, right| left.pair_id.cmp(&right.pair_id));
    let mut aligned = BTreeSet::new();
    let mut non_comparable = BTreeSet::new();
    let mut transport_warnings = BTreeSet::new();
    let mut acquisition_gaps = BTreeSet::new();
    for pair in &pairs {
        aligned.extend(pair.comparable_field_order.iter().cloned());
        non_comparable.extend(pair.non_comparable_field_order.iter().cloned());
        transport_warnings.extend(
            pair.transport_warning_order
                .iter()
                .map(|item| format!("{}:{item}", pair.pair_id)),
        );
        acquisition_gaps.extend(
            pair.acquisition_gap_order
                .iter()
                .map(|item| format!("{}:{item}", pair.pair_id)),
        );
    }
    let aligned = aligned.into_iter().collect::<Vec<_>>();
    let non_comparable = non_comparable.into_iter().collect::<Vec<_>>();
    let transport_warnings = transport_warnings.into_iter().collect::<Vec<_>>();
    let acquisition_gaps = acquisition_gaps.into_iter().collect::<Vec<_>>();
    let mut modality_gap_order = BTreeSet::new();
    for gap in &acquisition_gaps {
        if gap.contains("modality:") {
            modality_gap_order.insert(gap.clone());
        }
    }
    let modality_gap_order = modality_gap_order.into_iter().collect::<Vec<_>>();
    let disposition = if pairs.iter().all(|pair| pair.comparable)
        && transport_warnings.is_empty()
        && acquisition_gaps.is_empty()
    {
        CrossStudyContextDifferenceDisposition::Comparable
    } else if pairs.iter().any(|pair| pair.comparable) {
        CrossStudyContextDifferenceDisposition::Partial
    } else {
        CrossStudyContextDifferenceDisposition::NonComparable
    };
    let next_action = match disposition {
        CrossStudyContextDifferenceDisposition::Comparable => "retain the aligned context and proceed to cross-study analysis with the declared harmonization rules".into(),
        CrossStudyContextDifferenceDisposition::Partial => "stratify or acquire the listed missing context before transporting a finding across studies".into(),
        CrossStudyContextDifferenceDisposition::NonComparable => "do not pool the studies; resolve model, assay, timing, unit, or missingness differences first".into(),
    };
    let mut report = CrossStudyContextDifferenceReport {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
        objective: request.objective.clone(),
        study_order: studies.iter().map(|study| study.study_id.clone()).collect(),
        pair_order: pairs.iter().map(|pair| pair.pair_id.clone()).collect(),
        pairs,
        aligned_field_order: aligned,
        non_comparable_field_order: non_comparable,
        transport_warning_order: transport_warnings,
        acquisition_gap_order: acquisition_gaps,
        modality_gap_order,
        disposition,
        next_action,
        digest: ContentHash::of_bytes(b"unsealed-glioma-cross-study-context-difference"),
    };
    report.digest = ContentHash::of_value(&digest_input(&report))
        .map_err(|error| CrossStudyContextDifferenceError::Digest(error.to_string()))?;
    report.validate()?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(seed: &str) -> ContentHash {
        ContentHash::of_bytes(seed.as_bytes())
    }

    fn field(id: &str, domain: ContextFieldDomain, value: &str) -> StudyContextField {
        StudyContextField {
            field_id: id.into(),
            domain,
            value: Some(value.into()),
            unit: None,
            measured: true,
            source_digest: Some(hash(id)),
        }
    }

    fn study(id: &str, fields: Vec<StudyContextField>) -> StudyContextSpec {
        StudyContextSpec {
            study_id: id.into(),
            context_version: "ctx-1".into(),
            fields,
        }
    }

    fn request(studies: Vec<StudyContextSpec>) -> CrossStudyContextDifferenceRequest {
        CrossStudyContextDifferenceRequest {
            objective: "transport glioma invasion finding".into(),
            minimum_quality_milli: 700,
            studies,
            harmonization_rules: Vec::new(),
            modality_coverage: Vec::new(),
        }
    }

    #[test]
    fn equal_contexts_have_no_differences_and_are_comparable() {
        let output = analyze_glioma_cross_study_context_difference(&request(vec![
            study(
                "a",
                vec![
                    field("model", ContextFieldDomain::Model, "organoid"),
                    field("assay", ContextFieldDomain::Assay, "live-imaging"),
                ],
            ),
            study(
                "b",
                vec![
                    field("model", ContextFieldDomain::Model, "organoid"),
                    field("assay", ContextFieldDomain::Assay, "live-imaging"),
                ],
            ),
        ]))
        .expect("difference report");
        assert_eq!(
            output.disposition,
            CrossStudyContextDifferenceDisposition::Comparable
        );
        assert!(output.transport_warning_order.is_empty());
        assert!(output.acquisition_gap_order.is_empty());
        assert!(output.pairs[0].comparable);
    }

    #[test]
    fn measured_value_perturbation_is_retained_as_a_transport_warning() {
        let output = analyze_glioma_cross_study_context_difference(&request(vec![
            study(
                "a",
                vec![field("model", ContextFieldDomain::Model, "organoid")],
            ),
            study(
                "b",
                vec![field("model", ContextFieldDomain::Model, "xenograft")],
            ),
        ]))
        .expect("difference report");
        assert_eq!(
            output.pairs[0].differences[0].kind,
            ContextDifferenceKind::ValueDifference
        );
        assert_eq!(
            output.disposition,
            CrossStudyContextDifferenceDisposition::Partial
        );
        assert!(output
            .transport_warning_order
            .iter()
            .any(|item| item.contains("value-difference")));
    }

    #[test]
    fn missing_context_is_not_equivalent_to_equal_context() {
        let output = analyze_glioma_cross_study_context_difference(&request(vec![
            study(
                "a",
                vec![field("timing", ContextFieldDomain::Timing, "24h")],
            ),
            study("b", vec![]),
        ]))
        .expect("difference report");
        assert_eq!(
            output.pairs[0].differences[0].kind,
            ContextDifferenceKind::MissingRight
        );
        assert!(!output.pairs[0].comparable);
        assert!(output
            .acquisition_gap_order
            .iter()
            .any(|item| item.contains("missing-right")));
    }

    #[test]
    fn declared_value_and_unit_aliases_produce_harmonized_equality() {
        let mut left = field("dose", ContextFieldDomain::Intervention, "10");
        left.unit = Some("uM".into());
        let mut right = field("dose", ContextFieldDomain::Intervention, "0.01");
        right.unit = Some("mM".into());
        let mut request = request(vec![study("a", vec![left]), study("b", vec![right])]);
        request.harmonization_rules = vec![ContextHarmonizationRule {
            field_id: "dose".into(),
            canonical_unit: Some("mM".into()),
            value_aliases: vec![ContextValueAlias {
                field_id: "dose".into(),
                observed: "10".into(),
                canonical: "0.01".into(),
            }],
            unit_aliases: vec![ContextUnitAlias {
                field_id: "dose".into(),
                observed: "uM".into(),
                canonical: "mM".into(),
            }],
        }];
        let output =
            analyze_glioma_cross_study_context_difference(&request).expect("difference report");
        assert_eq!(
            output.pairs[0].differences[0].kind,
            ContextDifferenceKind::HarmonizedEqual
        );
        assert!(output.pairs[0].comparable);
    }

    #[test]
    fn permutation_of_studies_does_not_change_digest_and_coverage_gaps_are_explicit() {
        let a = study(
            "a",
            vec![field("model", ContextFieldDomain::Model, "organoid")],
        );
        let b = study(
            "b",
            vec![field("model", ContextFieldDomain::Model, "organoid")],
        );
        let mut left = request(vec![a.clone(), b.clone()]);
        left.modality_coverage = vec![
            ContextModalityCoverage {
                study_id: "a".into(),
                modality: GliomaModality::Imaging,
                available: true,
                quality_milli: 900,
                missing_reason: None,
            },
            ContextModalityCoverage {
                study_id: "b".into(),
                modality: GliomaModality::Imaging,
                available: false,
                quality_milli: 0,
                missing_reason: Some("not acquired".into()),
            },
        ];
        let right = CrossStudyContextDifferenceRequest {
            studies: vec![b, a],
            ..left.clone()
        };
        let left_report = analyze_glioma_cross_study_context_difference(&left).expect("left");
        let right_report = analyze_glioma_cross_study_context_difference(&right).expect("right");
        assert_eq!(left_report.digest, right_report.digest);
        assert!(left_report
            .modality_gap_order
            .iter()
            .any(|item| item.contains("Imaging")));
    }
}

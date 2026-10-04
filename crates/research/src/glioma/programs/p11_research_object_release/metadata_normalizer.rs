//! Deterministic, reversible metadata normalization for preclinical glioma research objects.
//!
//! This is a release-preparation product surface, not a free-form metadata cleanup script.  It
//! compiles typed local metadata through an approved mapping policy, records every source field
//! and transformation, exposes conflicting candidates instead of selecting a winner, and marks
//! inferred mappings until an accountable researcher confirms them.  It never reads raw payloads,
//! transmits data, infers a biological conclusion, or makes a clinical decision.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P11-F09";
pub const OUTPUT_SCHEMA: &str = "GliomaReleaseMetadataNormalization1@1";
pub const MAX_SOURCES: usize = 128;
pub const MAX_FIELDS: usize = 4096;
pub const MAX_TARGET_FIELDS: usize = 1024;
pub const MAX_RULES: usize = 2048;
pub const MAX_VOCABULARIES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataField {
    pub source_field_id: String,
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataSource {
    pub source_id: String,
    pub fields: Vec<MetadataField>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetMetadataField {
    pub key: String,
    pub required: bool,
    pub vocabulary_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VocabularyTerm {
    pub source_value: String,
    pub canonical_value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlledVocabulary {
    pub vocabulary_id: String,
    pub terms: Vec<VocabularyTerm>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetadataTransform {
    Identity,
    Trim,
    Lowercase,
    CollapseWhitespace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataMappingRule {
    pub rule_id: String,
    pub source_key: String,
    pub target_key: String,
    pub transform: MetadataTransform,
    pub vocabulary_id: Option<String>,
    pub approved: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseMetadataNormalizationRequest {
    pub target_schema: String,
    pub target_schema_version: String,
    pub target_fields: Vec<TargetMetadataField>,
    pub sources: Vec<MetadataSource>,
    pub vocabularies: Vec<ControlledVocabulary>,
    pub mapping_rules: Vec<MetadataMappingRule>,
    pub confirmed_inference_rule_ids: BTreeSet<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NormalizedFieldStatus {
    Approved,
    NeedsConfirmation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NormalizedMetadataField {
    pub target_key: String,
    pub normalized_value: String,
    pub source_field_ids: Vec<String>,
    pub rule_ids: Vec<String>,
    pub status: NormalizedFieldStatus,
    pub vocabulary_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReversibleMetadataChange {
    pub target_key: String,
    pub source_field_ids: Vec<String>,
    pub original_values: Vec<String>,
    pub normalized_value: String,
    pub transform: MetadataTransform,
    pub rule_ids: Vec<String>,
    pub reversible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataConflict {
    pub target_key: String,
    pub source_field_ids: Vec<String>,
    pub candidate_values: Vec<String>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataUnresolved {
    pub source_field_ids: Vec<String>,
    pub key: String,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NormalizationDisposition {
    Ready,
    NeedsConfirmation,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseMetadataNormalization {
    pub feature_id: String,
    pub output_schema: String,
    pub target_schema: String,
    pub target_schema_version: String,
    pub source_order: Vec<String>,
    pub fields: Vec<NormalizedMetadataField>,
    pub changes: Vec<ReversibleMetadataChange>,
    pub conflicts: Vec<MetadataConflict>,
    pub unresolved: Vec<MetadataUnresolved>,
    pub missing_required_order: Vec<String>,
    pub disposition: NormalizationDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MetadataNormalizationError {
    #[error("release metadata normalization request is invalid: {0}")]
    InvalidRequest(String),
    #[error("release metadata normalization output is invalid: {0}")]
    InvalidOutput(String),
    #[error("release metadata normalization digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn valid_identifier(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 160
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:/-@".contains(&byte))
}

fn valid_text(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 16_384 && !value.contains('\0')
}

fn protected_or_clinical_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    [
        "patient",
        "human",
        "subject_id",
        "direct_identifier",
        "diagnosis",
        "treatment",
        "triage",
        "enrollment",
    ]
    .iter()
    .any(|marker| key.contains(marker))
}

fn transform_value(value: &str, transform: MetadataTransform) -> String {
    match transform {
        MetadataTransform::Identity => value.to_owned(),
        MetadataTransform::Trim => value.trim().to_owned(),
        MetadataTransform::Lowercase => value.trim().to_ascii_lowercase(),
        MetadataTransform::CollapseWhitespace => {
            value.split_whitespace().collect::<Vec<_>>().join(" ")
        }
    }
}

fn digest_input(output: &ReleaseMetadataNormalization) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "target_schema": output.target_schema,
        "target_schema_version": output.target_schema_version,
        "source_order": output.source_order,
        "fields": output.fields,
        "changes": output.changes,
        "conflicts": output.conflicts,
        "unresolved": output.unresolved,
        "missing_required_order": output.missing_required_order,
        "disposition": output.disposition,
    })
}

impl ReleaseMetadataNormalization {
    pub fn validate(&self) -> Result<(), MetadataNormalizationError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !valid_text(&self.target_schema)
            || !valid_text(&self.target_schema_version)
            || !canonical(&self.source_order)
            || !canonical(&self.missing_required_order)
            || self
                .fields
                .windows(2)
                .any(|pair| pair[0].target_key >= pair[1].target_key)
            || self
                .changes
                .windows(2)
                .any(|pair| pair[0].target_key >= pair[1].target_key)
            || self
                .conflicts
                .windows(2)
                .any(|pair| pair[0].target_key >= pair[1].target_key)
            || self.unresolved.windows(2).any(|pair| {
                (&pair[0].key, &pair[0].source_field_ids)
                    >= (&pair[1].key, &pair[1].source_field_ids)
            })
        {
            return Err(MetadataNormalizationError::InvalidOutput(
                "identity, schema, ordering, or uniqueness invariants are invalid".into(),
            ));
        }
        for field in &self.fields {
            if !valid_identifier(&field.target_key)
                || !valid_text(&field.normalized_value)
                || field.source_field_ids.is_empty()
                || !canonical(&field.source_field_ids)
                || !canonical(&field.rule_ids)
                || protected_or_clinical_key(&field.target_key)
            {
                return Err(MetadataNormalizationError::InvalidOutput(
                    "normalized fields must be typed, canonical, non-empty, and non-clinical"
                        .into(),
                ));
            }
        }
        for change in &self.changes {
            if !valid_identifier(&change.target_key)
                || change.source_field_ids.is_empty()
                || !canonical(&change.source_field_ids)
                || change.original_values.is_empty()
                || !canonical(&change.rule_ids)
                || !valid_text(&change.normalized_value)
                || !change.reversible
            {
                return Err(MetadataNormalizationError::InvalidOutput(
                    "changes must retain source links and reversible values".into(),
                ));
            }
        }
        for conflict in &self.conflicts {
            if !valid_identifier(&conflict.target_key)
                || conflict.source_field_ids.is_empty()
                || !canonical(&conflict.source_field_ids)
                || conflict.candidate_values.len() < 2
                || !canonical(&conflict.candidate_values)
                || !valid_text(&conflict.reason)
            {
                return Err(MetadataNormalizationError::InvalidOutput(
                    "conflicts must preserve at least two canonical candidates".into(),
                ));
            }
        }
        for unresolved in &self.unresolved {
            if unresolved.source_field_ids.is_empty()
                || !canonical(&unresolved.source_field_ids)
                || !valid_text(&unresolved.key)
                || !valid_text(&unresolved.reason)
            {
                return Err(MetadataNormalizationError::InvalidOutput(
                    "unresolved mappings must identify their source and reason".into(),
                ));
            }
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| MetadataNormalizationError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(MetadataNormalizationError::InvalidOutput(
                "normalization digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &ReleaseMetadataNormalizationRequest,
) -> Result<(), MetadataNormalizationError> {
    if !valid_text(&request.target_schema)
        || !valid_text(&request.target_schema_version)
        || request.target_fields.is_empty()
        || request.target_fields.len() > MAX_TARGET_FIELDS
        || request.sources.is_empty()
        || request.sources.len() > MAX_SOURCES
        || request.mapping_rules.len() > MAX_RULES
        || request.vocabularies.len() > MAX_VOCABULARIES
    {
        return Err(MetadataNormalizationError::InvalidRequest(
            "schema, target fields, sources, and bounded policy inputs are required".into(),
        ));
    }
    let mut targets = BTreeSet::new();
    for target in &request.target_fields {
        if !valid_identifier(&target.key)
            || protected_or_clinical_key(&target.key)
            || !targets.insert(target.key.clone())
        {
            return Err(MetadataNormalizationError::InvalidRequest(
                "target keys must be unique, bounded, and non-clinical".into(),
            ));
        }
        if let Some(vocabulary_id) = &target.vocabulary_id {
            if !valid_identifier(vocabulary_id) {
                return Err(MetadataNormalizationError::InvalidRequest(
                    "target vocabulary identifiers must be valid".into(),
                ));
            }
        }
    }
    let mut source_ids = BTreeSet::new();
    let mut field_ids = BTreeSet::new();
    let mut field_count = 0usize;
    for source in &request.sources {
        if !valid_identifier(&source.source_id) || !source_ids.insert(source.source_id.clone()) {
            return Err(MetadataNormalizationError::InvalidRequest(
                "source identifiers must be unique and valid".into(),
            ));
        }
        field_count = field_count.saturating_add(source.fields.len());
        for field in &source.fields {
            if !valid_identifier(&field.source_field_id)
                || !valid_text(&field.key)
                || !valid_text(&field.value)
                || protected_or_clinical_key(&field.key)
                || !field_ids.insert(field.source_field_id.clone())
            {
                return Err(MetadataNormalizationError::InvalidRequest(
                    "metadata fields must be unique, bounded, and non-clinical".into(),
                ));
            }
        }
    }
    if field_count == 0 || field_count > MAX_FIELDS {
        return Err(MetadataNormalizationError::InvalidRequest(
            "metadata field count is outside the supported bound".into(),
        ));
    }
    let mut vocabulary_ids = BTreeSet::new();
    for vocabulary in &request.vocabularies {
        if !valid_identifier(&vocabulary.vocabulary_id)
            || !vocabulary_ids.insert(vocabulary.vocabulary_id.clone())
        {
            return Err(MetadataNormalizationError::InvalidRequest(
                "vocabulary identifiers must be unique and valid".into(),
            ));
        }
        let mut values = BTreeSet::new();
        for term in &vocabulary.terms {
            if !valid_text(&term.source_value)
                || !valid_text(&term.canonical_value)
                || !values.insert(term.source_value.clone())
            {
                return Err(MetadataNormalizationError::InvalidRequest(
                    "vocabulary terms must be unique and non-empty".into(),
                ));
            }
        }
    }
    let mut rule_ids = BTreeSet::new();
    for rule in &request.mapping_rules {
        if !valid_identifier(&rule.rule_id)
            || !rule_ids.insert(rule.rule_id.clone())
            || !valid_text(&rule.source_key)
            || !valid_identifier(&rule.target_key)
            || protected_or_clinical_key(&rule.source_key)
            || protected_or_clinical_key(&rule.target_key)
            || !targets.contains(&rule.target_key)
            || rule
                .vocabulary_id
                .as_ref()
                .is_some_and(|id| !vocabulary_ids.contains(id))
        {
            return Err(MetadataNormalizationError::InvalidRequest(
                "mapping rules must be unique, target declared fields, and reference known vocabularies".into(),
            ));
        }
    }
    if request
        .confirmed_inference_rule_ids
        .iter()
        .any(|id| !rule_ids.contains(id))
    {
        return Err(MetadataNormalizationError::InvalidRequest(
            "confirmed inference rules must be declared mapping rules".into(),
        ));
    }
    Ok(())
}

/// Normalize release metadata without silently choosing between conflicting sources.
pub fn normalize_glioma_release_metadata(
    request: &ReleaseMetadataNormalizationRequest,
) -> Result<ReleaseMetadataNormalization, MetadataNormalizationError> {
    validate_request(request)?;
    let targets = request
        .target_fields
        .iter()
        .map(|field| (field.key.clone(), field.clone()))
        .collect::<BTreeMap<_, _>>();
    let vocabularies = request
        .vocabularies
        .iter()
        .map(|vocabulary| {
            (
                vocabulary.vocabulary_id.clone(),
                vocabulary
                    .terms
                    .iter()
                    .map(|term| (term.source_value.clone(), term.canonical_value.clone()))
                    .collect::<BTreeMap<_, _>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut rules_by_source = BTreeMap::<String, Vec<&MetadataMappingRule>>::new();
    for rule in &request.mapping_rules {
        rules_by_source
            .entry(rule.source_key.clone())
            .or_default()
            .push(rule);
    }
    for rules in rules_by_source.values_mut() {
        rules.sort_by(|left, right| left.rule_id.cmp(&right.rule_id));
    }

    let mut candidates = BTreeMap::<
        String,
        Vec<(
            String,
            String,
            String,
            String,
            MetadataTransform,
            Option<String>,
            bool,
        )>,
    >::new();
    let mut unresolved = Vec::new();
    let mut source_order = request
        .sources
        .iter()
        .map(|source| source.source_id.clone())
        .collect::<Vec<_>>();
    source_order.sort();
    let mut sources = request.sources.iter().collect::<Vec<_>>();
    sources.sort_by(|left, right| left.source_id.cmp(&right.source_id));
    for source in sources {
        let mut fields = source.fields.iter().collect::<Vec<_>>();
        fields.sort_by(|left, right| left.source_field_id.cmp(&right.source_field_id));
        for field in fields {
            let Some(rules) = rules_by_source.get(&field.key) else {
                unresolved.push(MetadataUnresolved {
                    source_field_ids: vec![field.source_field_id.clone()],
                    key: field.key.clone(),
                    reason: "no-approved-or-inferred-mapping-rule".into(),
                });
                continue;
            };
            let target_keys = rules
                .iter()
                .map(|rule| rule.target_key.clone())
                .collect::<BTreeSet<_>>();
            if target_keys.len() != 1 || rules.len() != 1 {
                unresolved.push(MetadataUnresolved {
                    source_field_ids: vec![field.source_field_id.clone()],
                    key: field.key.clone(),
                    reason: "ambiguous-mapping-rules".into(),
                });
                continue;
            }
            let rule = rules[0];
            let Some(target) = targets.get(&rule.target_key) else {
                unresolved.push(MetadataUnresolved {
                    source_field_ids: vec![field.source_field_id.clone()],
                    key: field.key.clone(),
                    reason: "target-field-not-declared".into(),
                });
                continue;
            };
            let mut value = transform_value(&field.value, rule.transform);
            if let Some(vocabulary_id) = &rule.vocabulary_id {
                if let Some(mapped) = vocabularies
                    .get(vocabulary_id)
                    .and_then(|map| map.get(&value))
                {
                    value = mapped.clone();
                } else {
                    unresolved.push(MetadataUnresolved {
                        source_field_ids: vec![field.source_field_id.clone()],
                        key: field.key.clone(),
                        reason: format!("vocabulary-value-unmapped:{vocabulary_id}"),
                    });
                    continue;
                }
            }
            if !valid_text(&value) {
                unresolved.push(MetadataUnresolved {
                    source_field_ids: vec![field.source_field_id.clone()],
                    key: field.key.clone(),
                    reason: "normalization-produced-empty-value".into(),
                });
                continue;
            }
            let status =
                rule.approved || request.confirmed_inference_rule_ids.contains(&rule.rule_id);
            candidates.entry(target.key.clone()).or_default().push((
                field.source_field_id.clone(),
                field.value.clone(),
                value,
                rule.rule_id.clone(),
                rule.transform,
                rule.vocabulary_id
                    .clone()
                    .or_else(|| target.vocabulary_id.clone()),
                status,
            ));
        }
    }

    let mut fields = Vec::new();
    let mut changes = Vec::new();
    let mut conflicts = Vec::new();
    let mut pending_confirmation = false;
    for (target_key, mut values) in candidates {
        values.sort_by(|left, right| left.0.cmp(&right.0));
        let unique_values = values
            .iter()
            .map(|value| value.2.clone())
            .collect::<BTreeSet<_>>();
        let source_field_ids = values
            .iter()
            .map(|value| value.0.clone())
            .collect::<Vec<_>>();
        if unique_values.len() > 1 {
            conflicts.push(MetadataConflict {
                target_key,
                source_field_ids,
                candidate_values: unique_values.into_iter().collect(),
                reason: "multiple sources produce different canonical values; no winner selected"
                    .into(),
            });
            continue;
        }
        let normalized_value = values[0].2.clone();
        let rule_ids = values
            .iter()
            .map(|value| value.3.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let status = if values.iter().all(|value| value.6) {
            NormalizedFieldStatus::Approved
        } else {
            pending_confirmation = true;
            NormalizedFieldStatus::NeedsConfirmation
        };
        let vocabulary_id = values.iter().find_map(|value| value.5.clone());
        let transform = values[0].4;
        fields.push(NormalizedMetadataField {
            target_key: target_key.clone(),
            normalized_value: normalized_value.clone(),
            source_field_ids: source_field_ids.clone(),
            rule_ids: rule_ids.clone(),
            status,
            vocabulary_id,
        });
        changes.push(ReversibleMetadataChange {
            target_key,
            source_field_ids,
            original_values: values.iter().map(|value| value.1.clone()).collect(),
            normalized_value,
            transform,
            rule_ids,
            reversible: true,
        });
    }
    fields.sort_by(|left, right| left.target_key.cmp(&right.target_key));
    changes.sort_by(|left, right| left.target_key.cmp(&right.target_key));
    conflicts.sort_by(|left, right| left.target_key.cmp(&right.target_key));
    unresolved.sort_by(|left, right| {
        (left.key.clone(), left.source_field_ids.clone())
            .cmp(&(right.key.clone(), right.source_field_ids.clone()))
    });
    let mapped = fields
        .iter()
        .map(|field| field.target_key.clone())
        .collect::<BTreeSet<_>>();
    let mut missing_required_order = targets
        .values()
        .filter(|field| field.required && !mapped.contains(&field.key))
        .map(|field| field.key.clone())
        .collect::<Vec<_>>();
    missing_required_order.sort();
    if !conflicts.is_empty() || !missing_required_order.is_empty() {
        pending_confirmation = false;
    }
    let disposition =
        if !conflicts.is_empty() || !missing_required_order.is_empty() || !unresolved.is_empty() {
            NormalizationDisposition::Blocked
        } else if pending_confirmation {
            NormalizationDisposition::NeedsConfirmation
        } else {
            NormalizationDisposition::Ready
        };
    let mut output = ReleaseMetadataNormalization {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        target_schema: request.target_schema.clone(),
        target_schema_version: request.target_schema_version.clone(),
        source_order,
        fields,
        changes,
        conflicts,
        unresolved,
        missing_required_order,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-release-metadata-normalization"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| MetadataNormalizationError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(approved: bool) -> ReleaseMetadataNormalizationRequest {
        ReleaseMetadataNormalizationRequest {
            target_schema: "RO-Crate".into(),
            target_schema_version: "1.1".into(),
            target_fields: vec![
                TargetMetadataField {
                    key: "assay".into(),
                    required: true,
                    vocabulary_id: Some("assay-v1".into()),
                },
                TargetMetadataField {
                    key: "organism_model".into(),
                    required: true,
                    vocabulary_id: None,
                },
            ],
            sources: vec![MetadataSource {
                source_id: "manifest".into(),
                fields: vec![
                    MetadataField {
                        source_field_id: "manifest-assay".into(),
                        key: "assay_name".into(),
                        value: " imaging ".into(),
                    },
                    MetadataField {
                        source_field_id: "manifest-model".into(),
                        key: "model".into(),
                        value: "mouse".into(),
                    },
                ],
            }],
            vocabularies: vec![ControlledVocabulary {
                vocabulary_id: "assay-v1".into(),
                terms: vec![VocabularyTerm {
                    source_value: "imaging".into(),
                    canonical_value: "microscopy".into(),
                }],
            }],
            mapping_rules: vec![
                MetadataMappingRule {
                    rule_id: "rule-assay".into(),
                    source_key: "assay_name".into(),
                    target_key: "assay".into(),
                    transform: MetadataTransform::Lowercase,
                    vocabulary_id: Some("assay-v1".into()),
                    approved,
                },
                MetadataMappingRule {
                    rule_id: "rule-model".into(),
                    source_key: "model".into(),
                    target_key: "organism_model".into(),
                    transform: MetadataTransform::Trim,
                    vocabulary_id: None,
                    approved: true,
                },
            ],
            confirmed_inference_rule_ids: BTreeSet::new(),
        }
    }

    #[test]
    fn approved_normalization_is_deterministic_and_reversible() {
        let left = normalize_glioma_release_metadata(&request(true)).unwrap();
        let right = normalize_glioma_release_metadata(&request(true)).unwrap();
        assert_eq!(left, right);
        assert_eq!(left.disposition, NormalizationDisposition::Ready);
        assert_eq!(left.fields[0].normalized_value, "microscopy");
        assert!(left.changes.iter().all(|change| change.reversible));
        left.validate().unwrap();
    }

    #[test]
    fn unapproved_mapping_requires_confirmation_without_overwriting_source() {
        let output = normalize_glioma_release_metadata(&request(false)).unwrap();
        assert_eq!(
            output.disposition,
            NormalizationDisposition::NeedsConfirmation
        );
        assert_eq!(
            output.fields[0].status,
            NormalizedFieldStatus::NeedsConfirmation
        );
        assert_eq!(
            output.changes[0].original_values,
            vec![" imaging ".to_string()]
        );
    }

    #[test]
    fn conflicting_sources_are_blocked_and_no_value_is_selected() {
        let mut request = request(true);
        request.sources.push(MetadataSource {
            source_id: "qc".into(),
            fields: vec![MetadataField {
                source_field_id: "qc-assay".into(),
                key: "assay_name".into(),
                value: "sequencing".into(),
            }],
        });
        request.vocabularies[0].terms.push(VocabularyTerm {
            source_value: "sequencing".into(),
            canonical_value: "sequencing".into(),
        });
        let output = normalize_glioma_release_metadata(&request).unwrap();
        assert_eq!(output.disposition, NormalizationDisposition::Blocked);
        assert!(output
            .fields
            .iter()
            .all(|field| field.target_key != "assay"));
        assert_eq!(output.conflicts[0].target_key, "assay");
    }

    #[test]
    fn missing_required_and_unmapped_fields_fail_closed() {
        let mut request = request(true);
        request.mapping_rules.remove(1);
        let output = normalize_glioma_release_metadata(&request).unwrap();
        assert_eq!(output.disposition, NormalizationDisposition::Blocked);
        assert_eq!(output.missing_required_order, vec!["organism_model"]);
        assert!(output
            .unresolved
            .iter()
            .any(|item| item.reason.contains("no-approved")));
    }

    #[test]
    fn protected_metadata_is_rejected_before_normalization() {
        let mut request = request(true);
        request.sources[0].fields[0].key = "patient_id".into();
        assert!(matches!(
            normalize_glioma_release_metadata(&request),
            Err(MetadataNormalizationError::InvalidRequest(_))
        ));
    }
}

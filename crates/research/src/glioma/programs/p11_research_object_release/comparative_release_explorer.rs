//! Read-only provenance-linked comparative explorer for released preclinical glioma studies.
//!
//! The explorer joins only an already validated comparative object and its source study metadata.
//! Every visible cell carries a source digest and mapping relation; missing or non-equivalent
//! mappings remain blocked rather than being rendered as comparable.  The cache key is audience
//! and access-scope bound so an access change cannot reuse a protected view.

use super::multistudy_release_composer::{
    ComparativeResearchObject, ComparativeStudyObject, MappingRelation,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P11-F18";
pub const OUTPUT_SCHEMA: &str = "GliomaComparativeReleaseView1@1";
pub const MAX_CELLS: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComparativeReleaseExplorerRequest {
    pub comparative_object: ComparativeResearchObject,
    pub studies: Vec<ComparativeStudyObject>,
    pub audience_id: String,
    pub access_scope: String,
    pub target_concept_order: Vec<String>,
    pub requested_study_order: Vec<String>,
    pub access_epoch: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComparativeViewCell {
    pub target_concept: String,
    pub study_id: String,
    pub model_system: String,
    pub source_field_id: String,
    pub source_digest: ContentHash,
    pub relation: MappingRelation,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComparativeReleaseView {
    pub feature_id: String,
    pub output_schema: String,
    pub audience_id: String,
    pub access_scope: String,
    pub access_epoch: u64,
    pub comparative_digest: ContentHash,
    pub study_order: Vec<String>,
    pub cell_order: Vec<ComparativeViewCell>,
    pub unavailable_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub cache_key: ContentHash,
    pub protected_cache_evicted: bool,
    pub view_digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ComparativeExplorerError {
    #[error("comparative release explorer request is invalid: {0}")]
    InvalidRequest(String),
    #[error("comparative release explorer output is invalid: {0}")]
    InvalidOutput(String),
    #[error("comparative release explorer digest failed: {0}")]
    Digest(String),
}

fn identifier(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 160
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:/-@".contains(&byte))
}

fn text(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 2048 && !value.contains('\0')
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(view: &ComparativeReleaseView) -> serde_json::Value {
    serde_json::json!({
        "feature_id": view.feature_id,
        "output_schema": view.output_schema,
        "audience_id": view.audience_id,
        "access_scope": view.access_scope,
        "access_epoch": view.access_epoch,
        "comparative_digest": view.comparative_digest,
        "study_order": view.study_order,
        "cell_order": view.cell_order,
        "unavailable_order": view.unavailable_order,
        "blocked_order": view.blocked_order,
        "cache_key": view.cache_key,
        "protected_cache_evicted": view.protected_cache_evicted,
    })
}

fn cache_key(
    comparative_digest: &ContentHash,
    audience_id: &str,
    access_scope: &str,
    access_epoch: u64,
) -> Result<ContentHash, ComparativeExplorerError> {
    ContentHash::of_value(&serde_json::json!({
        "comparative_digest": comparative_digest,
        "audience_id": audience_id,
        "access_scope": access_scope,
        "access_epoch": access_epoch,
    }))
    .map_err(|error| ComparativeExplorerError::Digest(error.to_string()))
}

impl ComparativeReleaseView {
    pub fn validate(&self) -> Result<(), ComparativeExplorerError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !identifier(&self.audience_id)
            || !identifier(&self.access_scope)
            || self.access_epoch == 0
            || self.comparative_digest.as_str().len() != 64
            || !canonical(&self.study_order)
            || !canonical(&self.unavailable_order)
            || !canonical(&self.blocked_order)
            || self.cell_order.len() > MAX_CELLS
            || self.cache_key.as_str().len() != 64
            || self.view_digest.as_str().len() != 64
        {
            return Err(ComparativeExplorerError::InvalidOutput(
                "view identity, access binding, ordering, cell bound, or digest is invalid".into(),
            ));
        }
        for cell in &self.cell_order {
            if !text(&cell.target_concept)
                || !identifier(&cell.study_id)
                || !text(&cell.model_system)
                || !identifier(&cell.source_field_id)
                || cell.source_digest.as_str().len() != 64
                || !text(&cell.value)
            {
                return Err(ComparativeExplorerError::InvalidOutput(
                    "comparison cells must retain typed source-linked values".into(),
                ));
            }
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ComparativeExplorerError::Digest(error.to_string()))?;
        if expected != self.view_digest {
            return Err(ComparativeExplorerError::InvalidOutput(
                "view digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &ComparativeReleaseExplorerRequest,
) -> Result<(), ComparativeExplorerError> {
    request
        .comparative_object
        .validate()
        .map_err(|error| ComparativeExplorerError::InvalidRequest(error.to_string()))?;
    if !identifier(&request.audience_id)
        || !identifier(&request.access_scope)
        || request.access_epoch == 0
        || request.studies.is_empty()
        || !canonical(&request.target_concept_order)
        || request.target_concept_order.is_empty()
        || !canonical(&request.requested_study_order)
        || request.requested_study_order.is_empty()
    {
        return Err(ComparativeExplorerError::InvalidRequest(
            "validated comparative object, audience/access scope, epoch, and canonical study/concept selections are required".into(),
        ));
    }
    let mut studies = BTreeSet::new();
    for study in &request.studies {
        if !identifier(&study.study_id) || !studies.insert(study.study_id.clone()) {
            return Err(ComparativeExplorerError::InvalidRequest(
                "study IDs must be unique bounded identifiers".into(),
            ));
        }
    }
    if request
        .requested_study_order
        .iter()
        .any(|study| !studies.contains(study))
    {
        return Err(ComparativeExplorerError::InvalidRequest(
            "requested studies must exist in source metadata".into(),
        ));
    }
    Ok(())
}

/// Build a provenance-linked, read-only comparative view.
pub fn explore_glioma_comparative_release(
    request: &ComparativeReleaseExplorerRequest,
) -> Result<ComparativeReleaseView, ComparativeExplorerError> {
    validate_request(request)?;
    let mappings = request
        .comparative_object
        .bindings
        .iter()
        .filter(|binding| {
            request
                .target_concept_order
                .contains(&binding.target_concept)
        })
        .flat_map(|binding| {
            binding
                .study_order
                .iter()
                .zip(binding.source_field_order.iter())
                .map(move |(study, field)| {
                    (
                        binding.target_concept.clone(),
                        study.clone(),
                        field.clone(),
                        binding.relation,
                    )
                })
        })
        .collect::<Vec<_>>();
    let mapping_by_key = mappings
        .into_iter()
        .map(|(target, study, field, relation)| ((target, study, field), relation))
        .collect::<BTreeMap<_, _>>();
    let studies = request
        .studies
        .iter()
        .filter(|study| request.requested_study_order.contains(&study.study_id))
        .map(|study| (study.study_id.clone(), study))
        .collect::<BTreeMap<_, _>>();
    let mut cells = Vec::new();
    let mut unavailable = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    for target in &request.target_concept_order {
        for study_id in &request.requested_study_order {
            let Some(study) = studies.get(study_id) else {
                unavailable.insert(format!("{target}:{study_id}:study-missing"));
                continue;
            };
            let mut matched = false;
            for field in &study.fields {
                let qualified_key = (
                    target.clone(),
                    study_id.clone(),
                    format!("{study_id}:{}", field.field_id),
                );
                let unqualified_key = (target.clone(), study_id.clone(), field.field_id.clone());
                let Some(relation) = mapping_by_key
                    .get(&qualified_key)
                    .or_else(|| mapping_by_key.get(&unqualified_key))
                    .copied()
                else {
                    continue;
                };
                matched = true;
                if relation == MappingRelation::NonEquivalent {
                    blocked.insert(format!("{target}:{study_id}:non-equivalent"));
                    continue;
                }
                cells.push(ComparativeViewCell {
                    target_concept: target.clone(),
                    study_id: study.study_id.clone(),
                    model_system: study.model_system.clone(),
                    source_field_id: field.field_id.clone(),
                    source_digest: field.source_digest.clone(),
                    relation,
                    value: field.value.clone(),
                });
            }
            if !matched {
                unavailable.insert(format!("{target}:{study_id}:mapping-missing"));
            }
        }
    }
    cells.sort_by(|left, right| {
        left.target_concept
            .cmp(&right.target_concept)
            .then_with(|| left.study_id.cmp(&right.study_id))
            .then_with(|| left.source_field_id.cmp(&right.source_field_id))
    });
    if cells.len() > MAX_CELLS {
        return Err(ComparativeExplorerError::InvalidRequest(
            "requested comparative view exceeds cell bound".into(),
        ));
    }
    let key = cache_key(
        &request.comparative_object.digest,
        &request.audience_id,
        &request.access_scope,
        request.access_epoch,
    )?;
    let mut output = ComparativeReleaseView {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        audience_id: request.audience_id.clone(),
        access_scope: request.access_scope.clone(),
        access_epoch: request.access_epoch,
        comparative_digest: request.comparative_object.digest.clone(),
        study_order: request.requested_study_order.clone(),
        cell_order: cells,
        unavailable_order: unavailable.into_iter().collect(),
        blocked_order: blocked.into_iter().collect(),
        cache_key: key,
        protected_cache_evicted: true,
        view_digest: ContentHash::of_bytes(b"unsealed-glioma-comparative-release-view"),
    };
    output.view_digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ComparativeExplorerError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p11_research_object_release::multistudy_release_composer::{
        ComparativeFieldBinding, ComparativeReleaseDisposition, ComparativeStudyField,
        ComparativeStudyObject, StudyReleaseProvenance,
    };

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn study(id: &str, field_id: &str, concept: &str) -> ComparativeStudyObject {
        ComparativeStudyObject {
            study_id: id.into(),
            model_system: "organoid".into(),
            methods_digest: hash(&format!("{id}-methods")),
            provenance_digest: hash(&format!("{id}-provenance")),
            fields: vec![ComparativeStudyField {
                field_id: field_id.into(),
                concept: concept.into(),
                value: format!("value-{id}"),
                source_digest: hash(&format!("{id}-field")),
            }],
            limitations: vec!["preclinical".into()],
        }
    }

    fn request() -> ComparativeReleaseExplorerRequest {
        let source_a = study("study-a", "field-a", "invasion");
        let source_b = study("study-b", "field-b", "invasion");
        ComparativeReleaseExplorerRequest {
            comparative_object: ComparativeResearchObject {
                feature_id: "GAF-GLIOMA-P11-F14".into(),
                output_schema: "GliomaComparativeResearchObject1@1".into(),
                study_order: vec!["study-a".into(), "study-b".into()],
                provenance: vec![
                    StudyReleaseProvenance {
                        study_id: "study-a".into(),
                        model_system: "organoid".into(),
                        methods_digest: source_a.methods_digest.clone(),
                        provenance_digest: source_a.provenance_digest.clone(),
                        limitation_order: source_a.limitations.clone(),
                    },
                    StudyReleaseProvenance {
                        study_id: "study-b".into(),
                        model_system: "organoid".into(),
                        methods_digest: source_b.methods_digest.clone(),
                        provenance_digest: source_b.provenance_digest.clone(),
                        limitation_order: source_b.limitations.clone(),
                    },
                ],
                pooled_concept_order: vec!["invasion".into()],
                non_pooled_concept_order: Vec::new(),
                unavailable_concept_order: Vec::new(),
                bindings: vec![ComparativeFieldBinding {
                    target_concept: "invasion".into(),
                    study_order: vec!["study-a".into(), "study-b".into()],
                    source_field_order: vec!["field-a".into(), "field-b".into()],
                    relation: MappingRelation::Exact,
                    pooled: true,
                }],
                omission_order: Vec::new(),
                disposition: ComparativeReleaseDisposition::Comparable,
                digest: hash("comparative"),
            },
            studies: vec![source_a, source_b],
            audience_id: "reviewer".into(),
            access_scope: "consortium".into(),
            target_concept_order: vec!["invasion".into()],
            requested_study_order: vec!["study-a".into(), "study-b".into()],
            access_epoch: 100,
        }
    }

    #[test]
    fn explorer_links_each_visible_value_to_source_and_relation() {
        let mut request = request();
        let digest = ContentHash::of_value(&serde_json::json!({
            "feature_id": request.comparative_object.feature_id,
            "output_schema": request.comparative_object.output_schema,
            "study_order": request.comparative_object.study_order,
            "provenance": request.comparative_object.provenance,
            "pooled_concept_order": request.comparative_object.pooled_concept_order,
            "non_pooled_concept_order": request.comparative_object.non_pooled_concept_order,
            "unavailable_concept_order": request.comparative_object.unavailable_concept_order,
            "bindings": request.comparative_object.bindings,
            "omission_order": request.comparative_object.omission_order,
            "disposition": request.comparative_object.disposition,
        }))
        .unwrap();
        request.comparative_object.digest = digest;
        let output = explore_glioma_comparative_release(&request).unwrap();
        assert_eq!(output.cell_order.len(), 2);
        assert!(output
            .cell_order
            .iter()
            .all(|cell| cell.relation == MappingRelation::Exact));
        output.validate().unwrap();
    }

    #[test]
    fn missing_mapping_is_unavailable_not_zero_or_comparable() {
        let mut request = request();
        request.comparative_object.bindings[0].source_field_order = vec!["field-a".into()];
        let digest = ContentHash::of_value(&serde_json::json!({
            "feature_id": request.comparative_object.feature_id,
            "output_schema": request.comparative_object.output_schema,
            "study_order": request.comparative_object.study_order,
            "provenance": request.comparative_object.provenance,
            "pooled_concept_order": request.comparative_object.pooled_concept_order,
            "non_pooled_concept_order": request.comparative_object.non_pooled_concept_order,
            "unavailable_concept_order": request.comparative_object.unavailable_concept_order,
            "bindings": request.comparative_object.bindings,
            "omission_order": request.comparative_object.omission_order,
            "disposition": request.comparative_object.disposition,
        }))
        .unwrap();
        request.comparative_object.digest = digest;
        let output = explore_glioma_comparative_release(&request).unwrap();
        assert!(output
            .unavailable_order
            .iter()
            .any(|value| value == "invasion:study-b:mapping-missing"));
    }

    #[test]
    fn non_equivalent_mapping_is_blocked_and_access_changes_cache_key() {
        let mut request = request();
        request.comparative_object.bindings[0].relation = MappingRelation::NonEquivalent;
        let digest = ContentHash::of_value(&serde_json::json!({
            "feature_id": request.comparative_object.feature_id,
            "output_schema": request.comparative_object.output_schema,
            "study_order": request.comparative_object.study_order,
            "provenance": request.comparative_object.provenance,
            "pooled_concept_order": request.comparative_object.pooled_concept_order,
            "non_pooled_concept_order": request.comparative_object.non_pooled_concept_order,
            "unavailable_concept_order": request.comparative_object.unavailable_concept_order,
            "bindings": request.comparative_object.bindings,
            "omission_order": request.comparative_object.omission_order,
            "disposition": request.comparative_object.disposition,
        }))
        .unwrap();
        request.comparative_object.digest = digest;
        let first = explore_glioma_comparative_release(&request).unwrap();
        request.access_epoch += 1;
        let second = explore_glioma_comparative_release(&request).unwrap();
        assert!(first.cell_order.is_empty());
        assert!(!first.blocked_order.is_empty());
        assert_ne!(first.cache_key, second.cache_key);
        assert!(second.protected_cache_evicted);
    }

    #[test]
    fn view_mutation_breaks_digest() {
        let mut request = request();
        let digest = ContentHash::of_value(&serde_json::json!({
            "feature_id": request.comparative_object.feature_id,
            "output_schema": request.comparative_object.output_schema,
            "study_order": request.comparative_object.study_order,
            "provenance": request.comparative_object.provenance,
            "pooled_concept_order": request.comparative_object.pooled_concept_order,
            "non_pooled_concept_order": request.comparative_object.non_pooled_concept_order,
            "unavailable_concept_order": request.comparative_object.unavailable_concept_order,
            "bindings": request.comparative_object.bindings,
            "omission_order": request.comparative_object.omission_order,
            "disposition": request.comparative_object.disposition,
        }))
        .unwrap();
        request.comparative_object.digest = digest;
        let mut output = explore_glioma_comparative_release(&request).unwrap();
        output.access_scope = "mutated".into();
        assert!(matches!(
            output.validate(),
            Err(ComparativeExplorerError::InvalidOutput(_))
        ));
    }
}

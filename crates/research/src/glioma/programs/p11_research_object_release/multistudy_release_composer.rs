//! Provenance-preserving multi-study comparative release composition for preclinical glioma work.
//!
//! This feature composes already-qualified study metadata without pretending that different model
//! systems or assays are pooled.  Explicit mappings can mark concepts exact, comparable, or
//! non-equivalent; missing and non-equivalent fields remain visible in the comparative object.
//! Values are carried as typed metadata and source digests only, not as a new biological or
//! clinical conclusion.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P11-F14";
pub const OUTPUT_SCHEMA: &str = "GliomaComparativeResearchObject1@1";
pub const MAX_STUDIES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComparativeStudyField {
    pub field_id: String,
    pub concept: String,
    pub value: String,
    pub source_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComparativeStudyObject {
    pub study_id: String,
    pub model_system: String,
    pub methods_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub fields: Vec<ComparativeStudyField>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MappingRelation {
    Exact,
    Comparable,
    NonEquivalent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComparativeAssayMapping {
    pub source_concept: String,
    pub target_concept: String,
    pub relation: MappingRelation,
    pub evidence_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComparativeReleaseRequest {
    pub studies: Vec<ComparativeStudyObject>,
    pub mappings: Vec<ComparativeAssayMapping>,
    pub required_concept_order: Vec<String>,
    pub allow_comparable_pool: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComparativeFieldBinding {
    pub target_concept: String,
    pub study_order: Vec<String>,
    pub source_field_order: Vec<String>,
    pub relation: MappingRelation,
    pub pooled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StudyReleaseProvenance {
    pub study_id: String,
    pub model_system: String,
    pub methods_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub limitation_order: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparativeReleaseDisposition {
    Comparable,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComparativeResearchObject {
    pub feature_id: String,
    pub output_schema: String,
    pub study_order: Vec<String>,
    pub provenance: Vec<StudyReleaseProvenance>,
    pub pooled_concept_order: Vec<String>,
    pub non_pooled_concept_order: Vec<String>,
    pub unavailable_concept_order: Vec<String>,
    pub bindings: Vec<ComparativeFieldBinding>,
    pub omission_order: Vec<String>,
    pub disposition: ComparativeReleaseDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ComparativeReleaseError {
    #[error("comparative release request is invalid: {0}")]
    InvalidRequest(String),
    #[error("comparative release output is invalid: {0}")]
    InvalidOutput(String),
    #[error("comparative release digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
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
    !value.trim().is_empty() && value.len() <= 2048 && !value.contains('\0')
}

fn protected_concept(value: &str) -> bool {
    let value = value.to_ascii_lowercase();
    [
        "patient",
        "human",
        "diagnosis",
        "treatment",
        "triage",
        "enrollment",
    ]
    .iter()
    .any(|marker| value.contains(marker))
}

fn digest_input(output: &ComparativeResearchObject) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "study_order": output.study_order,
        "provenance": output.provenance,
        "pooled_concept_order": output.pooled_concept_order,
        "non_pooled_concept_order": output.non_pooled_concept_order,
        "unavailable_concept_order": output.unavailable_concept_order,
        "bindings": output.bindings,
        "omission_order": output.omission_order,
        "disposition": output.disposition,
    })
}

impl ComparativeResearchObject {
    pub fn validate(&self) -> Result<(), ComparativeReleaseError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !canonical(&self.study_order)
            || !canonical(&self.pooled_concept_order)
            || !canonical(&self.non_pooled_concept_order)
            || !canonical(&self.unavailable_concept_order)
            || !canonical(&self.omission_order)
            || self
                .provenance
                .windows(2)
                .any(|pair| pair[0].study_id >= pair[1].study_id)
        {
            return Err(ComparativeReleaseError::InvalidOutput(
                "comparative identity, ordering, or provenance partition is invalid".into(),
            ));
        }
        for item in &self.provenance {
            if !valid_identifier(&item.study_id)
                || !valid_text(&item.model_system)
                || item.methods_digest.as_str().len() != 64
                || item.provenance_digest.as_str().len() != 64
                || !canonical(&item.limitation_order)
            {
                return Err(ComparativeReleaseError::InvalidOutput(
                    "study provenance is incomplete or non-canonical".into(),
                ));
            }
        }
        for binding in &self.bindings {
            if !valid_text(&binding.target_concept)
                || !canonical(&binding.study_order)
                || !canonical(&binding.source_field_order)
            {
                return Err(ComparativeReleaseError::InvalidOutput(
                    "comparative bindings must retain canonical study and field links".into(),
                ));
            }
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ComparativeReleaseError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ComparativeReleaseError::InvalidOutput(
                "comparative object digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(request: &ComparativeReleaseRequest) -> Result<(), ComparativeReleaseError> {
    if request.studies.is_empty()
        || request.studies.len() > MAX_STUDIES
        || request.required_concept_order.is_empty()
        || !canonical(&request.required_concept_order)
        || request
            .required_concept_order
            .iter()
            .any(|concept| !valid_text(concept) || protected_concept(concept))
    {
        return Err(ComparativeReleaseError::InvalidRequest(
            "bounded studies and canonical non-clinical required concepts are required".into(),
        ));
    }
    let mut studies = BTreeSet::new();
    for study in &request.studies {
        if !valid_identifier(&study.study_id)
            || !studies.insert(study.study_id.clone())
            || !valid_text(&study.model_system)
            || protected_concept(&study.model_system)
            || study.methods_digest.as_str().len() != 64
            || study.provenance_digest.as_str().len() != 64
            || study.fields.is_empty()
            || !canonical(&study.limitations)
        {
            return Err(ComparativeReleaseError::InvalidRequest(
                "study identity, methods/provenance, fields, and limitations must be valid".into(),
            ));
        }
        let mut fields = BTreeSet::new();
        for field in &study.fields {
            if !valid_identifier(&field.field_id)
                || !fields.insert(field.field_id.clone())
                || !valid_text(&field.concept)
                || protected_concept(&field.concept)
                || !valid_text(&field.value)
                || field.source_digest.as_str().len() != 64
            {
                return Err(ComparativeReleaseError::InvalidRequest(
                    "study fields must be unique, typed, non-clinical, and content-addressed"
                        .into(),
                ));
            }
        }
    }
    let mut mappings = BTreeSet::new();
    for mapping in &request.mappings {
        if !valid_text(&mapping.source_concept)
            || !valid_text(&mapping.target_concept)
            || protected_concept(&mapping.source_concept)
            || protected_concept(&mapping.target_concept)
            || mapping.evidence_digest.as_str().len() != 64
            || !mappings.insert((
                mapping.source_concept.clone(),
                mapping.target_concept.clone(),
            ))
        {
            return Err(ComparativeReleaseError::InvalidRequest(
                "assay mappings must be unique, evidenced, and non-clinical".into(),
            ));
        }
    }
    Ok(())
}

/// Compose study-scoped release metadata without pooling non-equivalent measures.
pub fn compose_glioma_multistudy_release(
    request: &ComparativeReleaseRequest,
) -> Result<ComparativeResearchObject, ComparativeReleaseError> {
    validate_request(request)?;
    let mut studies = request.studies.iter().collect::<Vec<_>>();
    studies.sort_by(|left, right| left.study_id.cmp(&right.study_id));
    let mut mapping = BTreeMap::new();
    for item in &request.mappings {
        mapping.insert(
            (item.source_concept.clone(), item.target_concept.clone()),
            item.relation,
        );
    }
    let mut pooled = BTreeSet::new();
    let mut non_pooled = BTreeSet::new();
    let mut unavailable = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut bindings = Vec::new();
    for concept in &request.required_concept_order {
        let mut study_order = Vec::new();
        let mut source_field_order = Vec::new();
        let mut relations = Vec::new();
        for study in &studies {
            let mut candidates = study
                .fields
                .iter()
                .filter_map(|field| {
                    let relation = if field.concept == *concept {
                        Some(MappingRelation::Exact)
                    } else {
                        mapping
                            .get(&(field.concept.clone(), concept.clone()))
                            .copied()
                    }?;
                    Some((field, relation))
                })
                .collect::<Vec<_>>();
            candidates.sort_by(|left, right| left.0.field_id.cmp(&right.0.field_id));
            if candidates.len() != 1 {
                unavailable.insert(concept.clone());
                omissions.insert(format!("{concept}:{}:missing-or-ambiguous", study.study_id));
                continue;
            }
            let (field, relation) = candidates[0];
            study_order.push(study.study_id.clone());
            source_field_order.push(format!("{}:{}", study.study_id, field.field_id));
            relations.push(relation);
        }
        if study_order.len() != studies.len() {
            continue;
        }
        let relation = if relations
            .iter()
            .any(|relation| *relation == MappingRelation::NonEquivalent)
        {
            MappingRelation::NonEquivalent
        } else if relations
            .iter()
            .any(|relation| *relation == MappingRelation::Comparable)
        {
            MappingRelation::Comparable
        } else {
            MappingRelation::Exact
        };
        let can_pool = relation == MappingRelation::Exact
            || (relation == MappingRelation::Comparable && request.allow_comparable_pool);
        if can_pool {
            pooled.insert(concept.clone());
        } else {
            non_pooled.insert(concept.clone());
            omissions.insert(format!("{concept}:non-equivalent-mapping"));
        }
        bindings.push(ComparativeFieldBinding {
            target_concept: concept.clone(),
            study_order,
            source_field_order,
            relation,
            pooled: can_pool,
        });
    }
    let provenance = studies
        .iter()
        .map(|study| StudyReleaseProvenance {
            study_id: study.study_id.clone(),
            model_system: study.model_system.clone(),
            methods_digest: study.methods_digest.clone(),
            provenance_digest: study.provenance_digest.clone(),
            limitation_order: study.limitations.clone(),
        })
        .collect::<Vec<_>>();
    let disposition = if pooled.is_empty() && !non_pooled.is_empty() && unavailable.is_empty() {
        ComparativeReleaseDisposition::Partial
    } else if !unavailable.is_empty() {
        ComparativeReleaseDisposition::Partial
    } else {
        ComparativeReleaseDisposition::Comparable
    };
    let mut output = ComparativeResearchObject {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        study_order: studies.iter().map(|study| study.study_id.clone()).collect(),
        provenance,
        pooled_concept_order: pooled.into_iter().collect(),
        non_pooled_concept_order: non_pooled.into_iter().collect(),
        unavailable_concept_order: unavailable.into_iter().collect(),
        bindings,
        omission_order: omissions.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-comparative-release"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ComparativeReleaseError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }
    fn study(id: &str, concept: &str) -> ComparativeStudyObject {
        ComparativeStudyObject {
            study_id: id.into(),
            model_system: format!("organoid-{id}"),
            methods_digest: hash(&format!("methods-{id}")),
            provenance_digest: hash(&format!("provenance-{id}")),
            fields: vec![ComparativeStudyField {
                field_id: format!("{id}-field"),
                concept: concept.into(),
                value: "0.5".into(),
                source_digest: hash(&format!("field-{id}")),
            }],
            limitations: vec!["preclinical".into()],
        }
    }
    fn request() -> ComparativeReleaseRequest {
        ComparativeReleaseRequest {
            studies: vec![study("study-a", "invasion"), study("study-b", "invasion")],
            mappings: Vec::new(),
            required_concept_order: vec!["invasion".into()],
            allow_comparable_pool: true,
        }
    }

    #[test]
    fn exact_fields_are_composed_with_recoverable_provenance() {
        let output = compose_glioma_multistudy_release(&request()).unwrap();
        assert_eq!(
            output.disposition,
            ComparativeReleaseDisposition::Comparable
        );
        assert_eq!(output.pooled_concept_order, vec!["invasion"]);
        assert_eq!(output.provenance.len(), 2);
        output.validate().unwrap();
    }

    #[test]
    fn non_equivalent_mapping_is_never_pooled() {
        let mut request = request();
        request.studies[1].fields[0].concept = "migration".into();
        request.mappings.push(ComparativeAssayMapping {
            source_concept: "migration".into(),
            target_concept: "invasion".into(),
            relation: MappingRelation::NonEquivalent,
            evidence_digest: hash("mapping"),
        });
        let output = compose_glioma_multistudy_release(&request).unwrap();
        assert!(output.pooled_concept_order.is_empty());
        assert_eq!(output.non_pooled_concept_order, vec!["invasion"]);
    }

    #[test]
    fn missing_study_field_is_explicitly_unavailable() {
        let mut request = request();
        request.studies[1].fields[0].concept = "migration".into();
        let output = compose_glioma_multistudy_release(&request).unwrap();
        assert_eq!(output.unavailable_concept_order, vec!["invasion"]);
        assert!(!output.omission_order.is_empty());
    }

    #[test]
    fn comparable_mapping_requires_policy_to_pool() {
        let mut request = request();
        request.studies[1].fields[0].concept = "invasion-score".into();
        request.mappings.push(ComparativeAssayMapping {
            source_concept: "invasion-score".into(),
            target_concept: "invasion".into(),
            relation: MappingRelation::Comparable,
            evidence_digest: hash("mapping"),
        });
        request.allow_comparable_pool = false;
        let output = compose_glioma_multistudy_release(&request).unwrap();
        assert!(output.pooled_concept_order.is_empty());
        assert_eq!(output.non_pooled_concept_order, vec!["invasion"]);
    }

    #[test]
    fn post_compose_mutation_breaks_digest() {
        let mut output = compose_glioma_multistudy_release(&request()).unwrap();
        output.study_order.reverse();
        assert!(matches!(
            output.validate(),
            Err(ComparativeReleaseError::InvalidOutput(_))
        ));
    }
}

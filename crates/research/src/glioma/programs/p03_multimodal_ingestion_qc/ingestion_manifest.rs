//! Typed multimodal ingestion manifest and pre-QC admission for preclinical glioma studies.
//!
//! This is the first P03 boundary: it makes every incoming modality explicit, local, schema
//! versioned, de-identified, and content-addressed before harmonization or analysis.  Duplicate,
//! incomplete, incompatible, or unsafe artifacts are quarantined rather than silently dropped.

use crate::glioma_engine::{GliomaModality, GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P03-F01";
pub const OUTPUT_SCHEMA: &str = "GliomaMultimodalIngestionManifest1@1";
pub const MAX_ITEMS: usize = 100_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalIngestionItem {
    pub item_id: String,
    pub artifact: LocalArtifactRef,
    pub modality: GliomaModality,
    pub model_system: GliomaModelSystem,
    pub batch_id: String,
    pub schema_version: String,
    pub sample_count: u32,
    pub feature_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalIngestionManifestRequest {
    pub study_id: String,
    pub objective: String,
    pub item_order: Vec<MultimodalIngestionItem>,
    pub required_modality_order: Vec<GliomaModality>,
    pub required_model_system_order: Vec<GliomaModelSystem>,
    pub expected_schema_version: String,
    pub allow_incomplete: bool,
    pub max_items: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IngestionManifestDisposition {
    Ready,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalIngestionManifest {
    pub feature_id: String,
    pub output_schema: String,
    pub study_id: String,
    pub objective: String,
    pub accepted_order: Vec<String>,
    pub quarantined_order: Vec<String>,
    pub duplicate_order: Vec<String>,
    pub incompatible_order: Vec<String>,
    pub missing_modality_order: Vec<GliomaModality>,
    pub missing_model_system_order: Vec<GliomaModelSystem>,
    pub batch_order: Vec<String>,
    pub schema_version: String,
    pub quality_milli: u16,
    pub disposition: IngestionManifestDisposition,
    pub quarantine_reasons: Vec<String>,
    pub omissions: Vec<String>,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum IngestionManifestError {
    #[error("multimodal ingestion manifest request is invalid: {0}")]
    InvalidRequest(String),
    #[error("multimodal ingestion manifest output is invalid: {0}")]
    InvalidOutput(String),
    #[error("multimodal ingestion manifest digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &MultimodalIngestionManifest) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "study_id": output.study_id,
        "objective": output.objective,
        "accepted_order": output.accepted_order,
        "quarantined_order": output.quarantined_order,
        "duplicate_order": output.duplicate_order,
        "incompatible_order": output.incompatible_order,
        "missing_modality_order": output.missing_modality_order,
        "missing_model_system_order": output.missing_model_system_order,
        "batch_order": output.batch_order,
        "schema_version": output.schema_version,
        "quality_milli": output.quality_milli,
        "disposition": output.disposition,
        "quarantine_reasons": output.quarantine_reasons,
        "omissions": output.omissions,
    })
}

impl MultimodalIngestionManifest {
    pub fn validate(&self) -> Result<(), IngestionManifestError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.study_id.trim().is_empty()
            || self.objective.trim().is_empty()
            || self.schema_version.trim().is_empty()
            || !canonical(&self.accepted_order)
            || !canonical(&self.quarantined_order)
            || !canonical(&self.duplicate_order)
            || !canonical(&self.incompatible_order)
            || !canonical(&self.missing_modality_order)
            || !canonical(&self.missing_model_system_order)
            || !canonical(&self.batch_order)
            || !canonical(&self.quarantine_reasons)
            || !canonical(&self.omissions)
            || self.quality_milli > 1_000
            || self
                .accepted_order
                .iter()
                .any(|item| self.quarantined_order.contains(item))
            || self.digest.as_str().len() != 64
        {
            return Err(IngestionManifestError::InvalidOutput(
                "identity, canonical partitions, quality, or digest is invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| IngestionManifestError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(IngestionManifestError::Digest(
                "ingestion manifest digest does not match canonical content".into(),
            ));
        }
        Ok(())
    }
}

/// Admit local multimodal artifacts into the P03 QC pipeline.
pub fn build_glioma_multimodal_ingestion_manifest(
    request: &MultimodalIngestionManifestRequest,
) -> Result<MultimodalIngestionManifest, IngestionManifestError> {
    if request.study_id.trim().is_empty()
        || request.objective.trim().is_empty()
        || request.expected_schema_version.trim().is_empty()
        || request.max_items == 0
        || request.max_items > MAX_ITEMS
        || request.item_order.is_empty()
        || request.item_order.len() > request.max_items
        || request
            .required_modality_order
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || request
            .required_model_system_order
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(IngestionManifestError::InvalidRequest(
            "study/objective/schema, item bounds, or required coverage ordering is invalid".into(),
        ));
    }
    let mut items = BTreeMap::<String, &MultimodalIngestionItem>::new();
    let mut artifact_hashes = BTreeMap::<String, String>::new();
    let mut quarantined = BTreeSet::new();
    let mut duplicates = BTreeSet::new();
    let mut incompatible = BTreeSet::new();
    let mut reasons = BTreeSet::new();
    let mut accepted = BTreeSet::new();
    let mut modalities = BTreeSet::new();
    let mut models = BTreeSet::new();
    let mut batches = BTreeSet::new();
    for item in &request.item_order {
        if item.item_id.trim().is_empty()
            || item.batch_id.trim().is_empty()
            || item.schema_version.trim().is_empty()
            || item.sample_count == 0
            || item.feature_count == 0
            || items.insert(item.item_id.clone(), item).is_some()
        {
            return Err(IngestionManifestError::InvalidRequest(
                "items must have unique ids, batches, schemas, and positive dimensions".into(),
            ));
        }
        item.artifact
            .validate()
            .map_err(|error| IngestionManifestError::InvalidRequest(error.to_string()))?;
        let hash = item.artifact.content_hash.as_str().to_string();
        if let Some(previous) = artifact_hashes.insert(hash, item.item_id.clone()) {
            duplicates.insert(item.item_id.clone());
            quarantined.insert(item.item_id.clone());
            reasons.insert(format!("{}:duplicate-content-of:{previous}", item.item_id));
            continue;
        }
        if item.schema_version != request.expected_schema_version {
            incompatible.insert(item.item_id.clone());
            quarantined.insert(item.item_id.clone());
            reasons.insert(format!("{}:schema-version-mismatch", item.item_id));
            continue;
        }
        accepted.insert(item.item_id.clone());
        modalities.insert(item.modality);
        models.insert(item.model_system);
        batches.insert(item.batch_id.clone());
    }
    let missing_modalities = request
        .required_modality_order
        .iter()
        .filter(|modality| !modalities.contains(modality))
        .copied()
        .collect::<Vec<_>>();
    let missing_models = request
        .required_model_system_order
        .iter()
        .filter(|model| !models.contains(model))
        .copied()
        .collect::<Vec<_>>();
    if !missing_modalities.is_empty() {
        reasons.insert("required-modality-missing".into());
    }
    if !missing_models.is_empty() {
        reasons.insert("required-model-system-missing".into());
    }
    let coverage_denominator =
        request.required_modality_order.len() + request.required_model_system_order.len();
    let coverage_numerator = request
        .required_modality_order
        .iter()
        .filter(|modality| modalities.contains(modality))
        .count()
        + request
            .required_model_system_order
            .iter()
            .filter(|model| models.contains(model))
            .count();
    let coverage = if coverage_denominator == 0 {
        1_000
    } else {
        ((coverage_numerator * 1_000) / coverage_denominator) as u16
    };
    let integrity = if request.item_order.is_empty() {
        0
    } else {
        ((accepted.len() * 1_000) / request.item_order.len()) as u16
    };
    let quality = coverage.min(integrity);
    let disposition = if accepted.is_empty()
        || (!request.allow_incomplete
            && (!missing_modalities.is_empty() || !missing_models.is_empty()))
    {
        IngestionManifestDisposition::Blocked
    } else if !missing_modalities.is_empty()
        || !missing_models.is_empty()
        || !quarantined.is_empty()
    {
        IngestionManifestDisposition::Partial
    } else {
        IngestionManifestDisposition::Ready
    };
    let mut output = MultimodalIngestionManifest {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        study_id: request.study_id.clone(),
        objective: request.objective.clone(),
        accepted_order: accepted.into_iter().collect(),
        quarantined_order: quarantined.into_iter().collect(),
        duplicate_order: duplicates.into_iter().collect(),
        incompatible_order: incompatible.into_iter().collect(),
        missing_modality_order: missing_modalities,
        missing_model_system_order: missing_models,
        batch_order: batches.into_iter().collect(),
        schema_version: request.expected_schema_version.clone(),
        quality_milli: quality,
        disposition,
        quarantine_reasons: reasons.into_iter().collect(),
        omissions: if disposition == IngestionManifestDisposition::Ready {
            vec![]
        } else {
            vec!["ingestion-manifest-not-complete".into()]
        },
        digest: ContentHash::of_bytes(b"placeholder"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| IngestionManifestError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma_engine::GliomaModality;

    fn item(id: &str, schema: &str, hash: &str) -> MultimodalIngestionItem {
        MultimodalIngestionItem {
            item_id: id.into(),
            artifact: LocalArtifactRef {
                artifact_id: format!("artifact-{id}"),
                content_hash: ContentHash::of_bytes(hash.as_bytes()),
                content_type: "application/json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
            modality: GliomaModality::Genomics,
            model_system: GliomaModelSystem::Organoid,
            batch_id: "batch-1".into(),
            schema_version: schema.into(),
            sample_count: 4,
            feature_count: 8,
        }
    }

    fn request(items: Vec<MultimodalIngestionItem>) -> MultimodalIngestionManifestRequest {
        MultimodalIngestionManifestRequest {
            study_id: "study-1".into(),
            objective: "admit multimodal batch".into(),
            item_order: items,
            required_modality_order: vec![GliomaModality::Genomics],
            required_model_system_order: vec![GliomaModelSystem::Organoid],
            expected_schema_version: "schema-1".into(),
            allow_incomplete: true,
            max_items: 10,
        }
    }

    #[test]
    fn valid_local_manifest_is_ready() {
        let output = build_glioma_multimodal_ingestion_manifest(&request(vec![item(
            "item-a", "schema-1", "hash-a",
        )]))
        .unwrap();
        assert_eq!(output.disposition, IngestionManifestDisposition::Ready);
        assert_eq!(output.quality_milli, 1_000);
        output.validate().unwrap();
    }

    #[test]
    fn schema_mismatch_is_quarantined() {
        let output = build_glioma_multimodal_ingestion_manifest(&request(vec![item(
            "item-a",
            "schema-old",
            "hash-a",
        )]))
        .unwrap();
        assert_eq!(output.disposition, IngestionManifestDisposition::Blocked);
        assert_eq!(output.incompatible_order, vec!["item-a"]);
    }

    #[test]
    fn duplicate_content_is_explicit() {
        let output = build_glioma_multimodal_ingestion_manifest(&request(vec![
            item("item-a", "schema-1", "same"),
            item("item-b", "schema-1", "same"),
        ]))
        .unwrap();
        assert_eq!(output.disposition, IngestionManifestDisposition::Partial);
        assert_eq!(output.duplicate_order, vec!["item-b"]);
    }
}

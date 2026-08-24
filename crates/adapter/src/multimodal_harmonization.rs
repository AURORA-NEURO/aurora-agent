//! Deterministic multimodal research-object harmonization.
//!
//! Atlas feature: `AFA-adapter-P06-F02`.
//!
//! The harmonizer operates on typed modality manifests and content hashes, not raw imaging or
//! omics bytes. It detects unit and coordinate conflicts, preserves missingness and semantic-loss
//! declarations, and emits a local-only research object whose comparability verdict can be
//! replayed by downstream analysis and federation services.

use bioprism_foundation::{
    LossSeverity, ProvenanceLink, SemanticLoss, TypedResearchArtifact,
    PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P06-F02";
pub const FEATURE_VERSION: &str = "0.1.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModalityManifest {
    pub modality_id: String,
    pub modality_type: String,
    pub schema_version: String,
    pub source_digest: ContentHash,
    pub units: BTreeMap<String, String>,
    pub feature_names: Vec<String>,
    pub coordinate_system: Option<String>,
    pub qc_digest: Option<ContentHash>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalHarmonizationRequest {
    pub study_id: String,
    pub reference_schema: String,
    pub modalities: Vec<ModalityManifest>,
    pub required_modalities: Vec<String>,
    pub raw_data_local: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HarmonizationDecision {
    Comparable,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarmonizedResearchObject {
    pub schema_version: String,
    pub feature_id: String,
    pub study_id: String,
    pub reference_schema: String,
    pub decision: HarmonizationDecision,
    pub modality_order: Vec<String>,
    pub alignment: BTreeMap<String, Vec<String>>,
    pub omitted_modalities: Vec<String>,
    pub semantic_loss: Vec<SemanticLoss>,
    pub reasons: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

impl HarmonizedResearchObject {
    pub fn validate(&self) -> Result<(), HarmonizationError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION || self.feature_id != FEATURE_ID {
            return Err(HarmonizationError::Contract(
                "harmonization schema or feature mismatch".into(),
            ));
        }
        if self.study_id.trim().is_empty() || self.reference_schema.trim().is_empty() {
            return Err(HarmonizationError::InvalidRequest(
                "study and reference schema are required".into(),
            ));
        }
        if self.boundary != PRECLINICAL_BOUNDARY || !self.raw_data_local {
            return Err(HarmonizationError::Localization);
        }
        if self.modality_order.is_empty() || self.alignment.is_empty() || self.reasons.is_empty() {
            return Err(HarmonizationError::InvalidRequest(
                "modality order, alignment, and reasons are required".into(),
            ));
        }
        if self
            .modality_order
            .iter()
            .any(|modality| !self.alignment.contains_key(modality))
        {
            return Err(HarmonizationError::InvalidRequest(
                "every modality needs an alignment projection".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| HarmonizationError::Contract(error.to_string()))?;
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, HarmonizationError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| HarmonizationError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| HarmonizationError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum HarmonizationError {
    #[error("invalid multimodal harmonization request: {0}")]
    InvalidRequest(String),
    #[error("multimodal harmonization contract rejected: {0}")]
    Contract(String),
    #[error("raw multimodal data must remain local")]
    Localization,
    #[error("duplicate modality id {0}")]
    DuplicateModality(String),
    #[error("required modality is missing: {0}")]
    MissingRequiredModality(String),
    #[error("modality unit or coordinate conflict: {0}")]
    Incompatibility(String),
    #[error("cannot serialize multimodal harmonization: {0}")]
    Serialization(String),
}

pub fn harmonize_multimodal(
    request: &MultimodalHarmonizationRequest,
) -> Result<HarmonizedResearchObject, HarmonizationError> {
    validate_request(request)?;
    let mut modalities = request.modalities.clone();
    modalities.sort_by(|left, right| left.modality_id.cmp(&right.modality_id));
    let modality_order = modalities
        .iter()
        .map(|modality| modality.modality_id.clone())
        .collect::<Vec<_>>();
    let mut alignment = BTreeMap::new();
    let mut semantic_loss = Vec::new();
    let mut reasons = Vec::new();
    let mut omitted_modalities = Vec::new();
    for modality in &modalities {
        let mut features = modality.feature_names.clone();
        features.sort();
        features.dedup();
        alignment.insert(modality.modality_id.clone(), features);
        if modality.qc_digest.is_none() {
            semantic_loss.push(SemanticLoss {
                field: format!("{}.qc", modality.modality_id),
                reason: "modality supplied no independent QC digest".into(),
                severity: LossSeverity::Bounded,
            });
            reasons.push(format!(
                "{} has no QC digest; downstream interpretation must retain this limitation",
                modality.modality_id
            ));
        }
    }
    for required in &request.required_modalities {
        if !modalities
            .iter()
            .any(|modality| &modality.modality_type == required || &modality.modality_id == required)
        {
            omitted_modalities.push(required.clone());
        }
    }
    if !omitted_modalities.is_empty() {
        reasons.push(format!(
            "required modalities omitted: {}",
            omitted_modalities.join(", ")
        ));
    }
    if semantic_loss.is_empty() {
        reasons.push("all modality manifests supplied independent QC digests".into());
    }
    let decision = if !omitted_modalities.is_empty() {
        HarmonizationDecision::Partial
    } else {
        HarmonizationDecision::Comparable
    };
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "feature_id": FEATURE_ID,
        "study_id": request.study_id,
        "reference_schema": request.reference_schema,
        "decision": decision,
        "modality_order": modality_order,
        "alignment": alignment,
        "omitted_modalities": omitted_modalities,
        "semantic_loss": semantic_loss,
        "reasons": reasons,
        "raw_data_local": true,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let provenance = modalities
        .iter()
        .map(|modality| ProvenanceLink {
            source_id: modality.modality_id.clone(),
            relation: "harmonized-from-local-manifest".into(),
            digest: modality.source_digest.clone(),
        })
        .collect();
    let artifact = TypedResearchArtifact::from_payload(
        format!("harmonized:{}", request.study_id),
        "application/vnd.aurora.harmonized-research-object+json",
        &payload,
        Vec::new(),
        provenance,
    )
    .map_err(|error| HarmonizationError::Contract(error.to_string()))?;
    let object = HarmonizedResearchObject {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        study_id: request.study_id.clone(),
        reference_schema: request.reference_schema.clone(),
        decision,
        modality_order: modalities
            .iter()
            .map(|modality| modality.modality_id.clone())
            .collect(),
        alignment: modalities
            .iter()
            .map(|modality| {
                let mut features = modality.feature_names.clone();
                features.sort();
                features.dedup();
                (modality.modality_id.clone(), features)
            })
            .collect(),
        omitted_modalities,
        semantic_loss,
        reasons,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    object.validate()?;
    Ok(object)
}

fn validate_request(request: &MultimodalHarmonizationRequest) -> Result<(), HarmonizationError> {
    if request.study_id.trim().is_empty()
        || request.reference_schema.trim().is_empty()
        || request.modalities.is_empty()
        || !request.raw_data_local
    {
        return Err(if !request.raw_data_local {
            HarmonizationError::Localization
        } else {
            HarmonizationError::InvalidRequest(
                "study, reference schema, modalities, and local-data declaration are required"
                    .into(),
            )
        });
    }
    let mut modality_ids = BTreeSet::new();
    let mut modality_types = BTreeMap::<String, (&BTreeMap<String, String>, Option<&String>)>::new();
    for modality in &request.modalities {
        if modality.modality_id.trim().is_empty()
            || modality.modality_type.trim().is_empty()
            || modality.schema_version.trim().is_empty()
            || modality.feature_names.is_empty()
        {
            return Err(HarmonizationError::InvalidRequest(
                "modality id, type, schema, and feature names are required".into(),
            ));
        }
        if !modality_ids.insert(modality.modality_id.clone()) {
            return Err(HarmonizationError::DuplicateModality(
                modality.modality_id.clone(),
            ));
        }
        if let Some((units, coordinate)) = modality_types.get(&modality.modality_type) {
            if *units != &modality.units {
                return Err(HarmonizationError::Incompatibility(format!(
                    "units differ for modality type {}",
                    modality.modality_type
                )));
            }
            if (*coordinate).map(|value| value.as_str())
                != modality.coordinate_system.as_ref().map(String::as_str)
            {
                return Err(HarmonizationError::Incompatibility(format!(
                    "coordinate systems differ for modality type {}",
                    modality.modality_type
                )));
            }
        } else {
            modality_types.insert(
                modality.modality_type.clone(),
                (&modality.units, modality.coordinate_system.as_ref()),
            );
        }
    }
    for required in &request.required_modalities {
        if required.trim().is_empty() {
            return Err(HarmonizationError::InvalidRequest(
                "required modality names cannot be empty".into(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(id: &str, kind: &str, qc: bool) -> ModalityManifest {
        ModalityManifest {
            modality_id: id.into(),
            modality_type: kind.into(),
            schema_version: "ome-ngff/0.5".into(),
            source_digest: ContentHash::of_bytes(id.as_bytes()),
            units: [("intensity".into(), "a.u.".into())].into(),
            feature_names: vec!["z".into(), "a".into(), "a".into()],
            coordinate_system: Some("micron-v1".into()),
            qc_digest: qc.then(|| ContentHash::of_bytes(format!("qc:{id}").as_bytes())),
        }
    }

    fn request() -> MultimodalHarmonizationRequest {
        MultimodalHarmonizationRequest {
            study_id: "study:organoid-1".into(),
            reference_schema: "aurora-multimodal/1".into(),
            modalities: vec![manifest("rna", "transcriptomics", true), manifest("image", "imaging", true)],
            required_modalities: vec!["transcriptomics".into(), "imaging".into()],
            raw_data_local: true,
        }
    }

    #[test]
    fn harmonization_is_deterministic_and_sorted() {
        let mut reversed = request();
        reversed.modalities.reverse();
        let left = harmonize_multimodal(&request()).unwrap();
        let right = harmonize_multimodal(&reversed).unwrap();
        assert_eq!(left.modality_order, vec!["image", "rna"]);
        assert_eq!(left.digest().unwrap(), right.digest().unwrap());
        assert_eq!(left.alignment["rna"], vec!["a", "z"]);
        assert_eq!(left.decision, HarmonizationDecision::Comparable);
    }

    #[test]
    fn missing_required_modality_is_partial_not_silent() {
        let mut request = request();
        request.required_modalities.push("proteomics".into());
        let object = harmonize_multimodal(&request).unwrap();
        assert_eq!(object.decision, HarmonizationDecision::Partial);
        assert_eq!(object.omitted_modalities, vec!["proteomics"]);
    }

    #[test]
    fn units_conflict_is_rejected_before_artifact_creation() {
        let mut request = request();
        request.modalities[1].modality_type = "transcriptomics".into();
        request.modalities[1].units.insert("intensity".into(), "counts".into());
        assert!(matches!(
            harmonize_multimodal(&request).unwrap_err(),
            HarmonizationError::Incompatibility(_)
        ));
    }

    #[test]
    fn raw_data_egress_is_rejected() {
        let mut request = request();
        request.raw_data_local = false;
        assert!(matches!(
            harmonize_multimodal(&request).unwrap_err(),
            HarmonizationError::Localization
        ));
    }
}

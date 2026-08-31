//! Deterministic multimodal research-object harmonization.
//!
//! Atlas feature: `AFA-adapter-P06-F02`.
//!
//! The harmonizer operates on typed modality manifests and content hashes, not raw imaging or
//! omics bytes. It detects unit and coordinate conflicts, preserves missingness and semantic-loss
//! declarations, and emits a local-only research object whose comparability verdict can be
//! replayed by downstream analysis and federation services.

use bioprism_foundation::{
    LossSeverity, ProvenanceLink, SemanticLoss, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P06-F02";
pub const FEATURE_VERSION: &str = "0.1.0";
const MAX_TEXT_BYTES: usize = 512;
const MAX_MODALITIES: usize = 8192;
const MAX_FEATURES: usize = 16384;

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
    pub modalities: Vec<ModalityManifest>,
    pub required_modalities: Vec<String>,
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
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION || self.feature_id != FEATURE_ID
        {
            return Err(HarmonizationError::Contract(
                "harmonization schema or feature mismatch".into(),
            ));
        }
        validate_text("study_id", &self.study_id)?;
        validate_text("reference_schema", &self.reference_schema)?;
        if self.boundary != PRECLINICAL_BOUNDARY || !self.raw_data_local {
            return Err(HarmonizationError::Localization);
        }
        if self.modality_order.is_empty()
            || self.alignment.is_empty()
            || self.modalities.is_empty()
            || self.reasons.is_empty()
        {
            return Err(HarmonizationError::InvalidRequest(
                "modality order, alignment, and reasons are required".into(),
            ));
        }
        if self.modality_order.len() > MAX_MODALITIES
            || self.alignment.len() != self.modality_order.len()
        {
            return Err(HarmonizationError::InvalidRequest(
                "modality and alignment bounds or coverage are invalid".into(),
            ));
        }
        validate_unique_strings("modality_order", &self.modality_order)?;
        validate_unique_strings("required_modalities", &self.required_modalities)?;
        if self.modalities.len() > MAX_MODALITIES
            || self
                .modalities
                .windows(2)
                .any(|pair| pair[0].modality_id >= pair[1].modality_id)
        {
            return Err(HarmonizationError::InvalidRequest(
                "modality manifests must be in canonical order".into(),
            ));
        }
        validate_sorted_strings("omitted_modalities", &self.omitted_modalities)?;
        validate_sorted_strings("reasons", &self.reasons)?;
        for modality in &self.modality_order {
            let features = self.alignment.get(modality).ok_or_else(|| {
                HarmonizationError::InvalidRequest(
                    "every modality needs an alignment projection".into(),
                )
            })?;
            validate_sorted_strings("alignment.features", features)?;
        }
        for loss in &self.semantic_loss {
            validate_text("semantic_loss.field", &loss.field)?;
            validate_text("semantic_loss.reason", &loss.reason)?;
        }
        if self
            .semantic_loss
            .windows(2)
            .any(|pair| pair[0].field >= pair[1].field)
        {
            return Err(HarmonizationError::InvalidRequest(
                "semantic-loss ordering is not canonical".into(),
            ));
        }
        if self.artifact.artifact_id != format!("harmonized:{}", self.study_id)
            || self.artifact.content_type
                != "application/vnd.aurora.harmonized-research-object+json"
            || self.artifact.semantic_loss != self.semantic_loss
            || self.artifact.provenance != modality_provenance(&self.modalities)
        {
            return Err(HarmonizationError::Contract(
                "harmonized artifact is not bound to the research object".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| HarmonizationError::Contract(error.to_string()))?;
        self.artifact
            .verify_payload(&harmonized_payload(self))
            .map_err(|error| HarmonizationError::Contract(error.to_string()))?;
        let request = MultimodalHarmonizationRequest {
            study_id: self.study_id.clone(),
            reference_schema: self.reference_schema.clone(),
            modalities: self.modalities.clone(),
            required_modalities: self.required_modalities.clone(),
            raw_data_local: self.raw_data_local,
        };
        let expected = harmonize_multimodal_internal(&request, false)?;
        if self != &expected {
            return Err(HarmonizationError::Contract(
                "harmonized object is not derived from its retained modality manifests and requirements".into(),
            ));
        }
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

fn validate_text(field: &str, value: &str) -> Result<(), HarmonizationError> {
    if value.is_empty() || value.trim() != value {
        return Err(HarmonizationError::InvalidRequest(format!(
            "{field} must be non-empty and trimmed"
        )));
    }
    if value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(HarmonizationError::InvalidRequest(format!(
            "{field} is outside its bounded text contract"
        )));
    }
    Ok(())
}

fn validate_unique_strings(field: &str, values: &[String]) -> Result<(), HarmonizationError> {
    if values.len() > MAX_MODALITIES {
        return Err(HarmonizationError::InvalidRequest(format!(
            "{field} exceeds its item bound"
        )));
    }
    let mut unique = BTreeSet::new();
    for value in values {
        validate_text(field, value)?;
        if !unique.insert(value) {
            return Err(HarmonizationError::InvalidRequest(format!(
                "{field} contains duplicate values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_strings(field: &str, values: &[String]) -> Result<(), HarmonizationError> {
    if values.len() > MAX_FEATURES {
        return Err(HarmonizationError::InvalidRequest(format!(
            "{field} exceeds its item bound"
        )));
    }
    for value in values {
        validate_text(field, value)?;
    }
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(HarmonizationError::InvalidRequest(format!(
            "{field} ordering is not canonical"
        )));
    }
    Ok(())
}

fn modality_provenance(modalities: &[ModalityManifest]) -> Vec<ProvenanceLink> {
    modalities
        .iter()
        .map(|modality| ProvenanceLink {
            source_id: modality.modality_id.clone(),
            relation: "harmonized-from-local-manifest".into(),
            digest: modality.source_digest.clone(),
        })
        .collect()
}

fn harmonized_payload(object: &HarmonizedResearchObject) -> serde_json::Value {
    harmonized_payload_from_parts(
        &object.schema_version,
        &object.feature_id,
        &object.study_id,
        &object.reference_schema,
        &object.modalities,
        &object.required_modalities,
        object.decision,
        &object.modality_order,
        &object.alignment,
        &object.omitted_modalities,
        &object.semantic_loss,
        &object.reasons,
        &object.artifact.provenance,
        object.raw_data_local,
        &object.boundary,
    )
}

#[allow(clippy::too_many_arguments)]
fn harmonized_payload_from_parts(
    schema_version: &str,
    feature_id: &str,
    study_id: &str,
    reference_schema: &str,
    modalities: &[ModalityManifest],
    required_modalities: &[String],
    decision: HarmonizationDecision,
    modality_order: &[String],
    alignment: &BTreeMap<String, Vec<String>>,
    omitted_modalities: &[String],
    semantic_loss: &[SemanticLoss],
    reasons: &[String],
    provenance: &[ProvenanceLink],
    raw_data_local: bool,
    boundary: &str,
) -> serde_json::Value {
    json!({
        "schema_version": schema_version,
        "feature_id": feature_id,
        "study_id": study_id,
        "reference_schema": reference_schema,
        "modalities": modalities,
        "required_modalities": required_modalities,
        "decision": decision,
        "modality_order": modality_order,
        "alignment": alignment,
        "omitted_modalities": omitted_modalities,
        "semantic_loss": semantic_loss,
        "reasons": reasons,
        "provenance": provenance,
        "raw_data_local": raw_data_local,
        "boundary": boundary,
    })
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
    harmonize_multimodal_internal(request, true)
}

fn harmonize_multimodal_internal(
    request: &MultimodalHarmonizationRequest,
    validate_output: bool,
) -> Result<HarmonizedResearchObject, HarmonizationError> {
    validate_request(request)?;
    let mut modalities = request.modalities.clone();
    for modality in &mut modalities {
        modality.feature_names.sort();
        modality.feature_names.dedup();
    }
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
    let mut required_modalities = request.required_modalities.clone();
    required_modalities.sort();
    for required in &required_modalities {
        if !modalities.iter().any(|modality| {
            &modality.modality_type == required || &modality.modality_id == required
        }) {
            omitted_modalities.push(required.clone());
        }
    }
    omitted_modalities.sort();
    omitted_modalities.dedup();
    if !omitted_modalities.is_empty() {
        reasons.push(format!(
            "required modalities omitted: {}",
            omitted_modalities.join(", ")
        ));
    }
    if semantic_loss.is_empty() {
        reasons.push("all modality manifests supplied independent QC digests".into());
    }
    semantic_loss.sort_by(|left, right| left.field.cmp(&right.field));
    reasons.sort();
    reasons.dedup();
    let decision = if !omitted_modalities.is_empty() {
        HarmonizationDecision::Partial
    } else {
        HarmonizationDecision::Comparable
    };
    let provenance = modality_provenance(&modalities);
    let payload = harmonized_payload_from_parts(
        RESEARCH_CONTRACT_SCHEMA_VERSION,
        FEATURE_ID,
        &request.study_id,
        &request.reference_schema,
        &modalities,
        &required_modalities,
        decision,
        &modality_order,
        &alignment,
        &omitted_modalities,
        &semantic_loss,
        &reasons,
        &provenance,
        true,
        PRECLINICAL_BOUNDARY,
    );
    let artifact = TypedResearchArtifact::from_payload(
        format!("harmonized:{}", request.study_id),
        "application/vnd.aurora.harmonized-research-object+json",
        &payload,
        semantic_loss.clone(),
        provenance,
    )
    .map_err(|error| HarmonizationError::Contract(error.to_string()))?;
    let object = HarmonizedResearchObject {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        study_id: request.study_id.clone(),
        reference_schema: request.reference_schema.clone(),
        modalities,
        required_modalities,
        decision,
        modality_order,
        alignment,
        omitted_modalities,
        semantic_loss,
        reasons,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    if validate_output {
        object.validate()?;
    }
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
    validate_text("study_id", &request.study_id)?;
    validate_text("reference_schema", &request.reference_schema)?;
    if request.modalities.len() > MAX_MODALITIES {
        return Err(HarmonizationError::InvalidRequest(
            "modality count exceeds its bound".into(),
        ));
    }
    validate_unique_strings("required_modalities", &request.required_modalities)?;
    let mut modality_ids = BTreeSet::new();
    let mut modality_types =
        BTreeMap::<String, (&BTreeMap<String, String>, Option<&String>)>::new();
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
        validate_text("modality_id", &modality.modality_id)?;
        validate_text("modality_type", &modality.modality_type)?;
        validate_text("modality_schema_version", &modality.schema_version)?;
        if modality.feature_names.len() > MAX_FEATURES {
            return Err(HarmonizationError::InvalidRequest(
                "feature count exceeds its bound".into(),
            ));
        }
        for feature in &modality.feature_names {
            validate_text("feature_name", feature)?;
        }
        if modality.units.len() > MAX_FEATURES {
            return Err(HarmonizationError::InvalidRequest(
                "unit count exceeds its bound".into(),
            ));
        }
        for (name, unit) in &modality.units {
            validate_text("unit_name", name)?;
            validate_text("unit", unit)?;
        }
        if let Some(coordinate_system) = &modality.coordinate_system {
            validate_text("coordinate_system", coordinate_system)?;
        }
        if modality.source_digest == ContentHash::of_bytes(b"")
            || modality.qc_digest.as_ref() == Some(&ContentHash::of_bytes(b""))
        {
            return Err(HarmonizationError::InvalidRequest(
                "source and QC digests must be non-empty".into(),
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
            if (*coordinate).map(|value| value.as_str()) != modality.coordinate_system.as_deref() {
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
            modalities: vec![
                manifest("rna", "transcriptomics", true),
                manifest("image", "imaging", true),
            ],
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
        request.modalities[1]
            .units
            .insert("intensity".into(), "counts".into());
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

    #[test]
    fn required_modality_order_does_not_change_the_object_digest() {
        let mut reversed = request();
        reversed.required_modalities.reverse();
        assert_eq!(
            harmonize_multimodal(&request()).unwrap().digest().unwrap(),
            harmonize_multimodal(&reversed).unwrap().digest().unwrap()
        );
    }

    #[test]
    fn duplicate_required_modality_is_rejected() {
        let mut request = request();
        request.required_modalities.push("imaging".into());
        assert!(matches!(
            harmonize_multimodal(&request).unwrap_err(),
            HarmonizationError::InvalidRequest(_)
        ));
    }

    #[test]
    fn object_rejects_tampered_artifact_payload_binding() {
        let mut object = harmonize_multimodal(&request()).unwrap();
        object.reference_schema = "tampered-schema".into();
        let error = object.validate().unwrap_err();
        assert!(error.to_string().contains("digest mismatch"));
    }

    #[test]
    fn retained_modality_manifest_tampering_is_rejected() {
        let mut object = harmonize_multimodal(&request()).unwrap();
        object.modalities[0].schema_version = "forged-schema".into();
        assert!(object.validate().is_err());
    }

    #[test]
    fn harmonized_provenance_tampering_is_rejected() {
        let mut object = harmonize_multimodal(&request()).unwrap();
        object.artifact.provenance[0].digest = ContentHash::of_bytes(b"tampered");
        assert!(object.validate().is_err());
    }

    #[test]
    fn modality_manifest_feature_order_is_canonicalized() {
        let mut reordered = request();
        reordered.modalities.reverse();
        reordered.modalities[0].feature_names.reverse();
        let canonical = harmonize_multimodal(&request()).unwrap();
        let reordered = harmonize_multimodal(&reordered).unwrap();
        assert_eq!(canonical.digest().unwrap(), reordered.digest().unwrap());
    }
}

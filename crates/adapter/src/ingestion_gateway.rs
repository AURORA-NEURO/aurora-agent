//! Admission-controlled multimodal ingestion gateway.
//!
//! Atlas feature: `AFA-adapter-P06-F23`.
//!
//! The gateway is the prospective/high-throughput boundary around the lower-level multimodal
//! harmonizer. It accepts only typed modality descriptors (never raw experimental bytes), checks
//! locality and institutional authorization, delegates comparability checks to the harmonizer,
//! and emits one deterministic effect receipt per admitted bundle. A denied or incompletely
//! authorized request remains an explicit blocked receipt instead of being silently downgraded.

use crate::{
    harmonize_multimodal, HarmonizationError, HarmonizedResearchObject, ModalityManifest,
    MultimodalHarmonizationRequest,
};
use bioprism_foundation::{
    LossSeverity, ProvenanceLink, SemanticLoss, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P06-F23";
pub const CONTRACT_VERSION: &str = "1.0";

/// A metadata-only description of a locally-held modality payload. The payload itself is never
/// accepted by this contract, which makes accidental federation of raw data unrepresentable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawModalityBundle {
    pub bundle_id: String,
    pub modality_id: String,
    pub modality_type: String,
    pub schema_version: String,
    pub source_digest: ContentHash,
    pub units: BTreeMap<String, String>,
    pub feature_names: Vec<String>,
    pub coordinate_system: Option<String>,
    pub instrument_id: String,
    pub acquisition_label: String,
    pub raw_data_local: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IngestionGatewayRequest {
    pub request_id: String,
    pub study_id: String,
    pub reference_schema: String,
    pub bundles: Vec<RawModalityBundle>,
    pub required_modalities: Vec<String>,
    /// Institution policy decision for this admission. A true value is not sufficient without
    /// an independent authorization reference.
    pub policy_allow: bool,
    pub authorization_reference: Option<String>,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IngestionGatewayDecision {
    Admitted,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IngestionEffectReceipt {
    pub effect_id: String,
    pub bundle_id: String,
    pub action: String,
    pub authorized: bool,
    pub source_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IngestionGatewayReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub study_id: String,
    pub decision: IngestionGatewayDecision,
    pub harmonized: HarmonizedResearchObject,
    pub admitted_bundles: Vec<String>,
    pub omitted_bundles: Vec<String>,
    pub effect_receipts: Vec<IngestionEffectReceipt>,
    pub semantic_loss: Vec<SemanticLoss>,
    pub reasons: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

impl IngestionGatewayReceipt {
    pub fn validate(&self) -> Result<(), IngestionGatewayError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
        {
            return Err(IngestionGatewayError::Contract(
                "ingestion gateway contract identity mismatch".into(),
            ));
        }
        if self.request_id.trim().is_empty() || self.study_id.trim().is_empty() {
            return Err(IngestionGatewayError::InvalidRequest(
                "request and study identifiers are required".into(),
            ));
        }
        if self.boundary != PRECLINICAL_BOUNDARY || !self.raw_data_local {
            return Err(IngestionGatewayError::Localization);
        }
        if self.reasons.is_empty() || self.harmonized.study_id != self.study_id {
            return Err(IngestionGatewayError::InvalidRequest(
                "receipt reasons and matching harmonized study are required".into(),
            ));
        }
        self.harmonized
            .validate()
            .map_err(IngestionGatewayError::Harmonization)?;
        self.artifact
            .validate_metadata()
            .map_err(|error| IngestionGatewayError::Contract(error.to_string()))?;
        let admitted: BTreeSet<_> = self.admitted_bundles.iter().collect();
        if self.effect_receipts.len() != admitted.len()
            || self
                .effect_receipts
                .iter()
                .any(|effect| !effect.authorized || !admitted.contains(&effect.bundle_id))
        {
            return Err(IngestionGatewayError::InvalidRequest(
                "each admitted bundle needs one authorized effect receipt".into(),
            ));
        }
        if self.decision == IngestionGatewayDecision::Blocked && !self.effect_receipts.is_empty() {
            return Err(IngestionGatewayError::InvalidRequest(
                "blocked admissions cannot contain effects".into(),
            ));
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, IngestionGatewayError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| IngestionGatewayError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| IngestionGatewayError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum IngestionGatewayError {
    #[error("invalid multimodal ingestion gateway request: {0}")]
    InvalidRequest(String),
    #[error("ingestion gateway contract rejected: {0}")]
    Contract(String),
    #[error("raw multimodal data must remain local")]
    Localization,
    #[error("duplicate bundle id {0}")]
    DuplicateBundle(String),
    #[error("duplicate modality id {0}")]
    DuplicateModality(String),
    #[error("authorization is required for prospective ingestion admission")]
    AuthorizationRequired,
    #[error("multimodal harmonization failed: {0}")]
    Harmonization(#[from] HarmonizationError),
    #[error("cannot serialize multimodal ingestion gateway: {0}")]
    Serialization(String),
}

pub fn run_ingestion_gateway(
    request: &IngestionGatewayRequest,
) -> Result<IngestionGatewayReceipt, IngestionGatewayError> {
    validate_request(request)?;
    let mut bundles = request.bundles.clone();
    bundles.sort_by(|left, right| left.bundle_id.cmp(&right.bundle_id));
    let manifests = bundles
        .iter()
        .map(|bundle| ModalityManifest {
            modality_id: bundle.modality_id.clone(),
            modality_type: bundle.modality_type.clone(),
            schema_version: bundle.schema_version.clone(),
            source_digest: bundle.source_digest.clone(),
            units: bundle.units.clone(),
            feature_names: bundle.feature_names.clone(),
            coordinate_system: bundle.coordinate_system.clone(),
            qc_digest: None,
        })
        .collect::<Vec<_>>();
    let harmonized = harmonize_multimodal(&MultimodalHarmonizationRequest {
        study_id: request.study_id.clone(),
        reference_schema: request.reference_schema.clone(),
        modalities: manifests,
        required_modalities: request.required_modalities.clone(),
        raw_data_local: true,
    })?;
    let mut reasons = vec![
        "raw modality payloads remain institution-local; only typed descriptors crossed the gateway".into(),
    ];
    let mut semantic_loss = harmonized.semantic_loss.clone();
    let policy_authorized = request.policy_allow && request.authorization_reference.is_some();
    if !request.policy_allow {
        reasons.push("institution policy denied prospective admission".into());
    } else if request.authorization_reference.is_none() {
        reasons.push(
            "policy allowed the request but supplied no independent authorization reference".into(),
        );
    }
    if !policy_authorized {
        semantic_loss.push(SemanticLoss {
            field: "authorization".into(),
            reason: "admission authority was incomplete; no external effect was authorized".into(),
            severity: LossSeverity::DecisionRelevant,
        });
    }
    let harmonization_partial = !harmonized.omitted_modalities.is_empty();
    let decision = if !policy_authorized {
        IngestionGatewayDecision::Blocked
    } else if harmonization_partial {
        reasons.push("required modalities were omitted; admission is explicitly partial".into());
        IngestionGatewayDecision::Partial
    } else {
        reasons.push("policy, authorization, locality, and comparability gates passed".into());
        IngestionGatewayDecision::Admitted
    };
    let admitted_bundles = if decision == IngestionGatewayDecision::Blocked {
        Vec::new()
    } else {
        bundles
            .iter()
            .map(|bundle| bundle.bundle_id.clone())
            .collect::<Vec<_>>()
    };
    let omitted_bundles = bundles
        .iter()
        .filter(|bundle| !admitted_bundles.contains(&bundle.bundle_id))
        .map(|bundle| bundle.bundle_id.clone())
        .collect::<Vec<_>>();
    let effect_receipts = if decision == IngestionGatewayDecision::Blocked {
        Vec::new()
    } else {
        bundles
            .iter()
            .map(|bundle| IngestionEffectReceipt {
                effect_id: format!("effect:{}:{}", request.request_id, bundle.bundle_id),
                bundle_id: bundle.bundle_id.clone(),
                action: "admit-local-harmonization".into(),
                authorized: true,
                source_digest: bundle.source_digest.clone(),
            })
            .collect::<Vec<_>>()
    };
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "study_id": request.study_id,
        "decision": decision,
        "admitted_bundles": admitted_bundles,
        "omitted_bundles": omitted_bundles,
        "semantic_loss": semantic_loss,
        "reasons": reasons,
        "raw_data_local": true,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let provenance = bundles
        .iter()
        .map(|bundle| ProvenanceLink {
            source_id: bundle.bundle_id.clone(),
            relation: "admitted-from-local-modality-descriptor".into(),
            digest: bundle.source_digest.clone(),
        })
        .collect();
    let artifact = TypedResearchArtifact::from_payload(
        format!("ingestion-gateway:{}", request.request_id),
        "application/vnd.aurora.multimodal-ingestion-gateway+json",
        &payload,
        semantic_loss.clone(),
        provenance,
    )
    .map_err(|error| IngestionGatewayError::Contract(error.to_string()))?;
    let receipt = IngestionGatewayReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        study_id: request.study_id.clone(),
        decision,
        harmonized,
        admitted_bundles,
        omitted_bundles,
        effect_receipts,
        semantic_loss,
        reasons,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &IngestionGatewayRequest) -> Result<(), IngestionGatewayError> {
    if request.request_id.trim().is_empty()
        || request.study_id.trim().is_empty()
        || request.reference_schema.trim().is_empty()
        || request.bundles.is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
    {
        return if !request.raw_data_local || request.boundary != PRECLINICAL_BOUNDARY {
            Err(IngestionGatewayError::Localization)
        } else {
            Err(IngestionGatewayError::InvalidRequest(
                "request, study, reference schema, bundles, and boundary are required".into(),
            ))
        };
    }
    let mut bundle_ids = BTreeSet::new();
    let mut modality_ids = BTreeSet::new();
    for bundle in &request.bundles {
        if bundle.bundle_id.trim().is_empty()
            || bundle.modality_id.trim().is_empty()
            || bundle.modality_type.trim().is_empty()
            || bundle.schema_version.trim().is_empty()
            || bundle.instrument_id.trim().is_empty()
            || bundle.acquisition_label.trim().is_empty()
            || bundle.feature_names.is_empty()
        {
            return Err(IngestionGatewayError::InvalidRequest(
                "bundle identifiers, schema, instrument, acquisition label, and features are required".into(),
            ));
        }
        if !bundle.raw_data_local {
            return Err(IngestionGatewayError::Localization);
        }
        if !bundle_ids.insert(bundle.bundle_id.clone()) {
            return Err(IngestionGatewayError::DuplicateBundle(
                bundle.bundle_id.clone(),
            ));
        }
        if !modality_ids.insert(bundle.modality_id.clone()) {
            return Err(IngestionGatewayError::DuplicateModality(
                bundle.modality_id.clone(),
            ));
        }
    }
    if request
        .required_modalities
        .iter()
        .any(|name| name.trim().is_empty())
    {
        return Err(IngestionGatewayError::InvalidRequest(
            "required modality names cannot be empty".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bundle(id: &str, kind: &str) -> RawModalityBundle {
        RawModalityBundle {
            bundle_id: format!("bundle:{id}"),
            modality_id: id.into(),
            modality_type: kind.into(),
            schema_version: "ome-ngff/0.5".into(),
            source_digest: ContentHash::of_bytes(id.as_bytes()),
            units: [("intensity".into(), "a.u.".into())].into(),
            feature_names: vec!["z".into(), "a".into(), "a".into()],
            coordinate_system: Some("micron-v1".into()),
            instrument_id: format!("instrument:{id}"),
            acquisition_label: "batch-01".into(),
            raw_data_local: true,
        }
    }

    fn request() -> IngestionGatewayRequest {
        IngestionGatewayRequest {
            request_id: "gateway:1".into(),
            study_id: "study:organoid-1".into(),
            reference_schema: "aurora-multimodal/1".into(),
            bundles: vec![bundle("rna", "transcriptomics"), bundle("image", "imaging")],
            required_modalities: vec!["transcriptomics".into(), "imaging".into()],
            policy_allow: true,
            authorization_reference: Some("approval:institution-1".into()),
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn admission_is_deterministic_and_effects_are_signed_by_digest() {
        let mut reversed = request();
        reversed.bundles.reverse();
        let left = run_ingestion_gateway(&request()).unwrap();
        let right = run_ingestion_gateway(&reversed).unwrap();
        assert_eq!(left.digest().unwrap(), right.digest().unwrap());
        assert_eq!(left.decision, IngestionGatewayDecision::Admitted);
        assert_eq!(left.admitted_bundles, vec!["bundle:image", "bundle:rna"]);
        assert_eq!(left.effect_receipts.len(), 2);
    }

    #[test]
    fn missing_authority_blocks_without_effects() {
        let mut request = request();
        request.authorization_reference = None;
        let receipt = run_ingestion_gateway(&request).unwrap();
        assert_eq!(receipt.decision, IngestionGatewayDecision::Blocked);
        assert!(receipt.effect_receipts.is_empty());
        assert!(!receipt.semantic_loss.is_empty());
    }

    #[test]
    fn raw_payload_egress_is_rejected() {
        let mut request = request();
        request.bundles[0].raw_data_local = false;
        assert!(matches!(
            run_ingestion_gateway(&request).unwrap_err(),
            IngestionGatewayError::Localization
        ));
    }

    #[test]
    fn duplicate_bundle_is_rejected_before_harmonization() {
        let mut request = request();
        request.bundles[1].bundle_id = request.bundles[0].bundle_id.clone();
        assert!(matches!(
            run_ingestion_gateway(&request).unwrap_err(),
            IngestionGatewayError::DuplicateBundle(_)
        ));
    }
}

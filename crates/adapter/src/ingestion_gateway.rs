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
const MAX_TEXT_BYTES: usize = 512;
const MAX_BUNDLES: usize = 8_192;

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
    pub input: IngestionGatewayRequest,
    pub input_digest: ContentHash,
    pub request_id: String,
    pub study_id: String,
    pub reference_schema: String,
    pub required_modalities: Vec<String>,
    pub policy_allow: bool,
    pub authorization_reference: Option<String>,
    pub decision: IngestionGatewayDecision,
    pub harmonized: HarmonizedResearchObject,
    pub bundle_order: Vec<String>,
    pub source_digest_order: Vec<ContentHash>,
    pub bundle_digest: ContentHash,
    pub admitted_bundles: Vec<String>,
    pub omitted_bundles: Vec<String>,
    pub effect_receipts: Vec<IngestionEffectReceipt>,
    pub semantic_loss: Vec<SemanticLoss>,
    pub reasons: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

fn validate_text(field: &str, value: &str) -> Result<(), IngestionGatewayError> {
    if value.is_empty() || value.trim() != value {
        return Err(IngestionGatewayError::InvalidRequest(format!(
            "{field} must be non-empty and trimmed"
        )));
    }
    if value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(IngestionGatewayError::InvalidRequest(format!(
            "{field} is outside its bounded text contract"
        )));
    }
    Ok(())
}

fn ingestion_gateway_input_digest(
    request: &IngestionGatewayRequest,
) -> Result<ContentHash, IngestionGatewayError> {
    let value = serde_json::to_value(&canonical_ingestion_gateway_request(request))
        .map_err(|error| IngestionGatewayError::Serialization(error.to_string()))?;
    ContentHash::of_value(&value)
        .map_err(|error| IngestionGatewayError::Serialization(error.to_string()))
}

fn canonical_ingestion_gateway_request(
    request: &IngestionGatewayRequest,
) -> IngestionGatewayRequest {
    let mut canonical = request.clone();
    canonical.required_modalities.sort();
    for bundle in &mut canonical.bundles {
        bundle.feature_names.sort();
    }
    canonical
        .bundles
        .sort_by(|left, right| left.bundle_id.cmp(&right.bundle_id));
    canonical
}

fn validate_unique_strings(field: &str, values: &[String]) -> Result<(), IngestionGatewayError> {
    if values.len() > MAX_BUNDLES {
        return Err(IngestionGatewayError::InvalidRequest(format!(
            "{field} exceeds its item bound"
        )));
    }
    let mut unique = BTreeSet::new();
    for value in values {
        validate_text(field, value)?;
        if !unique.insert(value) {
            return Err(IngestionGatewayError::InvalidRequest(format!(
                "{field} contains duplicate values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_strings(field: &str, values: &[String]) -> Result<(), IngestionGatewayError> {
    validate_unique_strings(field, values)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(IngestionGatewayError::InvalidRequest(format!(
            "{field} ordering is not canonical"
        )));
    }
    Ok(())
}

fn validate_digest(field: &str, digest: &ContentHash) -> Result<(), IngestionGatewayError> {
    if digest.as_str().len() != 64
        || !digest
            .as_str()
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err(IngestionGatewayError::InvalidRequest(format!(
            "{field} must be a 64-character hex digest"
        )));
    }
    Ok(())
}

fn validate_digests(field: &str, digests: &[ContentHash]) -> Result<(), IngestionGatewayError> {
    if digests.len() > MAX_BUNDLES {
        return Err(IngestionGatewayError::InvalidRequest(format!(
            "{field} exceeds its item bound"
        )));
    }
    for digest in digests {
        validate_digest(field, digest)?;
    }
    Ok(())
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
        if self.reference_schema.trim().is_empty()
            || self.required_modalities.is_empty()
            || self.bundle_order.is_empty()
            || self.source_digest_order.is_empty()
            || self.reasons.is_empty()
        {
            return Err(IngestionGatewayError::InvalidRequest(
                "gateway schema, modality, bundle, digest, and reason closures are required".into(),
            ));
        }
        validate_text("request_id", &self.request_id)?;
        validate_text("study_id", &self.study_id)?;
        validate_text("reference_schema", &self.reference_schema)?;
        validate_text("boundary", &self.boundary)?;
        validate_sorted_strings("required_modalities", &self.required_modalities)?;
        validate_sorted_strings("bundle_order", &self.bundle_order)?;
        validate_sorted_strings("admitted_bundles", &self.admitted_bundles)?;
        validate_sorted_strings("omitted_bundles", &self.omitted_bundles)?;
        validate_digests("source_digest_order", &self.source_digest_order)?;
        validate_unique_strings("reasons", &self.reasons)?;
        if let Some(authorization) = &self.authorization_reference {
            validate_text("authorization_reference", authorization)?;
        }
        validate_digest("bundle_digest", &self.bundle_digest)?;
        let bundle_set = self.bundle_order.iter().cloned().collect::<BTreeSet<_>>();
        let admitted_set = self
            .admitted_bundles
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let omitted_set = self
            .omitted_bundles
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let classified = admitted_set
            .union(&omitted_set)
            .cloned()
            .collect::<BTreeSet<_>>();
        if classified != bundle_set
            || admitted_set.intersection(&omitted_set).next().is_some()
            || self.admitted_bundles.len() + self.omitted_bundles.len() != self.bundle_order.len()
            || self.source_digest_order.len() != self.bundle_order.len()
        {
            return Err(IngestionGatewayError::InvalidRequest(
                "gateway bundle states must partition the descriptor closure".into(),
            ));
        }
        if self.harmonized.study_id != self.study_id
            || self.harmonized.reference_schema != self.reference_schema
            || self.harmonized.raw_data_local != self.raw_data_local
        {
            return Err(IngestionGatewayError::InvalidRequest(
                "receipt and harmonized object identity differ".into(),
            ));
        }
        self.harmonized
            .validate()
            .map_err(IngestionGatewayError::Harmonization)?;
        let policy_authorized = self.policy_allow && self.authorization_reference.is_some();
        let expected_decision = if !policy_authorized {
            IngestionGatewayDecision::Blocked
        } else if !self.harmonized.omitted_modalities.is_empty() {
            IngestionGatewayDecision::Partial
        } else {
            IngestionGatewayDecision::Admitted
        };
        if self.decision != expected_decision {
            return Err(IngestionGatewayError::InvalidRequest(
                "gateway decision does not match policy, authorization, and harmonization state"
                    .into(),
            ));
        }
        let expected_admitted = if self.decision == IngestionGatewayDecision::Blocked {
            Vec::new()
        } else {
            self.bundle_order.clone()
        };
        let expected_omitted = if self.decision == IngestionGatewayDecision::Blocked {
            self.bundle_order.clone()
        } else {
            Vec::new()
        };
        if self.admitted_bundles != expected_admitted || self.omitted_bundles != expected_omitted {
            return Err(IngestionGatewayError::InvalidRequest(
                "gateway admitted and omitted bundles do not match decision".into(),
            ));
        }
        if self.effect_receipts.len() != self.admitted_bundles.len()
            || self
                .effect_receipts
                .iter()
                .enumerate()
                .any(|(index, effect)| {
                    !effect.authorized
                        || effect.bundle_id != self.admitted_bundles[index]
                        || effect.effect_id
                            != format!("effect:{}:{}", self.request_id, effect.bundle_id)
                        || effect.action != "admit-local-harmonization"
                        || effect.source_digest != self.source_digest_order[index]
                })
        {
            return Err(IngestionGatewayError::InvalidRequest(
                "each admitted bundle needs one exact authorized effect receipt".into(),
            ));
        }
        if self.decision == IngestionGatewayDecision::Blocked && !self.effect_receipts.is_empty() {
            return Err(IngestionGatewayError::InvalidRequest(
                "blocked admissions cannot contain effects".into(),
            ));
        }
        let mut expected_semantic_loss = self.harmonized.semantic_loss.clone();
        if !policy_authorized {
            expected_semantic_loss.push(SemanticLoss {
                field: "authorization".into(),
                reason: "admission authority was incomplete; no external effect was authorized"
                    .into(),
                severity: LossSeverity::DecisionRelevant,
            });
        }
        if self.semantic_loss != expected_semantic_loss {
            return Err(IngestionGatewayError::Contract(
                "gateway semantic-loss closure does not match harmonization and authorization"
                    .into(),
            ));
        }
        let mut expected_reasons = vec![
            "raw modality payloads remain institution-local; only typed descriptors crossed the gateway".to_string(),
        ];
        if !self.policy_allow {
            expected_reasons.push("institution policy denied prospective admission".into());
        } else if self.authorization_reference.is_none() {
            expected_reasons.push(
                "policy allowed the request but supplied no independent authorization reference"
                    .into(),
            );
        }
        if !policy_authorized {
            // The authorization loss is represented in semantic_loss; no extra reason is needed.
        } else if !self.harmonized.omitted_modalities.is_empty() {
            expected_reasons
                .push("required modalities were omitted; admission is explicitly partial".into());
        } else {
            expected_reasons
                .push("policy, authorization, locality, and comparability gates passed".into());
        }
        if self.reasons != expected_reasons {
            return Err(IngestionGatewayError::InvalidRequest(
                "gateway reasons are not bound to admission state".into(),
            ));
        }
        let mut expected_provenance = Vec::new();
        for (bundle_id, digest) in self.bundle_order.iter().zip(&self.source_digest_order) {
            expected_provenance.push(ProvenanceLink {
                source_id: bundle_id.clone(),
                relation: "admitted-from-local-modality-descriptor".into(),
                digest: digest.clone(),
            });
        }
        if self.artifact.artifact_id != format!("ingestion-gateway:{}", self.request_id)
            || self.artifact.content_type
                != "application/vnd.aurora.multimodal-ingestion-gateway+json"
            || self.artifact.semantic_loss != self.semantic_loss
            || self.artifact.provenance != expected_provenance
        {
            return Err(IngestionGatewayError::Contract(
                "gateway artifact is not bound to descriptor and admission state".into(),
            ));
        }
        let payload = json!({
            "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
            "contract_version": CONTRACT_VERSION,
            "feature_id": FEATURE_ID,
            "request_id": self.request_id,
            "study_id": self.study_id,
            "reference_schema": self.reference_schema,
            "required_modalities": self.required_modalities,
            "policy_allow": self.policy_allow,
            "authorization_reference": self.authorization_reference,
            "decision": self.decision,
            "admitted_bundles": self.admitted_bundles,
            "omitted_bundles": self.omitted_bundles,
            "effect_receipts": self.effect_receipts,
            "semantic_loss": self.semantic_loss,
            "reasons": self.reasons,
            "bundle_order": self.bundle_order,
            "source_digest_order": self.source_digest_order,
            "bundle_digest": self.bundle_digest,
            "raw_data_local": self.raw_data_local,
            "boundary": PRECLINICAL_BOUNDARY,
        });
        self.artifact
            .verify_payload(&payload)
            .map_err(|error| IngestionGatewayError::Contract(error.to_string()))?;
        self.artifact
            .validate_metadata()
            .map_err(|error| IngestionGatewayError::Contract(error.to_string()))?;
        validate_request(&self.input)?;
        if self.input_digest != ingestion_gateway_input_digest(&self.input)? {
            return Err(IngestionGatewayError::Contract(
                "ingestion gateway retained input digest does not match the request".into(),
            ));
        }
        let expected = build_ingestion_gateway(&self.input)?;
        if self != &expected {
            return Err(IngestionGatewayError::Contract(
                "ingestion gateway receipt is not derived from its retained request".into(),
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
    let receipt = build_ingestion_gateway(request)?;
    receipt.validate()?;
    Ok(receipt)
}

fn build_ingestion_gateway(
    request: &IngestionGatewayRequest,
) -> Result<IngestionGatewayReceipt, IngestionGatewayError> {
    validate_request(request)?;
    let mut bundles = request.bundles.clone();
    for bundle in &mut bundles {
        bundle.feature_names.sort();
    }
    bundles.sort_by(|left, right| left.bundle_id.cmp(&right.bundle_id));
    let mut required_modalities = request.required_modalities.clone();
    required_modalities.sort();
    let bundle_order = bundles
        .iter()
        .map(|bundle| bundle.bundle_id.clone())
        .collect::<Vec<_>>();
    let source_digest_order = bundles
        .iter()
        .map(|bundle| bundle.source_digest.clone())
        .collect::<Vec<_>>();
    let bundle_digest = ContentHash::of_value(
        &serde_json::to_value(&bundles)
            .map_err(|error| IngestionGatewayError::Serialization(error.to_string()))?,
    )
    .map_err(|error| IngestionGatewayError::Serialization(error.to_string()))?;
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
        required_modalities: required_modalities.clone(),
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
        "reference_schema": request.reference_schema,
        "required_modalities": required_modalities,
        "policy_allow": request.policy_allow,
        "authorization_reference": request.authorization_reference,
        "decision": decision,
        "admitted_bundles": admitted_bundles,
        "omitted_bundles": omitted_bundles,
        "effect_receipts": effect_receipts,
        "semantic_loss": semantic_loss,
        "reasons": reasons,
        "bundle_order": bundle_order,
        "source_digest_order": source_digest_order,
        "bundle_digest": bundle_digest,
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
        input: canonical_ingestion_gateway_request(request),
        input_digest: ingestion_gateway_input_digest(request)?,
        request_id: request.request_id.clone(),
        study_id: request.study_id.clone(),
        reference_schema: request.reference_schema.clone(),
        required_modalities,
        policy_allow: request.policy_allow,
        authorization_reference: request.authorization_reference.clone(),
        decision,
        harmonized,
        bundle_order,
        source_digest_order,
        bundle_digest,
        admitted_bundles,
        omitted_bundles,
        effect_receipts,
        semantic_loss,
        reasons,
        artifact,
        raw_data_local: request.raw_data_local,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    Ok(receipt)
}

fn validate_request(request: &IngestionGatewayRequest) -> Result<(), IngestionGatewayError> {
    if request.request_id.trim().is_empty()
        || request.study_id.trim().is_empty()
        || request.reference_schema.trim().is_empty()
        || request.bundles.is_empty()
        || request.bundles.len() > MAX_BUNDLES
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
    validate_text("request_id", &request.request_id)?;
    validate_text("study_id", &request.study_id)?;
    validate_text("reference_schema", &request.reference_schema)?;
    validate_text("boundary", &request.boundary)?;
    validate_unique_strings("required_modalities", &request.required_modalities)?;
    if let Some(authorization) = &request.authorization_reference {
        validate_text("authorization_reference", authorization)?;
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
        validate_text("bundle.bundle_id", &bundle.bundle_id)?;
        validate_text("bundle.modality_id", &bundle.modality_id)?;
        validate_text("bundle.modality_type", &bundle.modality_type)?;
        validate_text("bundle.schema_version", &bundle.schema_version)?;
        validate_text("bundle.instrument_id", &bundle.instrument_id)?;
        validate_text("bundle.acquisition_label", &bundle.acquisition_label)?;
        validate_digest("bundle.source_digest", &bundle.source_digest)?;
        validate_unique_strings("bundle.feature_names", &bundle.feature_names)?;
        for (key, value) in &bundle.units {
            validate_text("bundle.units.key", key)?;
            validate_text("bundle.units.value", value)?;
        }
        if let Some(coordinate_system) = &bundle.coordinate_system {
            validate_text("bundle.coordinate_system", coordinate_system)?;
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
            feature_names: vec!["z".into(), "a".into()],
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

    #[test]
    fn descriptor_feature_order_is_canonicalized() {
        let mut reversed = request();
        reversed.bundles[0].feature_names.reverse();
        assert_eq!(
            run_ingestion_gateway(&request()).unwrap().digest().unwrap(),
            run_ingestion_gateway(&reversed).unwrap().digest().unwrap()
        );
    }

    #[test]
    fn duplicate_feature_name_is_rejected() {
        let mut value = request();
        value.bundles[0].feature_names.push("a".into());
        assert!(run_ingestion_gateway(&value).is_err());
    }

    #[test]
    fn receipt_artifact_tampering_is_rejected() {
        let mut receipt = run_ingestion_gateway(&request()).unwrap();
        receipt.bundle_digest = ContentHash::of_bytes(b"tampered-bundles");
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn retained_request_tampering_is_rejected() {
        let mut receipt = run_ingestion_gateway(&request()).unwrap();
        receipt.input.reference_schema = "schema:tampered".into();
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn blank_authorization_reference_is_rejected() {
        let mut value = request();
        value.authorization_reference = Some(" ".into());
        assert!(run_ingestion_gateway(&value).is_err());
    }
}

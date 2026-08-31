//! Federated continual evidence API/protocol adapter.
//!
//! Atlas feature: `AFA-brain-P01-F24`. The adapter produces a canonical aggregate-only
//! response for a governed transport; it never opens a socket or exports raw observations.

use crate::federated_evidence_surveillance::{
    admit_federated_evidence, FederatedEvidenceDisposition, FederatedEvidenceFeedRequest,
};
use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-brain-P01-F24";
pub const CONTRACT_VERSION: &str = "brain-federated-protocol-adapter/1.0";
pub const PROTOCOL_VERSION: &str = "aurora-research-federated/1.0";
pub const ROUTE: &str = "/v1/research/evidence/federated/admit";
pub const METHOD: &str = "POST";
pub const RESPONSE_SCHEMA: &str = "FederatedEvidenceProtocolResponse1@1";
const PROTOCOL_CONTENT_TYPE: &str = "application/vnd.aurora.federated-protocol-response+json";
const MAX_TEXT_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedProtocolRequest {
    pub request: FederatedEvidenceFeedRequest,
    pub protocol_version: String,
    pub method: String,
    pub route: String,
    pub content_type: String,
    pub idempotency_key: String,
    pub response_schema: String,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signer_valid: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedProtocolReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub protocol_version: String,
    pub method: String,
    pub route: String,
    pub content_type: String,
    pub idempotency_key: String,
    pub response_schema: String,
    pub status_code: u16,
    pub disposition: FederatedEvidenceDisposition,
    pub federation_id: String,
    pub institution_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub endpoint: String,
    pub candidate_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub aggregate_order: Vec<ContentHash>,
    pub envelope_digest: ContentHash,
    pub request_digest: ContentHash,
    pub response_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedProtocolError {
    #[error("invalid federated protocol request: {0}")]
    Invalid(String),
    #[error("federated protocol artifact failed: {0}")]
    Artifact(String),
    #[error("federated protocol engine failed: {0}")]
    Engine(String),
}

impl FederatedProtocolReceipt {
    pub fn validate(&self) -> Result<(), FederatedProtocolError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.protocol_version != PROTOCOL_VERSION
            || self.method != METHOD
            || self.route != ROUTE
            || self.response_schema != RESPONSE_SCHEMA
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.content_type != "application/json"
            || self.candidate_order.is_empty()
            || self.effect_receipts.len() != 1
            || !self.raw_data_local
        {
            return Err(FederatedProtocolError::Invalid(
                "federated protocol identity, envelope, locality, or effects are incomplete".into(),
            ));
        }
        for (value, field) in [
            (&self.request_id, "request_id"),
            (&self.idempotency_key, "idempotency_key"),
            (&self.federation_id, "federation_id"),
            (&self.institution_id, "institution_id"),
            (&self.purpose, "purpose"),
            (&self.semantic_profile, "semantic_profile"),
            (&self.endpoint, "endpoint"),
            (&self.boundary, "boundary"),
        ] {
            validate_text(value, field)?;
        }
        for values in [
            &self.candidate_order,
            &self.admitted_order,
            &self.blocked_order,
            &self.unknown_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            validate_sorted_unique(values, "federated protocol collection")?;
        }
        let candidate_keys = identity_keys(&self.candidate_order);
        let admitted_keys = identity_keys(&self.admitted_order);
        let blocked_keys = identity_keys(&self.blocked_order);
        let unknown_keys = identity_keys(&self.unknown_order);
        if admitted_keys
            .union(&blocked_keys)
            .cloned()
            .collect::<BTreeSet<_>>()
            != candidate_keys
            || !admitted_keys.is_disjoint(&blocked_keys)
            || !unknown_keys.is_subset(&blocked_keys)
            || self.aggregate_order.len() != self.admitted_order.len()
        {
            return Err(FederatedProtocolError::Invalid(
                "federated protocol state is not a disjoint candidate partition".into(),
            ));
        }
        if self
            .aggregate_order
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
            || self
                .aggregate_order
                .iter()
                .any(|value| value.as_str().len() != 64)
        {
            return Err(FederatedProtocolError::Invalid(
                "federated aggregate ordering or digest is invalid".into(),
            ));
        }
        let expected_status = match self.disposition {
            FederatedEvidenceDisposition::Qualified => 200,
            FederatedEvidenceDisposition::Partial => 206,
            FederatedEvidenceDisposition::Unknown => 202,
            FederatedEvidenceDisposition::Blocked => {
                if self.status_code == 403 {
                    403
                } else {
                    422
                }
            }
        };
        if self.status_code != expected_status {
            return Err(FederatedProtocolError::Invalid(
                "federated protocol status does not match disposition".into(),
            ));
        }
        if self
            .omissions
            .iter()
            .any(|item| item == "protocol:raw-data-locality-failed")
            && self.disposition != FederatedEvidenceDisposition::Blocked
        {
            return Err(FederatedProtocolError::Invalid(
                "non-local federated responses must be blocked and retain locality evidence".into(),
            ));
        }
        let expected_effect = if self.disposition != FederatedEvidenceDisposition::Blocked
            && !self.admitted_order.is_empty()
        {
            format!("protocol:federated-response:{}", self.idempotency_key)
        } else {
            "block:unsafe-release".into()
        };
        if self.effect_receipts != [expected_effect] {
            return Err(FederatedProtocolError::Invalid(
                "federated protocol effect does not match disposition and admission".into(),
            ));
        }
        for digest in [
            &self.envelope_digest,
            &self.request_digest,
            &self.response_digest,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(FederatedProtocolError::Invalid(
                    "federated protocol digest is invalid".into(),
                ));
            }
        }
        let expected_envelope_digest = ContentHash::of_value(&json!({
            "federation_id": self.federation_id,
            "institution_id": self.institution_id,
            "purpose": self.purpose,
            "semantic_profile": self.semantic_profile,
            "candidate_order": self.candidate_order,
            "aggregate_order": self.aggregate_order,
            "replay_identity": self.replay_identity,
            "raw_data_local": self.raw_data_local,
        }))
        .map_err(|error| FederatedProtocolError::Artifact(error.to_string()))?;
        if self.envelope_digest != expected_envelope_digest {
            return Err(FederatedProtocolError::Invalid(
                "federated protocol envelope digest is not bound to response state".into(),
            ));
        }
        let expected_response_digest = ContentHash::of_value(&json!({
            "protocol_version": PROTOCOL_VERSION,
            "route": ROUTE,
            "request_id": self.request_id,
            "idempotency_key": self.idempotency_key,
            "status_code": self.status_code,
            "disposition": self.disposition,
            "federation_id": self.federation_id,
            "institution_id": self.institution_id,
            "aggregate_order": self.aggregate_order,
            "replay_identity": self.replay_identity,
            "raw_data_local": self.raw_data_local,
        }))
        .map_err(|error| FederatedProtocolError::Artifact(error.to_string()))?;
        if self.response_digest != expected_response_digest {
            return Err(FederatedProtocolError::Invalid(
                "federated protocol response digest is not bound to response state".into(),
            ));
        }
        let expected_artifact_id = format!("brain-federated-protocol:{}", self.request_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != PROTOCOL_CONTENT_TYPE
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(FederatedProtocolError::Invalid(
                "federated protocol artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| FederatedProtocolError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
            .map_err(|error| FederatedProtocolError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, FederatedProtocolError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| FederatedProtocolError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| FederatedProtocolError::Artifact(error.to_string()))
    }
}

fn validate_text(value: &str, field: &str) -> Result<(), FederatedProtocolError> {
    if value.trim() != value
        || value.is_empty()
        || value.len() > MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(FederatedProtocolError::Invalid(format!(
            "{field} is empty, padded, oversized, or contains control characters"
        )));
    }
    Ok(())
}

fn validate_unique(values: &[String], field: &str) -> Result<(), FederatedProtocolError> {
    let mut seen = BTreeSet::new();
    for value in values {
        validate_text(value, field)?;
        if !seen.insert(value.to_ascii_lowercase()) {
            return Err(FederatedProtocolError::Invalid(format!(
                "{field} contains a duplicate or case-colliding identity"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_unique(values: &[String], field: &str) -> Result<(), FederatedProtocolError> {
    validate_unique(values, field)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(FederatedProtocolError::Invalid(format!(
            "{field} is not in canonical order"
        )));
    }
    Ok(())
}

fn identity_keys(values: &[String]) -> BTreeSet<String> {
    values
        .iter()
        .map(|value| value.to_ascii_lowercase())
        .collect()
}

fn receipt_payload(receipt: &FederatedProtocolReceipt) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "request_id": receipt.request_id,
        "protocol_version": receipt.protocol_version,
        "method": receipt.method,
        "route": receipt.route,
        "content_type": receipt.content_type,
        "idempotency_key": receipt.idempotency_key,
        "response_schema": receipt.response_schema,
        "status_code": receipt.status_code,
        "disposition": receipt.disposition,
        "federation_id": receipt.federation_id,
        "institution_id": receipt.institution_id,
        "purpose": receipt.purpose,
        "semantic_profile": receipt.semantic_profile,
        "endpoint": receipt.endpoint,
        "candidate_order": receipt.candidate_order,
        "admitted_order": receipt.admitted_order,
        "blocked_order": receipt.blocked_order,
        "unknown_order": receipt.unknown_order,
        "aggregate_order": receipt.aggregate_order,
        "envelope_digest": receipt.envelope_digest,
        "request_digest": receipt.request_digest,
        "response_digest": receipt.response_digest,
        "replay_identity": receipt.replay_identity,
        "omissions": receipt.omissions,
        "uncertainty": receipt.uncertainty,
        "negative_evidence": receipt.negative_evidence,
        "effect_receipts": receipt.effect_receipts,
        "raw_data_local": receipt.raw_data_local,
        "boundary": receipt.boundary,
    })
}

pub fn federated_protocol_adapter_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["federated API gateway".into(), "consortium research SDK".into(), "federation steward".into()].into(), behavior: "maps a governed FederatedEvidenceFeed to a canonical aggregate-only protocol response with signer, purpose, locality, and replay gates".into(), value: "provides stable consortium API semantics while preventing raw-data movement and silent federation".into(), inputs: vec![TypedPort { name: "federated_protocol_request".into(), schema: "FederatedEvidenceProtocolRequest1@1".into(), required: true }], outputs: vec![TypedPort { name: "federated_protocol_response".into(), schema: RESPONSE_SCHEMA.into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["protocol:federated-response".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A0, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn serve_federated_protocol(
    request: &FederatedProtocolRequest,
) -> Result<FederatedProtocolReceipt, FederatedProtocolError> {
    validate_request(request)?;
    let evidence = admit_federated_evidence(&request.request)
        .map_err(|error| FederatedProtocolError::Engine(error.to_string()))?;
    let mut omissions = evidence.omissions.iter().cloned().collect::<BTreeSet<_>>();
    let uncertainty = evidence
        .uncertainty
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let negative = evidence
        .negative_evidence
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let outer_allowed = request.policy_allow
        && request.protected_closure
        && request.signer_valid
        && request.raw_data_local;
    if !request.policy_allow {
        omissions.insert("protocol:policy-denied".into());
    }
    if !request.protected_closure {
        omissions.insert("protocol:protected-closure-incomplete".into());
    }
    if !request.signer_valid {
        omissions.insert("protocol:signer-invalid".into());
    }
    if !request.raw_data_local {
        omissions.insert("protocol:raw-data-locality-failed".into());
    }
    let disposition = if outer_allowed {
        evidence.disposition
    } else {
        FederatedEvidenceDisposition::Blocked
    };
    let status_code = if !outer_allowed {
        403
    } else {
        match disposition {
            FederatedEvidenceDisposition::Qualified => 200,
            FederatedEvidenceDisposition::Partial => 206,
            FederatedEvidenceDisposition::Unknown => 202,
            FederatedEvidenceDisposition::Blocked => 422,
        }
    };
    let effect_receipts = if disposition != FederatedEvidenceDisposition::Blocked
        && !evidence.admitted_order.is_empty()
    {
        vec![format!(
            "protocol:federated-response:{}",
            request.idempotency_key
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let request_value = serde_json::to_value(request)
        .map_err(|error| FederatedProtocolError::Artifact(error.to_string()))?;
    let request_digest = ContentHash::of_value(&request_value)
        .map_err(|error| FederatedProtocolError::Artifact(error.to_string()))?;
    let raw_data_local = true;
    let envelope_digest = ContentHash::of_value(&json!({"federation_id": request.request.federation_id, "institution_id": request.request.institution_id, "purpose": request.request.purpose, "semantic_profile": request.request.semantic_profile, "candidate_order": evidence.candidate_order, "aggregate_order": evidence.aggregate_order, "replay_identity": request.replay_identity, "raw_data_local": raw_data_local}))
        .map_err(|error| FederatedProtocolError::Artifact(error.to_string()))?;
    let response_digest = ContentHash::of_value(&json!({"protocol_version": PROTOCOL_VERSION, "route": ROUTE, "request_id": request.request.request_id, "idempotency_key": request.idempotency_key, "status_code": status_code, "disposition": disposition, "federation_id": request.request.federation_id, "institution_id": request.request.institution_id, "aggregate_order": evidence.aggregate_order, "replay_identity": request.replay_identity, "raw_data_local": raw_data_local}))
        .map_err(|error| FederatedProtocolError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request.request_id, "protocol_version": PROTOCOL_VERSION, "method": METHOD, "route": ROUTE, "content_type": "application/json", "idempotency_key": request.idempotency_key, "response_schema": RESPONSE_SCHEMA, "status_code": status_code, "disposition": disposition, "federation_id": request.request.federation_id, "institution_id": request.request.institution_id, "purpose": request.request.purpose, "semantic_profile": request.request.semantic_profile, "endpoint": request.request.endpoint, "candidate_order": evidence.candidate_order, "admitted_order": evidence.admitted_order, "blocked_order": evidence.blocked_order, "unknown_order": evidence.unknown_order, "aggregate_order": evidence.aggregate_order, "envelope_digest": envelope_digest, "request_digest": request_digest, "response_digest": response_digest, "replay_identity": request.replay_identity, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "effect_receipts": effect_receipts, "raw_data_local": raw_data_local, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-federated-protocol:{}", request.request.request_id),
        PROTOCOL_CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| FederatedProtocolError::Artifact(error.to_string()))?;
    let receipt = FederatedProtocolReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request.request_id.clone(),
        protocol_version: PROTOCOL_VERSION.into(),
        method: METHOD.into(),
        route: ROUTE.into(),
        content_type: "application/json".into(),
        idempotency_key: request.idempotency_key.clone(),
        response_schema: RESPONSE_SCHEMA.into(),
        status_code,
        disposition,
        federation_id: request.request.federation_id.clone(),
        institution_id: request.request.institution_id.clone(),
        purpose: request.request.purpose.clone(),
        semantic_profile: request.request.semantic_profile.clone(),
        endpoint: request.request.endpoint.clone(),
        candidate_order: evidence.candidate_order.clone(),
        admitted_order: evidence.admitted_order.clone(),
        blocked_order: evidence.blocked_order.clone(),
        unknown_order: evidence.unknown_order.clone(),
        aggregate_order: evidence.aggregate_order.clone(),
        envelope_digest,
        request_digest,
        response_digest,
        replay_identity: request.replay_identity.clone(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts,
        artifact,
        raw_data_local,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &FederatedProtocolRequest) -> Result<(), FederatedProtocolError> {
    if request.protocol_version != PROTOCOL_VERSION
        || request.method != METHOD
        || request.route != ROUTE
        || request.content_type != "application/json"
        || request.idempotency_key.trim().is_empty()
        || request.response_schema != RESPONSE_SCHEMA
        || request.replay_identity != request.request.replay_identity
        || request.policy_allow != request.request.policy_allow
        || request.protected_closure != request.request.protected_closure
        || request.signer_valid != request.request.signer_valid
        || request.raw_data_local != request.request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(FederatedProtocolError::Invalid("federated protocol version, route, idempotency, replay, authority, locality, or boundary is incomplete".into()));
    }
    for (value, field) in [
        (&request.request.request_id, "request_id"),
        (&request.idempotency_key, "idempotency_key"),
        (&request.request.federation_id, "federation_id"),
        (&request.request.institution_id, "institution_id"),
        (&request.request.purpose, "purpose"),
        (&request.request.semantic_profile, "semantic_profile"),
        (&request.request.endpoint, "endpoint"),
        (&request.protocol_version, "protocol_version"),
        (&request.method, "method"),
        (&request.route, "route"),
        (&request.content_type, "content_type"),
        (&request.response_schema, "response_schema"),
        (&request.boundary, "boundary"),
    ] {
        validate_text(value, field)?;
    }
    validate_unique(&request.request.allowed_artifacts, "allowed_artifacts")?;
    if request.replay_identity.as_str().len() != 64 {
        return Err(FederatedProtocolError::Invalid(
            "federated protocol replay digest is invalid".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence_surveillance::EvidenceObservation;

    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }

    fn observation(id: &str, state: EvidenceState) -> EvidenceObservation {
        EvidenceObservation {
            evidence_id: format!("evidence:{id}"),
            source_id: format!("source:{id}"),
            study_id: "study:organoid".into(),
            modality: "imaging".into(),
            scope: "organoid:neural".into(),
            relevance_milli: 900,
            state,
            semantic_digest: hash(&format!("semantic:{id}")),
            artifact_digest: hash(&format!("artifact:{id}")),
            provenance_digest: hash(&format!("provenance:{id}")),
            replay_identity: hash("replay"),
            omissions: Vec::new(),
            negative_evidence: Vec::new(),
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    fn request(state: EvidenceState) -> FederatedProtocolRequest {
        let inner = FederatedEvidenceFeedRequest {
            request_id: "request:federated-protocol".into(),
            federation_id: "federation:commons".into(),
            institution_id: "institution:a".into(),
            purpose: "benchmarking".into(),
            semantic_profile: "preclinical-evidence/v1".into(),
            endpoint: "https://hub.example/research".into(),
            allowed_artifacts: vec!["qualified-evidence-summary".into()],
            observations: vec![observation("a", state)],
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            signer_valid: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        };
        FederatedProtocolRequest {
            request: inner,
            protocol_version: PROTOCOL_VERSION.into(),
            method: METHOD.into(),
            route: ROUTE.into(),
            content_type: "application/json".into(),
            idempotency_key: "idem:federated".into(),
            response_schema: RESPONSE_SCHEMA.into(),
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            signer_valid: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn manifest_is_a0_and_aggregate_only() {
        let manifest = federated_protocol_adapter_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A0);
        assert!(manifest.permissions.contains("protocol:federated-response"));
    }

    #[test]
    fn qualified_returns_200() {
        let receipt = serve_federated_protocol(&request(EvidenceState::Supported)).unwrap();
        assert_eq!(receipt.status_code, 200);
        assert_eq!(receipt.disposition, FederatedEvidenceDisposition::Qualified);
        assert!(!receipt.aggregate_order.is_empty());
    }

    #[test]
    fn unknown_returns_202() {
        let receipt = serve_federated_protocol(&request(EvidenceState::Unknown)).unwrap();
        assert_eq!(receipt.status_code, 202);
        assert_eq!(receipt.disposition, FederatedEvidenceDisposition::Unknown);
    }

    #[test]
    fn signer_returns_403() {
        let mut input = request(EvidenceState::Supported);
        input.signer_valid = false;
        input.request.signer_valid = false;
        let receipt = serve_federated_protocol(&input).unwrap();
        assert_eq!(receipt.status_code, 403);
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }

    #[test]
    fn state_lists_reject_cross_state_overlap() {
        let mut receipt = serve_federated_protocol(&request(EvidenceState::Supported)).unwrap();
        receipt.blocked_order = receipt.admitted_order.clone();
        let error = receipt
            .validate()
            .expect_err("a candidate cannot be both admitted and blocked");
        assert!(error.to_string().contains("state"));
    }

    #[test]
    fn locality_failure_is_blocked_and_retained() {
        let mut input = request(EvidenceState::Supported);
        input.raw_data_local = false;
        input.request.raw_data_local = false;
        let receipt = serve_federated_protocol(&input).unwrap();
        assert_eq!(receipt.disposition, FederatedEvidenceDisposition::Blocked);
        assert!(receipt.raw_data_local);
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item == "protocol:raw-data-locality-failed"));
        receipt.validate().unwrap();
    }

    #[test]
    fn response_and_payload_drift_are_rejected() {
        let receipt = serve_federated_protocol(&request(EvidenceState::Supported)).unwrap();
        let mut response_drift = receipt.clone();
        response_drift.status_code = 206;
        assert!(response_drift.validate().is_err());

        let mut payload_drift = receipt;
        payload_drift.endpoint = "https://other.example/research".into();
        assert!(payload_drift.validate().is_err());
    }

    #[test]
    fn padded_idempotency_identity_is_rejected() {
        let mut input = request(EvidenceState::Supported);
        input.idempotency_key = " idem:federated".into();
        assert!(serve_federated_protocol(&input).is_err());
    }

    #[test]
    fn route_is_versioned() {
        let mut input = request(EvidenceState::Supported);
        input.route = "/v1/other".into();
        assert!(serve_federated_protocol(&input).is_err());
    }

    #[test]
    fn digest_is_stable() {
        let receipt = serve_federated_protocol(&request(EvidenceState::Supported)).unwrap();
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
}

//! Federated continual evidence API/protocol adapter.
//!
//! Atlas feature: `AFA-brain-P01-F24`. The adapter produces a canonical aggregate-only
//! response for a governed transport; it never opens a socket or exports raw observations.

use crate::evidence_surveillance::EvidenceObservation;
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
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.content_type != "application/json"
            || self.idempotency_key.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.institution_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.endpoint.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(FederatedProtocolError::Invalid(
                "federated protocol identity, envelope, locality, or effects are incomplete".into(),
            ));
        }
        if self
            .admitted_order
            .iter()
            .chain(self.blocked_order.iter())
            .chain(self.unknown_order.iter())
            .any(|id| !self.candidate_order.contains(id))
        {
            return Err(FederatedProtocolError::Invalid(
                "federated protocol state is not covered by candidates".into(),
            ));
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
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(FederatedProtocolError::Invalid(
                    "federated protocol ordering is not canonical".into(),
                ));
            }
        }
        if self
            .aggregate_order
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(FederatedProtocolError::Invalid(
                "federated aggregate ordering is not canonical".into(),
            ));
        }
        if !matches!(self.status_code, 200 | 202 | 206 | 403 | 422) {
            return Err(FederatedProtocolError::Invalid(
                "federated protocol status code is invalid".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("protocol:federated-response:") && effect != "block:unsafe-release"
        }) {
            return Err(FederatedProtocolError::Invalid(
                "federated protocol effect is outside the governed response gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
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
    let request_value = serde_json::to_value(request)
        .map_err(|error| FederatedProtocolError::Artifact(error.to_string()))?;
    let request_digest = ContentHash::of_value(&request_value)
        .map_err(|error| FederatedProtocolError::Artifact(error.to_string()))?;
    let envelope_digest = ContentHash::of_value(&json!({"federation_id": request.request.federation_id, "institution_id": request.request.institution_id, "purpose": request.request.purpose, "semantic_profile": request.request.semantic_profile, "candidate_order": evidence.candidate_order, "aggregate_order": evidence.aggregate_order, "replay_identity": request.replay_identity}))
        .map_err(|error| FederatedProtocolError::Artifact(error.to_string()))?;
    let response_digest = ContentHash::of_value(&json!({"protocol_version": PROTOCOL_VERSION, "route": ROUTE, "request_id": request.request.request_id, "idempotency_key": request.idempotency_key, "status_code": status_code, "disposition": disposition, "federation_id": request.request.federation_id, "institution_id": request.request.institution_id, "aggregate_order": evidence.aggregate_order, "replay_identity": request.replay_identity}))
        .map_err(|error| FederatedProtocolError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request.request_id, "protocol_version": PROTOCOL_VERSION, "method": METHOD, "route": ROUTE, "content_type": "application/json", "idempotency_key": request.idempotency_key, "response_schema": RESPONSE_SCHEMA, "status_code": status_code, "disposition": disposition, "federation_id": request.request.federation_id, "institution_id": request.request.institution_id, "purpose": request.request.purpose, "semantic_profile": request.request.semantic_profile, "endpoint": request.request.endpoint, "candidate_order": evidence.candidate_order, "admitted_order": evidence.admitted_order, "blocked_order": evidence.blocked_order, "unknown_order": evidence.unknown_order, "aggregate_order": evidence.aggregate_order, "envelope_digest": envelope_digest, "request_digest": request_digest, "response_digest": response_digest, "replay_identity": request.replay_identity, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-federated-protocol:{}", request.request.request_id),
        "application/vnd.aurora.federated-protocol-response+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| FederatedProtocolError::Artifact(error.to_string()))?;
    let has_admitted = outer_allowed && !evidence.admitted_order.is_empty();
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
        effect_receipts: if has_admitted {
            vec![format!(
                "protocol:federated-response:{}",
                request.idempotency_key
            )]
        } else {
            vec!["block:unsafe-release".into()]
        },
        artifact,
        raw_data_local: true,
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
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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

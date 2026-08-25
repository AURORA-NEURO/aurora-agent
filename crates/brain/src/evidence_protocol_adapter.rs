//! Local evidence API/protocol adapter.
//!
//! Atlas feature: `AFA-brain-P01-F21`. This adapter exposes the evidence engine through a
//! versioned, idempotent local protocol. It does not open sockets or move data; an owning
//! runtime can transport the canonical request and response bytes.

use crate::evidence_surveillance::{
    surveil_evidence, EvidenceFeedRequest, EvidenceSurveillanceDisposition,
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

pub const FEATURE_ID: &str = "AFA-brain-P01-F21";
pub const CONTRACT_VERSION: &str = "brain-evidence-protocol-adapter/1.0";
pub const PROTOCOL_VERSION: &str = "aurora-research/1.0";
pub const ROUTE: &str = "/v1/research/evidence/surveil";
pub const METHOD: &str = "POST";
pub const RESPONSE_SCHEMA: &str = "EvidenceProtocolResponse1@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceProtocolRequest {
    pub request: EvidenceFeedRequest,
    pub protocol_version: String,
    pub method: String,
    pub route: String,
    pub content_type: String,
    pub idempotency_key: String,
    pub response_schema: String,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceProtocolReceipt {
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
    pub disposition: EvidenceSurveillanceDisposition,
    pub candidate_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub evidence_digest: ContentHash,
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
pub enum EvidenceProtocolError {
    #[error("invalid evidence protocol request: {0}")]
    Invalid(String),
    #[error("evidence protocol artifact failed: {0}")]
    Artifact(String),
    #[error("evidence protocol engine failed: {0}")]
    Engine(String),
}

impl EvidenceProtocolReceipt {
    pub fn validate(&self) -> Result<(), EvidenceProtocolError> {
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
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(EvidenceProtocolError::Invalid("protocol identity, route, idempotency, evidence, locality, or effects are incomplete".into()));
        }
        if self
            .qualified_order
            .iter()
            .chain(self.blocked_order.iter())
            .chain(self.unknown_order.iter())
            .any(|id| !self.candidate_order.contains(id))
        {
            return Err(EvidenceProtocolError::Invalid(
                "protocol evidence state is not covered by candidates".into(),
            ));
        }
        for values in [
            &self.candidate_order,
            &self.qualified_order,
            &self.blocked_order,
            &self.unknown_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(EvidenceProtocolError::Invalid(
                    "protocol ordering is not canonical".into(),
                ));
            }
        }
        if !matches!(self.status_code, 200 | 202 | 206 | 403 | 422) {
            return Err(EvidenceProtocolError::Invalid(
                "protocol status code is outside the versioned contract".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("protocol:local-response:") && effect != "block:unsafe-release"
        }) {
            return Err(EvidenceProtocolError::Invalid(
                "protocol effect is outside local response gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| EvidenceProtocolError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, EvidenceProtocolError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| EvidenceProtocolError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| EvidenceProtocolError::Artifact(error.to_string()))
    }
}

pub fn evidence_protocol_adapter_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["local API gateway".into(), "research SDK".into(), "protocol adapter steward".into()].into(), behavior: "maps a typed local EvidenceFeed request to a canonical versioned protocol response with idempotency and replay binding".into(), value: "lets local clients consume evidence surveillance through a stable API without external data movement".into(), inputs: vec![TypedPort { name: "evidence_protocol_request".into(), schema: "EvidenceProtocolRequest1@1".into(), required: true }], outputs: vec![TypedPort { name: "evidence_protocol_response".into(), schema: RESPONSE_SCHEMA.into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["protocol:local-response".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "mcp-basic-protocol".into(), state: EvidenceState::Supported, locator: Some("https://modelcontextprotocol.io/specification/2025-06-18/basic/index".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A0, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn serve_evidence_protocol(
    request: &EvidenceProtocolRequest,
) -> Result<EvidenceProtocolReceipt, EvidenceProtocolError> {
    validate_request(request)?;
    let evidence = surveil_evidence(&request.request)
        .map_err(|error| EvidenceProtocolError::Engine(error.to_string()))?;
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
    let outer_allowed = request.policy_allow && request.protected_closure && request.raw_data_local;
    if !request.policy_allow {
        omissions.insert("protocol:policy-denied".into());
    }
    if !request.protected_closure {
        omissions.insert("protocol:protected-closure-incomplete".into());
    }
    if !request.raw_data_local {
        omissions.insert("protocol:raw-data-locality-failed".into());
    }
    let disposition = if outer_allowed {
        evidence.disposition
    } else {
        EvidenceSurveillanceDisposition::Blocked
    };
    let status_code = if !outer_allowed {
        403
    } else {
        match disposition {
            EvidenceSurveillanceDisposition::Qualified => 200,
            EvidenceSurveillanceDisposition::Partial => 206,
            EvidenceSurveillanceDisposition::Unknown => 202,
            EvidenceSurveillanceDisposition::Blocked => 422,
        }
    };
    let request_value = serde_json::to_value(request)
        .map_err(|error| EvidenceProtocolError::Artifact(error.to_string()))?;
    let request_digest = ContentHash::of_value(&request_value)
        .map_err(|error| EvidenceProtocolError::Artifact(error.to_string()))?;
    let evidence_digest = evidence
        .digest()
        .map_err(|error| EvidenceProtocolError::Engine(error.to_string()))?;
    let response_digest = ContentHash::of_value(&json!({"protocol_version": PROTOCOL_VERSION, "route": ROUTE, "request_id": request.request.request_id, "idempotency_key": request.idempotency_key, "status_code": status_code, "disposition": disposition, "candidate_order": evidence.candidate_order, "replay_identity": request.replay_identity})).map_err(|error| EvidenceProtocolError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request.request_id, "protocol_version": PROTOCOL_VERSION, "method": METHOD, "route": ROUTE, "content_type": "application/json", "idempotency_key": request.idempotency_key, "response_schema": RESPONSE_SCHEMA, "status_code": status_code, "disposition": disposition, "candidate_order": evidence.candidate_order, "qualified_order": evidence.qualified_order, "blocked_order": evidence.blocked_order, "unknown_order": evidence.unknown_order, "evidence_digest": evidence_digest, "request_digest": request_digest, "response_digest": response_digest, "replay_identity": request.replay_identity, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-evidence-protocol:{}", request.request.request_id),
        "application/vnd.aurora.evidence-protocol-response+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| EvidenceProtocolError::Artifact(error.to_string()))?;
    let receipt = EvidenceProtocolReceipt {
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
        candidate_order: evidence.candidate_order.clone(),
        qualified_order: evidence.qualified_order.clone(),
        blocked_order: evidence.blocked_order.clone(),
        unknown_order: evidence.unknown_order.clone(),
        evidence_digest,
        request_digest,
        response_digest,
        replay_identity: request.replay_identity.clone(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts: if outer_allowed {
            vec![format!(
                "protocol:local-response:{}",
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

fn validate_request(request: &EvidenceProtocolRequest) -> Result<(), EvidenceProtocolError> {
    if request.protocol_version != PROTOCOL_VERSION
        || request.method != METHOD
        || request.route != ROUTE
        || request.content_type != "application/json"
        || request.idempotency_key.trim().is_empty()
        || request.response_schema != RESPONSE_SCHEMA
        || request.request.replay_identity != request.replay_identity
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(EvidenceProtocolError::Invalid("protocol version, route, idempotency, response schema, replay, or boundary is incomplete".into()));
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
    fn request(state: EvidenceState) -> EvidenceProtocolRequest {
        EvidenceProtocolRequest {
            request: EvidenceFeedRequest {
                request_id: "request:protocol".into(),
                study_id: "study:organoid".into(),
                scope: "organoid:neural".into(),
                query: "mechanism".into(),
                minimum_relevance_milli: 700,
                observations: vec![EvidenceObservation {
                    evidence_id: "evidence:a".into(),
                    source_id: "source:a".into(),
                    study_id: "study:organoid".into(),
                    modality: "imaging".into(),
                    scope: "organoid:neural".into(),
                    relevance_milli: 900,
                    state,
                    semantic_digest: hash("semantic"),
                    artifact_digest: hash("artifact"),
                    provenance_digest: hash("provenance"),
                    replay_identity: hash("replay"),
                    omissions: Vec::new(),
                    negative_evidence: Vec::new(),
                    raw_data_local: true,
                    boundary: PRECLINICAL_BOUNDARY.into(),
                }],
                replay_identity: hash("replay"),
                policy_allow: true,
                protected_closure: true,
                raw_data_local: true,
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            protocol_version: PROTOCOL_VERSION.into(),
            method: METHOD.into(),
            route: ROUTE.into(),
            content_type: "application/json".into(),
            idempotency_key: "idem:001".into(),
            response_schema: RESPONSE_SCHEMA.into(),
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a0_local() {
        let manifest = evidence_protocol_adapter_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A0);
    }
    #[test]
    fn qualified_request_returns_200() {
        let receipt = serve_evidence_protocol(&request(EvidenceState::Supported)).unwrap();
        assert_eq!(receipt.status_code, 200);
        assert!(receipt.effect_receipts[0].starts_with("protocol:"));
    }
    #[test]
    fn unknown_request_returns_202() {
        let receipt = serve_evidence_protocol(&request(EvidenceState::Unknown)).unwrap();
        assert_eq!(receipt.status_code, 202);
    }
    #[test]
    fn policy_denial_returns_403() {
        let mut input = request(EvidenceState::Supported);
        input.policy_allow = false;
        let receipt = serve_evidence_protocol(&input).unwrap();
        assert_eq!(receipt.status_code, 403);
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn invalid_route_is_rejected() {
        let mut input = request(EvidenceState::Supported);
        input.route = "/v1/other".into();
        assert!(serve_evidence_protocol(&input).is_err());
    }
    #[test]
    fn response_digest_is_stable() {
        let receipt = serve_evidence_protocol(&request(EvidenceState::Supported)).unwrap();
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
}

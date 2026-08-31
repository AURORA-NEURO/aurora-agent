//! Prospective high-throughput evidence API/protocol adapter.
//!
//! Atlas feature: `AFA-brain-P01-F23`. This module provides canonical local protocol
//! semantics for bounded batches; an application-owned gateway handles transport.

use crate::high_throughput_evidence_surveillance::{
    admit_high_throughput_evidence, HighThroughputDisposition, HighThroughputEvidenceFeedRequest,
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

pub const FEATURE_ID: &str = "AFA-brain-P01-F23";
pub const CONTRACT_VERSION: &str = "brain-throughput-protocol-adapter/1.0";
pub const PROTOCOL_VERSION: &str = "aurora-research-throughput/1.0";
pub const ROUTE: &str = "/v1/research/evidence/throughput/admit";
pub const METHOD: &str = "POST";
pub const RESPONSE_SCHEMA: &str = "ThroughputEvidenceProtocolResponse1@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputProtocolRequest {
    pub request: HighThroughputEvidenceFeedRequest,
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
pub struct ThroughputProtocolReceipt {
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
    pub disposition: HighThroughputDisposition,
    pub batch_id: String,
    pub partition: String,
    pub candidate_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub checkpoint_seq: u64,
    pub queue_digest: ContentHash,
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
pub enum ThroughputProtocolError {
    #[error("invalid throughput protocol request: {0}")]
    Invalid(String),
    #[error("throughput protocol artifact failed: {0}")]
    Artifact(String),
    #[error("throughput protocol engine failed: {0}")]
    Engine(String),
}

impl ThroughputProtocolReceipt {
    pub fn validate(&self) -> Result<(), ThroughputProtocolError> {
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
            || self.batch_id.trim().is_empty()
            || self.partition.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(ThroughputProtocolError::Invalid("throughput protocol identity, route, queue, idempotency, locality, or effects are incomplete".into()));
        }
        if self
            .admitted_order
            .iter()
            .chain(self.blocked_order.iter())
            .chain(self.unknown_order.iter())
            .any(|id| !self.candidate_order.contains(id))
        {
            return Err(ThroughputProtocolError::Invalid(
                "throughput protocol state is not covered by candidates".into(),
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
                return Err(ThroughputProtocolError::Invalid(
                    "throughput protocol ordering is not canonical".into(),
                ));
            }
        }
        if !matches!(self.status_code, 200 | 202 | 206 | 403 | 422) {
            return Err(ThroughputProtocolError::Invalid(
                "throughput protocol status code is invalid".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("protocol:local-throughput-response:")
                && effect != "block:unsafe-release"
        }) {
            return Err(ThroughputProtocolError::Invalid(
                "throughput protocol effect is outside local response gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ThroughputProtocolError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, ThroughputProtocolError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ThroughputProtocolError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ThroughputProtocolError::Artifact(error.to_string()))
    }
}

pub fn throughput_protocol_adapter_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["throughput API gateway".into(), "research operations SDK".into()].into(), behavior: "maps a bounded EvidenceFeed3 batch to a canonical queue/checkpoint protocol response with capacity omissions".into(), value: "provides stable high-throughput batch semantics without silent drops or raw-data export".into(), inputs: vec![TypedPort { name: "throughput_protocol_request".into(), schema: "ThroughputEvidenceProtocolRequest1@1".into(), required: true }], outputs: vec![TypedPort { name: "throughput_protocol_response".into(), schema: RESPONSE_SCHEMA.into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["protocol:local-throughput-response".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "cwl-v1.2".into(), state: EvidenceState::Supported, locator: Some("https://www.commonwl.org/specification/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A0, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn serve_throughput_protocol(
    request: &ThroughputProtocolRequest,
) -> Result<ThroughputProtocolReceipt, ThroughputProtocolError> {
    validate_request(request)?;
    let evidence = admit_high_throughput_evidence(&request.request)
        .map_err(|error| ThroughputProtocolError::Engine(error.to_string()))?;
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
        HighThroughputDisposition::Blocked
    };
    let status_code = if !outer_allowed {
        403
    } else {
        match disposition {
            HighThroughputDisposition::Qualified => 200,
            HighThroughputDisposition::Partial => 206,
            HighThroughputDisposition::Unknown => 202,
            HighThroughputDisposition::Blocked => 422,
        }
    };
    let request_value = serde_json::to_value(request)
        .map_err(|error| ThroughputProtocolError::Artifact(error.to_string()))?;
    let request_digest = ContentHash::of_value(&request_value)
        .map_err(|error| ThroughputProtocolError::Artifact(error.to_string()))?;
    let evidence_digest = evidence
        .digest()
        .map_err(|error| ThroughputProtocolError::Engine(error.to_string()))?;
    let response_digest = ContentHash::of_value(&json!({"protocol_version": PROTOCOL_VERSION, "route": ROUTE, "request_id": request.request.request_id, "idempotency_key": request.idempotency_key, "status_code": status_code, "disposition": disposition, "batch_id": request.request.batch_id, "partition": request.request.partition, "checkpoint_seq": evidence.checkpoint_seq, "queue_digest": evidence.queue_digest, "replay_identity": request.replay_identity})).map_err(|error| ThroughputProtocolError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request.request_id, "protocol_version": PROTOCOL_VERSION, "method": METHOD, "route": ROUTE, "content_type": "application/json", "idempotency_key": request.idempotency_key, "response_schema": RESPONSE_SCHEMA, "status_code": status_code, "disposition": disposition, "batch_id": request.request.batch_id, "partition": request.request.partition, "candidate_order": evidence.candidate_order, "admitted_order": evidence.admitted_order, "blocked_order": evidence.blocked_order, "unknown_order": evidence.unknown_order, "checkpoint_seq": evidence.checkpoint_seq, "queue_digest": evidence.queue_digest, "evidence_digest": evidence_digest, "request_digest": request_digest, "response_digest": response_digest, "replay_identity": request.replay_identity, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-throughput-protocol:{}", request.request.request_id),
        "application/vnd.aurora.throughput-protocol-response+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ThroughputProtocolError::Artifact(error.to_string()))?;
    let receipt = ThroughputProtocolReceipt {
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
        batch_id: request.request.batch_id.clone(),
        partition: request.request.partition.clone(),
        candidate_order: evidence.candidate_order.clone(),
        admitted_order: evidence.admitted_order.clone(),
        blocked_order: evidence.blocked_order.clone(),
        unknown_order: evidence.unknown_order.clone(),
        checkpoint_seq: evidence.checkpoint_seq,
        queue_digest: evidence.queue_digest.clone(),
        evidence_digest,
        request_digest,
        response_digest,
        replay_identity: request.replay_identity.clone(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts: if outer_allowed {
            vec![format!(
                "protocol:local-throughput-response:{}",
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

fn validate_request(request: &ThroughputProtocolRequest) -> Result<(), ThroughputProtocolError> {
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
        return Err(ThroughputProtocolError::Invalid("throughput protocol version, route, idempotency, response schema, replay, or boundary is incomplete".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence_surveillance::EvidenceObservation;
    fn hash(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn request(state: EvidenceState) -> ThroughputProtocolRequest {
        ThroughputProtocolRequest {
            request: HighThroughputEvidenceFeedRequest {
                request_id: "request:throughput-protocol".into(),
                batch_id: "batch:001".into(),
                partition: "partition:imaging".into(),
                max_items: 1,
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
            idempotency_key: "idem:throughput".into(),
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
        let m = throughput_protocol_adapter_manifest();
        m.validate().unwrap();
        assert_eq!(m.autonomy_tier, AutonomyTier::A0)
    }
    #[test]
    fn qualified_returns_200() {
        let r = serve_throughput_protocol(&request(EvidenceState::Supported)).unwrap();
        assert_eq!(r.status_code, 200)
    }
    #[test]
    fn unknown_returns_202() {
        let r = serve_throughput_protocol(&request(EvidenceState::Unknown)).unwrap();
        assert_eq!(r.status_code, 202)
    }
    #[test]
    fn policy_returns_403() {
        let mut q = request(EvidenceState::Supported);
        q.policy_allow = false;
        let r = serve_throughput_protocol(&q).unwrap();
        assert_eq!(r.status_code, 403)
    }
    #[test]
    fn route_is_versioned() {
        let mut q = request(EvidenceState::Supported);
        q.route = "/v1/other".into();
        assert!(serve_throughput_protocol(&q).is_err())
    }
    #[test]
    fn digest_stable() {
        let r = serve_throughput_protocol(&request(EvidenceState::Supported)).unwrap();
        assert_eq!(r.digest().unwrap(), r.digest().unwrap())
    }
}

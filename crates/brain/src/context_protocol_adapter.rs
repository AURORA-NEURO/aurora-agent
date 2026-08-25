//! Local context-compilation API/protocol adapter.
//!
//! Atlas feature: `AFA-brain-P03-F21`. The adapter exposes a versioned,
//! idempotent local protocol for Decision-Section context. It emits a typed
//! response and omission certificate; it does not open sockets or move raw data.

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

pub const FEATURE_ID: &str = "AFA-brain-P03-F21";
pub const CONTRACT_VERSION: &str = "brain-context-protocol-adapter/1.0";
pub const PROTOCOL_VERSION: &str = "aurora-research-context/1.0";
pub const ROUTE: &str = "/v1/research/context/compile";
pub const METHOD: &str = "POST";
pub const RESPONSE_SCHEMA: &str = "ContextProtocolResponse1@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextProtocolCandidate {
    pub context_id: String,
    pub context_digest: ContentHash,
    pub section_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub state: EvidenceState,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextProtocolRequest {
    pub request_id: String,
    pub query_id: String,
    pub study_id: String,
    pub scope: String,
    pub goal: String,
    pub required_context_order: Vec<String>,
    pub candidates: Vec<ContextProtocolCandidate>,
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
pub struct ContextProtocolReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub query_id: String,
    pub study_id: String,
    pub scope: String,
    pub protocol_version: String,
    pub method: String,
    pub route: String,
    pub content_type: String,
    pub idempotency_key: String,
    pub response_schema: String,
    pub status_code: u16,
    pub disposition: String,
    pub candidate_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub context_digest: ContentHash,
    pub section_digest: ContentHash,
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
pub enum ContextProtocolError {
    #[error("invalid context protocol request: {0}")]
    Invalid(String),
    #[error("context protocol artifact failed: {0}")]
    Artifact(String),
}

impl ContextProtocolReceipt {
    pub fn validate(&self) -> Result<(), ContextProtocolError> {
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
            || self.query_id.trim().is_empty()
            || self.study_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.content_type != "application/json"
            || self.idempotency_key.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
            || !matches!(self.disposition.as_str(), "ready" | "partial" | "unknown" | "blocked")
        {
            return Err(ContextProtocolError::Invalid("context protocol identity, route, idempotency, candidates, locality, or effects are incomplete".into()));
        }
        if self.qualified_order.iter().chain(self.blocked_order.iter()).chain(self.unknown_order.iter()).any(|id| !self.candidate_order.contains(id)) {
            return Err(ContextProtocolError::Invalid("context protocol state is not covered by candidates".into()));
        }
        for values in [&self.candidate_order, &self.qualified_order, &self.blocked_order, &self.unknown_order, &self.omissions, &self.uncertainty, &self.negative_evidence, &self.effect_receipts] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(ContextProtocolError::Invalid("context protocol ordering is not canonical".into()));
            }
        }
        if !matches!(self.status_code, 200 | 202 | 206 | 403 | 422) {
            return Err(ContextProtocolError::Invalid("context protocol status code is outside the versioned contract".into()));
        }
        for digest in [&self.context_digest, &self.section_digest, &self.request_digest, &self.response_digest, &self.replay_identity] {
            if digest.as_str().len() != 64 { return Err(ContextProtocolError::Invalid("context protocol digest is invalid".into())); }
        }
        if self.effect_receipts.iter().any(|effect| !effect.starts_with("protocol:local-context-response:") && effect != "block:unsafe-release") {
            return Err(ContextProtocolError::Invalid("context protocol effect is outside local response gate".into()));
        }
        self.artifact.validate_metadata().map_err(|error| ContextProtocolError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, ContextProtocolError> {
        self.validate()?;
        let value = serde_json::to_value(self).map_err(|error| ContextProtocolError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value).map_err(|error| ContextProtocolError::Artifact(error.to_string()))
    }
}

pub fn context_protocol_adapter_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["local HTTP gateway".into(), "research SDK".into(), "workflow operator".into()].into(), behavior: "maps a typed local DecisionQuery context request to a canonical versioned protocol response with omission and replay receipts".into(), value: "lets local clients consume certified Decision-Section context through stable HTTP, event, SDK, and protocol surfaces without external data movement".into(), inputs: vec![TypedPort { name: "context_protocol_request".into(), schema: "ContextProtocolRequest1@1".into(), required: true }], outputs: vec![TypedPort { name: "context_protocol_response".into(), schema: RESPONSE_SCHEMA.into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["protocol:local-context-response".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "mcp-basic-protocol".into(), state: EvidenceState::Supported, locator: Some("https://modelcontextprotocol.io/specification/2025-06-18/basic/index".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A0, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn serve_context_protocol(request: &ContextProtocolRequest) -> Result<ContextProtocolReceipt, ContextProtocolError> {
    validate_request(request)?;
    let required = request.required_context_order.iter().cloned().collect::<BTreeSet<_>>();
    if required.is_empty() || required.len() != request.required_context_order.len() || required.iter().any(|id| id.trim().is_empty()) {
        return Err(ContextProtocolError::Invalid("required context identifiers must be unique and non-empty".into()));
    }
    let mut candidate_map = std::collections::BTreeMap::new();
    for candidate in &request.candidates {
        if candidate_map.insert(candidate.context_id.clone(), candidate).is_some() { return Err(ContextProtocolError::Invalid("context protocol candidates must be unique".into())); }
    }
    let mut qualified = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for id in &required {
        let Some(candidate) = candidate_map.get(id) else { unknown.insert(id.clone()); omissions.insert(format!("context:{}:missing", id)); continue; };
        if !request.policy_allow || !request.protected_closure || !request.raw_data_local || !candidate.raw_data_local || candidate.boundary != PRECLINICAL_BOUNDARY {
            blocked.insert(id.clone()); omissions.insert(format!("context:{}:policy-locality-blocked", id));
        } else if candidate.replay_identity != request.replay_identity {
            unknown.insert(id.clone()); uncertainty.insert(format!("context:{}:replay-mismatch", id));
        } else {
            match candidate.state {
                EvidenceState::Proven | EvidenceState::Supported => qualified.insert(id.clone()),
                EvidenceState::Speculative | EvidenceState::Unknown => { unknown.insert(id.clone()); uncertainty.insert(format!("context:{}:evidence-uncertain", id)); false }
                EvidenceState::Contradicted => { blocked.insert(id.clone()); negative.insert(format!("context:{}:contradicted", id)); false }
            };
        }
    }
    let gates_open = request.policy_allow && request.protected_closure && request.raw_data_local;
    let disposition = if !gates_open { "blocked" } else if qualified.len() == required.len() { "ready" } else if !unknown.is_empty() { "unknown" } else { "partial" };
    let status_code = if !gates_open { 403 } else if disposition == "ready" { 200 } else if disposition == "partial" { 206 } else { 202 };
    if !request.policy_allow { omissions.insert("protocol:policy-denied".into()); }
    if !request.protected_closure { omissions.insert("protocol:protected-closure-incomplete".into()); }
    if !request.raw_data_local { omissions.insert("protocol:raw-data-locality-failed".into()); }
    let context_digest = ContentHash::of_value(&json!({"required_order": required, "qualified_order": qualified, "replay_identity": request.replay_identity})).map_err(|error| ContextProtocolError::Artifact(error.to_string()))?;
    let section_digest = ContentHash::of_value(&json!({"study_id": request.study_id, "scope": request.scope, "context_digest": context_digest, "qualified_order": qualified})).map_err(|error| ContextProtocolError::Artifact(error.to_string()))?;
    let request_value = serde_json::to_value(request).map_err(|error| ContextProtocolError::Artifact(error.to_string()))?;
    let request_digest = ContentHash::of_value(&request_value).map_err(|error| ContextProtocolError::Artifact(error.to_string()))?;
    let response_digest = ContentHash::of_value(&json!({"protocol_version": PROTOCOL_VERSION, "route": ROUTE, "request_id": request.request_id, "status_code": status_code, "disposition": disposition, "context_digest": context_digest, "section_digest": section_digest, "replay_identity": request.replay_identity})).map_err(|error| ContextProtocolError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "protocol_version": PROTOCOL_VERSION, "method": METHOD, "route": ROUTE, "response_schema": RESPONSE_SCHEMA, "status_code": status_code, "disposition": disposition, "context_digest": context_digest, "section_digest": section_digest, "request_digest": request_digest, "response_digest": response_digest, "replay_identity": request.replay_identity, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(format!("brain-context-protocol:{}", request.request_id), "application/vnd.aurora.context-protocol-response+json", &payload, Vec::new(), Vec::new()).map_err(|error| ContextProtocolError::Artifact(error.to_string()))?;
    let receipt = ContextProtocolReceipt { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), contract_version: CONTRACT_VERSION.into(), feature_id: FEATURE_ID.into(), request_id: request.request_id.clone(), query_id: request.query_id.clone(), study_id: request.study_id.clone(), scope: request.scope.clone(), protocol_version: PROTOCOL_VERSION.into(), method: METHOD.into(), route: ROUTE.into(), content_type: "application/json".into(), idempotency_key: request.idempotency_key.clone(), response_schema: RESPONSE_SCHEMA.into(), status_code, disposition: disposition.into(), candidate_order: required.iter().cloned().collect(), qualified_order: qualified.into_iter().collect(), blocked_order: blocked.into_iter().collect(), unknown_order: unknown.into_iter().collect(), context_digest, section_digest, request_digest, response_digest, replay_identity: request.replay_identity.clone(), omissions: omissions.into_iter().collect(), uncertainty: uncertainty.into_iter().collect(), negative_evidence: negative.into_iter().collect(), effect_receipts: if gates_open { vec![format!("protocol:local-context-response:{}", request.idempotency_key)] } else { vec!["block:unsafe-release".into()] }, artifact, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY.into() };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &ContextProtocolRequest) -> Result<(), ContextProtocolError> {
    if request.protocol_version != PROTOCOL_VERSION || request.method != METHOD || request.route != ROUTE || request.content_type != "application/json" || request.idempotency_key.trim().is_empty() || request.response_schema != RESPONSE_SCHEMA || request.boundary != PRECLINICAL_BOUNDARY || request.replay_identity.as_str().len() != 64 {
        return Err(ContextProtocolError::Invalid("protocol version, route, idempotency, response schema, replay, or boundary is incomplete".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash { ContentHash::of_bytes(value.as_bytes()) }
    fn request(state: EvidenceState) -> ContextProtocolRequest {
        let replay = hash("context-protocol-replay");
        ContextProtocolRequest { request_id: "request:context-protocol".into(), query_id: "query:context".into(), study_id: "study:organoid".into(), scope: "organoid:neural".into(), goal: "compile decision context".into(), required_context_order: vec!["context:a".into()], candidates: vec![ContextProtocolCandidate { context_id: "context:a".into(), context_digest: replay.clone(), section_digest: replay.clone(), replay_identity: replay.clone(), state, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY.into() }], protocol_version: PROTOCOL_VERSION.into(), method: METHOD.into(), route: ROUTE.into(), content_type: "application/json".into(), idempotency_key: "idem:context-001".into(), response_schema: RESPONSE_SCHEMA.into(), replay_identity: replay, policy_allow: true, protected_closure: true, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY.into() }
    }
    #[test] fn manifest_is_a0_local() { let manifest = context_protocol_adapter_manifest(); manifest.validate().unwrap(); assert_eq!(manifest.autonomy_tier, AutonomyTier::A0); }
    #[test] fn supported_context_returns_200() { let receipt = serve_context_protocol(&request(EvidenceState::Supported)).unwrap(); assert_eq!(receipt.status_code, 200); assert_eq!(receipt.disposition, "ready"); }
    #[test] fn unknown_context_returns_202() { let receipt = serve_context_protocol(&request(EvidenceState::Unknown)).unwrap(); assert_eq!(receipt.status_code, 202); }
    #[test] fn policy_denial_returns_403() { let mut input = request(EvidenceState::Supported); input.policy_allow = false; let receipt = serve_context_protocol(&input).unwrap(); assert_eq!(receipt.status_code, 403); assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]); }
    #[test] fn invalid_route_is_rejected() { let mut input = request(EvidenceState::Supported); input.route = "/v1/other".into(); assert!(serve_context_protocol(&input).is_err()); }
    #[test] fn response_digest_is_stable() { let receipt = serve_context_protocol(&request(EvidenceState::Supported)).unwrap(); assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap()); }
}

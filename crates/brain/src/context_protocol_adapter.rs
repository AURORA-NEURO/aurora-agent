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
const ADAPTER_CONTENT_TYPE: &str = "application/vnd.aurora.context-protocol-response+json";
const MAX_TEXT_BYTES: usize = 512;

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
            || self.request_id.trim().is_empty()
            || self.query_id.trim().is_empty()
            || self.study_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.content_type != "application/json"
            || self.idempotency_key.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
            || !matches!(
                self.disposition.as_str(),
                "ready" | "partial" | "unknown" | "blocked"
            )
        {
            return Err(ContextProtocolError::Invalid(
                "context protocol identity, route, idempotency, candidates, locality, or effects are incomplete".into(),
            ));
        }
        for (value, field) in [
            (&self.request_id, "request_id"),
            (&self.query_id, "query_id"),
            (&self.study_id, "study_id"),
            (&self.scope, "scope"),
            (&self.protocol_version, "protocol_version"),
            (&self.method, "method"),
            (&self.route, "route"),
            (&self.content_type, "content_type"),
            (&self.idempotency_key, "idempotency_key"),
            (&self.response_schema, "response_schema"),
            (&self.disposition, "disposition"),
            (&self.boundary, "boundary"),
        ] {
            validate_text(value, field)?;
        }
        for (values, field) in [
            (&self.candidate_order, "candidate_order"),
            (&self.qualified_order, "qualified_order"),
            (&self.blocked_order, "blocked_order"),
            (&self.unknown_order, "unknown_order"),
            (&self.omissions, "omissions"),
            (&self.uncertainty, "uncertainty"),
            (&self.negative_evidence, "negative_evidence"),
            (&self.effect_receipts, "effect_receipts"),
        ] {
            validate_sorted_unique(values, field)?;
        }
        let candidates = self
            .candidate_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut classified = self
            .qualified_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        classified.extend(self.blocked_order.iter().cloned());
        classified.extend(self.unknown_order.iter().cloned());
        if classified != candidates
            || !identity_keys(&self.qualified_order)
                .is_disjoint(&identity_keys(&self.blocked_order))
            || !identity_keys(&self.qualified_order)
                .is_disjoint(&identity_keys(&self.unknown_order))
            || !identity_keys(&self.blocked_order).is_disjoint(&identity_keys(&self.unknown_order))
        {
            return Err(ContextProtocolError::Invalid(
                "context protocol state is not a disjoint candidate partition".into(),
            ));
        }
        if !matches!(self.status_code, 200 | 202 | 206 | 403 | 422) {
            return Err(ContextProtocolError::Invalid(
                "context protocol status code is outside the versioned contract".into(),
            ));
        }
        for digest in [
            &self.context_digest,
            &self.section_digest,
            &self.request_digest,
            &self.response_digest,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(ContextProtocolError::Invalid(
                    "context protocol digest is invalid".into(),
                ));
            }
        }
        let expected_status = match self.disposition.as_str() {
            "ready" => 200,
            "partial" => 206,
            "unknown" => 202,
            "blocked" => 403,
            _ => {
                return Err(ContextProtocolError::Invalid(
                    "context protocol disposition is not supported".into(),
                ));
            }
        };
        if self.status_code != expected_status {
            return Err(ContextProtocolError::Invalid(
                "context protocol status does not match disposition".into(),
            ));
        }
        let expected_effect_receipts = if self.disposition == "blocked" {
            vec!["block:unsafe-release".into()]
        } else {
            vec![format!(
                "protocol:local-context-response:{}",
                self.idempotency_key
            )]
        };
        if self.effect_receipts != expected_effect_receipts {
            return Err(ContextProtocolError::Invalid(
                "context protocol effect does not match disposition".into(),
            ));
        }
        if !self.raw_data_local {
            return Err(ContextProtocolError::Invalid(
                "context protocol receipts must declare local emitted data".into(),
            ));
        }
        let expected_context_digest = ContentHash::of_value(&json!({
            "required_order": self.candidate_order,
            "qualified_order": self.qualified_order,
            "blocked_order": self.blocked_order,
            "unknown_order": self.unknown_order,
            "replay_identity": self.replay_identity,
            "raw_data_local": self.raw_data_local,
        }))
        .map_err(|error| ContextProtocolError::Artifact(error.to_string()))?;
        if self.context_digest != expected_context_digest {
            return Err(ContextProtocolError::Invalid(
                "context protocol digest is not bound to candidate outcomes".into(),
            ));
        }
        let expected_section_digest = ContentHash::of_value(&json!({
            "study_id": self.study_id,
            "scope": self.scope,
            "context_digest": self.context_digest,
            "qualified_order": self.qualified_order,
            "blocked_order": self.blocked_order,
            "unknown_order": self.unknown_order,
        }))
        .map_err(|error| ContextProtocolError::Artifact(error.to_string()))?;
        if self.section_digest != expected_section_digest {
            return Err(ContextProtocolError::Invalid(
                "context protocol section digest is not bound to outcomes".into(),
            ));
        }
        let expected_response_digest = ContentHash::of_value(&json!({
            "protocol_version": PROTOCOL_VERSION,
            "route": ROUTE,
            "request_id": self.request_id,
            "status_code": self.status_code,
            "disposition": self.disposition,
            "context_digest": self.context_digest,
            "section_digest": self.section_digest,
            "replay_identity": self.replay_identity,
            "raw_data_local": self.raw_data_local,
        }))
        .map_err(|error| ContextProtocolError::Artifact(error.to_string()))?;
        if self.response_digest != expected_response_digest {
            return Err(ContextProtocolError::Invalid(
                "context protocol response digest is not bound to response state".into(),
            ));
        }
        let expected_artifact_id = format!("brain-context-protocol:{}", self.request_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != ADAPTER_CONTENT_TYPE
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(ContextProtocolError::Invalid(
                "context protocol artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ContextProtocolError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
            .map_err(|error| ContextProtocolError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, ContextProtocolError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ContextProtocolError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ContextProtocolError::Artifact(error.to_string()))
    }
}

pub fn context_protocol_adapter_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["local HTTP gateway".into(), "research SDK".into(), "workflow operator".into()].into(), behavior: "maps a typed local DecisionQuery context request to a canonical versioned protocol response with omission and replay receipts".into(), value: "lets local clients consume certified Decision-Section context through stable HTTP, event, SDK, and protocol surfaces without external data movement".into(), inputs: vec![TypedPort { name: "context_protocol_request".into(), schema: "ContextProtocolRequest1@1".into(), required: true }], outputs: vec![TypedPort { name: "context_protocol_response".into(), schema: RESPONSE_SCHEMA.into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["protocol:local-context-response".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "mcp-basic-protocol".into(), state: EvidenceState::Supported, locator: Some("https://modelcontextprotocol.io/specification/2025-06-18/basic/index".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A0, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn serve_context_protocol(
    request: &ContextProtocolRequest,
) -> Result<ContextProtocolReceipt, ContextProtocolError> {
    validate_request(request)?;
    for (value, field) in [
        (&request.request_id, "request_id"),
        (&request.query_id, "query_id"),
        (&request.study_id, "study_id"),
        (&request.scope, "scope"),
        (&request.goal, "goal"),
        (&request.protocol_version, "protocol_version"),
        (&request.method, "method"),
        (&request.route, "route"),
        (&request.content_type, "content_type"),
        (&request.idempotency_key, "idempotency_key"),
        (&request.response_schema, "response_schema"),
        (&request.boundary, "boundary"),
    ] {
        validate_text(value, field)?;
    }
    validate_unique(&request.required_context_order, "required_context_order")?;
    let required = request
        .required_context_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if required.is_empty() || required.len() != request.required_context_order.len() {
        return Err(ContextProtocolError::Invalid(
            "required context identifiers must be unique and non-empty".into(),
        ));
    }
    let mut candidate_map = std::collections::BTreeMap::new();
    let mut candidate_keys = BTreeSet::new();
    for candidate in &request.candidates {
        for (value, field) in [
            (&candidate.context_id, "candidate.context_id"),
            (&candidate.boundary, "candidate.boundary"),
        ] {
            validate_text(value, field)?;
        }
        for (digest, field) in [
            (&candidate.context_digest, "candidate.context_digest"),
            (&candidate.section_digest, "candidate.section_digest"),
            (&candidate.replay_identity, "candidate.replay_identity"),
        ] {
            if digest.as_str().len() != 64 {
                return Err(ContextProtocolError::Invalid(format!(
                    "{field} must be a 64-character content hash"
                )));
            }
        }
        if !candidate_keys.insert(candidate.context_id.to_ascii_lowercase()) {
            return Err(ContextProtocolError::Invalid(
                "context protocol candidates must be unique and case-distinct".into(),
            ));
        }
        if candidate_map
            .insert(candidate.context_id.clone(), candidate)
            .is_some()
        {
            return Err(ContextProtocolError::Invalid(
                "context protocol candidates must be unique".into(),
            ));
        }
    }
    let mut qualified = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for id in &required {
        let Some(candidate) = candidate_map.get(id) else {
            unknown.insert(id.clone());
            omissions.insert(format!("context:{}:missing", id));
            continue;
        };
        if !request.policy_allow
            || !request.protected_closure
            || !request.raw_data_local
            || !candidate.raw_data_local
            || candidate.boundary != PRECLINICAL_BOUNDARY
        {
            blocked.insert(id.clone());
            omissions.insert(format!("context:{}:policy-locality-blocked", id));
        } else if candidate.replay_identity != request.replay_identity {
            unknown.insert(id.clone());
            uncertainty.insert(format!("context:{}:replay-mismatch", id));
        } else {
            match candidate.state {
                EvidenceState::Proven | EvidenceState::Supported => qualified.insert(id.clone()),
                EvidenceState::Speculative | EvidenceState::Unknown => {
                    unknown.insert(id.clone());
                    uncertainty.insert(format!("context:{}:evidence-uncertain", id));
                    false
                }
                EvidenceState::Contradicted => {
                    blocked.insert(id.clone());
                    negative.insert(format!("context:{}:contradicted", id));
                    false
                }
            };
        }
    }
    let locality_failure = !request.raw_data_local
        || required
            .iter()
            .filter_map(|id| candidate_map.get(id))
            .any(|candidate| !candidate.raw_data_local);
    if locality_failure {
        omissions.insert("protocol:raw-data-locality-failed".into());
    }
    let locality_gate = !locality_failure;
    let gates_open = request.policy_allow && request.protected_closure && locality_gate;
    let raw_data_local = true;
    let disposition = if !gates_open {
        "blocked"
    } else if qualified.len() == required.len() {
        "ready"
    } else if !unknown.is_empty() {
        "unknown"
    } else {
        "partial"
    };
    let status_code = if !gates_open {
        403
    } else if disposition == "ready" {
        200
    } else if disposition == "partial" {
        206
    } else {
        202
    };
    if !request.policy_allow {
        omissions.insert("protocol:policy-denied".into());
    }
    if !request.protected_closure {
        omissions.insert("protocol:protected-closure-incomplete".into());
    }
    let candidate_order = required.iter().cloned().collect::<Vec<_>>();
    let qualified_order = qualified.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let unknown_order = unknown.into_iter().collect::<Vec<_>>();
    let omissions = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence = negative.into_iter().collect::<Vec<_>>();
    let context_digest = ContentHash::of_value(&json!({"required_order": candidate_order, "qualified_order": qualified_order, "blocked_order": blocked_order, "unknown_order": unknown_order, "replay_identity": request.replay_identity, "raw_data_local": raw_data_local})).map_err(|error| ContextProtocolError::Artifact(error.to_string()))?;
    let section_digest = ContentHash::of_value(&json!({"study_id": request.study_id, "scope": request.scope, "context_digest": context_digest, "qualified_order": qualified_order, "blocked_order": blocked_order, "unknown_order": unknown_order})).map_err(|error| ContextProtocolError::Artifact(error.to_string()))?;
    let request_value = serde_json::to_value(request)
        .map_err(|error| ContextProtocolError::Artifact(error.to_string()))?;
    let request_digest = ContentHash::of_value(&request_value)
        .map_err(|error| ContextProtocolError::Artifact(error.to_string()))?;
    let effect_receipts = if gates_open {
        vec![format!(
            "protocol:local-context-response:{}",
            request.idempotency_key
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let response_digest = ContentHash::of_value(&json!({"protocol_version": PROTOCOL_VERSION, "route": ROUTE, "request_id": request.request_id, "status_code": status_code, "disposition": disposition, "context_digest": context_digest, "section_digest": section_digest, "replay_identity": request.replay_identity, "raw_data_local": raw_data_local})).map_err(|error| ContextProtocolError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "query_id": request.query_id, "study_id": request.study_id, "scope": request.scope, "protocol_version": PROTOCOL_VERSION, "method": METHOD, "route": ROUTE, "content_type": "application/json", "idempotency_key": request.idempotency_key, "response_schema": RESPONSE_SCHEMA, "status_code": status_code, "disposition": disposition, "candidate_order": candidate_order, "qualified_order": qualified_order, "blocked_order": blocked_order, "unknown_order": unknown_order, "context_digest": context_digest, "section_digest": section_digest, "request_digest": request_digest, "response_digest": response_digest, "replay_identity": request.replay_identity, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative_evidence, "effect_receipts": effect_receipts, "raw_data_local": raw_data_local, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-context-protocol:{}", request.request_id),
        ADAPTER_CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ContextProtocolError::Artifact(error.to_string()))?;
    let receipt = ContextProtocolReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        query_id: request.query_id.clone(),
        study_id: request.study_id.clone(),
        scope: request.scope.clone(),
        protocol_version: PROTOCOL_VERSION.into(),
        method: METHOD.into(),
        route: ROUTE.into(),
        content_type: "application/json".into(),
        idempotency_key: request.idempotency_key.clone(),
        response_schema: RESPONSE_SCHEMA.into(),
        status_code,
        disposition: disposition.into(),
        candidate_order,
        qualified_order,
        blocked_order,
        unknown_order,
        context_digest,
        section_digest,
        request_digest,
        response_digest,
        replay_identity: request.replay_identity.clone(),
        omissions,
        uncertainty,
        negative_evidence,
        effect_receipts,
        artifact,
        raw_data_local,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &ContextProtocolRequest) -> Result<(), ContextProtocolError> {
    if request.protocol_version != PROTOCOL_VERSION
        || request.method != METHOD
        || request.route != ROUTE
        || request.content_type != "application/json"
        || request.idempotency_key.trim().is_empty()
        || request.response_schema != RESPONSE_SCHEMA
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.replay_identity.as_str().len() != 64
    {
        return Err(ContextProtocolError::Invalid("protocol version, route, idempotency, response schema, replay, or boundary is incomplete".into()));
    }
    Ok(())
}

fn identity_keys(values: &[String]) -> BTreeSet<String> {
    values
        .iter()
        .map(|value| value.to_ascii_lowercase())
        .collect()
}

fn validate_text(value: &str, field: &str) -> Result<(), ContextProtocolError> {
    if value.trim().is_empty()
        || value != value.trim()
        || value.len() > MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(ContextProtocolError::Invalid(format!(
            "{field} must be bounded, non-empty text without padding or control characters"
        )));
    }
    Ok(())
}

fn validate_unique(values: &[String], field: &str) -> Result<(), ContextProtocolError> {
    let mut seen = BTreeSet::new();
    for value in values {
        validate_text(value, field)?;
        if !seen.insert(value.to_ascii_lowercase()) {
            return Err(ContextProtocolError::Invalid(format!(
                "{field} contains duplicate or case-colliding values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_unique(values: &[String], field: &str) -> Result<(), ContextProtocolError> {
    validate_unique(values, field)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ContextProtocolError::Invalid(format!(
            "{field} is not in canonical order"
        )));
    }
    Ok(())
}

fn receipt_payload(receipt: &ContextProtocolReceipt) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "request_id": receipt.request_id,
        "query_id": receipt.query_id,
        "study_id": receipt.study_id,
        "scope": receipt.scope,
        "protocol_version": receipt.protocol_version,
        "method": receipt.method,
        "route": receipt.route,
        "content_type": receipt.content_type,
        "idempotency_key": receipt.idempotency_key,
        "response_schema": receipt.response_schema,
        "status_code": receipt.status_code,
        "disposition": receipt.disposition,
        "candidate_order": receipt.candidate_order,
        "qualified_order": receipt.qualified_order,
        "blocked_order": receipt.blocked_order,
        "unknown_order": receipt.unknown_order,
        "context_digest": receipt.context_digest,
        "section_digest": receipt.section_digest,
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

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn request(state: EvidenceState) -> ContextProtocolRequest {
        let replay = hash("context-protocol-replay");
        ContextProtocolRequest {
            request_id: "request:context-protocol".into(),
            query_id: "query:context".into(),
            study_id: "study:organoid".into(),
            scope: "organoid:neural".into(),
            goal: "compile decision context".into(),
            required_context_order: vec!["context:a".into()],
            candidates: vec![ContextProtocolCandidate {
                context_id: "context:a".into(),
                context_digest: replay.clone(),
                section_digest: replay.clone(),
                replay_identity: replay.clone(),
                state,
                raw_data_local: true,
                boundary: PRECLINICAL_BOUNDARY.into(),
            }],
            protocol_version: PROTOCOL_VERSION.into(),
            method: METHOD.into(),
            route: ROUTE.into(),
            content_type: "application/json".into(),
            idempotency_key: "idem:context-001".into(),
            response_schema: RESPONSE_SCHEMA.into(),
            replay_identity: replay,
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a0_local() {
        let manifest = context_protocol_adapter_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A0);
    }
    #[test]
    fn supported_context_returns_200() {
        let receipt = serve_context_protocol(&request(EvidenceState::Supported)).unwrap();
        assert_eq!(receipt.status_code, 200);
        assert_eq!(receipt.disposition, "ready");
    }
    #[test]
    fn unknown_context_returns_202() {
        let receipt = serve_context_protocol(&request(EvidenceState::Unknown)).unwrap();
        assert_eq!(receipt.status_code, 202);
    }
    #[test]
    fn policy_denial_returns_403() {
        let mut input = request(EvidenceState::Supported);
        input.policy_allow = false;
        let receipt = serve_context_protocol(&input).unwrap();
        assert_eq!(receipt.status_code, 403);
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn invalid_route_is_rejected() {
        let mut input = request(EvidenceState::Supported);
        input.route = "/v1/other".into();
        assert!(serve_context_protocol(&input).is_err());
    }
    #[test]
    fn response_digest_is_stable() {
        let receipt = serve_context_protocol(&request(EvidenceState::Supported)).unwrap();
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
    #[test]
    fn locality_failure_is_blocked_and_retained() {
        let mut input = request(EvidenceState::Supported);
        input.candidates[0].raw_data_local = false;
        let receipt = serve_context_protocol(&input).unwrap();
        assert_eq!(receipt.status_code, 403);
        assert!(receipt.raw_data_local);
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item == "protocol:raw-data-locality-failed"));
        assert!(receipt.validate().is_ok());
    }
    #[test]
    fn response_artifact_payload_is_bound() {
        let mut receipt = serve_context_protocol(&request(EvidenceState::Supported)).unwrap();
        receipt.scope = "tampered-scope".into();
        assert!(receipt.validate().is_err());
    }
}

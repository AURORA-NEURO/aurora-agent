//! Multimodal context-compilation API/protocol adapter.
//!
//! Atlas feature: `AFA-brain-P03-F22`. The adapter exposes a versioned local
//! study×modality contract with explicit comparability and omission states.

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

pub const FEATURE_ID: &str = "AFA-brain-P03-F22";
pub const CONTRACT_VERSION: &str = "brain-multimodal-context-protocol-adapter/1.0";
pub const PROTOCOL_VERSION: &str = "aurora-research-context-multimodal/1.0";
pub const ROUTE: &str = "/v1/research/context/multimodal/compile";
pub const METHOD: &str = "POST";
pub const RESPONSE_SCHEMA: &str = "MultimodalContextProtocolResponse1@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalContextProtocolCell {
    pub study_id: String,
    pub modality: String,
    pub context_digest: ContentHash,
    pub section_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub state: EvidenceState,
    pub comparable: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalContextProtocolRequest {
    pub request_id: String,
    pub query_id: String,
    pub scope: String,
    pub goal: String,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub cells: Vec<MultimodalContextProtocolCell>,
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
pub struct MultimodalContextProtocolReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub query_id: String,
    pub scope: String,
    pub protocol_version: String,
    pub method: String,
    pub route: String,
    pub content_type: String,
    pub idempotency_key: String,
    pub response_schema: String,
    pub status_code: u16,
    pub disposition: String,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub cell_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub missing_order: Vec<String>,
    pub incompatible_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub context_digest: ContentHash,
    pub section_digest: ContentHash,
    pub comparability_digest: ContentHash,
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
pub enum MultimodalContextProtocolError {
    #[error("invalid multimodal context protocol request: {0}")]
    Invalid(String),
    #[error("multimodal context protocol artifact failed: {0}")]
    Artifact(String),
}

impl MultimodalContextProtocolReceipt {
    pub fn validate(&self) -> Result<(), MultimodalContextProtocolError> {
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
            || self.scope.trim().is_empty()
            || self.content_type != "application/json"
            || self.idempotency_key.trim().is_empty()
            || self.study_order.len() < 2
            || self.modality_order.len() < 2
            || self.cell_order.is_empty()
            || self.effect_receipts.is_empty()
            || !matches!(
                self.disposition.as_str(),
                "ready" | "partial" | "unknown" | "blocked"
            )
        {
            return Err(MultimodalContextProtocolError::Invalid("multimodal protocol identity, route, coverage, idempotency, locality, or effects are incomplete".into()));
        }
        if self
            .qualified_order
            .iter()
            .chain(self.missing_order.iter())
            .chain(self.incompatible_order.iter())
            .chain(self.unknown_order.iter())
            .any(|id| !self.cell_order.contains(id))
        {
            return Err(MultimodalContextProtocolError::Invalid(
                "multimodal protocol state is not covered by cells".into(),
            ));
        }
        for values in [
            &self.study_order,
            &self.modality_order,
            &self.cell_order,
            &self.qualified_order,
            &self.missing_order,
            &self.incompatible_order,
            &self.unknown_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(MultimodalContextProtocolError::Invalid(
                    "multimodal protocol ordering is not canonical".into(),
                ));
            }
        }
        if !matches!(self.status_code, 200 | 202 | 206 | 403 | 422) {
            return Err(MultimodalContextProtocolError::Invalid(
                "multimodal protocol status code is invalid".into(),
            ));
        }
        for digest in [
            &self.context_digest,
            &self.section_digest,
            &self.comparability_digest,
            &self.request_digest,
            &self.response_digest,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(MultimodalContextProtocolError::Invalid(
                    "multimodal protocol digest is invalid".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("protocol:local-multimodal-context-response:")
                && effect != "block:unsafe-release"
        }) {
            return Err(MultimodalContextProtocolError::Invalid(
                "multimodal protocol effect is outside local response gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| MultimodalContextProtocolError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, MultimodalContextProtocolError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| MultimodalContextProtocolError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| MultimodalContextProtocolError::Artifact(error.to_string()))
    }
}

pub fn multimodal_context_protocol_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["multimodal context API gateway".into(), "agent developer".into(), "research SDK".into()].into(), behavior: "maps a typed study×modality DecisionQuery to a canonical local protocol response with comparability and replay binding".into(), value: "provides stable multimodal context API semantics without exporting raw imaging or omics data".into(), inputs: vec![TypedPort { name: "multimodal_context_protocol_request".into(), schema: "MultimodalContextProtocolRequest1@1".into(), required: true }], outputs: vec![TypedPort { name: "multimodal_context_protocol_response".into(), schema: RESPONSE_SCHEMA.into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["protocol:local-multimodal-context-response".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "mcp-basic-protocol".into(), state: EvidenceState::Supported, locator: Some("https://modelcontextprotocol.io/specification/2025-06-18/basic/index".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A0, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn serve_multimodal_context_protocol(
    request: &MultimodalContextProtocolRequest,
) -> Result<MultimodalContextProtocolReceipt, MultimodalContextProtocolError> {
    validate_request(request)?;
    let studies = request.study_order.iter().cloned().collect::<BTreeSet<_>>();
    let modalities = request
        .modality_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if studies.len() < 2
        || modalities.len() < 2
        || studies.len() != request.study_order.len()
        || modalities.len() != request.modality_order.len()
        || studies.iter().any(|v| v.trim().is_empty())
        || modalities.iter().any(|v| v.trim().is_empty())
    {
        return Err(MultimodalContextProtocolError::Invalid(
            "study and modality identifiers must be unique and non-empty".into(),
        ));
    }
    let expected = studies
        .iter()
        .flat_map(|study| {
            modalities
                .iter()
                .map(move |modality| format!("{}|{}", study, modality))
        })
        .collect::<BTreeSet<_>>();
    let mut cells = std::collections::BTreeMap::new();
    for cell in &request.cells {
        let id = format!("{}|{}", cell.study_id, cell.modality);
        if cells.insert(id, cell).is_some() {
            return Err(MultimodalContextProtocolError::Invalid(
                "multimodal protocol cells must be unique".into(),
            ));
        }
    }
    let mut qualified = BTreeSet::new();
    let mut missing = BTreeSet::new();
    let mut incompatible = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for id in &expected {
        let Some(cell) = cells.get(id) else {
            missing.insert(id.clone());
            omissions.insert(format!("cell:{}:missing", id));
            continue;
        };
        if !request.policy_allow
            || !request.protected_closure
            || !request.raw_data_local
            || !cell.raw_data_local
            || cell.boundary != PRECLINICAL_BOUNDARY
        {
            incompatible.insert(id.clone());
            omissions.insert(format!("cell:{}:policy-locality-blocked", id));
        } else if !cell.comparable {
            incompatible.insert(id.clone());
            negative.insert(format!("cell:{}:incomparable", id));
        } else if cell.replay_identity != request.replay_identity {
            unknown.insert(id.clone());
            uncertainty.insert(format!("cell:{}:replay-mismatch", id));
        } else {
            match cell.state {
                EvidenceState::Proven | EvidenceState::Supported => {
                    qualified.insert(id.clone());
                }
                EvidenceState::Speculative | EvidenceState::Unknown => {
                    unknown.insert(id.clone());
                    uncertainty.insert(format!("cell:{}:evidence-uncertain", id));
                }
                EvidenceState::Contradicted => {
                    incompatible.insert(id.clone());
                    negative.insert(format!("cell:{}:contradicted", id));
                }
            }
        }
    }
    let gates_open = request.policy_allow && request.protected_closure && request.raw_data_local;
    let disposition = if !gates_open {
        "blocked"
    } else if qualified.len() == expected.len() {
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
    if !request.raw_data_local {
        omissions.insert("protocol:raw-data-locality-failed".into());
    }
    let candidate_order = expected.iter().cloned().collect::<Vec<_>>();
    let context_digest = ContentHash::of_value(&json!({"study_order": studies, "modality_order": modalities, "qualified_order": qualified, "replay_identity": request.replay_identity})).map_err(|error| MultimodalContextProtocolError::Artifact(error.to_string()))?;
    let comparability_digest = ContentHash::of_value(&json!({"study_order": request.study_order, "modality_order": request.modality_order, "candidate_order": candidate_order, "qualified_order": qualified, "replay_identity": request.replay_identity})).map_err(|error| MultimodalContextProtocolError::Artifact(error.to_string()))?;
    let section_digest = ContentHash::of_value(&json!({"scope": request.scope, "context_digest": context_digest, "comparability_digest": comparability_digest, "qualified_order": qualified})).map_err(|error| MultimodalContextProtocolError::Artifact(error.to_string()))?;
    let request_value = serde_json::to_value(request)
        .map_err(|error| MultimodalContextProtocolError::Artifact(error.to_string()))?;
    let request_digest = ContentHash::of_value(&request_value)
        .map_err(|error| MultimodalContextProtocolError::Artifact(error.to_string()))?;
    let response_digest = ContentHash::of_value(&json!({"protocol_version": PROTOCOL_VERSION, "route": ROUTE, "request_id": request.request_id, "status_code": status_code, "disposition": disposition, "comparability_digest": comparability_digest, "replay_identity": request.replay_identity})).map_err(|error| MultimodalContextProtocolError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "protocol_version": PROTOCOL_VERSION, "method": METHOD, "route": ROUTE, "response_schema": RESPONSE_SCHEMA, "status_code": status_code, "disposition": disposition, "candidate_order": candidate_order, "context_digest": context_digest, "section_digest": section_digest, "comparability_digest": comparability_digest, "request_digest": request_digest, "response_digest": response_digest, "replay_identity": request.replay_identity, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-multimodal-context-protocol:{}", request.request_id),
        "application/vnd.aurora.multimodal-context-protocol-response+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| MultimodalContextProtocolError::Artifact(error.to_string()))?;
    let receipt = MultimodalContextProtocolReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        query_id: request.query_id.clone(),
        scope: request.scope.clone(),
        protocol_version: PROTOCOL_VERSION.into(),
        method: METHOD.into(),
        route: ROUTE.into(),
        content_type: "application/json".into(),
        idempotency_key: request.idempotency_key.clone(),
        response_schema: RESPONSE_SCHEMA.into(),
        status_code,
        disposition: disposition.into(),
        study_order: studies.into_iter().collect(),
        modality_order: modalities.into_iter().collect(),
        cell_order: candidate_order,
        qualified_order: qualified.into_iter().collect(),
        missing_order: missing.into_iter().collect(),
        incompatible_order: incompatible.into_iter().collect(),
        unknown_order: unknown.into_iter().collect(),
        context_digest,
        section_digest,
        comparability_digest,
        request_digest,
        response_digest,
        replay_identity: request.replay_identity.clone(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts: if gates_open {
            vec![format!(
                "protocol:local-multimodal-context-response:{}",
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

fn validate_request(
    request: &MultimodalContextProtocolRequest,
) -> Result<(), MultimodalContextProtocolError> {
    if request.protocol_version != PROTOCOL_VERSION
        || request.method != METHOD
        || request.route != ROUTE
        || request.content_type != "application/json"
        || request.idempotency_key.trim().is_empty()
        || request.response_schema != RESPONSE_SCHEMA
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.replay_identity.as_str().len() != 64
    {
        return Err(MultimodalContextProtocolError::Invalid("multimodal protocol version, route, idempotency, response schema, replay, or boundary is incomplete".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn request(state: EvidenceState) -> MultimodalContextProtocolRequest {
        let replay = hash("multimodal-context-protocol-replay");
        let mut cells = Vec::new();
        for study in ["study:a", "study:b"] {
            for modality in ["imaging", "omics"] {
                cells.push(MultimodalContextProtocolCell {
                    study_id: study.into(),
                    modality: modality.into(),
                    context_digest: replay.clone(),
                    section_digest: replay.clone(),
                    replay_identity: replay.clone(),
                    state,
                    comparable: true,
                    raw_data_local: true,
                    boundary: PRECLINICAL_BOUNDARY.into(),
                });
            }
        }
        MultimodalContextProtocolRequest {
            request_id: "request:multimodal-context".into(),
            query_id: "query:context".into(),
            scope: "preclinical:organoid".into(),
            goal: "compile multimodal context".into(),
            study_order: vec!["study:a".into(), "study:b".into()],
            modality_order: vec!["imaging".into(), "omics".into()],
            cells,
            protocol_version: PROTOCOL_VERSION.into(),
            method: METHOD.into(),
            route: ROUTE.into(),
            content_type: "application/json".into(),
            idempotency_key: "idem:multimodal-context".into(),
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
        assert_eq!(
            multimodal_context_protocol_manifest().autonomy_tier,
            AutonomyTier::A0
        );
    }
    #[test]
    fn complete_matrix_returns_200() {
        let receipt =
            serve_multimodal_context_protocol(&request(EvidenceState::Supported)).unwrap();
        assert_eq!(receipt.status_code, 200);
        assert_eq!(receipt.disposition, "ready");
    }
    #[test]
    fn unknown_cell_returns_202() {
        let receipt = serve_multimodal_context_protocol(&request(EvidenceState::Unknown)).unwrap();
        assert_eq!(receipt.status_code, 202);
    }
    #[test]
    fn incomparability_is_retained() {
        let mut input = request(EvidenceState::Supported);
        input.cells[0].comparable = false;
        let receipt = serve_multimodal_context_protocol(&input).unwrap();
        assert!(receipt
            .negative_evidence
            .iter()
            .any(|item| item.contains("incomparable")));
    }
    #[test]
    fn policy_denial_returns_403() {
        let mut input = request(EvidenceState::Supported);
        input.policy_allow = false;
        let receipt = serve_multimodal_context_protocol(&input).unwrap();
        assert_eq!(receipt.status_code, 403);
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn response_digest_is_stable() {
        let receipt =
            serve_multimodal_context_protocol(&request(EvidenceState::Supported)).unwrap();
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
}

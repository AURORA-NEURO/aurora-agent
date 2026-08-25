//! Typed retrieval protocol gateway.
//!
//! Atlas feature: `AFA-brain-P02-F21`. The gateway negotiates a deterministic retrieval
//! session, executes only the institution-local synthesis contract, and leaves a replayable
//! transcript when protocol, policy, or protected-closure gates prevent release.

use crate::retrieval_synthesis::{
    synthesize_retrieval, ScopedRetrievalQuery, SynthesisDisposition,
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

pub const FEATURE_ID: &str = "AFA-brain-P02-F21";
pub const CONTRACT_VERSION: &str = "brain-retrieval-protocol-gateway/1.0";
pub const STAGE_ORDER: [&str; 5] = [
    "protocol:open",
    "protocol:authorize",
    "protocol:retrieve",
    "protocol:synthesize",
    "protocol:close",
];
pub const CAPABILITY_ORDER: [&str; 4] = [
    "capability:evidence-synthesis-v1",
    "capability:omission-receipt-v1",
    "capability:replay-v1",
    "capability:scoped-query-v1",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrievalProtocolRequest {
    pub request: ScopedRetrievalQuery,
    pub protocol_id: String,
    pub session_id: String,
    pub offered_capability_order: Vec<String>,
    pub required_capability_order: Vec<String>,
    pub requested_stage_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub budget_units: u32,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrievalProtocolReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub protocol_id: String,
    pub session_id: String,
    pub disposition: SynthesisDisposition,
    pub offered_capability_order: Vec<String>,
    pub required_capability_order: Vec<String>,
    pub negotiated_capability_order: Vec<String>,
    pub stage_order: Vec<String>,
    pub completed_stage_order: Vec<String>,
    pub blocked_stage_order: Vec<String>,
    pub action_receipts: Vec<String>,
    pub candidate_order: Vec<String>,
    pub ranked_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub negotiation_digest: ContentHash,
    pub transcript_digest: ContentHash,
    pub synthesis_digest: ContentHash,
    pub protocol_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub budget_units: u32,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RetrievalProtocolError {
    #[error("invalid retrieval protocol request: {0}")]
    Invalid(String),
    #[error("retrieval protocol artifact failed: {0}")]
    Artifact(String),
    #[error("retrieval protocol synthesis failed: {0}")]
    Engine(String),
}

impl RetrievalProtocolReceipt {
    pub fn validate(&self) -> Result<(), RetrievalProtocolError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.protocol_id.trim().is_empty()
            || self.session_id.trim().is_empty()
            || self.offered_capability_order.is_empty()
            || self.required_capability_order.is_empty()
            || self.stage_order != STAGE_ORDER
            || self.completed_stage_order.is_empty()
            || self.action_receipts.is_empty()
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
            || self.budget_units < STAGE_ORDER.len() as u32
        {
            return Err(RetrievalProtocolError::Invalid(
                "protocol identity, negotiation, stage, retrieval, locality, budget, or effects are incomplete".into(),
            ));
        }
        if !is_sorted_unique(&self.offered_capability_order)
            || !is_sorted_unique(&self.required_capability_order)
            || !is_sorted_unique(&self.negotiated_capability_order)
            || !is_sorted_unique(&self.action_receipts)
            || !is_sorted_unique(&self.candidate_order)
            || !is_sorted_unique(&self.ranked_order)
            || !is_sorted_unique(&self.qualified_order)
            || !is_sorted_unique(&self.blocked_order)
            || !is_sorted_unique(&self.unknown_order)
            || !is_sorted_unique(&self.omissions)
            || !is_sorted_unique(&self.uncertainty)
            || !is_sorted_unique(&self.negative_evidence)
            || !is_sorted_unique(&self.effect_receipts)
        {
            return Err(RetrievalProtocolError::Invalid(
                "protocol vectors are not canonical".into(),
            ));
        }
        if (self.disposition != SynthesisDisposition::Blocked
            && self
                .required_capability_order
                .iter()
                .any(|capability| !self.offered_capability_order.contains(capability)))
            || self
                .negotiated_capability_order
                .iter()
                .any(|capability| !self.required_capability_order.contains(capability))
            || self
                .ranked_order
                .iter()
                .chain(self.qualified_order.iter())
                .chain(self.blocked_order.iter())
                .chain(self.unknown_order.iter())
                .any(|id| !self.candidate_order.contains(id))
        {
            return Err(RetrievalProtocolError::Invalid(
                "protocol negotiation or synthesis state is not covered by its declaration".into(),
            ));
        }
        if self
            .completed_stage_order
            .iter()
            .chain(self.blocked_stage_order.iter())
            .any(|stage| !self.stage_order.iter().any(|expected| expected == stage))
            || self
                .completed_stage_order
                .iter()
                .any(|stage| self.blocked_stage_order.contains(stage))
        {
            return Err(RetrievalProtocolError::Invalid(
                "protocol stage transcript is not a disjoint subset of the declared protocol"
                    .into(),
            ));
        }
        for digest in [
            &self.negotiation_digest,
            &self.transcript_digest,
            &self.synthesis_digest,
            &self.protocol_digest,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(RetrievalProtocolError::Invalid(
                    "protocol digest is invalid".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("read:local-retrieval-protocol:")
                && effect != "block:unsafe-release"
        }) {
            return Err(RetrievalProtocolError::Invalid(
                "protocol effect is not read-only".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| RetrievalProtocolError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, RetrievalProtocolError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| RetrievalProtocolError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| RetrievalProtocolError::Artifact(error.to_string()))
    }
}

pub fn retrieval_protocol_gateway_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "brain".into(),
        consumers: ["research workflow operator".into(), "protocol integration engineer".into()]
            .into(),
        behavior: "negotiates a deterministic local retrieval protocol and emits replayable stage and evidence receipts".into(),
        value: "makes cross-runtime retrieval sessions interoperable without external fetching or silent protocol downgrade".into(),
        inputs: vec![TypedPort { name: "retrieval_protocol_request".into(), schema: "ResearchWorkflowSpec2@1".into(), required: true }],
        outputs: vec![TypedPort { name: "retrieval_protocol_receipt".into(), schema: "RetrievalProtocolReceipt1@1".into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(),
        permissions: ["read:local-research-artifacts".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "mcp-2025-06-18".into(), state: EvidenceState::Supported, locator: Some("https://modelcontextprotocol.io/specification/2025-06-18/basic/index".into()) }],
        authority_requirements: Vec::new(),
        autonomy_tier: AutonomyTier::A0,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn compile_retrieval_protocol(
    request: &RetrievalProtocolRequest,
) -> Result<RetrievalProtocolReceipt, RetrievalProtocolError> {
    validate_request(request)?;
    let synthesis = synthesize_retrieval(&request.request)
        .map_err(|error| RetrievalProtocolError::Engine(error.to_string()))?;
    let negotiated_capability_order = request
        .required_capability_order
        .iter()
        .filter(|capability| request.offered_capability_order.contains(capability))
        .cloned()
        .collect::<Vec<_>>();
    let missing_capabilities = request
        .required_capability_order
        .iter()
        .filter(|capability| !request.offered_capability_order.contains(capability))
        .cloned()
        .collect::<Vec<_>>();
    let protocol_gate = request.policy_allow
        && request.protected_closure
        && request.raw_data_local
        && request.budget_units >= STAGE_ORDER.len() as u32
        && missing_capabilities.is_empty();
    let disposition = if protocol_gate {
        synthesis.disposition
    } else {
        SynthesisDisposition::Blocked
    };
    let completed_stage_order: Vec<String> = if protocol_gate {
        STAGE_ORDER.iter().map(|stage| (*stage).into()).collect()
    } else {
        STAGE_ORDER[..2]
            .iter()
            .map(|stage| (*stage).into())
            .collect()
    };
    let blocked_stage_order: Vec<String> = if protocol_gate {
        Vec::new()
    } else {
        STAGE_ORDER[2..]
            .iter()
            .map(|stage| (*stage).into())
            .collect()
    };
    let mut omissions = synthesis.omissions.iter().cloned().collect::<BTreeSet<_>>();
    let mut uncertainty = synthesis
        .uncertainty
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let negative_evidence = synthesis
        .negative_evidence
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if !missing_capabilities.is_empty() {
        for capability in &missing_capabilities {
            omissions.insert(format!("protocol:capability-not-negotiated:{capability}"));
        }
    }
    if !request.policy_allow {
        omissions.insert("protocol:policy-denied".into());
    }
    if !request.protected_closure {
        omissions.insert("protocol:protected-closure-incomplete".into());
    }
    if !request.raw_data_local {
        omissions.insert("protocol:raw-data-locality-failed".into());
    }
    if request.budget_units < STAGE_ORDER.len() as u32 {
        omissions.insert("protocol:budget-exhausted".into());
    }
    if disposition == SynthesisDisposition::Blocked {
        uncertainty.insert("protocol:release-blocked-until-all-gates-pass".into());
    }
    let action_receipts = completed_stage_order
        .iter()
        .map(|stage| format!("stage:completed:{stage}"))
        .collect::<BTreeSet<_>>();
    let negotiation_digest = ContentHash::of_value(&json!({
        "protocol_id": request.protocol_id,
        "offered": request.offered_capability_order,
        "required": request.required_capability_order,
        "negotiated": negotiated_capability_order,
    }))
    .map_err(|error| RetrievalProtocolError::Artifact(error.to_string()))?;
    let transcript_digest = ContentHash::of_value(&json!({
        "session_id": request.session_id,
        "stage_order": STAGE_ORDER,
        "completed": completed_stage_order,
        "blocked": blocked_stage_order,
        "negotiation_digest": negotiation_digest,
        "replay_identity": request.replay_identity,
    }))
    .map_err(|error| RetrievalProtocolError::Artifact(error.to_string()))?;
    let protocol_digest = ContentHash::of_value(&json!({
        "feature_id": FEATURE_ID,
        "request_id": request.request.request_id,
        "protocol_id": request.protocol_id,
        "session_id": request.session_id,
        "disposition": disposition,
        "negotiation_digest": negotiation_digest,
        "transcript_digest": transcript_digest,
        "synthesis_digest": synthesis.synthesis_digest,
        "replay_identity": request.replay_identity,
    }))
    .map_err(|error| RetrievalProtocolError::Artifact(error.to_string()))?;
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request.request_id,
        "protocol_id": request.protocol_id,
        "session_id": request.session_id,
        "disposition": disposition,
        "offered_capability_order": request.offered_capability_order,
        "required_capability_order": request.required_capability_order,
        "negotiated_capability_order": negotiated_capability_order,
        "stage_order": STAGE_ORDER,
        "completed_stage_order": completed_stage_order,
        "blocked_stage_order": blocked_stage_order,
        "negotiation_digest": negotiation_digest,
        "transcript_digest": transcript_digest,
        "synthesis_digest": synthesis.synthesis_digest,
        "protocol_digest": protocol_digest,
        "replay_identity": request.replay_identity,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-retrieval-protocol:{}", request.session_id),
        "application/vnd.aurora.retrieval-protocol-receipt+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| RetrievalProtocolError::Artifact(error.to_string()))?;
    let effect_receipts = if matches!(
        disposition,
        SynthesisDisposition::Qualified | SynthesisDisposition::Partial
    ) {
        vec![format!(
            "read:local-retrieval-protocol:{}",
            request.session_id
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let receipt = RetrievalProtocolReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request.request_id.clone(),
        protocol_id: request.protocol_id.clone(),
        session_id: request.session_id.clone(),
        disposition,
        offered_capability_order: request.offered_capability_order.clone(),
        required_capability_order: request.required_capability_order.clone(),
        negotiated_capability_order,
        stage_order: STAGE_ORDER.iter().map(|stage| (*stage).into()).collect(),
        completed_stage_order,
        blocked_stage_order,
        action_receipts: action_receipts.into_iter().collect(),
        candidate_order: synthesis.candidate_order,
        ranked_order: synthesis.ranked_order,
        qualified_order: synthesis.qualified_order,
        blocked_order: synthesis.blocked_order,
        unknown_order: synthesis.unknown_order,
        negotiation_digest,
        transcript_digest,
        synthesis_digest: synthesis.synthesis_digest,
        protocol_digest,
        replay_identity: request.replay_identity.clone(),
        budget_units: request.budget_units,
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative_evidence.into_iter().collect(),
        effect_receipts,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &RetrievalProtocolRequest) -> Result<(), RetrievalProtocolError> {
    if request.protocol_id.trim().is_empty()
        || request.session_id.trim().is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.request.boundary != PRECLINICAL_BOUNDARY
        || request.offered_capability_order.is_empty()
        || request.required_capability_order.is_empty()
        || request.requested_stage_order != STAGE_ORDER
        || request.budget_units == 0
        || !is_sorted_unique(&request.offered_capability_order)
        || !is_sorted_unique(&request.required_capability_order)
        || request
            .offered_capability_order
            .iter()
            .any(|capability| !CAPABILITY_ORDER.contains(&capability.as_str()))
        || request
            .required_capability_order
            .iter()
            .any(|capability| !CAPABILITY_ORDER.contains(&capability.as_str()))
    {
        return Err(RetrievalProtocolError::Invalid(
            "protocol identity, capability declaration, stage order, budget, or boundary is invalid".into(),
        ));
    }
    Ok(())
}

fn is_sorted_unique(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }

    fn protocol_request() -> RetrievalProtocolRequest {
        RetrievalProtocolRequest {
            request: ScopedRetrievalQuery {
                request_id: "request:protocol".into(),
                study_id: "study:organoid".into(),
                scope: "organoid:neural".into(),
                query: "synaptic morphology".into(),
                minimum_support_milli: 700,
                candidates: vec![crate::retrieval_synthesis::RetrievalCandidate {
                    evidence_id: "evidence:one".into(),
                    source_id: "source:one".into(),
                    study_id: "study:organoid".into(),
                    scope: "organoid:neural".into(),
                    modality: "imaging".into(),
                    support_milli: 900,
                    state: EvidenceState::Supported,
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
            protocol_id: "protocol:retrieval-v1".into(),
            session_id: "session:one".into(),
            offered_capability_order: CAPABILITY_ORDER
                .iter()
                .map(|value| (*value).into())
                .collect(),
            required_capability_order: CAPABILITY_ORDER
                .iter()
                .map(|value| (*value).into())
                .collect(),
            requested_stage_order: STAGE_ORDER.iter().map(|value| (*value).into()).collect(),
            replay_identity: hash("replay"),
            budget_units: 8,
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn manifest_is_a0() {
        let manifest = retrieval_protocol_gateway_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A0);
    }

    #[test]
    fn protocol_negotiates_and_completes() {
        let receipt = compile_retrieval_protocol(&protocol_request()).unwrap();
        assert_eq!(receipt.disposition, SynthesisDisposition::Qualified);
        assert_eq!(receipt.completed_stage_order.len(), STAGE_ORDER.len());
        assert!(receipt.blocked_stage_order.is_empty());
        assert!(receipt.validate().is_ok());
    }

    #[test]
    fn missing_capability_blocks_release() {
        let mut request = protocol_request();
        request.offered_capability_order.pop();
        let receipt = compile_retrieval_protocol(&request).unwrap();
        assert_eq!(receipt.disposition, SynthesisDisposition::Blocked);
        assert!(!receipt.omissions.is_empty());
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }

    #[test]
    fn policy_denial_is_replayable() {
        let mut request = protocol_request();
        request.policy_allow = false;
        let receipt = compile_retrieval_protocol(&request).unwrap();
        assert_eq!(receipt.disposition, SynthesisDisposition::Blocked);
        assert!(receipt.digest().is_ok());
    }

    #[test]
    fn digest_is_stable() {
        let receipt = compile_retrieval_protocol(&protocol_request()).unwrap();
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
}

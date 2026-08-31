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
const PROTOCOL_CONTENT_TYPE: &str = "application/vnd.aurora.retrieval-protocol-receipt+json";
const MAX_TEXT_BYTES: usize = 512;

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
            || u64::from(self.budget_units) < u64::try_from(STAGE_ORDER.len()).unwrap_or(u64::MAX)
        {
            return Err(RetrievalProtocolError::Invalid(
                "protocol identity, negotiation, stage, retrieval, locality, budget, or effects are incomplete".into(),
            ));
        }
        for (value, field) in [
            (&self.request_id, "request_id"),
            (&self.protocol_id, "protocol_id"),
            (&self.session_id, "session_id"),
        ] {
            validate_text(value, field)?;
        }
        for (values, field) in [
            (&self.offered_capability_order, "offered_capability_order"),
            (&self.required_capability_order, "required_capability_order"),
            (
                &self.negotiated_capability_order,
                "negotiated_capability_order",
            ),
            (&self.action_receipts, "action_receipts"),
            (&self.candidate_order, "candidate_order"),
            (&self.blocked_order, "blocked_order"),
            (&self.unknown_order, "unknown_order"),
            (&self.omissions, "omissions"),
            (&self.uncertainty, "uncertainty"),
            (&self.negative_evidence, "negative_evidence"),
            (&self.effect_receipts, "effect_receipts"),
        ] {
            validate_sorted_unique(values, field)?;
        }
        for (values, field) in [
            (&self.ranked_order, "ranked_order"),
            (&self.qualified_order, "qualified_order"),
        ] {
            validate_unique(values, field)?;
        }
        if self
            .offered_capability_order
            .iter()
            .chain(self.required_capability_order.iter())
            .chain(self.negotiated_capability_order.iter())
            .any(|capability| !CAPABILITY_ORDER.contains(&capability.as_str()))
        {
            return Err(RetrievalProtocolError::Invalid(
                "protocol capability is not declared".into(),
            ));
        }
        let expected_negotiated = self
            .required_capability_order
            .iter()
            .filter(|capability| self.offered_capability_order.contains(capability))
            .cloned()
            .collect::<Vec<_>>();
        if self.negotiated_capability_order != expected_negotiated {
            return Err(RetrievalProtocolError::Invalid(
                "negotiated capabilities do not match the offered/required intersection".into(),
            ));
        }
        let candidate_keys = identity_keys(&self.candidate_order);
        if identity_keys(&self.ranked_order) != candidate_keys {
            return Err(RetrievalProtocolError::Invalid(
                "ranked order must contain every candidate exactly once".into(),
            ));
        }
        if self
            .ranked_order
            .iter()
            .any(|candidate| !self.candidate_order.contains(candidate))
            || self
                .qualified_order
                .iter()
                .any(|candidate| !self.candidate_order.contains(candidate))
            || self
                .blocked_order
                .iter()
                .any(|candidate| !self.candidate_order.contains(candidate))
            || self
                .unknown_order
                .iter()
                .any(|candidate| !self.blocked_order.contains(candidate))
        {
            return Err(RetrievalProtocolError::Invalid(
                "protocol state contains an identity outside its candidate set".into(),
            ));
        }
        let qualified_keys = identity_keys(&self.qualified_order);
        let blocked_keys = identity_keys(&self.blocked_order);
        let unknown_keys = identity_keys(&self.unknown_order);
        if !qualified_keys.is_disjoint(&blocked_keys)
            || !unknown_keys.is_subset(&blocked_keys)
            || qualified_keys
                .union(&blocked_keys)
                .cloned()
                .collect::<BTreeSet<_>>()
                != candidate_keys
        {
            return Err(RetrievalProtocolError::Invalid(
                "candidate states must partition candidates and keep unknown items blocked".into(),
            ));
        }
        if self.completed_stage_order.len() > self.stage_order.len()
            || self.completed_stage_order != self.stage_order[..self.completed_stage_order.len()]
            || self.blocked_stage_order != self.stage_order[self.completed_stage_order.len()..]
            || self
                .completed_stage_order
                .iter()
                .any(|stage| self.blocked_stage_order.contains(stage))
        {
            return Err(RetrievalProtocolError::Invalid(
                "protocol stage transcript is not a canonical prefix and suffix".into(),
            ));
        }
        let expected_action_receipts = self
            .completed_stage_order
            .iter()
            .map(|stage| format!("stage:completed:{stage}"))
            .collect::<Vec<_>>();
        let mut expected_action_receipts = expected_action_receipts;
        expected_action_receipts.sort();
        if self.action_receipts != expected_action_receipts {
            return Err(RetrievalProtocolError::Invalid(
                "protocol action receipts are not bound to completed stages".into(),
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
        let expected_effect_receipts = if matches!(
            self.disposition,
            SynthesisDisposition::Qualified | SynthesisDisposition::Partial
        ) {
            vec![format!("read:local-retrieval-protocol:{}", self.session_id)]
        } else {
            vec!["block:unsafe-release".into()]
        };
        if self.effect_receipts != expected_effect_receipts {
            return Err(RetrievalProtocolError::Invalid(
                "protocol effect receipts do not match disposition".into(),
            ));
        }
        if !self.raw_data_local
            && (self.disposition != SynthesisDisposition::Blocked
                || !self
                    .omissions
                    .iter()
                    .any(|item| item == "protocol:raw-data-locality-failed"))
        {
            return Err(RetrievalProtocolError::Invalid(
                "non-local protocols must be blocked and retain locality evidence".into(),
            ));
        }
        let expected_negotiation_digest = ContentHash::of_value(&json!({
            "protocol_id": self.protocol_id,
            "offered": self.offered_capability_order,
            "required": self.required_capability_order,
            "negotiated": self.negotiated_capability_order,
        }))
        .map_err(|error| RetrievalProtocolError::Artifact(error.to_string()))?;
        if self.negotiation_digest != expected_negotiation_digest {
            return Err(RetrievalProtocolError::Invalid(
                "negotiation digest is not bound to capabilities".into(),
            ));
        }
        let expected_transcript_digest = ContentHash::of_value(&json!({
            "session_id": self.session_id,
            "stage_order": self.stage_order,
            "completed": self.completed_stage_order,
            "blocked": self.blocked_stage_order,
            "negotiation_digest": self.negotiation_digest,
            "replay_identity": self.replay_identity,
        }))
        .map_err(|error| RetrievalProtocolError::Artifact(error.to_string()))?;
        if self.transcript_digest != expected_transcript_digest {
            return Err(RetrievalProtocolError::Invalid(
                "transcript digest is not bound to stage state".into(),
            ));
        }
        let expected_protocol_digest = ContentHash::of_value(&json!({
            "feature_id": FEATURE_ID,
            "request_id": self.request_id,
            "protocol_id": self.protocol_id,
            "session_id": self.session_id,
            "disposition": self.disposition,
            "negotiation_digest": self.negotiation_digest,
            "transcript_digest": self.transcript_digest,
            "synthesis_digest": self.synthesis_digest,
            "replay_identity": self.replay_identity,
            "raw_data_local": self.raw_data_local,
        }))
        .map_err(|error| RetrievalProtocolError::Artifact(error.to_string()))?;
        if self.protocol_digest != expected_protocol_digest {
            return Err(RetrievalProtocolError::Invalid(
                "protocol digest is not bound to transport state".into(),
            ));
        }
        if (self.disposition == SynthesisDisposition::Blocked
            && self.completed_stage_order.len() != 2)
            || (self.disposition != SynthesisDisposition::Blocked
                && self.completed_stage_order.len() != self.stage_order.len())
        {
            return Err(RetrievalProtocolError::Invalid(
                "protocol disposition does not match stage completion".into(),
            ));
        }
        let expected_artifact_id = format!("brain-retrieval-protocol:{}", self.session_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != PROTOCOL_CONTENT_TYPE
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(RetrievalProtocolError::Invalid(
                "protocol artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| RetrievalProtocolError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
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
        && u64::from(request.budget_units) >= u64::try_from(STAGE_ORDER.len()).unwrap_or(u64::MAX)
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
    if u64::from(request.budget_units) < u64::try_from(STAGE_ORDER.len()).unwrap_or(u64::MAX) {
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
        "raw_data_local": true,
    }))
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
        "action_receipts": action_receipts,
        "candidate_order": synthesis.candidate_order,
        "ranked_order": synthesis.ranked_order,
        "qualified_order": synthesis.qualified_order,
        "blocked_order": synthesis.blocked_order,
        "unknown_order": synthesis.unknown_order,
        "negotiation_digest": negotiation_digest,
        "transcript_digest": transcript_digest,
        "synthesis_digest": synthesis.synthesis_digest,
        "protocol_digest": protocol_digest,
        "replay_identity": request.replay_identity,
        "budget_units": request.budget_units,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "negative_evidence": negative_evidence,
        "effect_receipts": effect_receipts,
        "raw_data_local": true,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-retrieval-protocol:{}", request.session_id),
        PROTOCOL_CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| RetrievalProtocolError::Artifact(error.to_string()))?;
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
    for (value, field) in [
        (&request.protocol_id, "protocol_id"),
        (&request.session_id, "session_id"),
        (&request.boundary, "boundary"),
    ] {
        validate_text(value, field)?;
    }
    if request.boundary != PRECLINICAL_BOUNDARY
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
        || request.replay_identity != request.request.replay_identity
    {
        return Err(RetrievalProtocolError::Invalid(
            "protocol identity, capability declaration, stage order, budget, or boundary is invalid".into(),
        ));
    }
    for (values, field) in [
        (
            &request.offered_capability_order,
            "offered_capability_order",
        ),
        (
            &request.required_capability_order,
            "required_capability_order",
        ),
    ] {
        for value in values {
            validate_text(value, field)?;
        }
    }
    if request.replay_identity.as_str().len() != 64 {
        return Err(RetrievalProtocolError::Invalid(
            "protocol replay identity digest is invalid".into(),
        ));
    }
    Ok(())
}

fn is_sorted_unique(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn identity_keys(values: &[String]) -> BTreeSet<String> {
    values
        .iter()
        .map(|value| value.to_ascii_lowercase())
        .collect()
}

fn validate_text(value: &str, field: &str) -> Result<(), RetrievalProtocolError> {
    if value.trim().is_empty()
        || value != value.trim()
        || value.len() > MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(RetrievalProtocolError::Invalid(format!(
            "{field} must be bounded, non-empty text without padding or control characters"
        )));
    }
    Ok(())
}

fn validate_unique(values: &[String], field: &str) -> Result<(), RetrievalProtocolError> {
    let mut seen = BTreeSet::new();
    for value in values {
        validate_text(value, field)?;
        if !seen.insert(value.to_ascii_lowercase()) {
            return Err(RetrievalProtocolError::Invalid(format!(
                "{field} contains duplicate or case-colliding values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_unique(values: &[String], field: &str) -> Result<(), RetrievalProtocolError> {
    validate_unique(values, field)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(RetrievalProtocolError::Invalid(format!(
            "{field} is not in canonical order"
        )));
    }
    Ok(())
}

fn receipt_payload(receipt: &RetrievalProtocolReceipt) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "request_id": receipt.request_id,
        "protocol_id": receipt.protocol_id,
        "session_id": receipt.session_id,
        "disposition": receipt.disposition,
        "offered_capability_order": receipt.offered_capability_order,
        "required_capability_order": receipt.required_capability_order,
        "negotiated_capability_order": receipt.negotiated_capability_order,
        "stage_order": receipt.stage_order,
        "completed_stage_order": receipt.completed_stage_order,
        "blocked_stage_order": receipt.blocked_stage_order,
        "action_receipts": receipt.action_receipts,
        "candidate_order": receipt.candidate_order,
        "ranked_order": receipt.ranked_order,
        "qualified_order": receipt.qualified_order,
        "blocked_order": receipt.blocked_order,
        "unknown_order": receipt.unknown_order,
        "negotiation_digest": receipt.negotiation_digest,
        "transcript_digest": receipt.transcript_digest,
        "synthesis_digest": receipt.synthesis_digest,
        "protocol_digest": receipt.protocol_digest,
        "replay_identity": receipt.replay_identity,
        "budget_units": receipt.budget_units,
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
    fn locality_failure_is_blocked_and_retained() {
        let mut request = protocol_request();
        request.raw_data_local = false;
        let receipt = compile_retrieval_protocol(&request).unwrap();
        assert_eq!(receipt.disposition, SynthesisDisposition::Blocked);
        assert!(receipt.raw_data_local);
        assert!(receipt
            .omissions
            .iter()
            .any(|value| value == "protocol:raw-data-locality-failed"));
        assert!(receipt.validate().is_ok());
    }

    #[test]
    fn protocol_artifact_payload_is_bound() {
        let mut receipt = compile_retrieval_protocol(&protocol_request()).unwrap();
        receipt.session_id = "session:tampered".into();
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn case_mismatched_candidate_identity_is_rejected() {
        let mut receipt = compile_retrieval_protocol(&protocol_request()).unwrap();
        receipt.qualified_order[0] = receipt.qualified_order[0].to_ascii_uppercase();
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn digest_is_stable() {
        let receipt = compile_retrieval_protocol(&protocol_request()).unwrap();
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
}

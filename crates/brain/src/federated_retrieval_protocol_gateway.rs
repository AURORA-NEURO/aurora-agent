//! Typed federated retrieval protocol gateway.
//!
//! Atlas feature: `AFA-brain-P02-F24`. This product negotiates a federation-bound retrieval
//! session and never treats a protocol handshake as permission to move raw observations.

use crate::federated_retrieval_synthesis::{
    synthesize_federated_retrieval, FederatedRetrievalDisposition, FederatedRetrievalQuery,
};
use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-brain-P02-F24";
pub const CONTRACT_VERSION: &str = "brain-federated-retrieval-protocol-gateway/1.0";
pub const STAGE_ORDER: [&str; 5] = [
    "protocol:open",
    "protocol:authorize",
    "protocol:retrieve",
    "protocol:synthesize",
    "protocol:close",
];
pub const CAPABILITY_ORDER: [&str; 5] = [
    "capability:aggregate-envelope-v1",
    "capability:comparability-v1",
    "capability:evidence-synthesis-v1",
    "capability:omission-receipt-v1",
    "capability:replay-v1",
];
const PROTOCOL_CONTENT_TYPE: &str =
    "application/vnd.aurora.federated-retrieval-protocol-receipt+json";
const MAX_TEXT_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedRetrievalProtocolRequest {
    pub request: FederatedRetrievalQuery,
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
pub struct FederatedRetrievalProtocolReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub protocol_id: String,
    pub session_id: String,
    pub federation_id: String,
    pub institution_id: String,
    pub purpose: String,
    pub endpoint: String,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub disposition: FederatedRetrievalDisposition,
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
    pub aggregate_order: Vec<ContentHash>,
    pub comparability_digest: ContentHash,
    pub envelope_digest: ContentHash,
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
pub enum FederatedRetrievalProtocolError {
    #[error("invalid federated retrieval protocol request: {0}")]
    Invalid(String),
    #[error("federated retrieval protocol artifact failed: {0}")]
    Artifact(String),
    #[error("federated retrieval protocol synthesis failed: {0}")]
    Engine(String),
}

impl FederatedRetrievalProtocolReceipt {
    pub fn validate(&self) -> Result<(), FederatedRetrievalProtocolError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.study_order.len() < 2
            || self.modality_order.len() < 2
            || self.offered_capability_order.is_empty()
            || self.required_capability_order.is_empty()
            || self.stage_order != STAGE_ORDER
            || self.completed_stage_order.is_empty()
            || self.action_receipts.is_empty()
            || self.candidate_order.is_empty()
            || self.budget_units == 0
        {
            return Err(FederatedRetrievalProtocolError::Invalid(
                "federated protocol identity, coverage, negotiation, stages, locality, budget, or effects are incomplete".into(),
            ));
        }
        for (values, field) in [
            (&self.study_order, "study_order"),
            (&self.modality_order, "modality_order"),
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
        ] {
            validate_sorted_unique(values, field)?;
        }
        validate_unique(&self.ranked_order, "ranked_order")?;
        validate_unique(&self.qualified_order, "qualified_order")?;
        for (value, field) in [
            (&self.request_id, "request_id"),
            (&self.protocol_id, "protocol_id"),
            (&self.session_id, "session_id"),
            (&self.federation_id, "federation_id"),
            (&self.institution_id, "institution_id"),
            (&self.purpose, "purpose"),
            (&self.endpoint, "endpoint"),
            (&self.boundary, "boundary"),
        ] {
            validate_text(value, field)?;
        }
        validate_digest_order(&self.aggregate_order)?;
        if self
            .offered_capability_order
            .iter()
            .chain(self.required_capability_order.iter())
            .chain(self.negotiated_capability_order.iter())
            .any(|capability| !CAPABILITY_ORDER.contains(&capability.as_str()))
        {
            return Err(FederatedRetrievalProtocolError::Invalid(
                "federated protocol capability is outside the declared capability vocabulary"
                    .into(),
            ));
        }
        let expected_negotiated = self
            .required_capability_order
            .iter()
            .filter(|capability| self.offered_capability_order.contains(capability))
            .cloned()
            .collect::<Vec<_>>();
        if self.negotiated_capability_order != expected_negotiated {
            return Err(FederatedRetrievalProtocolError::Invalid(
                "federated negotiated capabilities do not equal the declared intersection".into(),
            ));
        }
        let candidate_keys = identity_keys(&self.candidate_order);
        let ranked_keys = identity_keys(&self.ranked_order);
        let qualified_keys = identity_keys(&self.qualified_order);
        let blocked_keys = identity_keys(&self.blocked_order);
        let unknown_keys = identity_keys(&self.unknown_order);
        if ranked_keys != candidate_keys
            || qualified_keys
                .union(&blocked_keys)
                .cloned()
                .collect::<BTreeSet<_>>()
                != candidate_keys
            || !qualified_keys.is_disjoint(&blocked_keys)
            || !unknown_keys.is_subset(&blocked_keys)
            || self.aggregate_order.len() != self.qualified_order.len()
        {
            return Err(FederatedRetrievalProtocolError::Invalid(
                "federated protocol synthesis state does not partition candidates".into(),
            ));
        }
        if self.completed_stage_order.len() > STAGE_ORDER.len()
            || self.completed_stage_order.len() + self.blocked_stage_order.len()
                != STAGE_ORDER.len()
        {
            return Err(FederatedRetrievalProtocolError::Invalid(
                "federated protocol stage transcript does not cover the protocol".into(),
            ));
        }
        let expected_completed = STAGE_ORDER[..self.completed_stage_order.len()]
            .iter()
            .map(|value| (*value).to_string())
            .collect::<Vec<_>>();
        let expected_blocked = STAGE_ORDER[self.completed_stage_order.len()..]
            .iter()
            .map(|value| (*value).to_string())
            .collect::<Vec<_>>();
        if self.completed_stage_order != expected_completed
            || self.blocked_stage_order != expected_blocked
        {
            return Err(FederatedRetrievalProtocolError::Invalid(
                "federated protocol stage transcript is not a canonical prefix and suffix".into(),
            ));
        }
        for digest in [
            &self.comparability_digest,
            &self.envelope_digest,
            &self.negotiation_digest,
            &self.transcript_digest,
            &self.synthesis_digest,
            &self.protocol_digest,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(FederatedRetrievalProtocolError::Invalid(
                    "federated protocol digest is invalid".into(),
                ));
            }
        }
        if !self.raw_data_local && self.disposition != FederatedRetrievalDisposition::Blocked {
            return Err(FederatedRetrievalProtocolError::Invalid(
                "non-local federated protocol receipts must be blocked".into(),
            ));
        }
        if !self.raw_data_local
            && !self
                .omissions
                .iter()
                .any(|omission| omission == "protocol:raw-data-locality-failed")
        {
            return Err(FederatedRetrievalProtocolError::Invalid(
                "non-local federated protocol receipts must retain a locality omission".into(),
            ));
        }
        let expected_effect_receipts = if matches!(
            self.disposition,
            FederatedRetrievalDisposition::Qualified | FederatedRetrievalDisposition::Partial
        ) {
            vec![format!("read:local-federated-protocol:{}", self.session_id)]
        } else {
            vec!["block:unsafe-release".into()]
        };
        if self.effect_receipts != expected_effect_receipts {
            return Err(FederatedRetrievalProtocolError::Invalid(
                "federated protocol effect does not match disposition".into(),
            ));
        }
        let expected_negotiation_digest = ContentHash::of_value(&json!({
            "protocol_id": self.protocol_id,
            "offered": self.offered_capability_order,
            "required": self.required_capability_order,
            "negotiated": self.negotiated_capability_order,
        }))
        .map_err(|error| FederatedRetrievalProtocolError::Artifact(error.to_string()))?;
        if self.negotiation_digest != expected_negotiation_digest {
            return Err(FederatedRetrievalProtocolError::Invalid(
                "federated negotiation digest is not bound to capabilities".into(),
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
        .map_err(|error| FederatedRetrievalProtocolError::Artifact(error.to_string()))?;
        if self.transcript_digest != expected_transcript_digest {
            return Err(FederatedRetrievalProtocolError::Invalid(
                "federated protocol transcript digest is not bound to stages".into(),
            ));
        }
        let expected_artifact_id =
            format!("brain-federated-retrieval-protocol:{}", self.session_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != PROTOCOL_CONTENT_TYPE
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(FederatedRetrievalProtocolError::Invalid(
                "federated protocol artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| FederatedRetrievalProtocolError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
            .map_err(|error| FederatedRetrievalProtocolError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, FederatedRetrievalProtocolError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| FederatedRetrievalProtocolError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| FederatedRetrievalProtocolError::Artifact(error.to_string()))
    }
}

fn validate_text(value: &str, field: &str) -> Result<(), FederatedRetrievalProtocolError> {
    if value.trim() != value
        || value.is_empty()
        || value.len() > MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(FederatedRetrievalProtocolError::Invalid(format!(
            "{field} is empty, padded, oversized, or contains control characters"
        )));
    }
    Ok(())
}

fn validate_unique(values: &[String], field: &str) -> Result<(), FederatedRetrievalProtocolError> {
    let mut seen = BTreeSet::new();
    for value in values {
        validate_text(value, field)?;
        if !seen.insert(value.to_ascii_lowercase()) {
            return Err(FederatedRetrievalProtocolError::Invalid(format!(
                "{field} contains a duplicate or case-colliding identity"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_unique(
    values: &[String],
    field: &str,
) -> Result<(), FederatedRetrievalProtocolError> {
    validate_unique(values, field)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(FederatedRetrievalProtocolError::Invalid(format!(
            "{field} is not in canonical order"
        )));
    }
    Ok(())
}

fn identity_keys(values: &[String]) -> BTreeSet<String> {
    values.iter().cloned().collect()
}

fn validate_digest_order(values: &[ContentHash]) -> Result<(), FederatedRetrievalProtocolError> {
    if values.windows(2).any(|pair| pair[0] >= pair[1])
        || values.iter().any(|value| value.as_str().len() != 64)
    {
        return Err(FederatedRetrievalProtocolError::Invalid(
            "federated aggregate ordering or digest is invalid".into(),
        ));
    }
    Ok(())
}

fn receipt_payload(receipt: &FederatedRetrievalProtocolReceipt) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "request_id": receipt.request_id,
        "protocol_id": receipt.protocol_id,
        "session_id": receipt.session_id,
        "federation_id": receipt.federation_id,
        "institution_id": receipt.institution_id,
        "purpose": receipt.purpose,
        "endpoint": receipt.endpoint,
        "study_order": receipt.study_order,
        "modality_order": receipt.modality_order,
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
        "aggregate_order": receipt.aggregate_order,
        "comparability_digest": receipt.comparability_digest,
        "envelope_digest": receipt.envelope_digest,
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

pub fn federated_retrieval_protocol_gateway_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "brain".into(),
        consumers: ["federation research steward".into(), "federated retrieval operator".into()].into(),
        behavior: "negotiates a purpose-bound federated retrieval protocol while retaining raw-data locality and aggregate-envelope evidence".into(),
        value: "makes federation capability, signer, approval, comparability, and denial state interoperable without silently moving protected observations".into(),
        inputs: vec![TypedPort { name: "federated_retrieval_protocol_request".into(), schema: "ResearchWorkflowSpec2@1".into(), required: true }],
        outputs: vec![TypedPort { name: "federated_retrieval_protocol_receipt".into(), schema: "FederatedRetrievalProtocolReceipt1@1".into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(),
        permissions: ["read:local-research-artifacts".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "ga4gh-drs".into(), state: EvidenceState::Supported, locator: Some("https://ga4gh.github.io/data-repository-service-schemas/preview/release/drs-1.3.0/docs/".into()) }],
        authority_requirements: vec![AuthorityRequirement {
            role: "federated retrieval protocol approver".into(),
            reason: "authorize capability negotiation and aggregate-only retrieval stages before A2 federation effects".into(),
        }],
        autonomy_tier: AutonomyTier::A2,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn compile_federated_retrieval_protocol(
    request: &FederatedRetrievalProtocolRequest,
) -> Result<FederatedRetrievalProtocolReceipt, FederatedRetrievalProtocolError> {
    validate_request(request)?;
    let synthesis = synthesize_federated_retrieval(&request.request)
        .map_err(|error| FederatedRetrievalProtocolError::Engine(error.to_string()))?;
    let negotiated = request
        .required_capability_order
        .iter()
        .filter(|value| request.offered_capability_order.contains(value))
        .cloned()
        .collect::<Vec<_>>();
    let missing = request
        .required_capability_order
        .iter()
        .filter(|value| !request.offered_capability_order.contains(value))
        .cloned()
        .collect::<Vec<_>>();
    let gate = request.policy_allow
        && request.protected_closure
        && request.raw_data_local
        && request.request.signer_valid
        && request.request.approval_valid
        && u64::from(request.budget_units) >= u64::try_from(STAGE_ORDER.len()).unwrap_or(u64::MAX)
        && missing.is_empty();
    let disposition = if gate {
        synthesis.disposition
    } else {
        FederatedRetrievalDisposition::Blocked
    };
    let completed: Vec<String> = if gate {
        STAGE_ORDER
            .iter()
            .map(|value| (*value).to_string())
            .collect()
    } else {
        STAGE_ORDER[..2]
            .iter()
            .map(|value| (*value).to_string())
            .collect()
    };
    let blocked_stages: Vec<String> = if gate {
        Vec::new()
    } else {
        STAGE_ORDER[2..]
            .iter()
            .map(|value| (*value).to_string())
            .collect()
    };
    let mut omissions = synthesis.omissions.iter().cloned().collect::<BTreeSet<_>>();
    let mut uncertainty = synthesis
        .uncertainty
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let negative = synthesis
        .negative_evidence
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    for capability in missing {
        omissions.insert(format!("protocol:capability-not-negotiated:{capability}"));
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
    if !request.request.signer_valid {
        omissions.insert("protocol:signer-invalid".into());
    }
    if !request.request.approval_valid {
        omissions.insert("protocol:approval-invalid".into());
    }
    if u64::from(request.budget_units) < u64::try_from(STAGE_ORDER.len()).unwrap_or(u64::MAX) {
        omissions.insert("protocol:budget-exhausted".into());
    }
    if disposition == FederatedRetrievalDisposition::Blocked {
        uncertainty.insert("protocol:release-blocked-until-all-gates-pass".into());
    }
    let action_receipts = completed
        .iter()
        .map(|stage| format!("stage:completed:{stage}"))
        .collect::<BTreeSet<_>>();
    let negotiation_digest = ContentHash::of_value(&json!({"protocol_id": request.protocol_id, "offered": request.offered_capability_order, "required": request.required_capability_order, "negotiated": negotiated})).map_err(|error| FederatedRetrievalProtocolError::Artifact(error.to_string()))?;
    let transcript_digest = ContentHash::of_value(&json!({"session_id": request.session_id, "stage_order": STAGE_ORDER, "completed": completed, "blocked": blocked_stages, "negotiation_digest": negotiation_digest, "replay_identity": request.replay_identity})).map_err(|error| FederatedRetrievalProtocolError::Artifact(error.to_string()))?;
    let protocol_digest = ContentHash::of_value(&json!({"feature_id": FEATURE_ID, "request_id": request.request.request_id, "protocol_id": request.protocol_id, "session_id": request.session_id, "federation_id": request.request.federation_id, "institution_id": request.request.institution_id, "purpose": request.request.purpose, "disposition": disposition, "comparability_digest": synthesis.comparability_digest, "envelope_digest": synthesis.envelope_digest, "negotiation_digest": negotiation_digest, "transcript_digest": transcript_digest, "synthesis_digest": synthesis.synthesis_digest, "replay_identity": request.replay_identity})).map_err(|error| FederatedRetrievalProtocolError::Artifact(error.to_string()))?;
    let effect_receipts = if matches!(
        disposition,
        FederatedRetrievalDisposition::Qualified | FederatedRetrievalDisposition::Partial
    ) {
        vec![format!(
            "read:local-federated-protocol:{}",
            request.session_id
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request.request_id, "protocol_id": request.protocol_id, "session_id": request.session_id, "federation_id": request.request.federation_id, "institution_id": request.request.institution_id, "purpose": request.request.purpose, "endpoint": request.request.endpoint, "study_order": request.request.study_ids, "modality_order": request.request.required_modalities, "disposition": disposition, "offered_capability_order": request.offered_capability_order, "required_capability_order": request.required_capability_order, "negotiated_capability_order": negotiated, "stage_order": STAGE_ORDER, "completed_stage_order": completed, "blocked_stage_order": blocked_stages, "action_receipts": action_receipts, "candidate_order": synthesis.candidate_order, "ranked_order": synthesis.ranked_order, "qualified_order": synthesis.qualified_order, "blocked_order": synthesis.blocked_order, "unknown_order": synthesis.unknown_order, "aggregate_order": synthesis.aggregate_order, "comparability_digest": synthesis.comparability_digest, "envelope_digest": synthesis.envelope_digest, "negotiation_digest": negotiation_digest, "transcript_digest": transcript_digest, "synthesis_digest": synthesis.synthesis_digest, "protocol_digest": protocol_digest, "replay_identity": request.replay_identity, "budget_units": request.budget_units, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "effect_receipts": effect_receipts, "raw_data_local": true, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-federated-retrieval-protocol:{}", request.session_id),
        PROTOCOL_CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| FederatedRetrievalProtocolError::Artifact(error.to_string()))?;
    let receipt = FederatedRetrievalProtocolReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request.request_id.clone(),
        protocol_id: request.protocol_id.clone(),
        session_id: request.session_id.clone(),
        federation_id: request.request.federation_id.clone(),
        institution_id: request.request.institution_id.clone(),
        purpose: request.request.purpose.clone(),
        endpoint: request.request.endpoint.clone(),
        study_order: request.request.study_ids.clone(),
        modality_order: request.request.required_modalities.clone(),
        disposition,
        offered_capability_order: request.offered_capability_order.clone(),
        required_capability_order: request.required_capability_order.clone(),
        negotiated_capability_order: negotiated,
        stage_order: STAGE_ORDER.iter().map(|value| (*value).into()).collect(),
        completed_stage_order: completed,
        blocked_stage_order: blocked_stages,
        action_receipts: action_receipts.into_iter().collect(),
        candidate_order: synthesis.candidate_order,
        ranked_order: synthesis.ranked_order,
        qualified_order: synthesis.qualified_order,
        blocked_order: synthesis.blocked_order,
        unknown_order: synthesis.unknown_order,
        aggregate_order: synthesis.aggregate_order,
        comparability_digest: synthesis.comparability_digest,
        envelope_digest: synthesis.envelope_digest,
        negotiation_digest,
        transcript_digest,
        synthesis_digest: synthesis.synthesis_digest,
        protocol_digest,
        replay_identity: request.replay_identity.clone(),
        budget_units: request.budget_units,
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(
    request: &FederatedRetrievalProtocolRequest,
) -> Result<(), FederatedRetrievalProtocolError> {
    if request.boundary != PRECLINICAL_BOUNDARY
        || request.request.boundary != PRECLINICAL_BOUNDARY
        || request.offered_capability_order.is_empty()
        || request.required_capability_order.is_empty()
        || request.requested_stage_order != STAGE_ORDER
        || request.budget_units == 0
    {
        return Err(FederatedRetrievalProtocolError::Invalid(
            "federated protocol identity, capabilities, stages, budget, or boundary are invalid"
                .into(),
        ));
    }
    for (value, field) in [
        (&request.protocol_id, "protocol_id"),
        (&request.session_id, "session_id"),
        (&request.boundary, "boundary"),
        (&request.request.boundary, "request.boundary"),
        (&request.request.federation_id, "federation_id"),
        (&request.request.institution_id, "institution_id"),
        (&request.request.purpose, "purpose"),
        (&request.request.endpoint, "endpoint"),
    ] {
        validate_text(value, field)?;
    }
    validate_sorted_unique(
        &request.offered_capability_order,
        "offered_capability_order",
    )?;
    validate_sorted_unique(
        &request.required_capability_order,
        "required_capability_order",
    )?;
    for capability in request
        .offered_capability_order
        .iter()
        .chain(request.required_capability_order.iter())
    {
        if !CAPABILITY_ORDER.contains(&capability.as_str()) {
            return Err(FederatedRetrievalProtocolError::Invalid(
                "federated protocol capability is outside the declared vocabulary".into(),
            ));
        }
    }
    if request.replay_identity.as_str().len() != 64
        || request.request.replay_identity.as_str().len() != 64
        || request.replay_identity != request.request.replay_identity
    {
        return Err(FederatedRetrievalProtocolError::Invalid(
            "federated protocol replay identity is invalid or mismatched".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retrieval_synthesis::RetrievalCandidate;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn request() -> FederatedRetrievalProtocolRequest {
        let candidate = RetrievalCandidate {
            evidence_id: "evidence:federated".into(),
            source_id: "source:federated".into(),
            study_id: "study:a".into(),
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
        };
        FederatedRetrievalProtocolRequest {
            request: FederatedRetrievalQuery {
                request_id: "request:federated-protocol".into(),
                federation_id: "federation:one".into(),
                institution_id: "institution:a".into(),
                purpose: "preclinical-evidence-benchmark".into(),
                semantic_profile: "profile:organoid-v1".into(),
                endpoint: "endpoint:local".into(),
                allowed_artifacts: vec!["qualified-evidence-summary".into()],
                study_ids: vec!["study:a".into(), "study:b".into()],
                scope: "organoid:neural".into(),
                minimum_support_milli: 700,
                required_modalities: vec!["imaging".into(), "transcriptomics".into()],
                candidates: vec![candidate],
                replay_identity: hash("replay"),
                policy_allow: true,
                protected_closure: true,
                signer_valid: true,
                approval_valid: true,
                raw_data_local: true,
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            protocol_id: "protocol:federated-v1".into(),
            session_id: "session:federated".into(),
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
    fn manifest_is_a2() {
        let manifest = federated_retrieval_protocol_gateway_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A2);
    }
    #[test]
    fn federated_protocol_blocks_incomplete_closure() {
        let mut value = request();
        value.protected_closure = false;
        let receipt = compile_federated_retrieval_protocol(&value).unwrap();
        assert_eq!(receipt.disposition, FederatedRetrievalDisposition::Blocked);
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item == "protocol:protected-closure-incomplete"));
    }
    #[test]
    fn federated_protocol_preserves_locality() {
        let receipt = compile_federated_retrieval_protocol(&request()).unwrap();
        assert!(receipt.raw_data_local);
        assert!(receipt
            .effect_receipts
            .iter()
            .all(|value| value.starts_with("read:local-federated-protocol:")
                || value == "block:unsafe-release"));
    }
    #[test]
    fn digest_is_stable() {
        let receipt = compile_federated_retrieval_protocol(&request()).unwrap();
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
    #[test]
    fn low_budget_and_locality_failures_are_retained() {
        let mut low_budget = request();
        low_budget.budget_units = 1;
        let receipt = compile_federated_retrieval_protocol(&low_budget).unwrap();
        assert_eq!(receipt.disposition, FederatedRetrievalDisposition::Blocked);
        assert_eq!(receipt.completed_stage_order.len(), 2);
        receipt.validate().unwrap();

        let mut locality = request();
        locality.raw_data_local = false;
        let receipt = compile_federated_retrieval_protocol(&locality).unwrap();
        assert!(receipt.raw_data_local);

        assert!(receipt
            .omissions
            .iter()
            .any(|value| value == "protocol:raw-data-locality-failed"));
        receipt.validate().unwrap();
    }
    #[test]
    fn protocol_transcript_and_artifact_payload_are_bound() {
        let mut transcript_drift = compile_federated_retrieval_protocol(&request()).unwrap();
        transcript_drift.completed_stage_order.pop();
        assert!(transcript_drift.validate().is_err());

        let mut payload_drift = compile_federated_retrieval_protocol(&request()).unwrap();
        payload_drift.protocol_id = "protocol:other".into();
        assert!(payload_drift.validate().is_err());
    }
    #[test]
    fn capability_aliases_are_rejected() {
        let mut input = request();
        input.offered_capability_order[0] = "CAPABILITY:aggregate-envelope-v1".into();
        assert!(compile_federated_retrieval_protocol(&input).is_err());
    }
    #[test]
    fn case_mismatched_ranked_identity_is_rejected() {
        let mut receipt = compile_federated_retrieval_protocol(&request()).unwrap();
        receipt.ranked_order[0] = receipt.ranked_order[0].to_ascii_uppercase();
        assert!(receipt.validate().is_err());
    }
}

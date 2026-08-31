//! Typed high-throughput retrieval protocol gateway.
//!
//! Atlas feature: `AFA-brain-P02-F23`. This product negotiates a batch retrieval session while
//! making queue identity, checkpoint continuity, overflow, and blocked release explicit.

use crate::retrieval_synthesis::SynthesisDisposition;
use crate::throughput_retrieval_synthesis::{
    synthesize_throughput_retrieval, ThroughputRetrievalQuery,
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

pub const FEATURE_ID: &str = "AFA-brain-P02-F23";
pub const CONTRACT_VERSION: &str = "brain-throughput-retrieval-protocol-gateway/1.0";
pub const STAGE_ORDER: [&str; 5] = [
    "protocol:open",
    "protocol:authorize",
    "protocol:retrieve",
    "protocol:synthesize",
    "protocol:close",
];
pub const CAPABILITY_ORDER: [&str; 4] = [
    "capability:batch-admission-v1",
    "capability:checkpoint-replay-v1",
    "capability:evidence-synthesis-v1",
    "capability:omission-receipt-v1",
];
const PROTOCOL_CONTENT_TYPE: &str =
    "application/vnd.aurora.throughput-retrieval-protocol-receipt+json";
const MAX_TEXT_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputRetrievalProtocolRequest {
    pub request: ThroughputRetrievalQuery,
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
pub struct ThroughputRetrievalProtocolReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub protocol_id: String,
    pub session_id: String,
    pub batch_id: String,
    pub partition: String,
    pub checkpoint_seq: u64,
    pub queue_digest: ContentHash,
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
pub enum ThroughputRetrievalProtocolError {
    #[error("invalid throughput retrieval protocol request: {0}")]
    Invalid(String),
    #[error("throughput retrieval protocol artifact failed: {0}")]
    Artifact(String),
    #[error("throughput retrieval protocol synthesis failed: {0}")]
    Engine(String),
}

impl ThroughputRetrievalProtocolReceipt {
    pub fn validate(&self) -> Result<(), ThroughputRetrievalProtocolError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.request_id.trim().is_empty()
            || self.protocol_id.trim().is_empty()
            || self.session_id.trim().is_empty()
            || self.batch_id.trim().is_empty()
            || self.partition.trim().is_empty()
            || self.checkpoint_seq == 0
            || self.offered_capability_order.is_empty()
            || self.required_capability_order.is_empty()
            || self.stage_order != STAGE_ORDER
            || self.completed_stage_order.is_empty()
            || self.action_receipts.is_empty()
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
            || self.budget_units == 0
        {
            return Err(ThroughputRetrievalProtocolError::Invalid(
                "throughput protocol identity, queue, negotiation, stages, budget, locality, or effects are incomplete".into(),
            ));
        }
        validate_text(&self.request_id, "request_id")?;
        validate_text(&self.protocol_id, "protocol_id")?;
        validate_text(&self.session_id, "session_id")?;
        validate_text(&self.batch_id, "batch_id")?;
        validate_text(&self.partition, "partition")?;
        for capability in self
            .offered_capability_order
            .iter()
            .chain(self.required_capability_order.iter())
            .chain(self.negotiated_capability_order.iter())
        {
            if !CAPABILITY_ORDER.contains(&capability.as_str()) {
                return Err(ThroughputRetrievalProtocolError::Invalid(
                    "throughput protocol capability is not declared".into(),
                ));
            }
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
        let expected_negotiated = self
            .required_capability_order
            .iter()
            .filter(|capability| self.offered_capability_order.contains(capability))
            .cloned()
            .collect::<Vec<_>>();
        if self.negotiated_capability_order != expected_negotiated {
            return Err(ThroughputRetrievalProtocolError::Invalid(
                "throughput negotiated capabilities do not match the offered/required intersection"
                    .into(),
            ));
        }
        let candidate_keys = identity_keys(&self.candidate_order);
        if identity_keys(&self.ranked_order) != candidate_keys {
            return Err(ThroughputRetrievalProtocolError::Invalid(
                "throughput ranked order must contain every candidate exactly once".into(),
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
            return Err(ThroughputRetrievalProtocolError::Invalid(
                "throughput protocol state contains an identity outside its candidate set".into(),
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
            return Err(ThroughputRetrievalProtocolError::Invalid(
                "throughput candidate states must partition candidates and keep unknown items blocked".into(),
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
            || self.completed_stage_order.len() > self.stage_order.len()
            || self.completed_stage_order != self.stage_order[..self.completed_stage_order.len()]
            || self.blocked_stage_order != self.stage_order[self.completed_stage_order.len()..]
        {
            return Err(ThroughputRetrievalProtocolError::Invalid(
                "throughput protocol stage transcript is invalid".into(),
            ));
        }
        let mut expected_action_receipts = self
            .completed_stage_order
            .iter()
            .map(|stage| format!("stage:completed:{stage}"))
            .collect::<Vec<_>>();
        expected_action_receipts.sort();
        if self.action_receipts != expected_action_receipts {
            return Err(ThroughputRetrievalProtocolError::Invalid(
                "throughput action receipts are not bound to completed stages".into(),
            ));
        }
        for digest in [
            &self.queue_digest,
            &self.negotiation_digest,
            &self.transcript_digest,
            &self.synthesis_digest,
            &self.protocol_digest,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(ThroughputRetrievalProtocolError::Invalid(
                    "throughput protocol digest is invalid".into(),
                ));
            }
        }
        let expected_effect_receipts = if matches!(
            self.disposition,
            SynthesisDisposition::Qualified | SynthesisDisposition::Partial
        ) {
            vec![format!(
                "read:local-throughput-protocol:{}",
                self.session_id
            )]
        } else {
            vec!["block:unsafe-release".into()]
        };
        if self.effect_receipts != expected_effect_receipts {
            return Err(ThroughputRetrievalProtocolError::Invalid(
                "throughput protocol effect receipts do not match disposition".into(),
            ));
        }
        if !self.raw_data_local
            && (self.disposition != SynthesisDisposition::Blocked
                || !self
                    .omissions
                    .iter()
                    .any(|item| item == "protocol:raw-data-locality-failed"))
        {
            return Err(ThroughputRetrievalProtocolError::Invalid(
                "non-local throughput protocols must be blocked and retain locality evidence"
                    .into(),
            ));
        }
        let expected_negotiation_digest = ContentHash::of_value(&json!({
            "protocol_id": self.protocol_id,
            "offered": self.offered_capability_order,
            "required": self.required_capability_order,
            "negotiated": self.negotiated_capability_order,
        }))
        .map_err(|error| ThroughputRetrievalProtocolError::Artifact(error.to_string()))?;
        if self.negotiation_digest != expected_negotiation_digest {
            return Err(ThroughputRetrievalProtocolError::Invalid(
                "throughput negotiation digest is not bound to capabilities".into(),
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
        .map_err(|error| ThroughputRetrievalProtocolError::Artifact(error.to_string()))?;
        if self.transcript_digest != expected_transcript_digest {
            return Err(ThroughputRetrievalProtocolError::Invalid(
                "throughput transcript digest is not bound to stage state".into(),
            ));
        }
        let expected_protocol_digest = ContentHash::of_value(&json!({
            "feature_id": FEATURE_ID,
            "request_id": self.request_id,
            "protocol_id": self.protocol_id,
            "session_id": self.session_id,
            "batch_id": self.batch_id,
            "partition": self.partition,
            "disposition": self.disposition,
            "queue_digest": self.queue_digest,
            "negotiation_digest": self.negotiation_digest,
            "transcript_digest": self.transcript_digest,
            "synthesis_digest": self.synthesis_digest,
            "replay_identity": self.replay_identity,
            "raw_data_local": self.raw_data_local,
        }))
        .map_err(|error| ThroughputRetrievalProtocolError::Artifact(error.to_string()))?;
        if self.protocol_digest != expected_protocol_digest {
            return Err(ThroughputRetrievalProtocolError::Invalid(
                "throughput protocol digest is not bound to transport state".into(),
            ));
        }
        if (self.disposition == SynthesisDisposition::Blocked
            && self.completed_stage_order.len() != 2)
            || (self.disposition != SynthesisDisposition::Blocked
                && self.completed_stage_order.len() != self.stage_order.len())
        {
            return Err(ThroughputRetrievalProtocolError::Invalid(
                "throughput protocol disposition does not match stage completion".into(),
            ));
        }
        let expected_artifact_id =
            format!("brain-throughput-retrieval-protocol:{}", self.session_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != PROTOCOL_CONTENT_TYPE
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(ThroughputRetrievalProtocolError::Invalid(
                "throughput protocol artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ThroughputRetrievalProtocolError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
            .map_err(|error| ThroughputRetrievalProtocolError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, ThroughputRetrievalProtocolError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ThroughputRetrievalProtocolError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ThroughputRetrievalProtocolError::Artifact(error.to_string()))
    }
}

pub fn throughput_retrieval_protocol_gateway_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "brain".into(),
        consumers: ["research workflow operator".into(), "throughput queue operator".into()].into(),
        behavior: "negotiates a checkpointed high-throughput retrieval protocol with queue and overflow receipts".into(),
        value: "prevents batch protocol retries and queue pressure from becoming silent evidence loss".into(),
        inputs: vec![TypedPort { name: "throughput_retrieval_protocol_request".into(), schema: "ResearchWorkflowSpec2@1".into(), required: true }],
        outputs: vec![TypedPort { name: "throughput_retrieval_protocol_receipt".into(), schema: "ThroughputRetrievalProtocolReceipt1@1".into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(),
        permissions: ["read:local-research-artifacts".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "ga4gh-wes".into(), state: EvidenceState::Supported, locator: Some("https://ga4gh.github.io/workflow-execution-service-schemas/docs/".into()) }],
        authority_requirements: Vec::new(),
        autonomy_tier: AutonomyTier::A1,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn compile_throughput_retrieval_protocol(
    request: &ThroughputRetrievalProtocolRequest,
) -> Result<ThroughputRetrievalProtocolReceipt, ThroughputRetrievalProtocolError> {
    validate_request(request)?;
    let synthesis = synthesize_throughput_retrieval(&request.request)
        .map_err(|error| ThroughputRetrievalProtocolError::Engine(error.to_string()))?;
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
        && u64::from(request.budget_units) >= u64::try_from(STAGE_ORDER.len()).unwrap_or(u64::MAX)
        && missing.is_empty();
    let disposition = if gate {
        synthesis.disposition
    } else {
        SynthesisDisposition::Blocked
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
    if u64::from(request.budget_units) < u64::try_from(STAGE_ORDER.len()).unwrap_or(u64::MAX) {
        omissions.insert("protocol:budget-exhausted".into());
    }
    if disposition == SynthesisDisposition::Blocked {
        uncertainty.insert("protocol:release-blocked-until-all-gates-pass".into());
    }
    let action_receipts = completed
        .iter()
        .map(|stage| format!("stage:completed:{stage}"))
        .collect::<BTreeSet<_>>();
    let negotiation_digest = ContentHash::of_value(&json!({"protocol_id": request.protocol_id, "offered": request.offered_capability_order, "required": request.required_capability_order, "negotiated": negotiated})).map_err(|error| ThroughputRetrievalProtocolError::Artifact(error.to_string()))?;
    let transcript_digest = ContentHash::of_value(&json!({"session_id": request.session_id, "stage_order": STAGE_ORDER, "completed": completed, "blocked": blocked_stages, "negotiation_digest": negotiation_digest, "replay_identity": request.replay_identity})).map_err(|error| ThroughputRetrievalProtocolError::Artifact(error.to_string()))?;
    let protocol_digest = ContentHash::of_value(&json!({"feature_id": FEATURE_ID, "request_id": request.request.request_id, "protocol_id": request.protocol_id, "session_id": request.session_id, "batch_id": request.request.batch_id, "partition": request.request.partition, "disposition": disposition, "queue_digest": synthesis.queue_digest, "negotiation_digest": negotiation_digest, "transcript_digest": transcript_digest, "synthesis_digest": synthesis.synthesis_digest, "replay_identity": request.replay_identity, "raw_data_local": true})).map_err(|error| ThroughputRetrievalProtocolError::Artifact(error.to_string()))?;
    let effect_receipts = if matches!(
        disposition,
        SynthesisDisposition::Qualified | SynthesisDisposition::Partial
    ) {
        vec![format!(
            "read:local-throughput-protocol:{}",
            request.session_id
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request.request_id, "protocol_id": request.protocol_id, "session_id": request.session_id, "batch_id": request.request.batch_id, "partition": request.request.partition, "checkpoint_seq": synthesis.checkpoint_seq, "queue_digest": synthesis.queue_digest, "disposition": disposition, "stage_order": STAGE_ORDER, "completed_stage_order": completed, "blocked_stage_order": blocked_stages, "negotiation_digest": negotiation_digest, "transcript_digest": transcript_digest, "synthesis_digest": synthesis.synthesis_digest, "protocol_digest": protocol_digest, "replay_identity": request.replay_identity, "effect_receipts": effect_receipts, "raw_data_local": true, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-throughput-retrieval-protocol:{}", request.session_id),
        PROTOCOL_CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ThroughputRetrievalProtocolError::Artifact(error.to_string()))?;
    let receipt = ThroughputRetrievalProtocolReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request.request_id.clone(),
        protocol_id: request.protocol_id.clone(),
        session_id: request.session_id.clone(),
        batch_id: request.request.batch_id.clone(),
        partition: request.request.partition.clone(),
        checkpoint_seq: synthesis.checkpoint_seq,
        queue_digest: synthesis.queue_digest,
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
    request: &ThroughputRetrievalProtocolRequest,
) -> Result<(), ThroughputRetrievalProtocolError> {
    if request.boundary != PRECLINICAL_BOUNDARY
        || request.request.boundary != PRECLINICAL_BOUNDARY
        || request.offered_capability_order.is_empty()
        || request.required_capability_order.is_empty()
        || request.requested_stage_order != STAGE_ORDER
        || request.budget_units == 0
    {
        return Err(ThroughputRetrievalProtocolError::Invalid("throughput protocol identity, queue, capabilities, stages, budget, or boundary are invalid".into()));
    }
    validate_text(&request.protocol_id, "protocol_id")?;
    validate_text(&request.session_id, "session_id")?;
    validate_text(&request.request.batch_id, "batch_id")?;
    validate_text(&request.request.partition, "partition")?;
    validate_sorted_unique(
        &request.offered_capability_order,
        "offered_capability_order",
    )?;
    validate_sorted_unique(
        &request.required_capability_order,
        "required_capability_order",
    )?;
    if request
        .offered_capability_order
        .iter()
        .chain(request.required_capability_order.iter())
        .any(|value| !CAPABILITY_ORDER.contains(&value.as_str()))
    {
        return Err(ThroughputRetrievalProtocolError::Invalid(
            "throughput capability is not declared".into(),
        ));
    }
    Ok(())
}

fn receipt_payload(receipt: &ThroughputRetrievalProtocolReceipt) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "request_id": receipt.request_id,
        "protocol_id": receipt.protocol_id,
        "session_id": receipt.session_id,
        "batch_id": receipt.batch_id,
        "partition": receipt.partition,
        "checkpoint_seq": receipt.checkpoint_seq,
        "queue_digest": receipt.queue_digest,
        "disposition": receipt.disposition,
        "stage_order": receipt.stage_order,
        "completed_stage_order": receipt.completed_stage_order,
        "blocked_stage_order": receipt.blocked_stage_order,
        "negotiation_digest": receipt.negotiation_digest,
        "transcript_digest": receipt.transcript_digest,
        "synthesis_digest": receipt.synthesis_digest,
        "protocol_digest": receipt.protocol_digest,
        "replay_identity": receipt.replay_identity,
        "effect_receipts": receipt.effect_receipts,
        "raw_data_local": receipt.raw_data_local,
        "boundary": receipt.boundary,
    })
}

fn validate_text(value: &str, field: &str) -> Result<(), ThroughputRetrievalProtocolError> {
    if value.trim().is_empty()
        || value != value.trim()
        || value.len() > MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(ThroughputRetrievalProtocolError::Invalid(format!(
            "{field} must be bounded, non-empty text without padding or control characters"
        )));
    }
    Ok(())
}

fn validate_unique(values: &[String], field: &str) -> Result<(), ThroughputRetrievalProtocolError> {
    let mut seen = BTreeSet::new();
    for value in values {
        validate_text(value, field)?;
        if !seen.insert(value.to_ascii_lowercase()) {
            return Err(ThroughputRetrievalProtocolError::Invalid(format!(
                "{field} contains duplicate or case-colliding values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_unique(
    values: &[String],
    field: &str,
) -> Result<(), ThroughputRetrievalProtocolError> {
    validate_unique(values, field)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ThroughputRetrievalProtocolError::Invalid(format!(
            "{field} must use canonical sorted order"
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retrieval_synthesis::RetrievalCandidate;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn request() -> ThroughputRetrievalProtocolRequest {
        let candidate = RetrievalCandidate {
            evidence_id: "evidence:batch".into(),
            source_id: "source:batch".into(),
            study_id: "study:batch".into(),
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
        ThroughputRetrievalProtocolRequest {
            request: ThroughputRetrievalQuery {
                request_id: "request:throughput-protocol".into(),
                batch_id: "batch:one".into(),
                partition: "partition:one".into(),
                max_items: 8,
                minimum_support_milli: 700,
                candidates: vec![candidate],
                replay_identity: hash("replay"),
                policy_allow: true,
                protected_closure: true,
                raw_data_local: true,
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            protocol_id: "protocol:throughput-v1".into(),
            session_id: "session:throughput".into(),
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
    fn manifest_is_a1() {
        let manifest = throughput_retrieval_protocol_gateway_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A1);
    }
    #[test]
    fn throughput_protocol_completes() {
        let receipt = compile_throughput_retrieval_protocol(&request()).unwrap();
        assert_eq!(receipt.disposition, SynthesisDisposition::Qualified);
        assert_eq!(receipt.checkpoint_seq, 1);
    }
    #[test]
    fn policy_blocks_with_queue_identity() {
        let mut value = request();
        value.policy_allow = false;
        let receipt = compile_throughput_retrieval_protocol(&value).unwrap();
        assert_eq!(receipt.disposition, SynthesisDisposition::Blocked);
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn locality_failure_is_blocked_and_retained() {
        let mut value = request();
        value.raw_data_local = false;
        value.request.raw_data_local = false;
        let receipt = compile_throughput_retrieval_protocol(&value).unwrap();
        assert_eq!(receipt.disposition, SynthesisDisposition::Blocked);
        assert!(receipt.raw_data_local);
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item == "protocol:raw-data-locality-failed"));
        receipt.validate().unwrap();
    }
    #[test]
    fn digest_is_stable() {
        let receipt = compile_throughput_retrieval_protocol(&request()).unwrap();
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }

    #[test]
    fn multi_candidate_ranking_preserves_semantic_order_and_state_partition() {
        let mut input = request();
        let mut second = input.request.candidates[0].clone();
        second.evidence_id = "evidence:aaa".into();
        second.source_id = "source:aaa".into();
        second.support_milli = 950;
        input.request.candidates.push(second);

        let receipt = compile_throughput_retrieval_protocol(&input).unwrap();
        assert_eq!(receipt.ranked_order, vec!["evidence:aaa", "evidence:batch"]);
        assert_eq!(
            receipt.qualified_order,
            vec!["evidence:aaa", "evidence:batch"]
        );
        receipt.validate().unwrap();
    }

    #[test]
    fn blocked_low_budget_receipts_and_payload_drift_are_rejected_or_bound() {
        let mut low_budget = request();
        low_budget.budget_units = 1;
        let receipt = compile_throughput_retrieval_protocol(&low_budget).unwrap();
        assert_eq!(receipt.disposition, SynthesisDisposition::Blocked);
        assert_eq!(receipt.completed_stage_order.len(), 2);
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);

        let mut drifted = compile_throughput_retrieval_protocol(&request()).unwrap();
        drifted.transcript_digest = hash("drift");
        assert!(drifted.validate().is_err());

        let mut identity_drift = compile_throughput_retrieval_protocol(&request()).unwrap();
        identity_drift.artifact.artifact_id = "artifact:other".into();
        assert!(identity_drift.validate().is_err());
    }

    #[test]
    fn case_mismatched_candidate_identity_is_rejected() {
        let mut receipt = compile_throughput_retrieval_protocol(&request()).unwrap();
        receipt.qualified_order[0] = receipt.qualified_order[0].to_ascii_uppercase();
        assert!(receipt.validate().is_err());
    }
}

//! Federated continual context API/protocol adapter.
//!
//! Atlas feature: `AFA-brain-P03-F24`. The adapter emits an aggregate-only,
//! locality-preserving federation receipt; transport and institution-local data
//! access remain application-owned.

use crate::contract_validation::{
    validate_partition, validate_sorted_unique, validate_text, validate_unique,
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

pub const FEATURE_ID: &str = "AFA-brain-P03-F24";
pub const CONTRACT_VERSION: &str = "brain-federated-context-protocol-adapter/1.0";
pub const PROTOCOL_VERSION: &str = "aurora-research-context-federated/1.0";
pub const ROUTE: &str = "/v1/research/context/federated/compile";
pub const METHOD: &str = "POST";
pub const RESPONSE_SCHEMA: &str = "FederatedContextProtocolResponse1@1";
const REQUEST_CONTENT_TYPE: &str = "application/json";
const ARTIFACT_CONTENT_TYPE: &str =
    "application/vnd.aurora.federated-context-protocol-response+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContextProtocolPeer {
    pub institution_id: String,
    pub endpoint: String,
    pub semantic_profile: String,
    pub context_digest: ContentHash,
    pub section_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub state: EvidenceState,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContextProtocolRequest {
    pub request_id: String,
    pub federation_id: String,
    pub purpose: String,
    pub scope: String,
    pub goal: String,
    pub semantic_profile: String,
    pub peers: Vec<FederatedContextProtocolPeer>,
    pub minimum_quorum: u16,
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
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContextProtocolReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub purpose: String,
    pub scope: String,
    pub goal: String,
    pub semantic_profile: String,
    pub protocol_version: String,
    pub method: String,
    pub route: String,
    pub content_type: String,
    pub idempotency_key: String,
    pub response_schema: String,
    pub status_code: u16,
    pub disposition: String,
    pub institution_order: Vec<String>,
    pub endpoint_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub aggregate_order: Vec<ContentHash>,
    pub minimum_quorum: u16,
    pub quorum: u16,
    pub checkpoint_seq: u64,
    pub envelope_digest: ContentHash,
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
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedContextProtocolError {
    #[error("invalid federated context protocol request: {0}")]
    Invalid(String),
    #[error("federated context protocol artifact failed: {0}")]
    Artifact(String),
}

impl From<String> for FederatedContextProtocolError {
    fn from(message: String) -> Self {
        Self::Invalid(message)
    }
}

impl FederatedContextProtocolReceipt {
    pub fn validate(&self) -> Result<(), FederatedContextProtocolError> {
        let candidate_count = u64::try_from(self.candidate_order.len()).map_err(|_| {
            FederatedContextProtocolError::Invalid(
                "federated candidate count exceeds checkpoint sequence width".into(),
            )
        })?;
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.protocol_version != PROTOCOL_VERSION
            || self.method != METHOD
            || self.route != ROUTE
            || self.content_type != REQUEST_CONTENT_TYPE
            || self.response_schema != RESPONSE_SCHEMA
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.institution_order.len() < 2
            || self.candidate_order.is_empty()
            || self.minimum_quorum == 0
            || usize::from(self.minimum_quorum) > self.candidate_order.len()
            || usize::from(self.quorum) != self.admitted_order.len()
            || self.checkpoint_seq != candidate_count
            || self.effect_receipts.is_empty()
        {
            return Err(FederatedContextProtocolError::Invalid(
                "federated context protocol identity, quorum, checkpoint, or effects are incomplete"
                    .into(),
            ));
        }
        for (value, field) in [
            (&self.request_id, "request_id"),
            (&self.federation_id, "federation_id"),
            (&self.purpose, "purpose"),
            (&self.scope, "scope"),
            (&self.goal, "goal"),
            (&self.semantic_profile, "semantic_profile"),
            (&self.idempotency_key, "idempotency_key"),
            (&self.boundary, "boundary"),
        ] {
            validate_text(value, field)?;
        }
        for (values, field) in [
            (&self.institution_order, "institution_order"),
            (&self.endpoint_order, "endpoint_order"),
            (&self.candidate_order, "candidate_order"),
            (&self.admitted_order, "admitted_order"),
            (&self.blocked_order, "blocked_order"),
            (&self.unknown_order, "unknown_order"),
            (&self.omissions, "omissions"),
            (&self.uncertainty, "uncertainty"),
            (&self.negative_evidence, "negative_evidence"),
            (&self.effect_receipts, "effect_receipts"),
        ] {
            validate_sorted_unique(values, field)?;
        }
        if self.institution_order != self.candidate_order {
            return Err(FederatedContextProtocolError::Invalid(
                "institution and candidate orders diverge".into(),
            ));
        }
        let aggregate_strings = self
            .aggregate_order
            .iter()
            .map(|digest| digest.as_str().to_owned())
            .collect::<Vec<_>>();
        validate_sorted_unique(&aggregate_strings, "aggregate_order")?;
        validate_partition(
            &self.candidate_order,
            &self.admitted_order,
            &self.blocked_order,
            &self.unknown_order,
            "federated context protocol",
        )?;
        if self.aggregate_order.len() > self.admitted_order.len() {
            return Err(FederatedContextProtocolError::Invalid(
                "federated aggregate order exceeds admitted quorum".into(),
            ));
        }
        if !matches!(
            self.disposition.as_str(),
            "ready" | "partial" | "unknown" | "blocked"
        ) {
            return Err(FederatedContextProtocolError::Invalid(
                "federated context protocol disposition is invalid".into(),
            ));
        }
        let global_gate_failed = [
            "protocol:policy-denied",
            "protocol:protected-closure-incomplete",
            "protocol:signer-invalid",
            "protocol:raw-data-locality-failed",
            "protocol:aggregate-only-required",
        ]
        .iter()
        .any(|item| self.omissions.iter().any(|omission| omission == item));
        let expected_status = match self.disposition.as_str() {
            "ready" if self.admitted_order.len() == self.candidate_order.len() => 200,
            "partial" if self.quorum >= self.minimum_quorum => 206,
            "unknown" if self.quorum < self.minimum_quorum && !self.unknown_order.is_empty() => 202,
            "blocked" if global_gate_failed => 403,
            "blocked" if self.quorum < self.minimum_quorum => 422,
            _ => {
                return Err(FederatedContextProtocolError::Invalid(
                    "federated context protocol disposition and quorum are inconsistent".into(),
                ));
            }
        };
        if self.status_code != expected_status {
            return Err(FederatedContextProtocolError::Invalid(
                "federated context protocol status does not match disposition".into(),
            ));
        }
        for digest in [
            &self.envelope_digest,
            &self.context_digest,
            &self.section_digest,
            &self.request_digest,
            &self.response_digest,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(FederatedContextProtocolError::Invalid(
                    "federated context protocol digest is invalid".into(),
                ));
            }
        }
        let expected_envelope = ContentHash::of_value(&json!({
            "federation_id": self.federation_id,
            "purpose": self.purpose,
            "semantic_profile": self.semantic_profile,
            "candidate_order": self.candidate_order,
            "aggregate_order": self.aggregate_order,
            "replay_identity": self.replay_identity,
            "aggregate_only": self.aggregate_only,
        }))
        .map_err(|error| FederatedContextProtocolError::Artifact(error.to_string()))?;
        let expected_context = ContentHash::of_value(&json!({
            "scope": self.scope,
            "envelope_digest": self.envelope_digest,
            "quorum": self.quorum,
        }))
        .map_err(|error| FederatedContextProtocolError::Artifact(error.to_string()))?;
        let expected_section = ContentHash::of_value(&json!({
            "goal": self.goal,
            "context_digest": self.context_digest,
            "admitted_order": self.admitted_order,
        }))
        .map_err(|error| FederatedContextProtocolError::Artifact(error.to_string()))?;
        let expected_response = ContentHash::of_value(&json!({
            "protocol_version": PROTOCOL_VERSION,
            "route": ROUTE,
            "request_id": self.request_id,
            "status_code": self.status_code,
            "disposition": self.disposition,
            "envelope_digest": self.envelope_digest,
            "replay_identity": self.replay_identity,
        }))
        .map_err(|error| FederatedContextProtocolError::Artifact(error.to_string()))?;
        if self.envelope_digest != expected_envelope
            || self.context_digest != expected_context
            || self.section_digest != expected_section
            || self.response_digest != expected_response
        {
            return Err(FederatedContextProtocolError::Invalid(
                "federated context protocol digest is not bound to its receipt fields".into(),
            ));
        }
        let expected_effects = if self.disposition == "blocked" {
            vec!["block:unsafe-release".to_string()]
        } else {
            vec![format!(
                "protocol:federated-context-response:{}",
                self.idempotency_key
            )]
        };
        if self.effect_receipts != expected_effects {
            return Err(FederatedContextProtocolError::Invalid(
                "federated context protocol effects do not match disposition".into(),
            ));
        }
        if !self.raw_data_local {
            return Err(FederatedContextProtocolError::Invalid(
                "federated context protocol receipts must declare local emitted data".into(),
            ));
        }
        if !self.aggregate_only
            && (self.disposition != "blocked"
                || !self
                    .omissions
                    .iter()
                    .any(|item| item == "protocol:aggregate-only-required"))
        {
            return Err(FederatedContextProtocolError::Invalid(
                "aggregate-only failure must remain blocked and explicit".into(),
            ));
        }
        let expected_artifact_id = format!("brain-federated-context-protocol:{}", self.request_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != ARTIFACT_CONTENT_TYPE
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(FederatedContextProtocolError::Invalid(
                "federated context protocol artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| FederatedContextProtocolError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
            .map_err(|error| FederatedContextProtocolError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, FederatedContextProtocolError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| FederatedContextProtocolError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| FederatedContextProtocolError::Artifact(error.to_string()))
    }
}

pub fn federated_context_protocol_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "brain".into(),
        consumers: [
            "federated context gateway".into(),
            "research consortium operator".into(),
            "research SDK".into(),
        ]
        .into(),
        behavior: "maps institution-local context attestations to an aggregate-only, quorum-aware federated protocol receipt".into(),
        value: "enables continual multi-institution context compilation without moving raw experimental data or hiding federation omissions".into(),
        inputs: vec![TypedPort {
            name: "federated_context_protocol_request".into(),
            schema: "FederatedContextProtocolRequest1@1".into(),
            required: true,
        }],
        outputs: vec![TypedPort {
            name: "federated_context_protocol_response".into(),
            schema: RESPONSE_SCHEMA.into(),
            required: true,
        }],
        effects: [
            Effect::ReadLocalData,
            Effect::ExecuteLocalComputation,
            Effect::WriteLocalArtifact,
            Effect::FederationExport,
        ]
        .into(),
        permissions: [
            "protocol:federated-context-response".into(),
            "federation:aggregate-only".into(),
        ]
        .into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference {
            source_id: "w3c-prov-o".into(),
            state: EvidenceState::Supported,
            locator: Some("https://www.w3.org/TR/prov-o/".into()),
        }],
        authority_requirements: vec![AuthorityRequirement {
            role: "federated context protocol approver".into(),
            reason: "authorize aggregate-only federation responses before export effects".into(),
        }],
        autonomy_tier: AutonomyTier::A2,
        surfaces: [
            ResearchSurface::Ui,
            ResearchSurface::Api,
            ResearchSurface::Sdk,
            ResearchSurface::Cli,
            ResearchSurface::McpTool,
            ResearchSurface::Protocol,
            ResearchSurface::Policy,
            ResearchSurface::Operator,
        ]
        .into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn serve_federated_context_protocol(
    request: &FederatedContextProtocolRequest,
) -> Result<FederatedContextProtocolReceipt, FederatedContextProtocolError> {
    validate_request(request)?;
    let mut peers = request.peers.clone();
    peers.sort_by(|left, right| left.institution_id.cmp(&right.institution_id));
    let candidate_order = peers
        .iter()
        .map(|peer| peer.institution_id.clone())
        .collect::<Vec<_>>();
    let mut endpoint_order = peers
        .iter()
        .map(|peer| peer.endpoint.clone())
        .collect::<Vec<_>>();
    endpoint_order.sort();

    let mut admitted = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut aggregate = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let open = request.policy_allow
        && request.protected_closure
        && request.signer_valid
        && request.raw_data_local
        && request.aggregate_only;
    for peer in &peers {
        if !open
            || !peer.signed_approval
            || !peer.raw_data_local
            || !peer.aggregate_only
            || peer.boundary != PRECLINICAL_BOUNDARY
        {
            blocked.insert(peer.institution_id.clone());
            omissions.insert(format!(
                "institution:{}:policy-approval-locality-blocked",
                peer.institution_id
            ));
        } else if peer.semantic_profile != request.semantic_profile {
            blocked.insert(peer.institution_id.clone());
            negative.insert(format!(
                "institution:{}:semantic-profile-mismatch",
                peer.institution_id
            ));
        } else if peer.replay_identity != request.replay_identity {
            unknown.insert(peer.institution_id.clone());
            uncertainty.insert(format!(
                "institution:{}:replay-mismatch",
                peer.institution_id
            ));
        } else if matches!(
            peer.state,
            EvidenceState::Unknown | EvidenceState::Speculative
        ) {
            unknown.insert(peer.institution_id.clone());
            uncertainty.insert(format!(
                "institution:{}:evidence-uncertain",
                peer.institution_id
            ));
        } else if matches!(peer.state, EvidenceState::Contradicted) {
            blocked.insert(peer.institution_id.clone());
            negative.insert(format!("institution:{}:contradicted", peer.institution_id));
        } else {
            admitted.insert(peer.institution_id.clone());
            aggregate.insert(peer.context_digest.clone());
        }
    }
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
    if !request.aggregate_only {
        omissions.insert("protocol:aggregate-only-required".into());
    }
    let disposition = if !open {
        "blocked"
    } else if admitted.len() == candidate_order.len()
        && admitted.len() >= usize::from(request.minimum_quorum)
    {
        "ready"
    } else if !unknown.is_empty() && admitted.len() < usize::from(request.minimum_quorum) {
        "unknown"
    } else if admitted.len() >= usize::from(request.minimum_quorum) {
        "partial"
    } else {
        "blocked"
    };
    let status_code = if !open {
        403
    } else if disposition == "ready" {
        200
    } else if disposition == "partial" {
        206
    } else if disposition == "unknown" {
        202
    } else {
        422
    };
    let institution_order = candidate_order.clone();
    let admitted_order = admitted.iter().cloned().collect::<Vec<_>>();
    let blocked_order = blocked.iter().cloned().collect::<Vec<_>>();
    let unknown_order = unknown.iter().cloned().collect::<Vec<_>>();
    let aggregate_order = aggregate.into_iter().collect::<Vec<_>>();
    let quorum = u16::try_from(admitted_order.len()).map_err(|_| {
        FederatedContextProtocolError::Invalid(
            "federated admitted quorum exceeds protocol bounds".into(),
        )
    })?;
    let checkpoint_seq = u64::try_from(candidate_order.len()).map_err(|_| {
        FederatedContextProtocolError::Invalid(
            "federated candidate count exceeds checkpoint sequence width".into(),
        )
    })?;
    let envelope_digest = ContentHash::of_value(&json!({
        "federation_id": request.federation_id,
        "purpose": request.purpose,
        "semantic_profile": request.semantic_profile,
        "candidate_order": candidate_order,
        "aggregate_order": aggregate_order,
        "replay_identity": request.replay_identity,
        "aggregate_only": request.aggregate_only,
    }))
    .map_err(|error| FederatedContextProtocolError::Artifact(error.to_string()))?;
    let context_digest = ContentHash::of_value(&json!({
        "scope": request.scope,
        "envelope_digest": envelope_digest,
        "quorum": quorum,
    }))
    .map_err(|error| FederatedContextProtocolError::Artifact(error.to_string()))?;
    let section_digest = ContentHash::of_value(&json!({
        "goal": request.goal,
        "context_digest": context_digest,
        "admitted_order": admitted_order,
    }))
    .map_err(|error| FederatedContextProtocolError::Artifact(error.to_string()))?;
    let request_digest = ContentHash::of_value(
        &serde_json::to_value(request)
            .map_err(|error| FederatedContextProtocolError::Artifact(error.to_string()))?,
    )
    .map_err(|error| FederatedContextProtocolError::Artifact(error.to_string()))?;
    let response_digest = ContentHash::of_value(&json!({
        "protocol_version": PROTOCOL_VERSION,
        "route": ROUTE,
        "request_id": request.request_id,
        "status_code": status_code,
        "disposition": disposition,
        "envelope_digest": envelope_digest,
        "replay_identity": request.replay_identity,
    }))
    .map_err(|error| FederatedContextProtocolError::Artifact(error.to_string()))?;
    let omissions = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence = negative.into_iter().collect::<Vec<_>>();
    let effect_receipts = if disposition != "blocked" {
        vec![format!(
            "protocol:federated-context-response:{}",
            request.idempotency_key
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "federation_id": request.federation_id,
        "purpose": request.purpose,
        "scope": request.scope,
        "goal": request.goal,
        "semantic_profile": request.semantic_profile,
        "protocol_version": PROTOCOL_VERSION,
        "method": METHOD,
        "route": ROUTE,
        "content_type": REQUEST_CONTENT_TYPE,
        "idempotency_key": request.idempotency_key,
        "response_schema": RESPONSE_SCHEMA,
        "status_code": status_code,
        "disposition": disposition,
        "institution_order": institution_order,
        "endpoint_order": endpoint_order,
        "candidate_order": candidate_order,
        "admitted_order": admitted_order,
        "blocked_order": blocked_order,
        "unknown_order": unknown_order,
        "aggregate_order": aggregate_order,
        "minimum_quorum": request.minimum_quorum,
        "quorum": quorum,
        "checkpoint_seq": checkpoint_seq,
        "envelope_digest": envelope_digest,
        "context_digest": context_digest,
        "section_digest": section_digest,
        "request_digest": request_digest,
        "response_digest": response_digest,
        "replay_identity": request.replay_identity,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "negative_evidence": negative_evidence,
        "effect_receipts": effect_receipts,
        "raw_data_local": true,
        "aggregate_only": request.aggregate_only,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-federated-context-protocol:{}", request.request_id),
        ARTIFACT_CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| FederatedContextProtocolError::Artifact(error.to_string()))?;
    let receipt = FederatedContextProtocolReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        purpose: request.purpose.clone(),
        scope: request.scope.clone(),
        goal: request.goal.clone(),
        semantic_profile: request.semantic_profile.clone(),
        protocol_version: PROTOCOL_VERSION.into(),
        method: METHOD.into(),
        route: ROUTE.into(),
        content_type: REQUEST_CONTENT_TYPE.into(),
        idempotency_key: request.idempotency_key.clone(),
        response_schema: RESPONSE_SCHEMA.into(),
        status_code,
        disposition: disposition.into(),
        institution_order,
        endpoint_order,
        candidate_order,
        admitted_order,
        blocked_order,
        unknown_order,
        aggregate_order,
        minimum_quorum: request.minimum_quorum,
        quorum,
        checkpoint_seq,
        envelope_digest,
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
        raw_data_local: true,
        aggregate_only: request.aggregate_only,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(
    request: &FederatedContextProtocolRequest,
) -> Result<(), FederatedContextProtocolError> {
    for (value, field) in [
        (&request.request_id, "request_id"),
        (&request.federation_id, "federation_id"),
        (&request.purpose, "purpose"),
        (&request.scope, "scope"),
        (&request.goal, "goal"),
        (&request.semantic_profile, "semantic_profile"),
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
    if request.protocol_version != PROTOCOL_VERSION
        || request.method != METHOD
        || request.route != ROUTE
        || request.content_type != REQUEST_CONTENT_TYPE
        || request.response_schema != RESPONSE_SCHEMA
        || request.peers.len() < 2
        || request.peers.len() > usize::from(u16::MAX)
        || request.minimum_quorum == 0
        || usize::from(request.minimum_quorum) > request.peers.len()
        || request.replay_identity.as_str().len() != 64
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(FederatedContextProtocolError::Invalid(
            "federated context protocol identity, route, quorum, replay, or boundary is invalid"
                .into(),
        ));
    }
    let mut institutions = Vec::with_capacity(request.peers.len());
    let mut endpoints = Vec::with_capacity(request.peers.len());
    for peer in &request.peers {
        for (value, field) in [
            (&peer.institution_id, "peer.institution_id"),
            (&peer.endpoint, "peer.endpoint"),
            (&peer.semantic_profile, "peer.semantic_profile"),
            (&peer.boundary, "peer.boundary"),
        ] {
            validate_text(value, field)?;
        }
        if peer.boundary != PRECLINICAL_BOUNDARY
            || peer.context_digest.as_str().len() != 64
            || peer.section_digest.as_str().len() != 64
            || peer.replay_identity.as_str().len() != 64
        {
            return Err(FederatedContextProtocolError::Invalid(
                "federated peer identity, digest, or boundary is invalid".into(),
            ));
        }
        institutions.push(peer.institution_id.clone());
        endpoints.push(peer.endpoint.clone());
    }
    validate_unique(&institutions, "peer.institution_id")?;
    validate_unique(&endpoints, "peer.endpoint")?;
    Ok(())
}

fn receipt_payload(receipt: &FederatedContextProtocolReceipt) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "request_id": receipt.request_id,
        "federation_id": receipt.federation_id,
        "purpose": receipt.purpose,
        "scope": receipt.scope,
        "goal": receipt.goal,
        "semantic_profile": receipt.semantic_profile,
        "protocol_version": receipt.protocol_version,
        "method": receipt.method,
        "route": receipt.route,
        "content_type": receipt.content_type,
        "idempotency_key": receipt.idempotency_key,
        "response_schema": receipt.response_schema,
        "status_code": receipt.status_code,
        "disposition": receipt.disposition,
        "institution_order": receipt.institution_order,
        "endpoint_order": receipt.endpoint_order,
        "candidate_order": receipt.candidate_order,
        "admitted_order": receipt.admitted_order,
        "blocked_order": receipt.blocked_order,
        "unknown_order": receipt.unknown_order,
        "aggregate_order": receipt.aggregate_order,
        "minimum_quorum": receipt.minimum_quorum,
        "quorum": receipt.quorum,
        "checkpoint_seq": receipt.checkpoint_seq,
        "envelope_digest": receipt.envelope_digest,
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
        "aggregate_only": receipt.aggregate_only,
        "boundary": receipt.boundary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }

    fn request(state: EvidenceState) -> FederatedContextProtocolRequest {
        let replay = hash("federated-context-replay");
        let peers = vec!["site:a", "site:b"]
            .into_iter()
            .map(|institution_id| FederatedContextProtocolPeer {
                institution_id: institution_id.into(),
                endpoint: format!("https://{institution_id}.local/context"),
                semantic_profile: "profile:v1".into(),
                context_digest: replay.clone(),
                section_digest: replay.clone(),
                replay_identity: replay.clone(),
                state: state.clone(),
                signed_approval: true,
                raw_data_local: true,
                aggregate_only: true,
                boundary: PRECLINICAL_BOUNDARY.into(),
            })
            .collect();
        FederatedContextProtocolRequest {
            request_id: "request:federated-context".into(),
            federation_id: "federation:preclinical".into(),
            purpose: "aggregate context".into(),
            scope: "preclinical:organoid".into(),
            goal: "compile federated context".into(),
            semantic_profile: "profile:v1".into(),
            peers,
            minimum_quorum: 2,
            protocol_version: PROTOCOL_VERSION.into(),
            method: METHOD.into(),
            route: ROUTE.into(),
            content_type: REQUEST_CONTENT_TYPE.into(),
            idempotency_key: "idem:federated-context".into(),
            response_schema: RESPONSE_SCHEMA.into(),
            replay_identity: replay,
            policy_allow: true,
            protected_closure: true,
            signer_valid: true,
            raw_data_local: true,
            aggregate_only: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn manifest_is_a2_aggregate_only() {
        let manifest = federated_context_protocol_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A2);
    }

    #[test]
    fn quorum_returns_200() {
        assert_eq!(
            serve_federated_context_protocol(&request(EvidenceState::Supported))
                .unwrap()
                .status_code,
            200
        );
    }

    #[test]
    fn unknown_peer_returns_202() {
        assert_eq!(
            serve_federated_context_protocol(&request(EvidenceState::Unknown))
                .unwrap()
                .status_code,
            202
        );
    }

    #[test]
    fn aggregate_gate_returns_403() {
        let mut value = request(EvidenceState::Supported);
        value.aggregate_only = false;
        let receipt = serve_federated_context_protocol(&value).unwrap();
        assert_eq!(receipt.status_code, 403);
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item == "protocol:aggregate-only-required"));
    }

    #[test]
    fn semantic_mismatch_is_negative() {
        let mut value = request(EvidenceState::Supported);
        value.peers[0].semantic_profile = "profile:other".into();
        let receipt = serve_federated_context_protocol(&value).unwrap();
        assert!(receipt
            .negative_evidence
            .iter()
            .any(|item| item.contains("semantic-profile-mismatch")));
    }

    #[test]
    fn locality_failure_is_blocked_and_retained() {
        let mut value = request(EvidenceState::Supported);
        value.raw_data_local = false;
        let receipt = serve_federated_context_protocol(&value).unwrap();
        assert_eq!(receipt.disposition, "blocked");
        assert!(receipt.raw_data_local);
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item == "protocol:raw-data-locality-failed"));
        assert!(receipt.validate().is_ok());
    }

    #[test]
    fn padded_peer_identity_is_rejected() {
        let mut value = request(EvidenceState::Supported);
        value.peers[0].institution_id.push(' ');
        assert!(serve_federated_context_protocol(&value).is_err());
    }

    #[test]
    fn duplicate_peer_endpoint_is_rejected() {
        let mut value = request(EvidenceState::Supported);
        value.peers[1].endpoint = value.peers[0].endpoint.clone();
        assert!(serve_federated_context_protocol(&value).is_err());
    }

    #[test]
    fn response_artifact_payload_is_bound() {
        let mut receipt =
            serve_federated_context_protocol(&request(EvidenceState::Supported)).unwrap();
        receipt.goal = "tampered goal".into();
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn digest_fields_are_bound() {
        let mut receipt =
            serve_federated_context_protocol(&request(EvidenceState::Supported)).unwrap();
        receipt.envelope_digest = hash("tampered-envelope");
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn digest_is_stable() {
        let receipt = serve_federated_context_protocol(&request(EvidenceState::Supported)).unwrap();
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
}

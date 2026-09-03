//! Prospective high-throughput context-compilation control plane for `AFA-ops-P03-F31`.
//!
//! The control plane admits a bounded, institution-local queue of typed context attestations and
//! aggregate peer summaries. It emits a deterministic certified-section receipt and federation
//! summary plan, but never schedules a process, contacts a peer, moves raw data, or makes a
//! biological or clinical decision.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-ops-P03-F31";
pub const CONTRACT_VERSION: &str =
    "ops-prospective-context-compilation-federated-control-plane/1.0";
pub const INPUT_SCHEMA: &str = "DecisionQuery3@1";
pub const OUTPUT_SCHEMA: &str = "CertifiedDecisionSection8@1";
pub const TOOL_NAME: &str = "ops_context_compilation_federated_control_plane";
const CONTENT_TYPE: &str = "application/vnd.aurora.ops-certified-decision-section-8+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextAttestation {
    pub context_id: String,
    pub stage: String,
    pub semantic_profile: String,
    pub digest: ContentHash,
    pub state: EvidenceState,
    pub local_only: bool,
    pub permitted: bool,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerOperationsSummary {
    pub peer_id: String,
    pub semantic_profile: String,
    pub summary_digest: ContentHash,
    pub signed: bool,
    pub aggregate_only: bool,
    pub raw_data_local: bool,
    pub queue_depth: u32,
    pub state: EvidenceState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionQuery {
    pub schema_version: String,
    pub request_id: String,
    pub federation_id: String,
    pub query_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_context_order: Vec<String>,
    pub contexts: Vec<ContextAttestation>,
    pub peers: Vec<PeerOperationsSummary>,
    pub minimum_peer_quorum: u16,
    pub capacity: u32,
    pub active_runs: u32,
    pub queue_depth: u32,
    pub max_queue_depth: u32,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_approved: bool,
    pub raw_data_local: bool,
    pub adversarial_events: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlDisposition {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CertifiedDecisionSection {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub query_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub disposition: ControlDisposition,
    pub context_order: Vec<String>,
    pub selected_context_order: Vec<String>,
    pub unresolved_context_order: Vec<String>,
    pub blocked_context_order: Vec<String>,
    pub missing_context_order: Vec<String>,
    pub peer_order: Vec<String>,
    pub qualified_peer_order: Vec<String>,
    pub missing_peer_order: Vec<String>,
    pub queue_depth: u32,
    pub active_runs: u32,
    pub capacity_exceeded_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub section_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ControlPlaneError {
    #[error("invalid context control request: {0}")]
    Invalid(String),
    #[error("context control artifact failed: {0}")]
    Artifact(String),
}
fn invalid(message: impl Into<String>) -> ControlPlaneError {
    ControlPlaneError::Invalid(message.into())
}
fn digest_is_valid(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}
fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

impl CertifiedDecisionSection {
    pub fn validate(&self) -> Result<(), ControlPlaneError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.query_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.context_order.is_empty()
            || self.peer_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(invalid(
                "control identity, locality, contexts, peers, or effects are incomplete",
            ));
        }
        for values in [
            &self.context_order,
            &self.selected_context_order,
            &self.unresolved_context_order,
            &self.blocked_context_order,
            &self.missing_context_order,
            &self.peer_order,
            &self.qualified_peer_order,
            &self.missing_peer_order,
            &self.capacity_exceeded_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(invalid("control ordering is not canonical"));
            }
        }
        let ids = self.context_order.iter().cloned().collect::<BTreeSet<_>>();
        let parts = self
            .selected_context_order
            .iter()
            .chain(self.unresolved_context_order.iter())
            .chain(self.blocked_context_order.iter())
            .cloned()
            .collect::<Vec<_>>();
        if parts.len() != ids.len() || parts.iter().cloned().collect::<BTreeSet<_>>() != ids {
            return Err(invalid("context states do not partition"));
        }
        let peers = self.peer_order.iter().cloned().collect::<BTreeSet<_>>();
        let peer_parts = self
            .qualified_peer_order
            .iter()
            .chain(self.missing_peer_order.iter())
            .cloned()
            .collect::<Vec<_>>();
        if peer_parts.len() != peers.len()
            || peer_parts.iter().cloned().collect::<BTreeSet<_>>() != peers
        {
            return Err(invalid("peer states do not partition"));
        }
        if ![
            &self.replay_identity,
            &self.section_digest,
            &self.artifact.content_hash,
        ]
        .iter()
        .all(|value| digest_is_valid(value))
        {
            return Err(invalid("control digest is invalid"));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ControlPlaneError::Artifact(error.to_string()))?;
        if self.artifact.content_type != CONTENT_TYPE {
            return Err(invalid("control artifact type is invalid"));
        }
        let expected = if self.disposition == ControlDisposition::Qualified {
            vec![
                format!("exchange:permitted-summaries:{}", self.request_id),
                format!("manage:local-capability:{}", self.request_id),
            ]
        } else {
            vec!["block:unsafe-release".into()]
        };
        if self.effect_receipts != expected {
            return Err(invalid("control effect receipts are invalid"));
        }
        Ok(())
    }
}

pub fn context_compilation_control_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "ops".into(), consumers: BTreeSet::from([String::from("research data steward"), String::from("institution node operator"), String::from("federation governor")]), behavior: "admits a bounded high-throughput context queue and typed peer summaries into a certified decision-section control receipt without scheduling or moving raw data".into(), value: "makes operational capacity, federation, context closure, and authorization visible before research automation is admitted".into(), inputs: vec![TypedPort { name: "decision_query".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "certified_decision_section".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: BTreeSet::from([Effect::ExecuteLocalComputation, Effect::FederationExport]), permissions: BTreeSet::from([String::from("operate:institution-node")]), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "opentelemetry".into(), state: EvidenceState::Supported, locator: Some("https://opentelemetry.io/docs/specs/".into()) }], authority_requirements: vec![AuthorityRequirement { role: "institution node operator".into(), reason: "high-throughput queue admission and aggregate summary exchange require governed authorization".into() }], autonomy_tier: AutonomyTier::A2, surfaces: BTreeSet::from([ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Protocol, ResearchSurface::Policy, ResearchSurface::Operator]), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn operate_context_compilation(
    request: &DecisionQuery,
) -> Result<CertifiedDecisionSection, ControlPlaneError> {
    validate_request(request)?;
    let mut contexts = request.contexts.clone();
    contexts.sort_by(|a, b| a.context_id.cmp(&b.context_id));
    let context_order = contexts
        .iter()
        .map(|context| context.context_id.clone())
        .collect::<Vec<_>>();
    let required = request
        .required_context_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let present = context_order.iter().cloned().collect::<BTreeSet<_>>();
    let mut missing = required
        .difference(&present)
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut selected = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for context in &contexts {
        if context.negative_result {
            negative.insert(format!("{}:negative-result", context.context_id));
        }
        if !required.contains(&context.context_id) {
            omissions.insert(format!("{}:not-required", context.context_id));
        }
        if context.semantic_profile != request.semantic_profile {
            uncertainty.insert(format!("{}:semantic-profile-mismatch", context.context_id));
            unresolved.insert(context.context_id.clone());
        } else if context.state == EvidenceState::Contradicted
            || !context.local_only
            || !context.permitted
        {
            blocked.insert(context.context_id.clone());
        } else if matches!(
            context.state,
            EvidenceState::Unknown | EvidenceState::Speculative
        ) {
            unresolved.insert(context.context_id.clone());
        } else if required.contains(&context.context_id) {
            selected.insert(context.context_id.clone());
        } else {
            unresolved.insert(context.context_id.clone());
        }
    }
    let mut peer_order = BTreeSet::new();
    let mut qualified_peers = BTreeSet::new();
    let mut missing_peers = BTreeSet::new();
    let mut contradictory_peer = false;
    for peer in &request.peers {
        peer_order.insert(peer.peer_id.clone());
        let valid = peer.semantic_profile == request.semantic_profile
            && peer.signed
            && peer.aggregate_only
            && peer.raw_data_local
            && digest_is_valid(&peer.summary_digest)
            && peer.queue_depth <= request.max_queue_depth
            && matches!(peer.state, EvidenceState::Proven | EvidenceState::Supported);
        if peer.state == EvidenceState::Contradicted {
            contradictory_peer = true;
        }
        if valid {
            qualified_peers.insert(peer.peer_id.clone());
        } else {
            missing_peers.insert(peer.peer_id.clone());
            uncertainty.insert(format!("peer:{}:not-qualified", peer.peer_id));
        }
    }
    let mut capacity: BTreeSet<String> = BTreeSet::new();
    if request.capacity == 0 || request.active_runs > request.capacity {
        capacity.insert("active-runs".into());
    }
    if request.queue_depth > request.max_queue_depth {
        capacity.insert("queue-depth".into());
    }
    if !capacity.is_empty() {
        uncertainty.insert("request:capacity-envelope-exceeded".into());
    }
    if qualified_peers.len() < request.minimum_peer_quorum as usize {
        uncertainty.insert("peer:minimum-quorum-unmet".into());
    }
    if !request.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.signed_approval {
        uncertainty.insert("request:signed-approval-missing".into());
    }
    if !request.federation_approved {
        uncertainty.insert("request:federation-approval-missing".into());
    }
    negative.extend(
        request
            .adversarial_events
            .iter()
            .map(|event| format!("adversarial:{event}")),
    );
    let global_block = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.federation_approved
        || !request.raw_data_local
        || !request.adversarial_events.is_empty()
        || !capacity.is_empty()
        || contradictory_peer;
    if global_block {
        blocked.extend(context_order.iter().cloned());
        selected.clear();
        unresolved.clear();
        missing.clear();
        omissions.insert("request:control-gate-blocked".into());
    }
    let disposition = if global_block || !blocked.is_empty() {
        ControlDisposition::Blocked
    } else if !missing.is_empty()
        || !unresolved.is_empty()
        || qualified_peers.len() < request.minimum_peer_quorum as usize
    {
        ControlDisposition::Unresolved
    } else {
        ControlDisposition::Qualified
    };
    let selected_context_order = selected.into_iter().collect::<Vec<_>>();
    let unresolved_context_order = unresolved.into_iter().collect::<Vec<_>>();
    let blocked_context_order = blocked.into_iter().collect::<Vec<_>>();
    let missing_context_order = missing.into_iter().collect::<Vec<_>>();
    let peer_order = peer_order.into_iter().collect::<Vec<_>>();
    let qualified_peer_order = qualified_peers.into_iter().collect::<Vec<_>>();
    let missing_peer_order = missing_peers.into_iter().collect::<Vec<_>>();
    let capacity_exceeded_order = capacity.into_iter().collect::<Vec<_>>();
    let omission_order = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty_order = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence_order = negative.into_iter().collect::<Vec<_>>();
    let effect_receipts = if disposition == ControlDisposition::Qualified {
        vec![
            format!("exchange:permitted-summaries:{}", request.request_id),
            format!("manage:local-capability:{}", request.request_id),
        ]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"federation_id":request.federation_id,"query_id":request.query_id,"purpose":request.purpose,"semantic_profile":request.semantic_profile,"disposition":disposition,"context_order":context_order,"selected_context_order":selected_context_order,"unresolved_context_order":unresolved_context_order,"blocked_context_order":blocked_context_order,"missing_context_order":missing_context_order,"peer_order":peer_order,"qualified_peer_order":qualified_peer_order,"missing_peer_order":missing_peer_order,"queue_depth":request.queue_depth,"active_runs":request.active_runs,"capacity_exceeded_order":capacity_exceeded_order,"omission_order":omission_order,"uncertainty_order":uncertainty_order,"negative_evidence_order":negative_evidence_order,"replay_identity":request.replay_identity,"effect_receipts":effect_receipts,"raw_data_local":request.raw_data_local,"boundary":PRECLINICAL_BOUNDARY});
    let section_digest = ContentHash::of_value(&payload)
        .map_err(|error| ControlPlaneError::Artifact(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("ops-certified-decision-section:{}", request.request_id),
        CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ControlPlaneError::Artifact(error.to_string()))?;
    let strings = |key: &str| -> Result<Vec<String>, ControlPlaneError> {
        let Some(values) = payload.get(key).and_then(Value::as_array) else {
            return Err(invalid(format!(
                "generated payload is missing array `{key}`"
            )));
        };
        values
            .iter()
            .map(|item| {
                item.as_str().map(str::to_string).ok_or_else(|| {
                    invalid(format!(
                        "generated payload array `{key}` contains a non-string"
                    ))
                })
            })
            .collect()
    };
    let section = CertifiedDecisionSection {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        query_id: request.query_id.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        disposition,
        context_order: strings("context_order")?,
        selected_context_order: strings("selected_context_order")?,
        unresolved_context_order: strings("unresolved_context_order")?,
        blocked_context_order: strings("blocked_context_order")?,
        missing_context_order: strings("missing_context_order")?,
        peer_order: strings("peer_order")?,
        qualified_peer_order: strings("qualified_peer_order")?,
        missing_peer_order: strings("missing_peer_order")?,
        queue_depth: request.queue_depth,
        active_runs: request.active_runs,
        capacity_exceeded_order: strings("capacity_exceeded_order")?,
        omission_order: strings("omission_order")?,
        uncertainty_order: strings("uncertainty_order")?,
        negative_evidence_order: strings("negative_evidence_order")?,
        replay_identity: request.replay_identity.clone(),
        section_digest,
        artifact,
        effect_receipts: strings("effect_receipts")?,
        raw_data_local: request.raw_data_local,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    section.validate()?;
    Ok(section)
}

pub fn operate_context_compilation_json(value: &Value) -> Result<Value, String> {
    let request: DecisionQuery = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid decision query: {error}"))?;
    let section = operate_context_compilation(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(section)
        .map_err(|error| format!("cannot serialize certified decision section: {error}"))
}
pub fn validate_context_compilation_json(
    value: &Value,
) -> Result<CertifiedDecisionSection, String> {
    let section: CertifiedDecisionSection = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid certified decision section: {error}"))?;
    section.validate().map_err(|error| error.to_string())?;
    Ok(section)
}

fn validate_request(request: &DecisionQuery) -> Result<(), ControlPlaneError> {
    if request.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
        || request.request_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.query_id.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_context_order.is_empty()
        || request
            .required_context_order
            .iter()
            .any(|context_id| context_id.trim().is_empty())
        || !canonical(&request.required_context_order)
        || request.contexts.is_empty()
        || request.peers.is_empty()
        || request.minimum_peer_quorum == 0
        || request.capacity == 0
        || request.max_queue_depth == 0
        || !digest_is_valid(&request.replay_identity)
        || !canonical(&request.adversarial_events)
        || !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(invalid(
            "control identity, closure, capacity, replay, locality, or boundary is invalid",
        ));
    }
    let mut ids = BTreeSet::new();
    for context in &request.contexts {
        if context.context_id.trim().is_empty()
            || !ids.insert(context.context_id.clone())
            || context.stage.trim().is_empty()
            || context.semantic_profile.trim().is_empty()
            || !digest_is_valid(&context.digest)
        {
            return Err(invalid(format!(
                "context {} is malformed or duplicated",
                context.context_id
            )));
        }
    }
    let mut peers = BTreeSet::new();
    for peer in &request.peers {
        if peer.peer_id.trim().is_empty()
            || !peers.insert(peer.peer_id.clone())
            || peer.semantic_profile.trim().is_empty()
            || !digest_is_valid(&peer.summary_digest)
        {
            return Err(invalid(format!(
                "peer {} is malformed or duplicated",
                peer.peer_id
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn request() -> DecisionQuery {
        let d = hash("ops-context");
        let context = |id: &str| ContextAttestation {
            context_id: id.into(),
            stage: "compile".into(),
            semantic_profile: "preclinical:v1".into(),
            digest: d.clone(),
            state: EvidenceState::Supported,
            local_only: true,
            permitted: true,
            negative_result: false,
        };
        let peer = PeerOperationsSummary {
            peer_id: "peer:a".into(),
            semantic_profile: "preclinical:v1".into(),
            summary_digest: d.clone(),
            signed: true,
            aggregate_only: true,
            raw_data_local: true,
            queue_depth: 2,
            state: EvidenceState::Supported,
        };
        DecisionQuery {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            request_id: "ops:one".into(),
            federation_id: "fed:ops".into(),
            query_id: "query:one".into(),
            purpose: "context-compile".into(),
            semantic_profile: "preclinical:v1".into(),
            required_context_order: vec!["context:a".into(), "context:b".into()],
            contexts: vec![context("context:b"), context("context:a")],
            peers: vec![peer],
            minimum_peer_quorum: 1,
            capacity: 4,
            active_runs: 1,
            queue_depth: 2,
            max_queue_depth: 8,
            replay_identity: d,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_approved: true,
            raw_data_local: true,
            adversarial_events: vec![],
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a2() {
        assert_eq!(
            context_compilation_control_manifest().autonomy_tier,
            AutonomyTier::A2
        );
    }
    #[test]
    fn qualified() {
        assert_eq!(
            operate_context_compilation(&request()).unwrap().disposition,
            ControlDisposition::Qualified
        );
    }
    #[test]
    fn deterministic() {
        let a = operate_context_compilation(&request()).unwrap();
        let b = operate_context_compilation(&request()).unwrap();
        assert_eq!(a.section_digest, b.section_digest);
    }
    #[test]
    fn missing_is_unresolved() {
        let mut v = request();
        v.contexts.pop();
        assert_eq!(
            operate_context_compilation(&v).unwrap().disposition,
            ControlDisposition::Unresolved
        );
    }
    #[test]
    fn unknown_is_unresolved() {
        let mut v = request();
        v.contexts[0].state = EvidenceState::Unknown;
        assert_eq!(
            operate_context_compilation(&v).unwrap().disposition,
            ControlDisposition::Unresolved
        );
    }
    #[test]
    fn policy_blocks() {
        let mut v = request();
        v.policy_allow = false;
        assert_eq!(
            operate_context_compilation(&v).unwrap().disposition,
            ControlDisposition::Blocked
        );
    }
    #[test]
    fn capacity_blocks() {
        let mut v = request();
        v.queue_depth = 99;
        assert_eq!(
            operate_context_compilation(&v).unwrap().disposition,
            ControlDisposition::Blocked
        );
    }
}

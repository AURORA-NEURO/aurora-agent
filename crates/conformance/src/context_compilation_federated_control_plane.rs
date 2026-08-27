//! Prospective high-throughput context-compilation federation control plane.
//!
//! Atlas feature `AFA-conformance-P03-F31`.  This is an admission and evidence boundary for
//! federated conformance workers: it never compiles private context or executes a suite, but
//! makes protocol, fixture, quorum, capacity, policy, locality, and replay decisions explicit.

use std::collections::BTreeSet;

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, LossSeverity, ProvenanceLink, ResearchSurface, SemanticLoss, TypedPort,
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-conformance-P03-F31";
pub const CONTRACT_VERSION: &str =
    "conformance-prospective-context-compilation-federated-control-plane/1.0";
pub const INPUT_SCHEMA: &str = "ContextCompilationFederatedBatch1@1";
pub const OUTPUT_SCHEMA: &str = "ContextCompilationFederatedReceipt1@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextCompilationCandidate {
    pub candidate_id: String,
    pub section_digest: ContentHash,
    pub context_digest: ContentHash,
    pub suite_id: String,
    pub protocol_version: String,
    pub evidence_state: EvidenceState,
    pub omission_count: u32,
    pub negative_result: bool,
    pub replay_identity: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConformancePeer {
    pub peer_id: String,
    pub suite_id: String,
    pub protocol_version: String,
    pub capabilities: BTreeSet<String>,
    pub fixture_digest: ContentHash,
    pub healthy: bool,
    pub signed_identity: bool,
    pub permitted_export: bool,
    pub raw_data_local: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextCompilationFederatedControlRequest {
    pub request_id: String,
    pub service_id: String,
    pub federation_id: String,
    pub purpose: String,
    pub batch_id: String,
    pub required_suite_id: String,
    pub required_protocol_version: String,
    pub required_capabilities: BTreeSet<String>,
    pub capacity: u32,
    pub active_runs: u32,
    pub checkpoint_seq: u64,
    pub candidates: Vec<ContextCompilationCandidate>,
    pub peers: Vec<ConformancePeer>,
    pub required_peer_quorum: u32,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub approval_token: String,
    pub network_permitted: bool,
    pub raw_data_local: bool,
    pub replay_identity: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextCompilationCandidateDecision {
    pub candidate_id: String,
    pub disposition: String,
    pub failed_gates: Vec<String>,
    pub conditional_gates: Vec<String>,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextCompilationFederatedControlReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub service_id: String,
    pub federation_id: String,
    pub purpose: String,
    pub batch_id: String,
    pub admission: String,
    pub peer_order: Vec<String>,
    pub accepted_peer_order: Vec<String>,
    pub incompatible_peer_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub conditional_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub decisions: Vec<ContextCompilationCandidateDecision>,
    pub checkpoint_seq: u64,
    pub checkpoint_digest: ContentHash,
    pub queue_digest: ContentHash,
    pub control_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub semantic_loss: Vec<SemanticLoss>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ContextCompilationFederatedControlError {
    #[error("invalid context-compilation federation control request: {0}")]
    Invalid(String),
    #[error("context-compilation federation artifact failed: {0}")]
    Artifact(String),
    #[error("context-compilation federation serialization failed: {0}")]
    Serialization(String),
}

fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}
fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

impl ContextCompilationFederatedControlReceipt {
    pub fn validate(&self) -> Result<(), ContextCompilationFederatedControlError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.service_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.batch_id.trim().is_empty()
            || self.peer_order.is_empty()
            || self.candidate_order.is_empty()
            || self.decisions.len() != self.candidate_order.len()
            || self.effect_receipts.is_empty()
            || !matches!(
                self.admission.as_str(),
                "admitted" | "degraded" | "approval_required" | "blocked" | "unknown"
            )
            || !digest(&self.checkpoint_digest)
            || !digest(&self.queue_digest)
            || !digest(&self.control_digest)
            || !digest(&self.replay_identity)
        {
            return Err(Self::invalid("control receipt identity, locality, admission, queues, effects, or digests are incomplete"));
        }
        for values in [
            &self.peer_order,
            &self.accepted_peer_order,
            &self.incompatible_peer_order,
            &self.candidate_order,
            &self.admitted_order,
            &self.conditional_order,
            &self.blocked_order,
            &self.unknown_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(Self::invalid("control receipt ordering is not canonical"));
            }
        }
        if self
            .decisions
            .iter()
            .map(|d| d.candidate_id.as_str())
            .collect::<Vec<_>>()
            != self
                .candidate_order
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
        {
            return Err(Self::invalid(
                "control decisions do not match candidate order",
            ));
        }
        if self
            .accepted_peer_order
            .iter()
            .chain(self.incompatible_peer_order.iter())
            .any(|id| !self.peer_order.contains(id))
            || self
                .admitted_order
                .iter()
                .chain(self.conditional_order.iter())
                .chain(self.blocked_order.iter())
                .chain(self.unknown_order.iter())
                .any(|id| !self.candidate_order.contains(id))
        {
            return Err(Self::invalid(
                "control state references an unknown peer or candidate",
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("operate:conformance-context:")
                && !effect.starts_with("approval-required:")
                && effect != "block:unsafe-release"
        }) {
            return Err(Self::invalid(
                "control effect is outside the conformance context gate",
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ContextCompilationFederatedControlError::Artifact(error.to_string()))
    }
    fn invalid(message: &str) -> ContextCompilationFederatedControlError {
        ContextCompilationFederatedControlError::Invalid(message.into())
    }
    pub fn digest(&self) -> Result<ContentHash, ContextCompilationFederatedControlError> {
        self.validate()?;
        let value = serde_json::to_value(self).map_err(|error| {
            ContextCompilationFederatedControlError::Serialization(error.to_string())
        })?;
        ContentHash::of_value(&value).map_err(|error| {
            ContextCompilationFederatedControlError::Serialization(error.to_string())
        })
    }
}

pub fn context_compilation_federated_control_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "conformance".into(),
        consumers: ["conformance operator".into(), "context compiler".into(), "federated release board".into()].into(),
        behavior: "admits prospective high-throughput context-compilation batches through pinned suite/protocol, peer quorum, fixture, capacity, policy, authority, locality, and replay gates".into(),
        value: "makes federated conformance context admission deterministic, recoverable, and honest about omissions and negative results".into(),
        inputs: vec![TypedPort { name: "context_compilation_federated_batch".into(), schema: INPUT_SCHEMA.into(), required: true }],
        outputs: vec![TypedPort { name: "context_compilation_federated_receipt".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact, Effect::FederationExport].into(),
        permissions: ["operate:conformance-context".into()].into(), determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "slsa-provenance-1.2".into(), state: EvidenceState::Supported, locator: Some("https://slsa.dev/spec/v1.2/provenance".into()) }, EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }],
        authority_requirements: vec![AuthorityRequirement { role: "conformance operator".into(), reason: "federated context admission changes shared validation state".into() }], autonomy_tier: AutonomyTier::A2,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Protocol, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn operate_context_compilation_federated_control(
    request: &ContextCompilationFederatedControlRequest,
) -> Result<ContextCompilationFederatedControlReceipt, ContextCompilationFederatedControlError> {
    if request.request_id.trim().is_empty()
        || request.service_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.batch_id.trim().is_empty()
        || request.required_suite_id.trim().is_empty()
        || request.required_protocol_version.trim().is_empty()
        || request.capacity == 0
        || request.active_runs > request.capacity
        || request.checkpoint_seq == 0
        || request.candidates.is_empty()
        || request.peers.is_empty()
        || request.required_peer_quorum == 0
        || !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
        || !digest(&request.replay_identity)
    {
        return Err(ContextCompilationFederatedControlError::Invalid("request identity, suite/protocol, capacity, checkpoint, peers, candidates, locality, replay, or boundary is invalid".into()));
    }
    if request.signed_approval && request.approval_token.trim().is_empty() {
        return Err(ContextCompilationFederatedControlError::Invalid(
            "signed approval requires an approval token".into(),
        ));
    }
    let mut peers = request.peers.clone();
    peers.sort_by(|a, b| a.peer_id.cmp(&b.peer_id));
    let mut candidates = request.candidates.clone();
    candidates.sort_by(|a, b| a.candidate_id.cmp(&b.candidate_id));
    if peers.windows(2).any(|p| p[0].peer_id == p[1].peer_id)
        || candidates
            .windows(2)
            .any(|p| p[0].candidate_id == p[1].candidate_id)
        || peers.iter().any(|p| p.peer_id.trim().is_empty())
        || candidates.iter().any(|c| c.candidate_id.trim().is_empty())
    {
        return Err(ContextCompilationFederatedControlError::Invalid(
            "peers and candidates require unique non-empty ids".into(),
        ));
    }
    let mut accepted_peers = Vec::new();
    let mut incompatible_peers = Vec::new();
    let mut semantic_loss = Vec::new();
    for peer in &peers {
        let compatible = peer.suite_id == request.required_suite_id
            && peer.protocol_version == request.required_protocol_version
            && peer.healthy
            && peer.signed_identity
            && peer.permitted_export
            && peer.raw_data_local
            && request.required_capabilities.is_subset(&peer.capabilities);
        if compatible {
            accepted_peers.push(peer.peer_id.clone());
        } else {
            incompatible_peers.push(peer.peer_id.clone());
            semantic_loss.push(SemanticLoss { field: format!("peer:{}", peer.peer_id), reason: "peer failed suite, protocol, health, identity, capability, export, or locality compatibility".into(), severity: LossSeverity::Bounded });
        }
    }
    let mut global_failed = BTreeSet::new();
    if !request.policy_allow {
        global_failed.insert("policy-allow".to_string());
    }
    if !request.protected_closure {
        global_failed.insert("protected-closure".to_string());
    }
    if !request.signed_approval {
        global_failed.insert("signed-approval".to_string());
    }
    if !request.network_permitted {
        global_failed.insert("network-permission".to_string());
    }
    if request.active_runs >= request.capacity {
        global_failed.insert("capacity".to_string());
    }
    if accepted_peers.len() < request.required_peer_quorum as usize {
        global_failed.insert("peer-quorum".to_string());
    }
    let mut admitted = Vec::new();
    let mut conditional = Vec::new();
    let mut blocked = Vec::new();
    let unknown = Vec::new();
    let mut decisions = Vec::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for candidate in &candidates {
        let mut failed = global_failed.clone();
        let mut pending = BTreeSet::new();
        if candidate.suite_id != request.required_suite_id {
            failed.insert("candidate-suite".into());
        }
        if candidate.protocol_version != request.required_protocol_version {
            failed.insert("candidate-protocol".into());
        }
        if candidate.replay_identity != request.replay_identity {
            failed.insert("candidate-replay".into());
        }
        if !digest(&candidate.section_digest) || !digest(&candidate.context_digest) {
            failed.insert("typed-digests".into());
        }
        match candidate.evidence_state {
            EvidenceState::Contradicted => {
                failed.insert("contradicted-evidence".into());
            }
            EvidenceState::Unknown | EvidenceState::Speculative => {
                pending.insert("evidence-state".into());
                uncertainty.insert(format!("{}:evidence-state", candidate.candidate_id));
            }
            _ => {}
        }
        if candidate.omission_count > 0 {
            pending.insert("omission-closure".into());
            omissions.insert(format!(
                "{}:omissions={}",
                candidate.candidate_id, candidate.omission_count
            ));
        }
        if candidate.negative_result {
            negative.insert(format!("{}:negative-result", candidate.candidate_id));
        } else {
            omissions.insert(format!(
                "{}:negative-result-not-observed",
                candidate.candidate_id
            ));
        }
        let disposition = if !failed.is_empty() {
            "blocked"
        } else if !pending.is_empty() {
            "conditional"
        } else {
            "admitted"
        };
        match disposition {
            "blocked" => blocked.push(candidate.candidate_id.clone()),
            "conditional" => conditional.push(candidate.candidate_id.clone()),
            _ => admitted.push(candidate.candidate_id.clone()),
        }
        decisions.push(ContextCompilationCandidateDecision {
            candidate_id: candidate.candidate_id.clone(),
            disposition: disposition.into(),
            failed_gates: failed.into_iter().collect(),
            conditional_gates: pending.into_iter().collect(),
            negative_result: candidate.negative_result,
        });
    }
    let admission = if !global_failed.is_empty() || !blocked.is_empty() {
        "blocked"
    } else if !conditional.is_empty() {
        "approval_required"
    } else if !incompatible_peers.is_empty() {
        "degraded"
    } else {
        "admitted"
    };
    let peer_order = peers.iter().map(|p| p.peer_id.clone()).collect::<Vec<_>>();
    let candidate_order = candidates
        .iter()
        .map(|c| c.candidate_id.clone())
        .collect::<Vec<_>>();
    let checkpoint_payload = json!({"batch_id":request.batch_id,"checkpoint_seq":request.checkpoint_seq,"candidate_order":candidate_order,"peer_order":peer_order});
    let queue_payload = json!({"capacity":request.capacity,"active_runs":request.active_runs,"admitted":admitted,"conditional":conditional,"blocked":blocked});
    let checkpoint_digest = ContentHash::of_value(&checkpoint_payload)
        .map_err(|e| ContextCompilationFederatedControlError::Serialization(e.to_string()))?;
    let queue_digest = ContentHash::of_value(&queue_payload)
        .map_err(|e| ContextCompilationFederatedControlError::Serialization(e.to_string()))?;
    let control_payload = json!({"admission":admission,"checkpoint_digest":checkpoint_digest,"queue_digest":queue_digest,"semantic_loss":semantic_loss,"decisions":decisions});
    let control_digest = ContentHash::of_value(&control_payload)
        .map_err(|e| ContextCompilationFederatedControlError::Serialization(e.to_string()))?;
    let effects = if admission == "admitted" || admission == "degraded" {
        vec![format!(
            "operate:conformance-context:{}",
            request.service_id
        )]
    } else if admission == "approval_required" {
        vec![
            "approval-required:conformance-context".into(),
            "block:unsafe-release".into(),
        ]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"service_id":request.service_id,"federation_id":request.federation_id,"purpose":request.purpose,"batch_id":request.batch_id,"admission":admission,"peer_order":peer_order,"accepted_peer_order":accepted_peers,"incompatible_peer_order":incompatible_peers,"candidate_order":candidate_order,"admitted_order":admitted,"conditional_order":conditional,"blocked_order":blocked,"unknown_order":unknown,"decisions":decisions,"checkpoint_seq":request.checkpoint_seq,"checkpoint_digest":checkpoint_digest,"queue_digest":queue_digest,"control_digest":control_digest,"replay_identity":request.replay_identity,"semantic_loss":semantic_loss,"omissions":omissions,"uncertainty":uncertainty,"negative_evidence":negative});
    let artifact = TypedResearchArtifact::from_payload(
        format!("{}:context-compilation-control", request.request_id),
        "application/vnd.aurora.conformance-context-control+json",
        &payload,
        semantic_loss.clone(),
        vec![ProvenanceLink {
            source_id: request.batch_id.clone(),
            relation: "compiled-from-context-compilation-control".into(),
            digest: control_digest.clone(),
        }],
    )
    .map_err(|e| ContextCompilationFederatedControlError::Artifact(e.to_string()))?;
    let receipt = ContextCompilationFederatedControlReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        service_id: request.service_id.clone(),
        federation_id: request.federation_id.clone(),
        purpose: request.purpose.clone(),
        batch_id: request.batch_id.clone(),
        admission: admission.into(),
        peer_order,
        accepted_peer_order: accepted_peers,
        incompatible_peer_order: incompatible_peers,
        candidate_order,
        admitted_order: admitted,
        conditional_order: conditional,
        blocked_order: blocked,
        unknown_order: unknown,
        decisions,
        checkpoint_seq: request.checkpoint_seq,
        checkpoint_digest,
        queue_digest,
        control_digest,
        replay_identity: request.replay_identity.clone(),
        semantic_loss,
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts: effects,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash() -> ContentHash {
        ContentHash::of_bytes(b"context-control")
    }
    fn request() -> ContextCompilationFederatedControlRequest {
        let h = hash();
        ContextCompilationFederatedControlRequest {
            request_id: "req".into(),
            service_id: "svc".into(),
            federation_id: "fed".into(),
            purpose: "benchmark".into(),
            batch_id: "batch".into(),
            required_suite_id: "fiber-suite".into(),
            required_protocol_version: "1.0".into(),
            required_capabilities: ["context-compile".into()].into(),
            capacity: 4,
            active_runs: 0,
            checkpoint_seq: 1,
            candidates: vec![
                ContextCompilationCandidate {
                    candidate_id: "z".into(),
                    section_digest: h.clone(),
                    context_digest: h.clone(),
                    suite_id: "fiber-suite".into(),
                    protocol_version: "1.0".into(),
                    evidence_state: EvidenceState::Supported,
                    omission_count: 0,
                    negative_result: true,
                    replay_identity: h.clone(),
                },
                ContextCompilationCandidate {
                    candidate_id: "a".into(),
                    section_digest: h.clone(),
                    context_digest: h.clone(),
                    suite_id: "fiber-suite".into(),
                    protocol_version: "1.0".into(),
                    evidence_state: EvidenceState::Supported,
                    omission_count: 0,
                    negative_result: false,
                    replay_identity: h.clone(),
                },
            ],
            peers: vec![ConformancePeer {
                peer_id: "p1".into(),
                suite_id: "fiber-suite".into(),
                protocol_version: "1.0".into(),
                capabilities: ["context-compile".into()].into(),
                fixture_digest: h,
                healthy: true,
                signed_identity: true,
                permitted_export: true,
                raw_data_local: true,
            }],
            required_peer_quorum: 1,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            approval_token: "approval".into(),
            network_permitted: true,
            raw_data_local: true,
            replay_identity: hash(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn deterministic_batch_replays_in_canonical_order() {
        let r1 = operate_context_compilation_federated_control(&request()).unwrap();
        let r2 = operate_context_compilation_federated_control(&request()).unwrap();
        assert_eq!(r1, r2);
        assert_eq!(r1.candidate_order, ["a", "z"]);
    }
    #[test]
    fn unknown_evidence_is_conditional() {
        let mut r = request();
        r.candidates[0].evidence_state = EvidenceState::Unknown;
        let out = operate_context_compilation_federated_control(&r).unwrap();
        assert_eq!(out.admission, "approval_required");
        assert!(out.uncertainty.iter().any(|v| v.contains("evidence-state")));
    }
    #[test]
    fn quorum_failure_blocks_and_retains_negative() {
        let mut r = request();
        r.required_peer_quorum = 2;
        let out = operate_context_compilation_federated_control(&r).unwrap();
        assert_eq!(out.admission, "blocked");
        assert!(out
            .negative_evidence
            .iter()
            .any(|v| v.contains("z:negative-result")));
    }
    #[test]
    fn policy_denial_fails_closed() {
        let mut r = request();
        r.policy_allow = false;
        let out = operate_context_compilation_federated_control(&r).unwrap();
        assert_eq!(out.admission, "blocked");
        assert!(out.effect_receipts.contains(&"block:unsafe-release".into()));
    }
    #[test]
    fn manifest_is_valid_a2_contract() {
        let manifest = context_compilation_federated_control_manifest();
        assert_eq!(manifest.capability_id, FEATURE_ID);
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A2);
        assert!(manifest.validate().is_ok());
    }
}



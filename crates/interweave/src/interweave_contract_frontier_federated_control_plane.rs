//! Prospective high-throughput Interweave contract frontier control plane.
//!
//! Atlas feature `AFA-interweave-P25-F31`.  This module is the typed admission boundary between
//! Interweave workflow batches and a federated runner.  It does not execute jobs or trust remote
//! claims: it negotiates protocol capability, checks queue/checkpoint capacity, records per-job
//! evidence state, and emits a replayable control artifact before an operator releases work.

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

pub fn feature_id() -> &'static str {
    "AFA-interweave-P25-F31"
}
pub fn contract_version() -> &'static str {
    "interweave-prospective-frontier-control-plane/1.0"
}
pub fn input_schema() -> &'static str {
    "InterweaveControlBatch1@1"
}
pub fn output_schema() -> &'static str {
    "InterweaveControlReceipt1@1"
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterweaveJob {
    pub job_id: String,
    pub workflow_id: String,
    pub protocol_version: String,
    pub component_digest: ContentHash,
    pub input_digest: ContentHash,
    pub capability_digests: Vec<ContentHash>,
    pub required_dimensions: BTreeSet<String>,
    pub evidence_state: EvidenceState,
    pub negative_result: bool,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterweavePeer {
    pub peer_id: String,
    pub protocol_version: String,
    pub capabilities: BTreeSet<String>,
    pub checkpoint_digest: ContentHash,
    pub healthy: bool,
    pub signed_identity: bool,
    pub permitted_export: bool,
    pub raw_data_local: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterweaveControlPlaneRequest {
    pub request_id: String,
    pub service_id: String,
    pub federation_id: String,
    pub purpose: String,
    pub batch_id: String,
    pub required_protocol_version: String,
    pub required_capabilities: BTreeSet<String>,
    pub capacity: u32,
    pub active_runs: u32,
    pub checkpoint_seq: u64,
    pub jobs: Vec<InterweaveJob>,
    pub peers: Vec<InterweavePeer>,
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
pub struct InterweaveJobDecision {
    pub job_id: String,
    pub disposition: String,
    pub failed_gates: Vec<String>,
    pub conditional_gates: Vec<String>,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterweaveControlReceipt {
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
    pub job_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub conditional_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub decisions: Vec<InterweaveJobDecision>,
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
pub enum InterweaveControlError {
    #[error("invalid Interweave frontier control request: {0}")]
    Invalid(String),
    #[error("Interweave frontier control artifact failed: {0}")]
    Artifact(String),
}

fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}
fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

impl InterweaveControlReceipt {
    pub fn validate(&self) -> Result<(), InterweaveControlError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != contract_version()
            || self.feature_id != feature_id()
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.service_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.batch_id.trim().is_empty()
            || self.peer_order.is_empty()
            || self.job_order.is_empty()
            || !matches!(
                self.admission.as_str(),
                "admitted" | "degraded" | "approval_required" | "blocked" | "unknown"
            )
            || self.effect_receipts.is_empty()
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
            &self.job_order,
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
        if self.decisions.len() != self.job_order.len()
            || self
                .decisions
                .iter()
                .map(|d| d.job_id.as_str())
                .collect::<Vec<_>>()
                != self
                    .job_order
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
        {
            return Err(Self::invalid("control decisions do not match job order"));
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
                .any(|id| !self.job_order.contains(id))
        {
            return Err(Self::invalid(
                "control state references an unknown peer or job",
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("operate:interweave-frontier:")
                && !effect.starts_with("approval-required:")
                && effect != "block:unsafe-release"
        }) {
            return Err(Self::invalid("control effect is outside the frontier gate"));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| InterweaveControlError::Artifact(error.to_string()))
    }
    fn invalid(message: &str) -> InterweaveControlError {
        InterweaveControlError::Invalid(message.into())
    }
}

pub fn interweave_contract_frontier_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: feature_id().into(), version: contract_version().into(), owner_crate: "interweave".into(),
        consumers: ["platform reliability engineer".into(), "consortium operator".into(), "workflow scheduler".into()].into(),
        behavior: "admits prospective high-throughput Interweave job batches through version, peer, queue, checkpoint, capability, policy, authority, and locality gates".into(),
        value: "keeps federated workflow composition deterministic and recoverable while refusing unsupported protocol claims, unsafe effects, and hidden data movement".into(),
        inputs: vec![TypedPort { name: "interweave_control_batch".into(), schema: input_schema().into(), required: true }],
        outputs: vec![TypedPort { name: "interweave_control_receipt".into(), schema: output_schema().into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::WriteLocalArtifact, Effect::FederationExport].into(), permissions: ["operate:interweave-frontier".into()].into(), determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "mcp".into(), state: EvidenceState::Supported, locator: Some("https://modelcontextprotocol.io/specification/2025-06-18/basic/index".into()) }, EvidenceReference { source_id: "opentelemetry".into(), state: EvidenceState::Supported, locator: Some("https://opentelemetry.io/docs/specs/".into()) }],
        authority_requirements: vec![AuthorityRequirement { role: "consortium operator".into(), reason: "federated job admission changes shared execution state".into() }], autonomy_tier: AutonomyTier::A2,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Protocol, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn operate_interweave_frontier(
    request: &InterweaveControlPlaneRequest,
) -> Result<InterweaveControlReceipt, InterweaveControlError> {
    if request.request_id.trim().is_empty()
        || request.service_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.batch_id.trim().is_empty()
        || request.required_protocol_version.trim().is_empty()
        || request.capacity == 0
        || request.active_runs > request.capacity
        || request.checkpoint_seq == 0
        || request.jobs.is_empty()
        || request.peers.is_empty()
        || request.required_peer_quorum == 0
        || request.required_peer_quorum as usize > request.peers.len()
        || request
            .required_capabilities
            .iter()
            .any(|capability| capability.trim().is_empty())
        || !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
        || !digest(&request.replay_identity)
        || (request.signed_approval && request.approval_token.trim().is_empty())
    {
        return Err(InterweaveControlError::Invalid("control request identity, quorum, queue, locality, approval, replay, or boundary is invalid".into()));
    }
    let mut peers = request.peers.clone();
    peers.sort_by(|left, right| left.peer_id.cmp(&right.peer_id));
    if peers
        .windows(2)
        .any(|pair| pair[0].peer_id == pair[1].peer_id)
        || peers.iter().any(|peer| {
            peer.peer_id.trim().is_empty()
                || !digest(&peer.checkpoint_digest)
                || !peer.raw_data_local
        })
    {
        return Err(InterweaveControlError::Invalid(
            "peer identities must be unique, local, and digest-complete".into(),
        ));
    }
    let mut jobs = request.jobs.clone();
    jobs.sort_by(|left, right| left.job_id.cmp(&right.job_id));
    if jobs.windows(2).any(|pair| pair[0].job_id == pair[1].job_id)
        || jobs.iter().any(|job| {
            job.job_id.trim().is_empty()
                || job.workflow_id.trim().is_empty()
                || job.protocol_version.trim().is_empty()
                || job.idempotency_key.trim().is_empty()
                || !digest(&job.component_digest)
                || !digest(&job.input_digest)
                || job.capability_digests.is_empty()
                || job.capability_digests.iter().any(|value| !digest(value))
        })
    {
        return Err(InterweaveControlError::Invalid(
            "jobs must have unique ids, idempotency keys, workflow identity, and complete digests"
                .into(),
        ));
    }
    let peer_order = peers
        .iter()
        .map(|peer| peer.peer_id.clone())
        .collect::<Vec<_>>();
    let mut accepted_peer_order = Vec::new();
    let mut incompatible_peer_order = Vec::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut semantic_loss = Vec::new();
    for peer in &peers {
        if peer.protocol_version != request.required_protocol_version {
            incompatible_peer_order.push(peer.peer_id.clone());
            semantic_loss.push(SemanticLoss {
                field: format!("peer:{}:protocol_version", peer.peer_id),
                reason: "peer protocol is not the pinned frontier version".into(),
                severity: LossSeverity::DecisionRelevant,
            });
            continue;
        }
        if !peer.permitted_export {
            omissions.insert(format!("{}:export-denied", peer.peer_id));
            continue;
        }
        if !peer.healthy || !peer.signed_identity {
            uncertainty.insert(format!("{}:health-or-signature-unresolved", peer.peer_id));
            continue;
        }
        if !peer
            .capabilities
            .is_superset(&request.required_capabilities)
        {
            omissions.insert(format!("{}:required-capability-missing", peer.peer_id));
            continue;
        }
        accepted_peer_order.push(peer.peer_id.clone());
    }
    let mut job_order = Vec::new();
    let mut admitted_order = Vec::new();
    let mut conditional_order = Vec::new();
    let mut blocked_order = Vec::new();
    let mut unknown_order = Vec::new();
    let mut decisions = Vec::new();
    let mut negative_evidence = BTreeSet::new();
    let mut global_block: BTreeSet<String> = BTreeSet::new();
    if !request.policy_allow {
        global_block.insert("policy-allow".into());
    }
    if !request.protected_closure {
        global_block.insert("protected-closure".into());
    }
    if !request.signed_approval {
        global_block.insert("signed-approval".into());
    }
    if !request.network_permitted {
        global_block.insert("federation-permission".into());
    }
    if request.active_runs >= request.capacity {
        global_block.insert("capacity".into());
    }
    if accepted_peer_order.len() < request.required_peer_quorum as usize {
        global_block.insert("peer-quorum".into());
    }
    for job in &jobs {
        job_order.push(job.job_id.clone());
        let mut failed = BTreeSet::new();
        let mut conditional = BTreeSet::new();
        global_block.iter().for_each(|gate| {
            failed.insert(gate.clone());
        });
        if job.protocol_version != request.required_protocol_version {
            failed.insert("job-protocol-version".into());
        }
        if job
            .required_dimensions
            .iter()
            .any(|dimension| dimension.trim().is_empty())
        {
            failed.insert("dimension-contract".into());
        }
        if job.evidence_state == EvidenceState::Contradicted {
            failed.insert("contradicted-evidence".into());
        } else if matches!(
            job.evidence_state,
            EvidenceState::Unknown | EvidenceState::Speculative
        ) {
            conditional.insert("evidence-state".into());
            uncertainty.insert(format!("{}:evidence-state", job.job_id));
        }
        if job.negative_result {
            negative_evidence.insert(format!("{}:negative-result", job.job_id));
        }
        let disposition = if !failed.is_empty() {
            blocked_order.push(job.job_id.clone());
            "blocked"
        } else if !conditional.is_empty() {
            conditional_order.push(job.job_id.clone());
            "conditional"
        } else if accepted_peer_order.len() < request.required_peer_quorum as usize {
            unknown_order.push(job.job_id.clone());
            "unknown"
        } else {
            admitted_order.push(job.job_id.clone());
            "admitted"
        };
        decisions.push(InterweaveJobDecision {
            job_id: job.job_id.clone(),
            disposition: disposition.into(),
            failed_gates: failed.into_iter().collect(),
            conditional_gates: conditional.into_iter().collect(),
            negative_result: job.negative_result,
        });
    }
    if jobs.iter().any(|job| job.negative_result) {
        omissions.insert("negative-results-retained-in-receipt".into());
    }
    let admission = if !global_block.is_empty() || !blocked_order.is_empty() {
        "blocked"
    } else if !conditional_order.is_empty() {
        "approval_required"
    } else if !unknown_order.is_empty() {
        "unknown"
    } else if !incompatible_peer_order.is_empty() {
        "degraded"
    } else {
        "admitted"
    };
    let checkpoint_payload = json!({"batch_id": request.batch_id, "checkpoint_seq": request.checkpoint_seq, "peer_order": peer_order, "accepted_peer_order": accepted_peer_order, "job_order": job_order});
    let checkpoint_digest = ContentHash::of_value(&checkpoint_payload)
        .map_err(|error| InterweaveControlError::Invalid(error.to_string()))?;
    let queue_payload = json!({"batch_id": request.batch_id, "job_order": job_order, "capacity": request.capacity, "active_runs": request.active_runs});
    let queue_digest = ContentHash::of_value(&queue_payload)
        .map_err(|error| InterweaveControlError::Invalid(error.to_string()))?;
    let control_payload = json!({"feature_id": feature_id(), "request_id": request.request_id, "admission": admission, "peer_order": peer_order, "accepted_peer_order": accepted_peer_order, "incompatible_peer_order": incompatible_peer_order, "job_order": job_order, "admitted_order": admitted_order, "conditional_order": conditional_order, "blocked_order": blocked_order, "unknown_order": unknown_order, "checkpoint_digest": checkpoint_digest, "queue_digest": queue_digest, "replay_identity": request.replay_identity});
    let control_digest = ContentHash::of_value(&control_payload)
        .map_err(|error| InterweaveControlError::Invalid(error.to_string()))?;
    let effect_receipts = if admission == "admitted" {
        vec!["operate:interweave-frontier:admit".into()]
    } else if admission == "approval_required" {
        vec![
            "approval-required:interweave-frontier".into(),
            "block:unsafe-release".into(),
        ]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({"feature_id": feature_id(), "request_id": request.request_id, "service_id": request.service_id, "federation_id": request.federation_id, "purpose": request.purpose, "batch_id": request.batch_id, "admission": admission, "peer_order": peer_order, "accepted_peer_order": accepted_peer_order, "incompatible_peer_order": incompatible_peer_order, "job_order": job_order, "admitted_order": admitted_order, "conditional_order": conditional_order, "blocked_order": blocked_order, "unknown_order": unknown_order, "decisions": decisions, "checkpoint_seq": request.checkpoint_seq, "checkpoint_digest": checkpoint_digest, "queue_digest": queue_digest, "control_digest": control_digest, "replay_identity": request.replay_identity});
    let artifact = TypedResearchArtifact::from_payload(
        format!("{}:interweave-control", request.request_id),
        "application/vnd.aurora.interweave-control+json",
        &payload,
        semantic_loss.clone(),
        vec![ProvenanceLink {
            source_id: request.batch_id.clone(),
            relation: "compiled-from-interweave-control-batch".into(),
            digest: control_digest.clone(),
        }],
    )
    .map_err(|error| InterweaveControlError::Artifact(error.to_string()))?;
    let receipt = InterweaveControlReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: contract_version().into(),
        feature_id: feature_id().into(),
        request_id: request.request_id.clone(),
        service_id: request.service_id.clone(),
        federation_id: request.federation_id.clone(),
        purpose: request.purpose.clone(),
        batch_id: request.batch_id.clone(),
        admission: admission.into(),
        peer_order,
        accepted_peer_order,
        incompatible_peer_order,
        job_order,
        admitted_order,
        conditional_order,
        blocked_order,
        unknown_order,
        decisions,
        checkpoint_seq: request.checkpoint_seq,
        checkpoint_digest,
        queue_digest,
        control_digest,
        replay_identity: request.replay_identity.clone(),
        semantic_loss,
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative_evidence.into_iter().collect(),
        effect_receipts,
        artifact,
        raw_data_local: request.raw_data_local,
        boundary: request.boundary.clone(),
    };
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn job(id: &str, state: EvidenceState) -> InterweaveJob {
        InterweaveJob {
            job_id: id.into(),
            workflow_id: "scientific-claim-reproduction".into(),
            protocol_version: "interweave/1".into(),
            component_digest: hash("component"),
            input_digest: hash("input"),
            capability_digests: vec![hash("capability")],
            required_dimensions: ["C2".into(), "C6".into(), "C10".into()].into(),
            evidence_state: state,
            negative_result: id == "negative",
            idempotency_key: format!("idem-{id}"),
        }
    }
    fn request(jobs: Vec<InterweaveJob>) -> InterweaveControlPlaneRequest {
        InterweaveControlPlaneRequest {
            request_id: "request-1".into(),
            service_id: "service-1".into(),
            federation_id: "federation-1".into(),
            purpose: "preclinical-research".into(),
            batch_id: "batch-1".into(),
            required_protocol_version: "interweave/1".into(),
            required_capabilities: ["typed-workflow".into()].into(),
            capacity: 8,
            active_runs: 0,
            checkpoint_seq: 1,
            jobs,
            peers: vec![InterweavePeer {
                peer_id: "peer-a".into(),
                protocol_version: "interweave/1".into(),
                capabilities: ["typed-workflow".into()].into(),
                checkpoint_digest: hash("checkpoint"),
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
            replay_identity: hash("replay"),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn deterministic_batch_admits_and_replays() {
        let request = request(vec![
            job("z", EvidenceState::Proven),
            job("a", EvidenceState::Supported),
        ]);
        let first = operate_interweave_frontier(&request).unwrap();
        assert_eq!(first, operate_interweave_frontier(&request).unwrap());
        assert_eq!(first.admission, "admitted");
        assert_eq!(first.job_order, vec!["a", "z"]);
    }
    #[test]
    fn unknown_evidence_is_explicitly_conditional() {
        let receipt =
            operate_interweave_frontier(&request(vec![job("unknown", EvidenceState::Unknown)]))
                .unwrap();
        assert_eq!(receipt.admission, "approval_required");
        assert!(receipt
            .uncertainty
            .iter()
            .any(|item| item.contains("evidence-state")));
    }
    #[test]
    fn quorum_failure_blocks_and_preserves_negative() {
        let mut request = request(vec![job("negative", EvidenceState::Supported)]);
        request.required_peer_quorum = 2;
        request.peers.push(InterweavePeer {
            peer_id: "peer-b".into(),
            protocol_version: "interweave/0".into(),
            capabilities: ["typed-workflow".into()].into(),
            checkpoint_digest: hash("checkpoint-b"),
            healthy: true,
            signed_identity: true,
            permitted_export: true,
            raw_data_local: true,
        });
        let receipt = operate_interweave_frontier(&request).unwrap();
        assert_eq!(receipt.admission, "blocked");
        assert!(receipt
            .negative_evidence
            .iter()
            .any(|item| item.contains("negative-result")));
    }
    #[test]
    fn policy_denial_fails_closed() {
        let mut request = request(vec![job("blocked", EvidenceState::Proven)]);
        request.policy_allow = false;
        let receipt = operate_interweave_frontier(&request).unwrap();
        assert_eq!(receipt.admission, "blocked");
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn manifest_is_valid_a2_contract() {
        assert!(interweave_contract_frontier_manifest().validate().is_ok());
    }
}



//! Federated continual computational-execution control plane.
//!
//! Atlas feature: `AFA-atlasx-P12-F32`.
//!
//! The control plane admits a typed execution graph and peer capability attestations, but does
//! not dispatch code.  It deterministically plans dependencies, records checkpoints and effects,
//! and returns a content-addressed `ExecutionRun8`-compatible artifact.  Every missing,
//! contradictory, stale, revoked, unauthorized, or adversarial condition remains observable.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-atlasx-P12-F32";
pub const CONTRACT_VERSION: &str =
    "atlasx-federated-continual-computational-execution-control-plane/1.0";
pub const INPUT_SCHEMA: &str = "ResearchWorkflowSpec4@1";
pub const OUTPUT_SCHEMA: &str = "ExecutionRun8@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.atlasx-execution-run-8+json";
pub const MAX_NODES: usize = 4096;
pub const MAX_PEERS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionNode5 {
    pub node_id: String,
    pub capability_id: String,
    pub actor: String,
    pub dependency_order: Vec<String>,
    pub evidence_state: EvidenceState,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub policy_allowed: bool,
    pub signed_approval: bool,
    pub local_only: bool,
    pub aggregate_only: bool,
    pub stale: bool,
    pub revoked: bool,
    pub negative_result: bool,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerAttestation5 {
    pub peer_id: String,
    pub institution_id: String,
    pub capability_order: Vec<String>,
    pub protocol_version: String,
    pub semantic_profile: String,
    pub replay_identity: ContentHash,
    pub provenance_digest: ContentHash,
    pub signed_identity: bool,
    pub policy_allowed: bool,
    pub federation_allowed: bool,
    pub healthy: bool,
    pub stale: bool,
    pub revoked: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub omission_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchWorkflowSpec4 {
    pub schema_version: String,
    pub workflow_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_node_order: Vec<String>,
    pub required_peer_order: Vec<String>,
    pub required_capability_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub minimum_peer_count: u32,
    pub minimum_node_count: u32,
    pub max_parallelism: u32,
    pub max_cost_units: u64,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_allow: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub adversarial_event_order: Vec<String>,
    pub boundary: String,
    pub nodes: Vec<ExecutionNode5>,
    pub peers: Vec<PeerAttestation5>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionDisposition {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionRun8 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub workflow_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub disposition: ExecutionDisposition,
    pub node_order: Vec<String>,
    pub selected_node_order: Vec<String>,
    pub unresolved_node_order: Vec<String>,
    pub blocked_node_order: Vec<String>,
    pub missing_node_order: Vec<String>,
    pub peer_order: Vec<String>,
    pub selected_peer_order: Vec<String>,
    pub unresolved_peer_order: Vec<String>,
    pub blocked_peer_order: Vec<String>,
    pub missing_peer_order: Vec<String>,
    pub capability_order: Vec<String>,
    pub selected_capability_order: Vec<String>,
    pub missing_capability_order: Vec<String>,
    pub checkpoint_order: Vec<String>,
    pub event_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub contradiction_order: Vec<String>,
    pub dependency_cycle_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub provenance_digest: ContentHash,
    pub reasons: Vec<String>,
    pub run_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub autonomy_tier: AutonomyTier,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum FederatedExecutionError {
    #[error("invalid atlasx federated execution request or receipt: {0}")]
    Invalid(String),
    #[error("atlasx execution artifact failed: {0}")]
    Artifact(String),
}

fn invalid(message: impl Into<String>) -> FederatedExecutionError {
    FederatedExecutionError::Invalid(message.into())
}
fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}
fn digest_valid(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}
fn insert_all(target: &mut BTreeSet<String>, values: &[String]) {
    target.extend(values.iter().cloned());
}

pub fn federated_execution_control_plane_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "atlasx".into(),
        consumers: ["imaging core scientist".into(), "workflow operator".into(), "federation verifier".into()].into(),
        behavior: "plans a typed computational execution graph across policy-separated institutions and emits replayable local run metadata without dispatching code".into(),
        value: "makes dependency closure, peer capability, checkpoint, provenance, replay, capacity, and locality gates independently auditable before high-throughput execution".into(),
        inputs: vec![TypedPort { name: "research_workflow_spec".into(), schema: INPUT_SCHEMA.into(), required: true }],
        outputs: vec![TypedPort { name: "execution_run".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: [Effect::ExecuteLocalComputation, Effect::FederationExport].into(),
        permissions: ["operate:institution-node".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![
            EvidenceReference { source_id: "cwl".into(), state: EvidenceState::Supported, locator: Some("https://www.commonwl.org/specification/".into()) },
            EvidenceReference { source_id: "ga4gh-wes".into(), state: EvidenceState::Supported, locator: Some("https://ga4gh.github.io/workflow-execution-service-schemas/docs/".into()) },
            EvidenceReference { source_id: "opentelemetry".into(), state: EvidenceState::Supported, locator: Some("https://opentelemetry.io/docs/specs/".into()) },
        ],
        authority_requirements: vec![AuthorityRequirement { role: "institution execution operator".into(), reason: "A2 federation and local capability management require explicit institutional authority".into() }],
        autonomy_tier: AutonomyTier::A2,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Protocol, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

fn validate_request(request: &ResearchWorkflowSpec4) -> Result<(), FederatedExecutionError> {
    if request.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
        || request.workflow_id.trim().is_empty()
        || request.requester.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_node_order.is_empty()
        || request.required_peer_order.is_empty()
        || request.required_capability_order.is_empty()
        || !canonical(&request.required_node_order)
        || !canonical(&request.required_peer_order)
        || !canonical(&request.required_capability_order)
        || !canonical(&request.adversarial_event_order)
        || request.minimum_node_count == 0
        || request.minimum_peer_count == 0
        || request.max_parallelism == 0
        || request.max_cost_units == 0
        || !digest_valid(&request.replay_identity)
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
        || request.nodes.is_empty()
        || request.nodes.len() > MAX_NODES
        || request.peers.len() > MAX_PEERS
    {
        return Err(invalid("workflow identity, closure, capacity, replay, locality, boundary, or bounds are invalid"));
    }
    let mut node_ids = BTreeSet::new();
    for node in &request.nodes {
        if node.node_id.trim().is_empty()
            || node.capability_id.trim().is_empty()
            || node.actor.trim().is_empty()
            || !canonical(&node.dependency_order)
            || node.dependency_order.contains(&node.node_id)
            || !digest_valid(&node.provenance_digest)
            || !digest_valid(&node.replay_identity)
            || !canonical(&node.omission_order)
            || !canonical(&node.uncertainty_order)
            || !node_ids.insert(node.node_id.clone())
        {
            return Err(invalid(
                "node identity, dependency, digest, or ordering is invalid",
            ));
        }
    }
    let mut peer_ids = BTreeSet::new();
    for peer in &request.peers {
        if peer.peer_id.trim().is_empty()
            || peer.institution_id.trim().is_empty()
            || peer.protocol_version.trim().is_empty()
            || peer.semantic_profile != request.semantic_profile
            || peer.capability_order.is_empty()
            || !canonical(&peer.capability_order)
            || !canonical(&peer.omission_order)
            || !digest_valid(&peer.replay_identity)
            || !digest_valid(&peer.provenance_digest)
            || !peer_ids.insert(peer.peer_id.clone())
        {
            return Err(invalid(
                "peer identity, profile, capabilities, digest, or ordering is invalid",
            ));
        }
    }
    Ok(())
}

fn cycle_nodes(nodes: &BTreeMap<String, &ExecutionNode5>) -> BTreeSet<String> {
    fn visit(
        id: &str,
        nodes: &BTreeMap<String, &ExecutionNode5>,
        stack: &mut Vec<String>,
        done: &mut BTreeSet<String>,
        cycles: &mut BTreeSet<String>,
    ) {
        if stack.iter().any(|item| item == id) {
            if let Some(position) = stack.iter().position(|item| item == id) {
                cycles.extend(stack[position..].iter().cloned());
            }
            return;
        }
        if !done.insert(id.to_string()) {
            return;
        }
        stack.push(id.to_string());
        if let Some(node) = nodes.get(id) {
            for dependency in &node.dependency_order {
                if nodes.contains_key(dependency) {
                    visit(dependency, nodes, stack, done, cycles);
                }
            }
        }
        stack.pop();
    }
    let mut done = BTreeSet::new();
    let mut cycles = BTreeSet::new();
    for id in nodes.keys() {
        visit(id, nodes, &mut Vec::new(), &mut done, &mut cycles);
    }
    cycles
}

fn partition(
    universe: &[String],
    parts: &[&[String]],
    label: &str,
) -> Result<(), FederatedExecutionError> {
    let expected = universe.iter().cloned().collect::<BTreeSet<_>>();
    let mut flattened = Vec::new();
    if expected.len() != universe.len() {
        return Err(invalid(format!("{label} universe contains duplicates")));
    }
    for part in parts {
        if !canonical(part) || part.iter().any(|item| !expected.contains(item)) {
            return Err(invalid(format!("{label} state is not canonical")));
        }
        flattened.extend_from_slice(part);
    }
    if flattened.len() != expected.len()
        || flattened.iter().cloned().collect::<BTreeSet<_>>() != expected
    {
        return Err(invalid(format!(
            "{label} states do not form a complete partition"
        )));
    }
    Ok(())
}

impl ExecutionRun8 {
    pub fn validate(&self) -> Result<(), FederatedExecutionError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.content_type != CONTENT_TYPE
            || !self.raw_data_local
            || !self.aggregate_only
            || self.autonomy_tier != AutonomyTier::A2
            || self.workflow_id.trim().is_empty()
            || self.requester.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.node_order.is_empty()
            || self.peer_order.is_empty()
            || self.capability_order.is_empty()
            || self.checkpoint_order.is_empty()
            || self.reasons.is_empty()
            || self.effect_receipts.is_empty()
            || !digest_valid(&self.replay_identity)
            || !digest_valid(&self.provenance_digest)
            || !digest_valid(&self.run_digest)
            || self.artifact.content_hash != self.run_digest
        {
            return Err(invalid("execution identity, closure, digest, locality, autonomy, or effects are incomplete"));
        }
        for values in [
            &self.node_order,
            &self.selected_node_order,
            &self.unresolved_node_order,
            &self.blocked_node_order,
            &self.missing_node_order,
            &self.peer_order,
            &self.selected_peer_order,
            &self.unresolved_peer_order,
            &self.blocked_peer_order,
            &self.missing_peer_order,
            &self.capability_order,
            &self.selected_capability_order,
            &self.missing_capability_order,
            &self.checkpoint_order,
            &self.event_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.contradiction_order,
            &self.dependency_cycle_order,
        ] {
            if !canonical(values) {
                return Err(invalid("execution receipt ordering is not canonical"));
            }
        }
        partition(
            &self.node_order,
            &[
                &self.selected_node_order,
                &self.unresolved_node_order,
                &self.blocked_node_order,
                &self.missing_node_order,
            ],
            "node",
        )?;
        partition(
            &self.peer_order,
            &[
                &self.selected_peer_order,
                &self.unresolved_peer_order,
                &self.blocked_peer_order,
                &self.missing_peer_order,
            ],
            "peer",
        )?;
        partition(
            &self.capability_order,
            &[
                &self.selected_capability_order,
                &self.missing_capability_order,
            ],
            "capability",
        )?;
        if self.disposition == ExecutionDisposition::Qualified
            && self.effect_receipts
                != [
                    "manage:local-capability".to_string(),
                    "exchange:permitted-summaries".to_string(),
                ]
        {
            return Err(invalid("qualified execution effects are invalid"));
        }
        if self.disposition != ExecutionDisposition::Qualified
            && self.effect_receipts != ["block:unsafe-release".to_string()]
        {
            return Err(invalid("non-qualified execution must fail closed"));
        }
        self.artifact
            .validate_metadata()
            .map_err(|e| FederatedExecutionError::Artifact(e.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, FederatedExecutionError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|e| FederatedExecutionError::Artifact(e.to_string()))?,
        )
        .map_err(|e| FederatedExecutionError::Artifact(e.to_string()))
    }
}

pub fn plan_federated_execution(
    spec: &ResearchWorkflowSpec4,
) -> Result<ExecutionRun8, FederatedExecutionError> {
    validate_request(spec)?;
    let node_map = spec
        .nodes
        .iter()
        .map(|node| (node.node_id.clone(), node))
        .collect::<BTreeMap<_, _>>();
    let cycle_order = cycle_nodes(&node_map).into_iter().collect::<Vec<_>>();
    let required_nodes = spec
        .required_node_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut node_ids = required_nodes.clone();
    node_ids.extend(node_map.keys().cloned());
    let node_order = node_ids.into_iter().collect::<Vec<_>>();
    let mut state = BTreeMap::<String, ExecutionDisposition>::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut contradictions = BTreeSet::new();
    for id in &node_order {
        let Some(node) = node_map.get(id) else {
            state.insert(id.clone(), ExecutionDisposition::Blocked);
            omissions.insert(format!("missing required node: {id}"));
            continue;
        };
        insert_all(&mut omissions, &node.omission_order);
        insert_all(&mut uncertainty, &node.uncertainty_order);
        if node.negative_result {
            negative.insert(id.clone());
        }
        if node.evidence_state == EvidenceState::Contradicted {
            contradictions.insert(id.clone());
        }
        let hard_block = node.revoked
            || !node.policy_allowed
            || !node.signed_approval
            || !node.local_only
            || !node.aggregate_only;
        let soft_unknown = node.stale
            || node.replay_identity != spec.replay_identity
            || !node.omission_order.is_empty()
            || !node.uncertainty_order.is_empty()
            || matches!(
                node.evidence_state,
                EvidenceState::Unknown | EvidenceState::Speculative | EvidenceState::Contradicted
            )
            || cycle_order.contains(id);
        state.insert(
            id.clone(),
            if hard_block {
                ExecutionDisposition::Blocked
            } else if soft_unknown {
                ExecutionDisposition::Unresolved
            } else {
                ExecutionDisposition::Qualified
            },
        );
    }
    for id in &node_order {
        if state.get(id) == Some(&ExecutionDisposition::Qualified) {
            if let Some(node) = node_map.get(id) {
                if node
                    .dependency_order
                    .iter()
                    .any(|dep| !node_map.contains_key(dep))
                {
                    state.insert(id.clone(), ExecutionDisposition::Unresolved);
                    omissions.insert(format!("{id}:missing-dependency"));
                } else if node
                    .dependency_order
                    .iter()
                    .any(|dep| state.get(dep) != Some(&ExecutionDisposition::Qualified))
                {
                    state.insert(id.clone(), ExecutionDisposition::Unresolved);
                    uncertainty.insert(format!("{id}:dependency-not-qualified"));
                }
            }
        }
    }
    let selected_node_order = node_order
        .iter()
        .filter(|id| state.get(*id) == Some(&ExecutionDisposition::Qualified))
        .cloned()
        .collect::<Vec<_>>();
    let unresolved_node_order = node_order
        .iter()
        .filter(|id| state.get(*id) == Some(&ExecutionDisposition::Unresolved))
        .cloned()
        .collect::<Vec<_>>();
    let blocked_node_order = node_order
        .iter()
        .filter(|id| {
            state.get(*id) == Some(&ExecutionDisposition::Blocked) && node_map.contains_key(*id)
        })
        .cloned()
        .collect::<Vec<_>>();
    let missing_node_order = node_order
        .iter()
        .filter(|id| !node_map.contains_key(*id))
        .cloned()
        .collect::<Vec<_>>();
    let peer_map = spec
        .peers
        .iter()
        .map(|peer| (peer.peer_id.clone(), peer))
        .collect::<BTreeMap<_, _>>();
    let mut peer_ids = spec
        .required_peer_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    peer_ids.extend(peer_map.keys().cloned());
    let peer_order = peer_ids.into_iter().collect::<Vec<_>>();
    let mut peer_state = BTreeMap::new();
    let mut selected_peers = Vec::new();
    let mut unresolved_peers = Vec::new();
    let mut blocked_peers = Vec::new();
    let mut missing_peers = Vec::new();
    for id in &peer_order {
        let Some(peer) = peer_map.get(id) else {
            peer_state.insert(id.clone(), ExecutionDisposition::Blocked);
            missing_peers.push(id.clone());
            omissions.insert(format!("missing required peer: {id}"));
            continue;
        };
        insert_all(&mut omissions, &peer.omission_order);
        let hard = peer.revoked
            || !peer.signed_identity
            || !peer.policy_allowed
            || !peer.federation_allowed
            || !peer.raw_data_local
            || !peer.aggregate_only
            || peer.replay_identity != spec.replay_identity;
        let soft = peer.stale || !peer.healthy;
        let value = if hard {
            ExecutionDisposition::Blocked
        } else if soft {
            ExecutionDisposition::Unresolved
        } else {
            ExecutionDisposition::Qualified
        };
        peer_state.insert(id.clone(), value);
        match value {
            ExecutionDisposition::Qualified => selected_peers.push(id.clone()),
            ExecutionDisposition::Unresolved => unresolved_peers.push(id.clone()),
            ExecutionDisposition::Blocked => blocked_peers.push(id.clone()),
        };
    }
    let mut capabilities = spec
        .required_capability_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    for node in &spec.nodes {
        capabilities.insert(node.capability_id.clone());
    }
    for peer in &spec.peers {
        insert_all(&mut capabilities, &peer.capability_order);
    }
    let capability_order = capabilities.into_iter().collect::<Vec<_>>();
    let declared_capabilities = spec
        .nodes
        .iter()
        .map(|node| node.capability_id.clone())
        .chain(
            spec.peers
                .iter()
                .flat_map(|peer| peer.capability_order.iter().cloned()),
        )
        .collect::<BTreeSet<_>>();
    let missing_required_capabilities = spec
        .required_capability_order
        .iter()
        .filter(|capability| !declared_capabilities.contains(*capability))
        .count();
    let selected_capability_order = capability_order
        .iter()
        .filter(|cap| {
            spec.nodes.iter().any(|node| {
                node.capability_id == **cap
                    && state.get(&node.node_id) == Some(&ExecutionDisposition::Qualified)
            })
        })
        .cloned()
        .collect::<Vec<_>>();
    let missing_capability_order = capability_order
        .iter()
        .filter(|cap| !selected_capability_order.contains(cap))
        .cloned()
        .collect::<Vec<_>>();
    let checkpoint_order = node_order
        .iter()
        .enumerate()
        .map(|(index, id)| format!("checkpoint:{:04}:{}", index + 1, id))
        .collect::<Vec<_>>();
    let mut event_order = node_order
        .iter()
        .map(|id| format!("plan:{id}"))
        .chain(peer_order.iter().map(|id| format!("peer:{id}")))
        .collect::<Vec<_>>();
    event_order.sort();
    let disposition = if !spec.policy_allow
        || !spec.protected_closure
        || !spec.signed_approval
        || !spec.federation_allow
        || !spec.raw_data_local
        || !spec.aggregate_only
        || !spec.adversarial_event_order.is_empty()
        || (selected_node_order.len() < spec.minimum_node_count as usize
            && unresolved_node_order.is_empty())
        || (selected_peers.len() < spec.minimum_peer_count as usize && unresolved_peers.is_empty())
        || !missing_node_order.is_empty()
        || !missing_peers.is_empty()
        || !blocked_node_order.is_empty()
        || !blocked_peers.is_empty()
        || missing_required_capabilities > 0
    {
        ExecutionDisposition::Blocked
    } else if !unresolved_node_order.is_empty() || !unresolved_peers.is_empty() {
        ExecutionDisposition::Unresolved
    } else {
        ExecutionDisposition::Qualified
    };
    let reasons = vec![match disposition { ExecutionDisposition::Qualified => "all execution planning and federation gates passed".into(), ExecutionDisposition::Unresolved => "stale, uncertain, contradictory, omitted, dependency, or health evidence prevents execution admission".into(), ExecutionDisposition::Blocked => "policy, approval, closure, capability, peer, locality, or adversarial gates blocked execution admission".into() }];
    let effect_receipts = if disposition == ExecutionDisposition::Qualified {
        vec![
            "manage:local-capability".into(),
            "exchange:permitted-summaries".into(),
        ]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let provenance_digest = spec
        .nodes
        .iter()
        .map(|node| node.provenance_digest.to_string())
        .chain(
            spec.peers
                .iter()
                .map(|peer| peer.provenance_digest.to_string()),
        )
        .collect::<Vec<_>>();
    let payload = json!({ "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "workflow_id": spec.workflow_id, "requester": spec.requester, "purpose": spec.purpose, "semantic_profile": spec.semantic_profile, "disposition": disposition, "node_order": node_order, "selected_node_order": selected_node_order, "unresolved_node_order": unresolved_node_order, "blocked_node_order": blocked_node_order, "missing_node_order": missing_node_order, "peer_order": peer_order, "selected_peer_order": selected_peers, "unresolved_peer_order": unresolved_peers, "blocked_peer_order": blocked_peers, "missing_peer_order": missing_peers, "capability_order": capability_order, "selected_capability_order": selected_capability_order, "missing_capability_order": missing_capability_order, "checkpoint_order": checkpoint_order, "event_order": event_order, "omission_order": omissions, "uncertainty_order": uncertainty, "negative_evidence_order": negative, "contradiction_order": contradictions, "dependency_cycle_order": cycle_order, "replay_identity": spec.replay_identity, "provenance_digest": provenance_digest, "reasons": reasons, "effect_receipts": effect_receipts, "raw_data_local": spec.raw_data_local, "aggregate_only": spec.aggregate_only, "autonomy_tier": AutonomyTier::A2, "boundary": PRECLINICAL_BOUNDARY });
    let artifact = TypedResearchArtifact::from_payload(
        format!("execution-run:{}", spec.workflow_id),
        CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|e| FederatedExecutionError::Artifact(e.to_string()))?;
    let run_digest = artifact.content_hash.clone();
    let receipt = ExecutionRun8 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        workflow_id: spec.workflow_id.clone(),
        requester: spec.requester.clone(),
        purpose: spec.purpose.clone(),
        semantic_profile: spec.semantic_profile.clone(),
        disposition,
        node_order: serde_json::from_value(payload["node_order"].clone()).unwrap(),
        selected_node_order: serde_json::from_value(payload["selected_node_order"].clone())
            .unwrap(),
        unresolved_node_order: serde_json::from_value(payload["unresolved_node_order"].clone())
            .unwrap(),
        blocked_node_order: serde_json::from_value(payload["blocked_node_order"].clone()).unwrap(),
        missing_node_order: serde_json::from_value(payload["missing_node_order"].clone()).unwrap(),
        peer_order: serde_json::from_value(payload["peer_order"].clone()).unwrap(),
        selected_peer_order: serde_json::from_value(payload["selected_peer_order"].clone())
            .unwrap(),
        unresolved_peer_order: serde_json::from_value(payload["unresolved_peer_order"].clone())
            .unwrap(),
        blocked_peer_order: serde_json::from_value(payload["blocked_peer_order"].clone()).unwrap(),
        missing_peer_order: serde_json::from_value(payload["missing_peer_order"].clone()).unwrap(),
        capability_order: serde_json::from_value(payload["capability_order"].clone()).unwrap(),
        selected_capability_order: serde_json::from_value(
            payload["selected_capability_order"].clone(),
        )
        .unwrap(),
        missing_capability_order: serde_json::from_value(
            payload["missing_capability_order"].clone(),
        )
        .unwrap(),
        checkpoint_order: serde_json::from_value(payload["checkpoint_order"].clone()).unwrap(),
        event_order: serde_json::from_value(payload["event_order"].clone()).unwrap(),
        omission_order: omissions.into_iter().collect(),
        uncertainty_order: uncertainty.into_iter().collect(),
        negative_evidence_order: negative.into_iter().collect(),
        contradiction_order: contradictions.into_iter().collect(),
        dependency_cycle_order: serde_json::from_value(payload["dependency_cycle_order"].clone())
            .unwrap(),
        replay_identity: spec.replay_identity.clone(),
        provenance_digest: ContentHash::of_bytes(provenance_digest.join("|").as_bytes()),
        reasons,
        run_digest,
        artifact,
        effect_receipts,
        raw_data_local: spec.raw_data_local,
        aggregate_only: spec.aggregate_only,
        autonomy_tier: AutonomyTier::A2,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(label: &str) -> ContentHash {
        ContentHash::of_bytes(label.as_bytes())
    }
    fn node(id: &str, dep: Vec<String>) -> ExecutionNode5 {
        ExecutionNode5 {
            node_id: id.into(),
            capability_id: format!("cap:{id}"),
            actor: "operator".into(),
            dependency_order: dep,
            evidence_state: EvidenceState::Supported,
            provenance_digest: hash(id),
            replay_identity: hash("replay"),
            policy_allowed: true,
            signed_approval: true,
            local_only: true,
            aggregate_only: true,
            stale: false,
            revoked: false,
            negative_result: false,
            omission_order: Vec::new(),
            uncertainty_order: Vec::new(),
        }
    }
    fn peer() -> PeerAttestation5 {
        PeerAttestation5 {
            peer_id: "peer:a".into(),
            institution_id: "site:a".into(),
            capability_order: vec!["cap:run:1".into()],
            protocol_version: "wes-1.0".into(),
            semantic_profile: "imaging".into(),
            replay_identity: hash("replay"),
            provenance_digest: hash("peer"),
            signed_identity: true,
            policy_allowed: true,
            federation_allowed: true,
            healthy: true,
            stale: false,
            revoked: false,
            raw_data_local: true,
            aggregate_only: true,
            omission_order: Vec::new(),
        }
    }
    fn spec(nodes: Vec<ExecutionNode5>) -> ResearchWorkflowSpec4 {
        ResearchWorkflowSpec4 {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            workflow_id: "workflow:1".into(),
            requester: "imaging scientist".into(),
            purpose: "reproducible execution".into(),
            semantic_profile: "imaging".into(),
            required_node_order: vec!["run:1".into()],
            required_peer_order: vec!["peer:a".into()],
            required_capability_order: vec!["cap:run:1".into()],
            replay_identity: hash("replay"),
            minimum_peer_count: 1,
            minimum_node_count: 1,
            max_parallelism: 4,
            max_cost_units: 100,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_allow: true,
            raw_data_local: true,
            aggregate_only: true,
            adversarial_event_order: Vec::new(),
            boundary: PRECLINICAL_BOUNDARY.into(),
            nodes,
            peers: vec![peer()],
        }
    }
    #[test]
    fn qualified_plan_is_replayable() {
        let run = plan_federated_execution(&spec(vec![node("run:1", Vec::new())])).unwrap();
        assert_eq!(run.disposition, ExecutionDisposition::Qualified);
        assert_eq!(run.effect_receipts.len(), 2);
    }
    #[test]
    fn dependency_order_is_deterministic() {
        let mut a = node("run:1", vec!["run:0".into()]);
        let b = node("run:0", Vec::new());
        let result = plan_federated_execution(&spec(vec![a.clone(), b])).unwrap();
        assert_eq!(result.selected_node_order, vec!["run:0", "run:1"]);
        a.dependency_order = vec!["missing".into()];
        let unresolved = plan_federated_execution(&spec(vec![a])).unwrap();
        assert_eq!(unresolved.disposition, ExecutionDisposition::Unresolved);
    }
    #[test]
    fn unknown_evidence_is_unresolved() {
        let mut item = node("run:1", Vec::new());
        item.evidence_state = EvidenceState::Unknown;
        let result = plan_federated_execution(&spec(vec![item])).unwrap();
        assert_eq!(result.disposition, ExecutionDisposition::Unresolved);
    }
    #[test]
    fn revoked_peer_blocks() {
        let mut item = peer();
        item.revoked = true;
        let mut request = spec(vec![node("run:1", Vec::new())]);
        request.peers = vec![item];
        let result = plan_federated_execution(&request).unwrap();
        assert_eq!(result.disposition, ExecutionDisposition::Blocked);
    }
    #[test]
    fn adversarial_event_blocks_without_erasure() {
        let mut request = spec(vec![node("run:1", Vec::new())]);
        request.adversarial_event_order = vec!["prompt-injection".into()];
        let result = plan_federated_execution(&request).unwrap();
        assert_eq!(result.disposition, ExecutionDisposition::Blocked);
        assert!(result.event_order.iter().any(|event| event == "plan:run:1"));
    }
    #[test]
    fn manifest_is_valid() {
        federated_execution_control_plane_manifest()
            .validate()
            .unwrap();
    }
}

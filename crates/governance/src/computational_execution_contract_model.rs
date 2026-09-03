//! Governance-owned computational-execution contract model (`AFA-governance-P12-F08`).
//!
//! This module validates a declared research DAG and emits a deterministic, dry-run contract.
//! It never dispatches code, contacts instruments, moves raw data, or makes clinical decisions.

use bioprism_foundation::{PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-governance-P12-F08";
pub const CONTRACT_VERSION: &str =
    "governance-federated-continual-computational-execution-contract-model/1.0";
pub const INPUT_SCHEMA: &str = "ExecutionContractRequest5@1";
pub const OUTPUT_SCHEMA: &str = "GovernanceExecutionContract8@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.governance-execution-contract-8+json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionEvidenceState {
    Proven,
    Supported,
    Unknown,
    Unmeasured,
    Contradicted,
    Negative,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionNode5 {
    pub node_id: String,
    pub dependency_order: Vec<String>,
    pub executor: String,
    pub input_schema: String,
    pub output_schema: String,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub estimated_units: u64,
    pub evidence_state: ExecutionEvidenceState,
    pub deterministic: bool,
    pub local_only: bool,
    pub permitted: bool,
    pub signed: bool,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionPeer5 {
    pub peer_id: String,
    pub origin: String,
    pub workflow_id: String,
    pub semantic_profile: String,
    pub checkpoint: u64,
    pub contract_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub evidence_state: ExecutionEvidenceState,
    pub signed: bool,
    pub aggregate_only: bool,
    pub raw_data_local: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionContractRequest5 {
    pub schema_version: String,
    pub request_id: String,
    pub federation_id: String,
    pub workflow_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub engine_version: String,
    pub nodes: Vec<ExecutionNode5>,
    pub peers: Vec<ExecutionPeer5>,
    pub checkpoint: u64,
    pub max_budget_units: u64,
    pub minimum_peer_quorum: usize,
    pub policy_allowed: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_allowed: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub replay_identity: ContentHash,
    pub adversarial_event_order: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceExecutionArtifact8 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceExecutionContract8 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub workflow_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub engine_version: String,
    pub checkpoint: u64,
    pub disposition: String,
    pub node_order: Vec<String>,
    pub planned_node_order: Vec<String>,
    pub unresolved_node_order: Vec<String>,
    pub blocked_node_order: Vec<String>,
    pub cycle_order: Vec<String>,
    pub missing_dependency_order: Vec<String>,
    pub peer_order: Vec<String>,
    pub qualified_peer_order: Vec<String>,
    pub missing_peer_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub total_units: u64,
    pub replay_identity: ContentHash,
    pub contract_digest: ContentHash,
    pub artifact: GovernanceExecutionArtifact8,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum GovernanceExecutionContractError {
    #[error("invalid execution contract request: {0}")]
    Invalid(String),
    #[error("execution contract artifact failed: {0}")]
    Artifact(String),
}

fn valid_hash(hash: &ContentHash) -> bool {
    hash.as_str().len() == 64 && hash.as_str().bytes().all(|b| b.is_ascii_hexdigit())
}
fn ordered(values: &[String]) -> bool {
    values.windows(2).all(|w| w[0] < w[1])
}

pub fn computational_execution_contract_model_manifest() -> Value {
    json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "capability_id": FEATURE_ID, "version": CONTRACT_VERSION,
        "owner_crate": "governance", "consumers": ["computational biologist", "workflow compiler engineer", "governance steward"],
        "behavior": "validate a federated computational workflow contract and emit a deterministic dry-run execution artifact",
        "value": "makes graph closure, evidence, budget, replay, peer, policy, and locality conditions auditable before dispatch",
        "input_schema": INPUT_SCHEMA, "output_schema": OUTPUT_SCHEMA,
        "effects": ["exchange:aggregate-contract", "retain:execution-contract", "block:unsafe-release"],
        "permissions": ["read:local-workflow-manifests", "evaluate:capability-runs"], "autonomy_tier": "A1", "boundary": PRECLINICAL_BOUNDARY,
    })
}

fn validate_request(
    request: &ExecutionContractRequest5,
) -> Result<(), GovernanceExecutionContractError> {
    if request.schema_version != INPUT_SCHEMA
        || [
            &request.request_id,
            &request.federation_id,
            &request.workflow_id,
            &request.requester,
            &request.purpose,
            &request.semantic_profile,
            &request.engine_version,
        ]
        .iter()
        .any(|s| s.trim().is_empty())
        || request.nodes.is_empty()
        || request.peers.is_empty()
        || request.checkpoint == 0
        || request.max_budget_units == 0
        || request.minimum_peer_quorum == 0
        || !valid_hash(&request.replay_identity)
        || !ordered(&request.adversarial_event_order)
        || !request.raw_data_local
        || !request.aggregate_only
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(GovernanceExecutionContractError::Invalid(
            "identity, bounds, nodes, peers, replay, locality, or boundary is invalid".into(),
        ));
    }
    let mut nodes = BTreeSet::new();
    for node in &request.nodes {
        if node.node_id.trim().is_empty()
            || !nodes.insert(node.node_id.clone())
            || node.executor.trim().is_empty()
            || node.input_schema.trim().is_empty()
            || node.output_schema.trim().is_empty()
            || node.estimated_units == 0
            || !valid_hash(&node.artifact_digest)
            || !valid_hash(&node.provenance_digest)
            || !valid_hash(&node.replay_identity)
        {
            return Err(GovernanceExecutionContractError::Invalid(
                "node identity, schemas, budget, or digests are invalid".into(),
            ));
        }
    }
    let mut peers = BTreeSet::new();
    for peer in &request.peers {
        if peer.peer_id.trim().is_empty()
            || !peers.insert(peer.peer_id.clone())
            || peer.origin.trim().is_empty()
            || peer.workflow_id.trim().is_empty()
            || peer.semantic_profile.trim().is_empty()
            || peer.checkpoint == 0
            || !valid_hash(&peer.contract_digest)
            || !valid_hash(&peer.replay_identity)
        {
            return Err(GovernanceExecutionContractError::Invalid(
                "peer identity, checkpoint, or digests are invalid".into(),
            ));
        }
    }
    Ok(())
}

impl GovernanceExecutionContract8 {
    pub fn validate(&self) -> Result<(), GovernanceExecutionContractError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.content_type != CONTENT_TYPE
            || !self.raw_data_local
            || !self.aggregate_only
            || self.checkpoint == 0
            || self.node_order.is_empty()
            || self.peer_order.is_empty()
            || self.effect_receipts.is_empty()
            || !matches!(
                self.disposition.as_str(),
                "qualified" | "unresolved" | "blocked"
            )
        {
            return Err(GovernanceExecutionContractError::Invalid(
                "identity, graph, peers, locality, or effects are incomplete".into(),
            ));
        }
        for values in [
            &self.node_order,
            &self.planned_node_order,
            &self.unresolved_node_order,
            &self.blocked_node_order,
            &self.cycle_order,
            &self.missing_dependency_order,
            &self.peer_order,
            &self.qualified_peer_order,
            &self.missing_peer_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !ordered(values) {
                return Err(GovernanceExecutionContractError::Invalid(
                    "execution ordering is not canonical".into(),
                ));
            }
        }
        let nodes = self.node_order.iter().cloned().collect::<BTreeSet<_>>();
        let states = self
            .planned_node_order
            .iter()
            .chain(&self.unresolved_node_order)
            .chain(&self.blocked_node_order)
            .cloned()
            .collect::<Vec<_>>();
        let peers = self.peer_order.iter().cloned().collect::<BTreeSet<_>>();
        let peer_states = self
            .qualified_peer_order
            .iter()
            .chain(&self.missing_peer_order)
            .cloned()
            .collect::<Vec<_>>();
        if nodes.len() != self.node_order.len()
            || states.len() != nodes.len()
            || BTreeSet::from_iter(states) != nodes
            || peers.len() != self.peer_order.len()
            || peer_states.len() != peers.len()
            || BTreeSet::from_iter(peer_states) != peers
        {
            return Err(GovernanceExecutionContractError::Invalid(
                "node or peer outcomes do not partition".into(),
            ));
        }
        if !valid_hash(&self.replay_identity)
            || !valid_hash(&self.contract_digest)
            || self.artifact.content_hash != self.contract_digest
            || self
                .artifact
                .provenance_digests
                .iter()
                .any(|h| !valid_hash(h))
        {
            return Err(GovernanceExecutionContractError::Artifact(
                "contract or provenance digest is invalid".into(),
            ));
        }
        if self.disposition == "qualified"
            && self.effect_receipts
                != vec!["exchange:aggregate-contract", "retain:execution-contract"]
        {
            return Err(GovernanceExecutionContractError::Invalid(
                "qualified execution effects are invalid".into(),
            ));
        }
        if self.disposition != "qualified" && self.effect_receipts != vec!["block:unsafe-release"] {
            return Err(GovernanceExecutionContractError::Invalid(
                "non-qualified execution must block release".into(),
            ));
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<ContentHash, GovernanceExecutionContractError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|e| GovernanceExecutionContractError::Artifact(e.to_string()))?,
        )
        .map_err(|e| GovernanceExecutionContractError::Artifact(e.to_string()))
    }
}

pub fn model_computational_execution_contract(
    request: &ExecutionContractRequest5,
) -> Result<GovernanceExecutionContract8, GovernanceExecutionContractError> {
    validate_request(request)?;
    let mut nodes = request.nodes.clone();
    nodes.sort_by(|a, b| a.node_id.cmp(&b.node_id));
    let node_order = nodes.iter().map(|n| n.node_id.clone()).collect::<Vec<_>>();
    let mut remaining = nodes
        .iter()
        .map(|n| {
            (
                n.node_id.clone(),
                n.dependency_order
                    .iter()
                    .filter(|d| node_order.contains(d))
                    .count(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut ready = remaining
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(id, _)| id.clone())
        .collect::<Vec<_>>();
    ready.sort();
    let mut plan = Vec::new();
    while let Some(id) = ready.first().cloned() {
        ready.remove(0);
        plan.push(id.clone());
        for node in &nodes {
            if node.dependency_order.contains(&id) {
                if let Some(count) = remaining.get_mut(&node.node_id) {
                    *count = count.saturating_sub(1);
                    if *count == 0 {
                        ready.push(node.node_id.clone());
                        ready.sort();
                    }
                }
            }
        }
    }
    let cycles = remaining
        .iter()
        .filter(|(_, count)| **count > 0)
        .map(|(id, _)| id.clone())
        .collect::<BTreeSet<_>>();
    let by_id = nodes
        .iter()
        .map(|n| (n.node_id.clone(), n))
        .collect::<BTreeMap<_, _>>();
    let missing = nodes
        .iter()
        .flat_map(|n| {
            n.dependency_order
                .iter()
                .filter(|d| !by_id.contains_key(*d))
                .map(|d| format!("{}:missing:{}", n.node_id, d))
        })
        .collect::<BTreeSet<_>>();
    let mut planned = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut total = 0u64;
    for node in &nodes {
        total = total.saturating_add(node.estimated_units);
        let id = node.node_id.clone();
        if node.negative_result {
            negative.insert(format!("{id}:negative-result"));
        }
        let hard = cycles.contains(&id)
            || node.replay_identity != request.replay_identity
            || !node.local_only
            || !node.permitted
            || !node.signed
            || !node.deterministic
            || matches!(
                node.evidence_state,
                ExecutionEvidenceState::Contradicted | ExecutionEvidenceState::Negative
            );
        if hard {
            if cycles.contains(&id)
                || matches!(
                    node.evidence_state,
                    ExecutionEvidenceState::Contradicted | ExecutionEvidenceState::Negative
                )
                || !node.local_only
            {
                blocked.insert(id);
            } else {
                unresolved.insert(id);
            }
        } else if !matches!(
            node.evidence_state,
            ExecutionEvidenceState::Proven | ExecutionEvidenceState::Supported
        ) {
            unresolved.insert(id.clone());
            uncertainty.insert(format!("{id}:evidence-state"));
        } else if node
            .dependency_order
            .iter()
            .any(|d| !by_id.contains_key(d) || cycles.contains(d))
        {
            blocked.insert(id);
        } else {
            planned.insert(id);
        }
    }
    if !missing.is_empty() {
        omissions.insert("request:missing-dependency-closure".into());
    }
    if total > request.max_budget_units {
        omissions.insert(format!("request:budget-exceeded:{total}"));
    }
    let peer_order = request
        .peers
        .iter()
        .map(|p| p.peer_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let qualified_peers = request
        .peers
        .iter()
        .filter(|p| {
            p.workflow_id == request.workflow_id
                && p.semantic_profile == request.semantic_profile
                && p.checkpoint >= request.checkpoint
                && p.replay_identity == request.replay_identity
                && p.signed
                && p.aggregate_only
                && p.raw_data_local
                && matches!(
                    p.evidence_state,
                    ExecutionEvidenceState::Proven | ExecutionEvidenceState::Supported
                )
        })
        .map(|p| p.peer_id.clone())
        .collect::<BTreeSet<_>>();
    let missing_peers = peer_order
        .iter()
        .filter(|p| !qualified_peers.contains(*p))
        .cloned()
        .collect::<Vec<_>>();
    if qualified_peers.len() < request.minimum_peer_quorum {
        uncertainty.insert("peer:minimum-quorum-unmet".into());
    }
    uncertainty.extend(
        request
            .adversarial_event_order
            .iter()
            .map(|e| format!("adversarial:{e}")),
    );
    let global_block = !request.policy_allowed
        || !request.protected_closure
        || !request.signed_approval
        || !request.federation_allowed
        || !request.raw_data_local
        || !request.aggregate_only
        || !request.adversarial_event_order.is_empty();
    if global_block {
        blocked.extend(node_order.iter().cloned());
        planned.clear();
        unresolved.clear();
        omissions.insert("request:governance-or-adversarial-blocked".into());
    }
    let disposition = if global_block {
        "blocked"
    } else if !blocked.is_empty()
        || !unresolved.is_empty()
        || !missing.is_empty()
        || total > request.max_budget_units
        || qualified_peers.len() < request.minimum_peer_quorum
        || planned.is_empty()
    {
        "unresolved"
    } else {
        "qualified"
    };
    if disposition != "qualified" {
        omissions.insert("request:execution-contract-not-release-ready".into());
    }
    let mut payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"federation_id":request.federation_id,"workflow_id":request.workflow_id,"requester":request.requester,"purpose":request.purpose,"semantic_profile":request.semantic_profile,"engine_version":request.engine_version,"checkpoint":request.checkpoint,"disposition":disposition,"node_order":node_order,"planned_node_order":planned.into_iter().collect::<Vec<_>>(),"unresolved_node_order":unresolved.into_iter().collect::<Vec<_>>(),"blocked_node_order":blocked.into_iter().collect::<Vec<_>>(),"cycle_order":cycles.into_iter().collect::<Vec<_>>(),"missing_dependency_order":missing.into_iter().collect::<Vec<_>>(),"peer_order":peer_order,"qualified_peer_order":qualified_peers.into_iter().collect::<Vec<_>>(),"missing_peer_order":missing_peers,"omission_order":omissions.into_iter().collect::<Vec<_>>(),"uncertainty_order":uncertainty.into_iter().collect::<Vec<_>>(),"negative_evidence_order":negative.into_iter().collect::<Vec<_>>(),"total_units":total,"replay_identity":request.replay_identity,"raw_data_local":true,"aggregate_only":true,"boundary":PRECLINICAL_BOUNDARY});
    let digest = ContentHash::of_value(&payload)
        .map_err(|e| GovernanceExecutionContractError::Artifact(e.to_string()))?;
    payload["contract_digest"] = json!(digest);
    payload["artifact"] = json!({"artifact_id":format!("governance-execution-contract-8:{}", request.request_id),"content_type":CONTENT_TYPE,"content_hash":digest,"semantic_loss":payload["omission_order"],"provenance_digests":nodes.iter().map(|n|n.provenance_digest.clone()).collect::<BTreeSet<_>>().into_iter().collect::<Vec<_>>(),"boundary":PRECLINICAL_BOUNDARY});
    payload["effect_receipts"] = if disposition == "qualified" {
        json!(["exchange:aggregate-contract", "retain:execution-contract"])
    } else {
        json!(["block:unsafe-release"])
    };
    let output: GovernanceExecutionContract8 = serde_json::from_value(payload)
        .map_err(|e| GovernanceExecutionContractError::Artifact(e.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn h(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn node(id: &str, deps: Vec<String>) -> ExecutionNode5 {
        ExecutionNode5 {
            node_id: id.into(),
            dependency_order: deps,
            executor: "engine".into(),
            input_schema: "in@1".into(),
            output_schema: "out@1".into(),
            artifact_digest: h("artifact"),
            provenance_digest: h("provenance"),
            replay_identity: h("replay"),
            estimated_units: 2,
            evidence_state: ExecutionEvidenceState::Supported,
            deterministic: true,
            local_only: true,
            permitted: true,
            signed: true,
            negative_result: false,
        }
    }
    fn peer() -> ExecutionPeer5 {
        ExecutionPeer5 {
            peer_id: "peer".into(),
            origin: "site".into(),
            workflow_id: "wf".into(),
            semantic_profile: "profile".into(),
            checkpoint: 1,
            contract_digest: h("contract"),
            replay_identity: h("replay"),
            evidence_state: ExecutionEvidenceState::Supported,
            signed: true,
            aggregate_only: true,
            raw_data_local: true,
        }
    }
    fn request() -> ExecutionContractRequest5 {
        ExecutionContractRequest5 {
            schema_version: INPUT_SCHEMA.into(),
            request_id: "req".into(),
            federation_id: "fed".into(),
            workflow_id: "wf".into(),
            requester: "researcher".into(),
            purpose: "dry run".into(),
            semantic_profile: "profile".into(),
            engine_version: "engine@1".into(),
            nodes: vec![node("a", vec![]), node("b", vec!["a".into()])],
            peers: vec![peer()],
            checkpoint: 1,
            max_budget_units: 8,
            minimum_peer_quorum: 1,
            policy_allowed: true,
            protected_closure: true,
            signed_approval: true,
            federation_allowed: true,
            raw_data_local: true,
            aggregate_only: true,
            replay_identity: h("replay"),
            adversarial_event_order: vec![],
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn qualified_contract_is_releasable() {
        let output = model_computational_execution_contract(&request()).unwrap();
        assert_eq!(output.disposition, "qualified");
        assert_eq!(output.planned_node_order, vec!["a", "b"]);
    }
    #[test]
    fn cycles_remain_blocked() {
        let mut input = request();
        input.nodes[0].dependency_order = vec!["b".into()];
        let output = model_computational_execution_contract(&input).unwrap();
        assert_eq!(output.disposition, "unresolved");
        assert!(!output.cycle_order.is_empty());
    }
    #[test]
    fn policy_denial_blocks() {
        let mut input = request();
        input.policy_allowed = false;
        let output = model_computational_execution_contract(&input).unwrap();
        assert_eq!(output.disposition, "blocked");
        assert_eq!(output.effect_receipts, vec!["block:unsafe-release"]);
    }
}

//! Federated workflow-fabric planning (`AFA-ids-P20-F15`).
//!
//! The fabric compiles typed stage and peer attestations into a deterministic
//! plan. It records dependencies, checkpoints, compensation, quorum, budget,
//! and locality gates before any institution-local executor is allowed to act.

use crate::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-ids-P20-F15";
pub const CONTRACT_VERSION: &str = "ids-federated-continual-workflow-fabric/1.0";
pub const INPUT_SCHEMA: &str = "FederatedWorkflowRequest7@1";
pub const OUTPUT_SCHEMA: &str = "FederatedWorkflowReceipt9@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.federated-workflow-receipt-9+json";
pub const PRECLINICAL_BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";
pub const MAX_STAGES: usize = 16_384;
pub const MAX_PEERS: usize = 1_024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowEvidenceState {
    Proven,
    Supported,
    Unknown,
    Unmeasured,
    Contradicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowStage8 {
    pub stage_id: String,
    pub kind: String,
    pub dependency_ids: Vec<String>,
    pub input_digest: ContentHash,
    pub checkpoint_id: String,
    pub compensation_action: String,
    pub estimated_units: u64,
    pub evidence_state: WorkflowEvidenceState,
    pub local: bool,
    pub aggregate_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowPeer7 {
    pub peer_id: String,
    pub workflow_id: String,
    pub semantic_profile: String,
    pub checkpoint: u64,
    pub workflow_digest: ContentHash,
    pub evidence_state: WorkflowEvidenceState,
    pub signed: bool,
    pub local: bool,
    pub aggregate_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedWorkflowRequest7 {
    pub request_id: String,
    pub workflow_id: String,
    pub federation_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub stages: Vec<WorkflowStage8>,
    pub peers: Vec<WorkflowPeer7>,
    pub checkpoint: u64,
    pub minimum_peer_quorum: usize,
    pub max_budget_units: u64,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_approved: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedWorkflowReceipt9Artifact {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedWorkflowReceipt9 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub federation_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub checkpoint: u64,
    pub disposition: String,
    pub stage_order: Vec<String>,
    pub selected_stage_order: Vec<String>,
    pub unresolved_stage_order: Vec<String>,
    pub blocked_stage_order: Vec<String>,
    pub missing_dependency_order: Vec<String>,
    pub cycle_order: Vec<String>,
    pub checkpoint_order: Vec<String>,
    pub compensation_order: Vec<String>,
    pub peer_order: Vec<String>,
    pub qualified_peer_order: Vec<String>,
    pub missing_peer_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub total_units: u64,
    pub budget_remaining: u64,
    pub replay_identity: ContentHash,
    pub workflow_digest: ContentHash,
    pub artifact: FederatedWorkflowReceipt9Artifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedWorkflowError {
    #[error("invalid federated workflow request: {0}")]
    Invalid(String),
    #[error("federated workflow receipt failed validation: {0}")]
    Receipt(String),
}

pub fn federated_workflow_fabric_manifest() -> serde_json::Value {
    json!({
        "schema_version":"aurora-research-contract/1.0", "capability_id":FEATURE_ID, "version":CONTRACT_VERSION, "owner_crate":"ids",
        "consumers":["research workflow compiler","federation operator","laboratory integration steward","replay auditor"],
        "behavior":"compiles typed stage dependencies and aggregate peer attestations into a locality-safe federated workflow plan",
        "value":"prevents cycles, missing checkpoints, budget overflow, peer disagreement, and unsafe effects from becoming executable research workflows",
        "input_schema":INPUT_SCHEMA, "output_schema":OUTPUT_SCHEMA, "effects":["exchange:workflow-summaries","manage:local-capability"],
        "permissions":["read:local-workflow-manifests","request:federated-workflow-plan"], "autonomy_tier":"A2", "boundary":PRECLINICAL_BOUNDARY
    })
}

fn valid_digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().bytes().all(|b| b.is_ascii_hexdigit())
}
fn ordered(values: &[String]) -> bool {
    values.windows(2).all(|w| w[0] < w[1])
}

impl FederatedWorkflowReceipt9 {
    pub fn validate(&self) -> Result<(), FederatedWorkflowError> {
        if self.schema_version != "aurora-research-contract/1.0"
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.checkpoint == 0
            || self.stage_order.is_empty()
            || self.peer_order.is_empty()
            || self.effect_receipts.is_empty()
            || !["qualified", "unresolved", "blocked"].contains(&self.disposition.as_str())
        {
            return Err(FederatedWorkflowError::Receipt("workflow identity, locality, stages, peers, checkpoint, disposition, or effects are incomplete".into()));
        }
        for values in [
            &self.stage_order,
            &self.selected_stage_order,
            &self.unresolved_stage_order,
            &self.blocked_stage_order,
            &self.missing_dependency_order,
            &self.cycle_order,
            &self.checkpoint_order,
            &self.compensation_order,
            &self.peer_order,
            &self.qualified_peer_order,
            &self.missing_peer_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !ordered(values) {
                return Err(FederatedWorkflowError::Receipt(
                    "federated workflow ordering is not canonical".into(),
                ));
            }
        }
        let stages = BTreeSet::from_iter(self.stage_order.iter().cloned());
        let states = self
            .selected_stage_order
            .iter()
            .chain(&self.unresolved_stage_order)
            .chain(&self.blocked_stage_order)
            .cloned()
            .collect::<Vec<_>>();
        let peers = BTreeSet::from_iter(self.peer_order.iter().cloned());
        let peer_states = self
            .qualified_peer_order
            .iter()
            .chain(&self.missing_peer_order)
            .cloned()
            .collect::<Vec<_>>();
        if stages.len() != self.stage_order.len()
            || states.len() != stages.len()
            || BTreeSet::from_iter(states) != stages
            || peers.len() != self.peer_order.len()
            || peer_states.len() != peers.len()
            || BTreeSet::from_iter(peer_states) != peers
        {
            return Err(FederatedWorkflowError::Receipt(
                "workflow stage or peer states do not partition".into(),
            ));
        }
        if !valid_digest(&self.replay_identity)
            || !valid_digest(&self.workflow_digest)
            || self.artifact.content_hash != self.workflow_digest
            || self.artifact.content_type != CONTENT_TYPE
            || self
                .artifact
                .provenance_digests
                .iter()
                .any(|d| !valid_digest(d))
        {
            return Err(FederatedWorkflowError::Receipt(
                "workflow digest or artifact metadata is inconsistent".into(),
            ));
        }
        if self.effect_receipts.iter().any(|e| {
            !e.starts_with("exchange:workflow-summaries:")
                && !e.starts_with("manage:local-capability:")
                && e != "block:unsafe-release"
        }) {
            return Err(FederatedWorkflowError::Receipt(
                "effect is outside governed workflow gate".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(request: &FederatedWorkflowRequest7) -> Result<(), FederatedWorkflowError> {
    if request.request_id.trim().is_empty()
        || request.workflow_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.stages.is_empty()
        || request.stages.len() > MAX_STAGES
        || request.peers.is_empty()
        || request.peers.len() > MAX_PEERS
        || request.checkpoint == 0
        || request.minimum_peer_quorum == 0
        || request.max_budget_units == 0
        || !valid_digest(&request.replay_identity)
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
    {
        return Err(FederatedWorkflowError::Invalid(
            "workflow identity, stages, peers, bounds, replay, or locality is invalid".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for stage in &request.stages {
        if stage.stage_id.trim().is_empty()
            || stage.kind.trim().is_empty()
            || stage.checkpoint_id.trim().is_empty()
            || stage.compensation_action.trim().is_empty()
            || !valid_digest(&stage.input_digest)
            || !ids.insert(stage.stage_id.clone())
        {
            return Err(FederatedWorkflowError::Invalid(
                "stage identity, digest, compensation, or uniqueness is invalid".into(),
            ));
        }
    }
    let mut peer_ids = BTreeSet::new();
    for peer in &request.peers {
        if peer.peer_id.trim().is_empty()
            || peer.workflow_id.trim().is_empty()
            || peer.semantic_profile.trim().is_empty()
            || peer.checkpoint == 0
            || !valid_digest(&peer.workflow_digest)
            || !peer_ids.insert(peer.peer_id.clone())
        {
            return Err(FederatedWorkflowError::Invalid(
                "peer identity, checkpoint, digest, or uniqueness is invalid".into(),
            ));
        }
    }
    Ok(())
}

pub fn compile_federated_workflow(
    request: &FederatedWorkflowRequest7,
) -> Result<FederatedWorkflowReceipt9, FederatedWorkflowError> {
    validate_request(request)?;
    let mut stages = request.stages.clone();
    stages.sort_by(|a, b| a.stage_id.cmp(&b.stage_id));
    let order = stages
        .iter()
        .map(|s| s.stage_id.clone())
        .collect::<Vec<_>>();
    let by_id = stages
        .iter()
        .map(|s| (s.stage_id.clone(), s))
        .collect::<BTreeMap<_, _>>();
    let mut indegree = stages
        .iter()
        .map(|s| {
            (
                s.stage_id.clone(),
                s.dependency_ids
                    .iter()
                    .filter(|d| by_id.contains_key(*d))
                    .count(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut children: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut missing = BTreeSet::new();
    for s in &stages {
        for d in &s.dependency_ids {
            if by_id.contains_key(d) {
                children
                    .entry(d.clone())
                    .or_default()
                    .push(s.stage_id.clone());
            } else {
                missing.insert(format!("{}:{}", s.stage_id, d));
            }
        }
    }
    let mut queue = VecDeque::from_iter(
        indegree
            .iter()
            .filter(|(_, n)| **n == 0)
            .map(|(id, _)| id.clone()),
    );
    let mut topo = Vec::new();
    while let Some(id) = queue.pop_front() {
        topo.push(id.clone());
        if let Some(next) = children.get(&id) {
            for child in next {
                let n = indegree.get_mut(child).expect("child");
                *n -= 1;
                if *n == 0 {
                    queue.push_back(child.clone());
                }
            }
        }
    }
    let cycles = order
        .iter()
        .filter(|id| !topo.contains(id))
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut selected = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut checkpoints = BTreeSet::new();
    let mut compensations = BTreeSet::new();
    let mut total = 0_u64;
    for s in &stages {
        let id = s.stage_id.clone();
        total = total.saturating_add(s.estimated_units);
        checkpoints.insert(s.checkpoint_id.clone());
        compensations.insert(s.compensation_action.clone());
        if cycles.contains(&id) {
            blocked.insert(id.clone());
            omissions.insert(format!("{id}:dependency-cycle"));
        } else if s.dependency_ids.iter().any(|d| !by_id.contains_key(d)) {
            unresolved.insert(id.clone());
        } else if !s.local || !s.aggregate_only {
            blocked.insert(id.clone());
            omissions.insert(format!("{id}:raw-data-locality"));
        } else if s.evidence_state == WorkflowEvidenceState::Contradicted {
            blocked.insert(id.clone());
            negative.insert(format!("{id}:contradicted"));
        } else if !matches!(
            s.evidence_state,
            WorkflowEvidenceState::Proven | WorkflowEvidenceState::Supported
        ) {
            unresolved.insert(id.clone());
            uncertainty.insert(format!("{id}:evidence-state"));
        } else {
            selected.insert(id);
        }
    }
    let mut peers = request.peers.clone();
    peers.sort_by(|a, b| a.peer_id.cmp(&b.peer_id));
    let peer_order = peers.iter().map(|p| p.peer_id.clone()).collect::<Vec<_>>();
    let qualified = peers
        .iter()
        .filter(|p| {
            p.workflow_id == request.workflow_id
                && p.semantic_profile == request.semantic_profile
                && p.checkpoint == request.checkpoint
                && p.signed
                && p.local
                && p.aggregate_only
                && p.workflow_digest == request.replay_identity
                && matches!(
                    p.evidence_state,
                    WorkflowEvidenceState::Proven | WorkflowEvidenceState::Supported
                )
        })
        .map(|p| p.peer_id.clone())
        .collect::<BTreeSet<_>>();
    let missing_peers = peer_order
        .iter()
        .filter(|p| !qualified.contains(*p))
        .cloned()
        .collect::<BTreeSet<_>>();
    if qualified.len() < request.minimum_peer_quorum {
        uncertainty.insert("peer:minimum-quorum-unmet".into());
    }
    if total > request.max_budget_units {
        omissions.insert(format!("request:budget-exceeded:{total}"));
    }
    let global = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.federation_approved
        || !request.raw_data_local
        || !request.aggregate_only;
    if global {
        blocked.extend(order.iter().cloned());
        selected.clear();
        unresolved.clear();
        omissions.insert("request:governance-or-locality-denied".into());
    }
    let so = selected.iter().cloned().collect::<Vec<_>>();
    let uo = unresolved.iter().cloned().collect::<Vec<_>>();
    let bo = blocked.iter().cloned().collect::<Vec<_>>();
    let qpo = qualified.iter().cloned().collect::<Vec<_>>();
    let mpo = missing_peers.iter().cloned().collect::<Vec<_>>();
    let disposition = if global || so.is_empty() && uo.is_empty() {
        "blocked"
    } else if !uo.is_empty()
        || !bo.is_empty()
        || qpo.len() < request.minimum_peer_quorum
        || total > request.max_budget_units
    {
        "unresolved"
    } else {
        "qualified"
    };
    if disposition != "qualified" {
        omissions.insert("request:workflow-plan-not-closed".into());
    }
    let mut payload = json!({"schema_version":"aurora-research-contract/1.0","contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"workflow_id":request.workflow_id,"federation_id":request.federation_id,"purpose":request.purpose,"semantic_profile":request.semantic_profile,"checkpoint":request.checkpoint,"disposition":disposition,"stage_order":order,"selected_stage_order":so,"unresolved_stage_order":uo,"blocked_stage_order":bo,"missing_dependency_order":missing.iter().cloned().collect::<Vec<_>>(),"cycle_order":cycles.iter().cloned().collect::<Vec<_>>(),"checkpoint_order":checkpoints.iter().cloned().collect::<Vec<_>>(),"compensation_order":compensations.iter().cloned().collect::<Vec<_>>(),"peer_order":peer_order,"qualified_peer_order":qpo,"missing_peer_order":mpo,"omission_order":omissions.iter().cloned().collect::<Vec<_>>(),"uncertainty_order":uncertainty.iter().cloned().collect::<Vec<_>>(),"negative_evidence_order":negative.iter().cloned().collect::<Vec<_>>(),"total_units":total,"budget_remaining":request.max_budget_units.saturating_sub(total),"replay_identity":request.replay_identity,"raw_data_local":true,"aggregate_only":true,"boundary":PRECLINICAL_BOUNDARY});
    let wd = ContentHash::of_value(&payload)
        .map_err(|e| FederatedWorkflowError::Receipt(e.to_string()))?;
    payload["workflow_digest"] = json!(wd);
    payload["artifact"] = json!({"artifact_id":format!("federated-workflow-receipt-9:{}",request.workflow_id),"content_type":CONTENT_TYPE,"content_hash":wd,"semantic_loss":omissions.iter().cloned().collect::<Vec<_>>(),"provenance_digests":stages.iter().map(|s|s.input_digest.clone()).collect::<BTreeSet<_>>().into_iter().collect::<Vec<_>>(),"boundary":PRECLINICAL_BOUNDARY});
    payload["effect_receipts"] = json!(if disposition == "qualified" {
        vec![
            format!("exchange:workflow-summaries:{}", request.request_id),
            format!("manage:local-capability:{}", request.request_id),
        ]
    } else {
        vec!["block:unsafe-release".to_string()]
    });
    let receipt: FederatedWorkflowReceipt9 = serde_json::from_value(payload)
        .map_err(|e| FederatedWorkflowError::Receipt(e.to_string()))?;
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn h(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn stage(id: &str, deps: Vec<&str>) -> WorkflowStage8 {
        WorkflowStage8 {
            stage_id: id.into(),
            kind: "compute".into(),
            dependency_ids: deps.into_iter().map(str::to_string).collect(),
            input_digest: h(id),
            checkpoint_id: format!("cp-{id}"),
            compensation_action: format!("undo-{id}"),
            estimated_units: 1,
            evidence_state: WorkflowEvidenceState::Supported,
            local: true,
            aggregate_only: true,
        }
    }
    fn req(stages: Vec<WorkflowStage8>) -> FederatedWorkflowRequest7 {
        FederatedWorkflowRequest7 {
            request_id: "wf:req".into(),
            workflow_id: "wf:1".into(),
            federation_id: "fed:1".into(),
            purpose: "research".into(),
            semantic_profile: "ome".into(),
            stages,
            peers: vec![WorkflowPeer7 {
                peer_id: "p1".into(),
                workflow_id: "wf:1".into(),
                semantic_profile: "ome".into(),
                checkpoint: 1,
                workflow_digest: h("wf:1"),
                evidence_state: WorkflowEvidenceState::Supported,
                signed: true,
                local: true,
                aggregate_only: true,
            }],
            checkpoint: 1,
            minimum_peer_quorum: 1,
            max_budget_units: 10,
            replay_identity: h("wf:1"),
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_approved: true,
            raw_data_local: true,
            aggregate_only: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a2() {
        assert_eq!(federated_workflow_fabric_manifest()["autonomy_tier"], "A2");
    }
    #[test]
    fn nominal_is_qualified() {
        let r = compile_federated_workflow(&req(vec![stage("a", vec![]), stage("b", vec!["a"])]))
            .unwrap();
        assert_eq!(r.disposition, "qualified");
    }
    #[test]
    fn missing_dependency_is_unresolved() {
        let r = compile_federated_workflow(&req(vec![stage("a", vec!["missing"])])).unwrap();
        assert_eq!(r.disposition, "unresolved");
        assert!(!r.missing_dependency_order.is_empty());
    }
    #[test]
    fn cycle_is_blocked() {
        let r =
            compile_federated_workflow(&req(vec![stage("a", vec!["b"]), stage("b", vec!["a"])]))
                .unwrap();
        assert_eq!(r.disposition, "blocked");
    }
    #[test]
    fn policy_denial_blocks() {
        let mut q = req(vec![stage("a", vec![])]);
        q.policy_allow = false;
        let r = compile_federated_workflow(&q).unwrap();
        assert_eq!(r.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn unknown_evidence_is_unresolved() {
        let mut s = stage("a", vec![]);
        s.evidence_state = WorkflowEvidenceState::Unknown;
        let r = compile_federated_workflow(&req(vec![s])).unwrap();
        assert_eq!(r.disposition, "unresolved");
    }
    #[test]
    fn stage_order_is_canonical() {
        let r =
            compile_federated_workflow(&req(vec![stage("z", vec![]), stage("a", vec![])])).unwrap();
        assert_eq!(r.stage_order, vec!["a", "z"]);
    }
}

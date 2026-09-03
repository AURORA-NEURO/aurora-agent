//! Federated continual computational-execution workbench (`AFA-ids-P12-F20`).
//!
//! The workbench compiles workflow node attestations into a deterministic dry-run plan. It
//! does not launch jobs, access external networks, move raw research data, or make clinical
//! decisions; execution is delegated to separately authorized institutional gateways.

use crate::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-ids-P12-F20";
pub const CONTRACT_VERSION: &str =
    "ids-federated-continual-computational-execution-research-workbench/1.0";
pub const INPUT_SCHEMA: &str = "ComputationalExecutionRequest6@1";
pub const OUTPUT_SCHEMA: &str = "ComputationalExecutionReport9@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.computational-execution-report-9+json";
pub const PRECLINICAL_BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";
pub const MAX_NODES: usize = 4096;
pub const MAX_PEERS: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionEvidenceState {
    Proven,
    Supported,
    Unknown,
    Unmeasured,
    Contradicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationNode6 {
    pub node_id: String,
    pub dependency_ids: Vec<String>,
    pub executor_id: String,
    pub input_schema: String,
    pub output_schema: String,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub estimated_units: u64,
    pub evidence_state: ExecutionEvidenceState,
    pub deterministic: bool,
    pub local_only: bool,
    pub permitted: bool,
    pub signed: bool,
    pub replay_identity: ContentHash,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationPeer6 {
    pub peer_id: String,
    pub origin: String,
    pub workflow_id: String,
    pub semantic_profile: String,
    pub checkpoint: u64,
    pub execution_digest: ContentHash,
    pub evidence_state: ExecutionEvidenceState,
    pub signed: bool,
    pub aggregate_only: bool,
    pub raw_data_local: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationalExecutionRequest6 {
    pub request_id: String,
    pub federation_id: String,
    pub workflow_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub engine_version: String,
    pub nodes: Vec<ComputationNode6>,
    pub peers: Vec<ComputationPeer6>,
    pub checkpoint: u64,
    pub max_budget_units: u64,
    pub minimum_peer_quorum: usize,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_approved: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub replay_identity: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationalExecutionArtifact9 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationalExecutionReport9 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub workflow_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
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
    pub retry_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub total_units: u64,
    pub replay_identity: ContentHash,
    pub execution_digest: ContentHash,
    pub artifact: ComputationalExecutionArtifact9,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ComputationalExecutionError {
    #[error("invalid computational execution request: {0}")]
    Invalid(String),
    #[error("computational execution artifact failed: {0}")]
    Artifact(String),
}

pub fn computational_execution_manifest() -> serde_json::Value {
    json!({"schema_version":"aurora-research-contract/1.0","capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"ids","consumers":["computational biologist","workflow compiler engineer","federation steward"],"behavior":"compiles federated workflow node attestations into a deterministic dry-run execution plan","value":"exposes dependency, cycle, budget, provenance, replay, peer, and policy gates before any job dispatch","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["exchange:permitted-summaries","manage:local-capability"],"permissions":["read:local-workflow-manifests"],"autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY})
}

impl ComputationalExecutionReport9 {
    pub fn validate(&self) -> Result<(), ComputationalExecutionError> {
        if self.schema_version != "aurora-research-contract/1.0"
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || !all_nonempty([
                &self.request_id,
                &self.federation_id,
                &self.workflow_id,
                &self.requester,
                &self.purpose,
                &self.semantic_profile,
            ])
            || self.checkpoint == 0
            || self.node_order.is_empty()
            || self.peer_order.is_empty()
            || self.effect_receipts.is_empty()
            || !["qualified", "unresolved", "blocked"].contains(&self.disposition.as_str())
        {
            return Err(ComputationalExecutionError::Invalid(
                "identity, checkpoint, locality, nodes, peers, or effects are incomplete".into(),
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
            &self.retry_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|w| w[0] >= w[1]) {
                return Err(ComputationalExecutionError::Invalid(
                    "computational execution ordering is not canonical".into(),
                ));
            }
        }
        let nodes = BTreeSet::from_iter(self.node_order.iter().cloned());
        let parts = self
            .planned_node_order
            .iter()
            .chain(&self.unresolved_node_order)
            .chain(&self.blocked_node_order)
            .cloned()
            .collect::<BTreeSet<_>>();
        if nodes != parts || nodes.len() != self.node_order.len() {
            return Err(ComputationalExecutionError::Invalid(
                "node states do not partition".into(),
            ));
        }
        let peers = BTreeSet::from_iter(self.peer_order.iter().cloned());
        let pp = self
            .qualified_peer_order
            .iter()
            .chain(&self.missing_peer_order)
            .cloned()
            .collect::<BTreeSet<_>>();
        if peers != pp || peers.len() != self.peer_order.len() {
            return Err(ComputationalExecutionError::Invalid(
                "peer states do not partition".into(),
            ));
        }
        if self.artifact.content_type != CONTENT_TYPE
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.content_hash != self.execution_digest
        {
            return Err(ComputationalExecutionError::Artifact(
                "artifact metadata or digest is inconsistent".into(),
            ));
        }
        if self.effect_receipts.iter().any(|e| {
            !e.starts_with("exchange:permitted-summaries:")
                && !e.starts_with("manage:local-capability:")
                && e != "block:unsafe-release"
        }) {
            return Err(ComputationalExecutionError::Invalid(
                "effect is outside execution gate".into(),
            ));
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<ContentHash, ComputationalExecutionError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|e| ComputationalExecutionError::Artifact(e.to_string()))?,
        )
        .map_err(|e| ComputationalExecutionError::Artifact(e.to_string()))
    }
}
fn all_nonempty<const N: usize>(values: [&String; N]) -> bool {
    values.iter().all(|v| !v.trim().is_empty())
}

pub fn compile_computational_execution(
    request: &ComputationalExecutionRequest6,
) -> Result<ComputationalExecutionReport9, ComputationalExecutionError> {
    validate_request(request)?;
    let mut nodes = request.nodes.clone();
    nodes.sort_by(|a, b| a.node_id.cmp(&b.node_id));
    let node_order = nodes.iter().map(|x| x.node_id.clone()).collect::<Vec<_>>();
    let mut peers = request.peers.clone();
    peers.sort_by(|a, b| a.peer_id.cmp(&b.peer_id));
    let peer_order = peers.iter().map(|x| x.peer_id.clone()).collect::<Vec<_>>();
    let by_id = nodes
        .iter()
        .map(|n| (n.node_id.clone(), n))
        .collect::<BTreeMap<_, _>>();
    let mut missing = BTreeSet::new();
    for n in &nodes {
        for dep in &n.dependency_ids {
            if !by_id.contains_key(dep) {
                missing.insert(format!("{}:missing:{}", n.node_id, dep));
            }
        }
    }
    let mut indegree = nodes
        .iter()
        .map(|n| {
            (
                n.node_id.clone(),
                n.dependency_ids
                    .iter()
                    .filter(|d| by_id.contains_key(*d))
                    .count(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut ready = indegree
        .iter()
        .filter(|(_, v)| **v == 0)
        .map(|(k, _)| k.clone())
        .collect::<BTreeSet<_>>();
    let mut plan = Vec::new();
    while let Some(id) = ready.pop_first() {
        plan.push(id.clone());
        for n in &nodes {
            if n.dependency_ids.contains(&id) {
                let d = indegree.get_mut(&n.node_id).unwrap();
                *d -= 1;
                if *d == 0 {
                    ready.insert(n.node_id.clone());
                }
            }
        }
    }
    let cycles = indegree
        .iter()
        .filter(|(_, v)| **v > 0)
        .map(|(k, _)| k.clone())
        .collect::<BTreeSet<_>>();
    let mut planned = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut retry = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut total = 0u64;
    for n in &nodes {
        total = total.saturating_add(n.estimated_units);
        if n.negative_result {
            negative.insert(format!("{}:negative-result", n.node_id));
        }
        let hard = n.replay_identity != request.replay_identity
            || !n.local_only
            || !n.permitted
            || !n.signed
            || !n.deterministic
            || n.evidence_state == ExecutionEvidenceState::Contradicted
            || cycles.contains(&n.node_id);
        if hard {
            if n.evidence_state == ExecutionEvidenceState::Contradicted
                || cycles.contains(&n.node_id)
                || !n.local_only
            {
                blocked.insert(n.node_id.clone());
            } else {
                unresolved.insert(n.node_id.clone());
                retry.insert(format!("{}:replay-or-authorization", n.node_id));
            }
            if n.evidence_state == ExecutionEvidenceState::Contradicted {
                negative.insert(format!("{}:contradicted", n.node_id));
            }
        } else if !matches!(
            n.evidence_state,
            ExecutionEvidenceState::Proven | ExecutionEvidenceState::Supported
        ) {
            unresolved.insert(n.node_id.clone());
            uncertainty.insert(format!("{}:evidence-state", n.node_id));
        } else if n
            .dependency_ids
            .iter()
            .any(|d| !by_id.contains_key(d) || cycles.contains(d))
        {
            blocked.insert(n.node_id.clone());
        } else {
            planned.insert(n.node_id.clone());
        }
    }
    let mut qp = BTreeSet::new();
    let mut mp = BTreeSet::new();
    for p in &peers {
        let ok = p.workflow_id == request.workflow_id
            && p.semantic_profile == request.semantic_profile
            && p.checkpoint == request.checkpoint
            && p.signed
            && p.aggregate_only
            && p.raw_data_local
            && matches!(
                p.evidence_state,
                ExecutionEvidenceState::Proven | ExecutionEvidenceState::Supported
            );
        if ok {
            qp.insert(p.peer_id.clone());
        } else {
            mp.insert(p.peer_id.clone());
            uncertainty.insert(format!("peer:{}:not-qualified", p.peer_id));
        }
    }
    if !missing.is_empty() {
        omissions.insert("request:missing-dependency-closure".into());
    }
    if !cycles.is_empty() {
        negative.insert("request:cycle-detected".into());
    }
    if total > request.max_budget_units {
        omissions.insert(format!("request:budget-exceeded:{}", total));
    }
    if qp.len() < request.minimum_peer_quorum {
        uncertainty.insert("peer:minimum-quorum-unmet".into());
    }
    let global = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.federation_approved
        || !request.raw_data_local
        || !request.aggregate_only;
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
    let disposition = if global || !blocked.is_empty() {
        "blocked"
    } else if !cycles.is_empty()
        || !missing.is_empty()
        || !unresolved.is_empty()
        || total > request.max_budget_units
        || qp.len() < request.minimum_peer_quorum
        || planned.is_empty()
    {
        "unresolved"
    } else {
        "qualified"
    };
    if global {
        blocked.extend(node_order.iter().cloned());
        planned.clear();
        unresolved.clear();
    }
    if disposition != "qualified" {
        omissions.insert("request:execution-not-release-ready".into());
    }
    let payload = json!({"schema_version":"aurora-research-contract/1.0","contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"federation_id":request.federation_id,"workflow_id":request.workflow_id,"requester":request.requester,"purpose":request.purpose,"semantic_profile":request.semantic_profile,"checkpoint":request.checkpoint,"disposition":disposition,"node_order":node_order,"planned_node_order":planned,"unresolved_node_order":unresolved,"blocked_node_order":blocked,"cycle_order":cycles,"missing_dependency_order":missing,"peer_order":peer_order,"qualified_peer_order":qp,"missing_peer_order":mp,"retry_order":retry,"omission_order":omissions,"uncertainty_order":uncertainty,"negative_evidence_order":negative,"total_units":total,"replay_identity":request.replay_identity,"boundary":PRECLINICAL_BOUNDARY});
    let digest = ContentHash::of_value(&payload)
        .map_err(|e| ComputationalExecutionError::Artifact(e.to_string()))?;
    let artifact = ComputationalExecutionArtifact9 {
        artifact_id: format!("computational-execution-report-9:{}", request.request_id),
        content_type: CONTENT_TYPE.into(),
        content_hash: digest.clone(),
        semantic_loss: Vec::new(),
        provenance_digests: nodes
            .iter()
            .map(|x| x.provenance_digest.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let effects = if disposition == "qualified" {
        vec![
            format!("exchange:permitted-summaries:{}", request.request_id),
            format!("manage:local-capability:{}", request.request_id),
        ]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let receipt = ComputationalExecutionReport9 {
        schema_version: "aurora-research-contract/1.0".into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        workflow_id: request.workflow_id.clone(),
        requester: request.requester.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        checkpoint: request.checkpoint,
        disposition: disposition.into(),
        node_order: payload["node_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        planned_node_order: payload["planned_node_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        unresolved_node_order: payload["unresolved_node_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        blocked_node_order: payload["blocked_node_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        cycle_order: payload["cycle_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        missing_dependency_order: payload["missing_dependency_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        peer_order: payload["peer_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        qualified_peer_order: payload["qualified_peer_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        missing_peer_order: payload["missing_peer_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        retry_order: payload["retry_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        omission_order: payload["omission_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        uncertainty_order: payload["uncertainty_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        negative_evidence_order: payload["negative_evidence_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        total_units: total,
        replay_identity: request.replay_identity.clone(),
        execution_digest: digest,
        artifact,
        effect_receipts: effects,
        raw_data_local: true,
        aggregate_only: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}
fn validate_request(r: &ComputationalExecutionRequest6) -> Result<(), ComputationalExecutionError> {
    if !all_nonempty([
        &r.request_id,
        &r.federation_id,
        &r.workflow_id,
        &r.requester,
        &r.purpose,
        &r.semantic_profile,
        &r.engine_version,
    ]) || r.nodes.is_empty()
        || r.nodes.len() > MAX_NODES
        || r.peers.is_empty()
        || r.peers.len() > MAX_PEERS
        || r.checkpoint == 0
        || r.max_budget_units == 0
        || r.minimum_peer_quorum == 0
        || r.replay_identity.as_str().len() != 64
        || r.boundary != PRECLINICAL_BOUNDARY
        || !r.raw_data_local
        || !r.aggregate_only
    {
        return Err(ComputationalExecutionError::Invalid("request identity, bounds, nodes, peers, budget, replay, locality, or boundary is invalid".into()));
    }
    let mut ids = BTreeSet::new();
    for n in &r.nodes {
        if n.node_id.trim().is_empty()
            || !ids.insert(n.node_id.clone())
            || n.executor_id.trim().is_empty()
            || n.input_schema.trim().is_empty()
            || n.output_schema.trim().is_empty()
            || n.estimated_units == 0
            || n.artifact_digest.as_str().len() != 64
            || n.provenance_digest.as_str().len() != 64
            || n.replay_identity.as_str().len() != 64
        {
            return Err(ComputationalExecutionError::Invalid(
                "node identity, executor, schemas, bounds, or digest is invalid".into(),
            ));
        }
    }
    let mut ids = BTreeSet::new();
    for p in &r.peers {
        if p.peer_id.trim().is_empty()
            || !ids.insert(p.peer_id.clone())
            || p.origin.trim().is_empty()
            || p.workflow_id.trim().is_empty()
            || p.execution_digest.as_str().len() != 64
        {
            return Err(ComputationalExecutionError::Invalid(
                "peer identity, uniqueness, origin, workflow, or digest is invalid".into(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn h(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn req() -> ComputationalExecutionRequest6 {
        let n = ComputationNode6 {
            node_id: "node:a".into(),
            dependency_ids: Vec::new(),
            executor_id: "executor:a".into(),
            input_schema: "Input@1".into(),
            output_schema: "Output@1".into(),
            artifact_digest: h("a"),
            provenance_digest: h("p"),
            estimated_units: 5,
            evidence_state: ExecutionEvidenceState::Supported,
            deterministic: true,
            local_only: true,
            permitted: true,
            signed: true,
            replay_identity: h("r"),
            negative_result: false,
        };
        let p = ComputationPeer6 {
            peer_id: "peer:a".into(),
            origin: "site:a".into(),
            workflow_id: "workflow:1".into(),
            semantic_profile: "neuro:v1".into(),
            checkpoint: 2,
            execution_digest: h("e"),
            evidence_state: ExecutionEvidenceState::Supported,
            signed: true,
            aggregate_only: true,
            raw_data_local: true,
        };
        ComputationalExecutionRequest6 {
            request_id: "request:compute".into(),
            federation_id: "federation:compute".into(),
            workflow_id: "workflow:1".into(),
            requester: "computational-biologist".into(),
            purpose: "federated-dry-run".into(),
            semantic_profile: "neuro:v1".into(),
            engine_version: "1.0".into(),
            nodes: vec![n],
            peers: vec![p],
            checkpoint: 2,
            max_budget_units: 20,
            minimum_peer_quorum: 1,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_approved: true,
            raw_data_local: true,
            aggregate_only: true,
            replay_identity: h("r"),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(computational_execution_manifest()["autonomy_tier"], "A1");
    }
    #[test]
    fn nominal_is_qualified() {
        let r = compile_computational_execution(&req()).unwrap();
        assert_eq!(r.disposition, "qualified");
        assert_eq!(r.digest().unwrap(), r.digest().unwrap());
    }
    #[test]
    fn cycle_is_blocked() {
        let mut r = req();
        r.nodes[0].dependency_ids.push("node:a".into());
        assert_eq!(
            compile_computational_execution(&r).unwrap().disposition,
            "blocked"
        );
    }
    #[test]
    fn missing_dependency_is_blocked() {
        let mut r = req();
        r.nodes[0].dependency_ids.push("node:missing".into());
        assert_eq!(
            compile_computational_execution(&r).unwrap().disposition,
            "blocked"
        );
    }
    #[test]
    fn unknown_is_unresolved() {
        let mut r = req();
        r.nodes[0].evidence_state = ExecutionEvidenceState::Unknown;
        assert_eq!(
            compile_computational_execution(&r).unwrap().disposition,
            "unresolved"
        );
    }
    #[test]
    fn budget_is_unresolved() {
        let mut r = req();
        r.max_budget_units = 1;
        assert_eq!(
            compile_computational_execution(&r).unwrap().disposition,
            "unresolved"
        );
    }
}

//! Local computational-execution contract model (`AFA-onco-P12-F05`).
//!
//! A typed, deterministic representation of a preclinical workflow graph. It validates identity,
//! dependency closure, replay/provenance, and local-only boundaries; it never dispatches a job,
//! instrument, or clinical action.

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-onco-P12-F05";
pub const CONTRACT_VERSION: &str =
    "onco-local-single-study-computational-execution-contract-model/1.0";
pub const INPUT_SCHEMA: &str = "ResearchWorkflowSpec1@1";
pub const OUTPUT_SCHEMA: &str = "ExecutionRun2@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.onco-execution-run-2+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionNode1 {
    pub node_id: String,
    pub depends_on: Vec<String>,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub deterministic: bool,
    pub local_only: bool,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchWorkflowSpec1 {
    pub schema_version: String,
    pub request_id: String,
    pub consumer: String,
    pub purpose: String,
    pub scope: String,
    pub replay_identity: ContentHash,
    pub policy_allowed: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
    pub nodes: Vec<ExecutionNode1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionRunArtifact2 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionRun2 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub consumer: String,
    pub purpose: String,
    pub scope: String,
    pub disposition: String,
    pub node_order: Vec<String>,
    pub valid_order: Vec<String>,
    pub invalid_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub run_digest: ContentHash,
    pub artifact: ExecutionRunArtifact2,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ComputationalExecutionContractError {
    #[error("invalid computational-execution contract request or receipt: {0}")]
    Invalid(String),
    #[error("computational-execution contract artifact failed: {0}")]
    Artifact(String),
}
fn ordered(v: &[String]) -> bool {
    v.windows(2).all(|w| w[0] < w[1])
}
fn digest(v: &ContentHash) -> bool {
    v.as_str().len() == 64 && v.as_str().bytes().all(|b| b.is_ascii_hexdigit())
}

pub fn computational_execution_contract_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "onco".into(), consumers: ["benchmark curator".into(), "workflow schema steward".into(), "replay auditor".into()].into(), behavior: "validate and canonicalize local preclinical research execution graphs into replayable run contracts without dispatching work".into(), value: "gives benchmark curators a stable typed graph and replay identity while keeping execution authority outside the domain model".into(), inputs: vec![TypedPort { name: "research_workflow_spec".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "execution_run".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: BTreeSet::new(), permissions: ["read:local-research-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "ro-crate-1.3".into(), state: EvidenceState::Supported, locator: Some("https://www.researchobject.org/ro-crate/specification.html".into()) }, EvidenceReference { source_id: "cwl".into(), state: EvidenceState::Supported, locator: Some("https://www.commonwl.org/specification/".into()) }, EvidenceReference { source_id: "ga4gh-wes".into(), state: EvidenceState::Supported, locator: Some("https://ga4gh.github.io/workflow-execution-service-schemas/docs/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A0, surfaces: [ResearchSurface::Ui, ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Protocol, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

fn validate_request(r: &ResearchWorkflowSpec1) -> Result<(), ComputationalExecutionContractError> {
    if r.schema_version != INPUT_SCHEMA
        || [&r.request_id, &r.consumer, &r.purpose, &r.scope]
            .iter()
            .any(|v| v.trim().is_empty())
        || !digest(&r.replay_identity)
        || r.boundary != PRECLINICAL_BOUNDARY
        || r.nodes.is_empty()
    {
        return Err(ComputationalExecutionContractError::Invalid(
            "workflow identity, replay, boundary, or node closure is invalid".into(),
        ));
    }
    let all_ids = r
        .nodes
        .iter()
        .map(|node| node.node_id.clone())
        .collect::<BTreeSet<_>>();
    let mut ids = BTreeSet::new();
    for n in &r.nodes {
        if n.node_id.trim().is_empty()
            || !ids.insert(n.node_id.clone())
            || !ordered(&n.depends_on)
            || n.depends_on.iter().any(|d| !all_ids.contains(d))
            || !digest(&n.artifact_digest)
            || !digest(&n.provenance_digest)
            || n.replay_identity != r.replay_identity
        {
            return Err(ComputationalExecutionContractError::Invalid(format!(
                "node {} identity, dependency, digest, or replay is invalid",
                n.node_id
            )));
        }
    }
    Ok(())
}
fn acyclic(nodes: &[ExecutionNode1]) -> bool {
    let ids = nodes
        .iter()
        .map(|n| n.node_id.clone())
        .collect::<BTreeSet<_>>();
    let mut indegree = ids
        .iter()
        .map(|id| (id.clone(), 0usize))
        .collect::<BTreeMap<_, _>>();
    let mut edges: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for n in nodes {
        for d in &n.depends_on {
            *indegree.get_mut(&n.node_id).unwrap() += 1;
            edges
                .entry(d.clone())
                .or_default()
                .insert(n.node_id.clone());
        }
    }
    let mut ready = indegree
        .iter()
        .filter(|(_, x)| **x == 0)
        .map(|(id, _)| id.clone())
        .collect::<BTreeSet<_>>();
    let mut seen = 0;
    while let Some(id) = ready.iter().next().cloned() {
        ready.remove(&id);
        seen += 1;
        if let Some(children) = edges.get(&id) {
            for child in children {
                let n = indegree.get_mut(child).unwrap();
                *n -= 1;
                if *n == 0 {
                    ready.insert(child.clone());
                }
            }
        }
    }
    seen == ids.len()
}

impl ExecutionRun2 {
    pub fn validate(&self) -> Result<(), ComputationalExecutionContractError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.content_type != CONTENT_TYPE
            || !self.raw_data_local
            || !self.aggregate_only
            || !matches!(
                self.disposition.as_str(),
                "qualified" | "partial" | "blocked"
            )
            || self.node_order.is_empty()
        {
            return Err(ComputationalExecutionContractError::Invalid(
                "execution identity, locality, disposition, or node closure is incomplete".into(),
            ));
        }
        for v in [
            &self.node_order,
            &self.valid_order,
            &self.invalid_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
        ] {
            if !ordered(v) {
                return Err(ComputationalExecutionContractError::Invalid(
                    "execution ordering is not canonical".into(),
                ));
            }
        }
        let ids = self.node_order.iter().cloned().collect::<BTreeSet<_>>();
        let states = self
            .valid_order
            .iter()
            .chain(&self.invalid_order)
            .cloned()
            .collect::<Vec<_>>();
        if ids.len() != self.node_order.len()
            || states.len() != ids.len()
            || states.iter().cloned().collect::<BTreeSet<_>>() != ids
        {
            return Err(ComputationalExecutionContractError::Invalid(
                "execution node states do not partition".into(),
            ));
        }
        if !digest(&self.replay_identity)
            || !digest(&self.run_digest)
            || self.artifact.content_hash != self.run_digest
            || !self.artifact.provenance_digests.iter().all(digest)
        {
            return Err(ComputationalExecutionContractError::Artifact(
                "execution digest is inconsistent".into(),
            ));
        }
        Ok(())
    }
}

pub fn model_computational_execution_contract(
    r: &ResearchWorkflowSpec1,
) -> Result<ExecutionRun2, ComputationalExecutionContractError> {
    validate_request(r)?;
    let node_order = r
        .nodes
        .iter()
        .map(|n| n.node_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut valid = BTreeSet::new();
    let mut invalid = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let uncertainty: BTreeSet<String> = BTreeSet::new();
    let mut negative: BTreeSet<String> = BTreeSet::new();
    let provenance = r
        .nodes
        .iter()
        .map(|n| n.provenance_digest.clone())
        .collect::<BTreeSet<_>>();
    let cycle = !acyclic(&r.nodes);
    if cycle {
        invalid.extend(node_order.iter().cloned());
        omissions.insert("workflow:dependency-cycle".into());
    }
    for n in &r.nodes {
        if cycle {
            continue;
        } else if !n.local_only || !n.deterministic || n.replay_identity != r.replay_identity {
            invalid.insert(n.node_id.clone());
            omissions.insert(format!("{}:local-determinism-or-replay", n.node_id));
        } else {
            valid.insert(n.node_id.clone());
        }
    }
    if !r.policy_allowed {
        omissions.insert("workflow:policy-denied".into());
    }
    if !r.protected_closure {
        omissions.insert("workflow:protected-closure-incomplete".into());
    }
    if !r.raw_data_local {
        omissions.insert("workflow:raw-data-not-local".into());
    }
    if !r.aggregate_only {
        omissions.insert("workflow:aggregate-only-required".into());
    }
    if r.nodes.iter().any(|n| n.required && !n.deterministic) {
        negative.insert("workflow:nondeterministic-required-node".into());
    }
    let global_block =
        !r.policy_allowed || !r.protected_closure || !r.raw_data_local || !r.aggregate_only;
    let disposition = if global_block || !invalid.is_empty() {
        "blocked"
    } else if valid.is_empty() {
        "partial"
    } else {
        "qualified"
    };
    if global_block {
        invalid.extend(node_order.iter().cloned());
        valid.clear();
    }
    let payload = json!({"node_order":node_order,"valid_order":valid,"invalid_order":invalid,"omission_order":omissions,"uncertainty_order":uncertainty,"negative_evidence_order":negative,"replay_identity":r.replay_identity});
    let run_digest = ContentHash::of_value(&payload)
        .map_err(|e| ComputationalExecutionContractError::Artifact(e.to_string()))?;
    let strings = |k: &str| {
        payload[k]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    };
    let out = ExecutionRun2 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: r.request_id.clone(),
        consumer: r.consumer.clone(),
        purpose: r.purpose.clone(),
        scope: r.scope.clone(),
        disposition: disposition.into(),
        node_order: strings("node_order"),
        valid_order: strings("valid_order"),
        invalid_order: strings("invalid_order"),
        omission_order: strings("omission_order"),
        uncertainty_order: strings("uncertainty_order"),
        negative_evidence_order: strings("negative_evidence_order"),
        replay_identity: r.replay_identity.clone(),
        run_digest: run_digest.clone(),
        artifact: ExecutionRunArtifact2 {
            artifact_id: format!("onco-execution-run:{}", r.request_id),
            content_type: CONTENT_TYPE.into(),
            content_hash: run_digest,
            semantic_loss: if disposition == "qualified" {
                Vec::new()
            } else {
                vec!["execution-not-dispatched".into()]
            },
            provenance_digests: provenance.into_iter().collect(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        },
        raw_data_local: true,
        aggregate_only: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    out.validate()?;
    Ok(out)
}
pub fn model_computational_execution_contract_json(
    v: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let r: ResearchWorkflowSpec1 = serde_json::from_value(v.clone())
        .map_err(|e| format!("invalid execution contract request: {e}"))?;
    serde_json::to_value(model_computational_execution_contract(&r).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}
pub fn validate_computational_execution_contract_json(
    v: &serde_json::Value,
) -> Result<ExecutionRun2, String> {
    let out: ExecutionRun2 = serde_json::from_value(v.clone())
        .map_err(|e| format!("invalid execution contract receipt: {e}"))?;
    out.validate().map_err(|e| e.to_string())?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn h(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn req() -> ResearchWorkflowSpec1 {
        ResearchWorkflowSpec1 {
            schema_version: INPUT_SCHEMA.into(),
            request_id: "run-1".into(),
            consumer: "curator".into(),
            purpose: "benchmark graph".into(),
            scope: "organoid".into(),
            replay_identity: h("r"),
            policy_allowed: true,
            protected_closure: true,
            raw_data_local: true,
            aggregate_only: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
            nodes: vec![
                ExecutionNode1 {
                    node_id: "a".into(),
                    depends_on: vec![],
                    artifact_digest: h("a"),
                    provenance_digest: h("p"),
                    replay_identity: h("r"),
                    deterministic: true,
                    local_only: true,
                    required: true,
                },
                ExecutionNode1 {
                    node_id: "b".into(),
                    depends_on: vec!["a".into()],
                    artifact_digest: h("b"),
                    provenance_digest: h("p"),
                    replay_identity: h("r"),
                    deterministic: true,
                    local_only: true,
                    required: true,
                },
            ],
        }
    }
    #[test]
    fn manifest_is_a0() {
        assert_eq!(
            computational_execution_contract_manifest().autonomy_tier,
            AutonomyTier::A0
        )
    }
    #[test]
    fn qualified_graph() {
        assert_eq!(
            model_computational_execution_contract(&req())
                .unwrap()
                .disposition,
            "qualified"
        )
    }
    #[test]
    fn cycle_blocks() {
        let mut r = req();
        r.nodes[0].depends_on = vec!["b".into()];
        assert_eq!(
            model_computational_execution_contract(&r)
                .unwrap()
                .disposition,
            "blocked"
        )
    }
    #[test]
    fn policy_blocks() {
        let mut r = req();
        r.policy_allowed = false;
        assert_eq!(
            model_computational_execution_contract(&r)
                .unwrap()
                .disposition,
            "blocked"
        )
    }
}

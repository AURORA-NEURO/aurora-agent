//! Local single-study computational-execution assurance for `AFA-weavelang-P12-F25`.
//!
//! This verifier consumes a caller-declared research workflow graph and emits a deterministic,
//! evidence-bearing execution-run envelope. It does not dispatch tools, contact networks, move
//! raw data, or claim that a scientific result is true. Unknown/unmeasured evidence, cycles,
//! missing dependencies, unauthorized effects, budget exhaustion, and adversarial events remain
//! explicit and fail closed.

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-weavelang-P12-F25";
pub const CONTRACT_VERSION: &str = "weavelang-local-computational-execution-assurance-harness/1.0";
pub const INPUT_SCHEMA: &str = "ResearchWorkflowSpec1@1";
pub const OUTPUT_SCHEMA: &str = "ExecutionRun7@1";
pub const TOOL_NAME: &str = "weavelang_computational_execution_assurance";
const CONTENT_TYPE: &str = "application/vnd.aurora.weavelang-execution-run-7+json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionEvidenceState {
    Proven,
    Supported,
    Unknown,
    Unmeasured,
    Contradicted,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionNode {
    pub node_id: String,
    pub capability_id: String,
    pub dependency_order: Vec<String>,
    pub effect: String,
    pub resource: String,
    pub cost: f64,
    pub evidence_state: ExecutionEvidenceState,
    pub input_digest: ContentHash,
    pub output_digest: ContentHash,
    pub permitted: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResearchWorkflowSpec {
    pub schema_version: String,
    pub request_id: String,
    pub workflow_id: String,
    pub actor_id: String,
    pub nodes: Vec<ExecutionNode>,
    pub budgets: BTreeMap<String, f64>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub adversarial_events: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionRunDisposition {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionRunReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub actor_id: String,
    pub disposition: ExecutionRunDisposition,
    pub node_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub missing_dependency_order: Vec<String>,
    pub cycle_order: Vec<String>,
    pub unauthorized_effect_order: Vec<String>,
    pub budget_exhausted_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub run_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ComputationalExecutionError {
    #[error("invalid computational execution request: {0}")]
    Invalid(String),
    #[error("computational execution artifact failed: {0}")]
    Artifact(String),
    #[error("computational execution JSON failed: {0}")]
    Json(String),
}

fn invalid(message: impl Into<String>) -> ComputationalExecutionError {
    ComputationalExecutionError::Invalid(message.into())
}
fn digest_is_valid(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}
fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

impl ExecutionRunReceipt {
    pub fn validate(&self) -> Result<(), ComputationalExecutionError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.actor_id.trim().is_empty()
            || self.node_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(invalid(
                "execution identity, graph, locality, or effects are incomplete",
            ));
        }
        for values in [
            &self.node_order,
            &self.selected_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.missing_dependency_order,
            &self.cycle_order,
            &self.unauthorized_effect_order,
            &self.budget_exhausted_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(invalid("execution ordering is not canonical"));
            }
        }
        let ids = self.node_order.iter().cloned().collect::<BTreeSet<_>>();
        let parts = self
            .selected_order
            .iter()
            .chain(self.unresolved_order.iter())
            .chain(self.blocked_order.iter())
            .cloned()
            .collect::<Vec<_>>();
        if parts.len() != ids.len() || parts.iter().cloned().collect::<BTreeSet<_>>() != ids {
            return Err(invalid("execution node states do not partition the graph"));
        }
        for value in [
            &self.replay_identity,
            &self.run_digest,
            &self.artifact.content_hash,
        ] {
            if !digest_is_valid(value) {
                return Err(invalid("execution digest is invalid"));
            }
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ComputationalExecutionError::Artifact(error.to_string()))?;
        if self.artifact.content_type != CONTENT_TYPE {
            return Err(invalid("execution artifact type is invalid"));
        }
        if self.disposition == ExecutionRunDisposition::Qualified
            && self.effect_receipts != [format!("verify:weavelang-execution:{}", self.workflow_id)]
        {
            return Err(invalid("qualified execution effect is invalid"));
        }
        if self.disposition != ExecutionRunDisposition::Qualified
            && self.effect_receipts != ["block:unsafe-release"]
        {
            return Err(invalid("non-qualified execution must block release"));
        }
        Ok(())
    }
}

pub fn computational_execution_assurance_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "weavelang".into(),
        consumers: BTreeSet::from([
            String::from("platform reliability engineer"),
            String::from("research workflow operator"),
            String::from("downstream execution gateway"),
        ]),
        behavior: "verifies a bounded local research execution graph, evidence closure, effect allow-list, replay identity, and resource budget without dispatching any effect".into(),
        value: "turns a WeaveLang execution plan into a deterministic ExecutionRun7 assurance artifact while making missing evidence, cycles, and unsafe effects impossible to mistake for success".into(),
        inputs: vec![TypedPort { name: "research_workflow_spec".into(), schema: INPUT_SCHEMA.into(), required: true }],
        outputs: vec![TypedPort { name: "execution_run".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: BTreeSet::from([Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact]),
        permissions: BTreeSet::from([String::from("evaluate:capability-runs")]),
        determinism: Determinism::ByteStable,
        evidence: vec![
            EvidenceReference { source_id: "cwl".into(), state: EvidenceState::Supported, locator: Some("https://www.commonwl.org/specification/".into()) },
            EvidenceReference { source_id: "ga4gh-wes".into(), state: EvidenceState::Supported, locator: Some("https://ga4gh.github.io/workflow-execution-service-schemas/docs/".into()) },
            EvidenceReference { source_id: "slsa-provenance-1.2".into(), state: EvidenceState::Supported, locator: Some("https://slsa.dev/spec/v1.2/provenance".into()) },
        ],
        authority_requirements: Vec::new(),
        autonomy_tier: AutonomyTier::A0,
        surfaces: BTreeSet::from([ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Protocol, ResearchSurface::Policy, ResearchSurface::Operator]),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn assure_computational_execution(
    request: &ResearchWorkflowSpec,
) -> Result<ExecutionRunReceipt, ComputationalExecutionError> {
    validate_request(request)?;
    let mut nodes = request.nodes.clone();
    nodes.sort_by(|a, b| a.node_id.cmp(&b.node_id));
    let node_order = nodes
        .iter()
        .map(|node| node.node_id.clone())
        .collect::<Vec<_>>();
    let ids = node_order.iter().cloned().collect::<BTreeSet<_>>();
    let mut missing = BTreeSet::new();
    let mut cycles = BTreeSet::new();
    let mut unauthorized = BTreeSet::new();
    let mut budget_exhausted = BTreeSet::new();
    let mut selected = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let allowed = [
        "read-local",
        "execute-local-computation",
        "write-local-artifact",
    ];
    for node in &nodes {
        for dependency in &node.dependency_order {
            if !ids.contains(dependency) {
                missing.insert(format!("{}->{dependency}", node.node_id));
            }
        }
        if !allowed.contains(&node.effect.as_str()) || !node.permitted {
            unauthorized.insert(node.node_id.clone());
        }
        if node.cost.is_sign_negative()
            || !node.cost.is_finite()
            || request.budgets.get(&node.resource).copied().unwrap_or(0.0) < node.cost
        {
            budget_exhausted.insert(node.node_id.clone());
        }
        match node.evidence_state {
            ExecutionEvidenceState::Contradicted => {
                blocked.insert(node.node_id.clone());
                negative.insert(format!("{}:contradicted", node.node_id));
            }
            ExecutionEvidenceState::Unknown => {
                unresolved.insert(node.node_id.clone());
                uncertainty.insert(format!("{}:unknown", node.node_id));
            }
            ExecutionEvidenceState::Unmeasured => {
                unresolved.insert(node.node_id.clone());
                omissions.insert(format!("{}:unmeasured", node.node_id));
            }
            ExecutionEvidenceState::Proven | ExecutionEvidenceState::Supported => {}
        }
    }
    let mut marks = BTreeMap::<String, u8>::new();
    fn visit(
        node: &str,
        by_id: &BTreeMap<String, ExecutionNode>,
        marks: &mut BTreeMap<String, u8>,
        cycles: &mut BTreeSet<String>,
    ) {
        if marks.get(node) == Some(&1) {
            cycles.insert(node.to_string());
            return;
        }
        if marks.get(node) == Some(&2) {
            return;
        }
        marks.insert(node.to_string(), 1);
        if let Some(value) = by_id.get(node) {
            for dependency in &value.dependency_order {
                if by_id.contains_key(dependency) {
                    visit(dependency, by_id, marks, cycles);
                }
            }
        }
        marks.insert(node.to_string(), 2);
    }
    let by_id = nodes
        .iter()
        .cloned()
        .map(|node| (node.node_id.clone(), node))
        .collect::<BTreeMap<_, _>>();
    for id in &node_order {
        visit(id, &by_id, &mut marks, &mut cycles);
    }
    let global_block = !request.policy_allow
        || !request.protected_closure
        || !request.raw_data_local
        || !request.adversarial_events.is_empty();
    if !request.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.raw_data_local {
        negative.insert("request:raw-data-locality-violation".into());
    }
    negative.extend(
        request
            .adversarial_events
            .iter()
            .map(|event| format!("adversarial:{event}")),
    );
    blocked.extend(unauthorized.iter().cloned());
    blocked.extend(budget_exhausted.iter().cloned());
    blocked.extend(cycles.iter().cloned());
    blocked.extend(
        missing
            .iter()
            .filter_map(|value| value.split("->").next().map(str::to_string)),
    );
    if !cycles.is_empty() {
        blocked.extend(node_order.iter().cloned());
    }
    if global_block {
        blocked.extend(node_order.iter().cloned());
        selected.clear();
        unresolved.clear();
        omissions.insert("request:weavelang-release-gate-blocked".into());
    }
    if !global_block {
        selected.extend(
            node_order
                .iter()
                .filter(|id| !blocked.contains(*id) && !unresolved.contains(*id))
                .cloned(),
        );
    }
    let disposition = if global_block || !blocked.is_empty() {
        ExecutionRunDisposition::Blocked
    } else if !unresolved.is_empty() || !missing.is_empty() {
        ExecutionRunDisposition::Unresolved
    } else {
        ExecutionRunDisposition::Qualified
    };
    let selected_order = selected.into_iter().collect::<Vec<_>>();
    let unresolved_order = unresolved.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let missing_dependency_order = missing.into_iter().collect::<Vec<_>>();
    let cycle_order = cycles.into_iter().collect::<Vec<_>>();
    let unauthorized_effect_order = unauthorized.into_iter().collect::<Vec<_>>();
    let budget_exhausted_order = budget_exhausted.into_iter().collect::<Vec<_>>();
    let omission_order = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty_order = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence_order = negative.into_iter().collect::<Vec<_>>();
    let effect_receipts = if disposition == ExecutionRunDisposition::Qualified {
        vec![format!(
            "verify:weavelang-execution:{}",
            request.workflow_id
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "workflow_id": request.workflow_id, "actor_id": request.actor_id, "disposition": disposition, "node_order": node_order, "selected_order": selected_order, "unresolved_order": unresolved_order, "blocked_order": blocked_order, "missing_dependency_order": missing_dependency_order, "cycle_order": cycle_order, "unauthorized_effect_order": unauthorized_effect_order, "budget_exhausted_order": budget_exhausted_order, "omission_order": omission_order, "uncertainty_order": uncertainty_order, "negative_evidence_order": negative_evidence_order, "replay_identity": request.replay_identity, "effect_receipts": effect_receipts, "raw_data_local": request.raw_data_local, "boundary": PRECLINICAL_BOUNDARY});
    let run_digest = ContentHash::of_value(&payload)
        .map_err(|error| ComputationalExecutionError::Artifact(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("weavelang-execution-run:{}", request.workflow_id),
        CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ComputationalExecutionError::Artifact(error.to_string()))?;
    let receipt = ExecutionRunReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        actor_id: request.actor_id.clone(),
        disposition,
        node_order: payload["node_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        selected_order: payload["selected_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        unresolved_order: payload["unresolved_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        blocked_order: payload["blocked_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        missing_dependency_order: payload["missing_dependency_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        cycle_order: payload["cycle_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        unauthorized_effect_order: payload["unauthorized_effect_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        budget_exhausted_order: payload["budget_exhausted_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        omission_order: payload["omission_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        uncertainty_order: payload["uncertainty_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        negative_evidence_order: payload["negative_evidence_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        replay_identity: request.replay_identity.clone(),
        run_digest,
        artifact,
        effect_receipts: payload["effect_receipts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        raw_data_local: request.raw_data_local,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

pub fn assure_computational_execution_json(value: &Value) -> Result<Value, String> {
    let request: ResearchWorkflowSpec = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid research workflow spec: {error}"))?;
    let receipt = assure_computational_execution(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize execution run: {error}"))
}

pub fn validate_computational_execution_json(value: &Value) -> Result<ExecutionRunReceipt, String> {
    let receipt: ExecutionRunReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid execution run receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    Ok(receipt)
}

fn validate_request(request: &ResearchWorkflowSpec) -> Result<(), ComputationalExecutionError> {
    if request.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
        || request.request_id.trim().is_empty()
        || request.workflow_id.trim().is_empty()
        || request.actor_id.trim().is_empty()
        || request.nodes.is_empty()
        || !digest_is_valid(&request.replay_identity)
        || !canonical(&request.adversarial_events)
        || !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(invalid(
            "workflow identity, graph, replay, locality, or boundary is invalid",
        ));
    }
    if request
        .budgets
        .values()
        .any(|value| !value.is_finite() || value.is_sign_negative())
    {
        return Err(invalid("workflow budgets must be finite and non-negative"));
    }
    let mut ids = BTreeSet::new();
    for node in &request.nodes {
        if node.node_id.trim().is_empty()
            || !ids.insert(node.node_id.clone())
            || node.capability_id.trim().is_empty()
            || node.effect.trim().is_empty()
            || node.resource.trim().is_empty()
            || !canonical(&node.dependency_order)
            || !digest_is_valid(&node.input_digest)
            || !digest_is_valid(&node.output_digest)
        {
            return Err(invalid(format!(
                "workflow node {} is malformed or duplicated",
                node.node_id
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
    fn request() -> ResearchWorkflowSpec {
        let h = hash("execution");
        ResearchWorkflowSpec {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            request_id: "exec:one".into(),
            workflow_id: "workflow:one".into(),
            actor_id: "operator:one".into(),
            nodes: vec![ExecutionNode {
                node_id: "node:a".into(),
                capability_id: "cap:a".into(),
                dependency_order: vec![],
                effect: "execute-local-computation".into(),
                resource: "cpu".into(),
                cost: 1.0,
                evidence_state: ExecutionEvidenceState::Supported,
                input_digest: h.clone(),
                output_digest: h.clone(),
                permitted: true,
            }],
            budgets: BTreeMap::from([(String::from("cpu"), 2.0)]),
            replay_identity: h,
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            adversarial_events: vec![],
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a0() {
        assert_eq!(
            computational_execution_assurance_manifest().autonomy_tier,
            AutonomyTier::A0
        );
    }
    #[test]
    fn qualified() {
        assert_eq!(
            assure_computational_execution(&request())
                .unwrap()
                .disposition,
            ExecutionRunDisposition::Qualified
        );
    }
    #[test]
    fn deterministic() {
        let a = assure_computational_execution(&request()).unwrap();
        let b = assure_computational_execution(&request()).unwrap();
        assert_eq!(a.run_digest, b.run_digest);
    }
    #[test]
    fn unknown_is_unresolved() {
        let mut value = request();
        value.nodes[0].evidence_state = ExecutionEvidenceState::Unknown;
        assert_eq!(
            assure_computational_execution(&value).unwrap().disposition,
            ExecutionRunDisposition::Unresolved
        );
    }
    #[test]
    fn unauthorized_effect_blocks() {
        let mut value = request();
        value.nodes[0].effect = "external-network".into();
        assert_eq!(
            assure_computational_execution(&value).unwrap().disposition,
            ExecutionRunDisposition::Blocked
        );
    }
    #[test]
    fn cycle_blocks() {
        let mut value = request();
        value.nodes.push(ExecutionNode {
            node_id: "node:b".into(),
            capability_id: "cap:b".into(),
            dependency_order: vec!["node:a".into()],
            effect: "execute-local-computation".into(),
            resource: "cpu".into(),
            cost: 1.0,
            evidence_state: ExecutionEvidenceState::Supported,
            input_digest: hash("b"),
            output_digest: hash("b"),
            permitted: true,
        });
        value.nodes[0].dependency_order = vec!["node:b".into()];
        assert_eq!(
            assure_computational_execution(&value).unwrap().disposition,
            ExecutionRunDisposition::Blocked
        );
    }
    #[test]
    fn adversarial_blocks() {
        let mut value = request();
        value.adversarial_events = vec!["poisoned-artifact".into()];
        assert_eq!(
            assure_computational_execution(&value).unwrap().disposition,
            ExecutionRunDisposition::Blocked
        );
    }
}

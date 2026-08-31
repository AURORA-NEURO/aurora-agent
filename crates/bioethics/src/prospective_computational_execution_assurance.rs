//! Prospective high-throughput computational-execution assurance harness.
//!
//! Atlas feature: `AFA-bioethics-P12-F27`.
//!
//! This harness verifies a caller-supplied execution graph. It computes deterministic dependency
//! order and release evidence, but it never starts a process, submits a job, contacts a provider,
//! or touches an instrument. Unknown dependencies, cycles, contradictions, budget exhaustion,
//! and policy failures remain explicit in `ExecutionRun7`.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, LossSeverity, ProvenanceLink, ResearchSurface, SemanticLoss, TypedPort,
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-bioethics-P12-F27";
pub const CONTRACT_VERSION: &str = "bioethics-prospective-computational-execution-assurance/1.0";
pub const INPUT_SCHEMA: &str = "ResearchWorkflowSpec4@1";
pub const OUTPUT_SCHEMA: &str = "ExecutionRun7@1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionDisposition {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionNode {
    pub node_id: String,
    pub dependency_order: Vec<String>,
    pub effect_kind: String,
    pub evidence_state: EvidenceState,
    pub replay_identity: ContentHash,
    pub artifact_digest: Option<ContentHash>,
    pub provenance_digest: Option<ContentHash>,
    pub estimated_cost: u32,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchWorkflowSpec {
    pub request_id: String,
    pub run_id: String,
    pub workflow_id: String,
    pub plan_schema: String,
    pub scope: String,
    pub nodes: Vec<ExecutionNode>,
    pub replay_identity: ContentHash,
    pub budget_units: u32,
    pub max_budget_units: u32,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub federated_summary_only: bool,
    pub adversarial_events: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionRun {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub run_id: String,
    pub workflow_id: String,
    pub scope: String,
    pub disposition: ExecutionDisposition,
    pub plan_order: Vec<String>,
    pub topological_order: Vec<String>,
    pub completed_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub cycle_order: Vec<String>,
    pub missing_dependency_order: Vec<String>,
    pub compensation_order: Vec<String>,
    pub decisions: Vec<Value>,
    pub checkpoint_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub run_digest: ContentHash,
    pub semantic_loss: Vec<SemanticLoss>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub federation_export: String,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ExecutionAssuranceError {
    #[error("invalid computational execution assurance request: {0}")]
    Invalid(String),
    #[error("computational execution assurance artifact failed: {0}")]
    Artifact(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

impl ExecutionRun {
    pub fn validate(&self) -> Result<(), ExecutionAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.federation_export != "aggregate-digest-only"
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.run_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.plan_order.is_empty()
            || self.topological_order.len() > self.plan_order.len()
            || self.decisions.len() != self.plan_order.len()
            || self.effect_receipts.is_empty()
        {
            return Err(ExecutionAssuranceError::Invalid(
                "execution identity, locality, plan, export mode, decisions, or effects are incomplete".into(),
            ));
        }
        for values in [
            &self.plan_order,
            &self.completed_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.cycle_order,
            &self.missing_dependency_order,
            &self.compensation_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(ExecutionAssuranceError::Invalid(
                    "execution orders and evidence annotations are not canonical".into(),
                ));
            }
        }
        let plan = self.plan_order.iter().cloned().collect::<BTreeSet<_>>();
        if self.decisions.iter().enumerate().any(|(index, decision)| {
            decision.get("node_id").and_then(Value::as_str) != Some(self.plan_order[index].as_str())
        }) {
            return Err(ExecutionAssuranceError::Invalid(
                "execution decisions do not match plan order".into(),
            ));
        }
        let mut partition = BTreeSet::<String>::new();
        for id in self
            .completed_order
            .iter()
            .chain(self.unresolved_order.iter())
            .chain(self.blocked_order.iter())
        {
            if !partition.insert(id.clone()) || !plan.contains(id) {
                return Err(ExecutionAssuranceError::Invalid(
                    "execution disposition partition is duplicated or out of scope".into(),
                ));
            }
        }
        if partition != plan {
            return Err(ExecutionAssuranceError::Invalid(
                "execution disposition partition is incomplete".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("verify:execution-plan:") && effect != "block:unsafe-release"
        }) {
            return Err(ExecutionAssuranceError::Invalid(
                "execution effect is outside the verification-only gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ExecutionAssuranceError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, ExecutionAssuranceError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|error| ExecutionAssuranceError::Artifact(error.to_string()))?,
        )
        .map_err(|error| ExecutionAssuranceError::Artifact(error.to_string()))
    }
}

pub fn capability_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "bioethics".into(),
        consumers: BTreeSet::from([
            "computational execution operator".into(),
            "federated research workflow".into(),
            "release evidence reviewer".into(),
        ]),
        behavior: "verifies a bounded research execution graph and emits replayable plan evidence without dispatching work".into(),
        value: "prevents cycles, missing dependencies, policy violations, non-local effects, and unreplayable plans from entering computation".into(),
        inputs: vec![TypedPort { name: "research_workflow_spec".into(), schema: INPUT_SCHEMA.into(), required: true }],
        outputs: vec![TypedPort { name: "execution_run".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: BTreeSet::from([Effect::ReadLocalData, Effect::WriteLocalArtifact]),
        permissions: BTreeSet::from(["evaluate:capability-runs".into()]),
        determinism: Determinism::ByteStable,
        evidence: vec![
            EvidenceReference { source_id: "cwl".into(), state: EvidenceState::Supported, locator: Some("https://www.commonwl.org/specification/".into()) },
            EvidenceReference { source_id: "ga4gh-wes".into(), state: EvidenceState::Supported, locator: Some("https://ga4gh.github.io/workflow-execution-service-schemas/docs/".into()) },
            EvidenceReference { source_id: "slsa-1.2".into(), state: EvidenceState::Supported, locator: Some("https://slsa.dev/spec/v1.2/provenance".into()) },
        ],
        authority_requirements: vec![AuthorityRequirement { role: "computational-execution-operator".into(), reason: "execution-plan release evidence".into() }],
        autonomy_tier: AutonomyTier::A1,
        surfaces: BTreeSet::from([ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Protocol, ResearchSurface::Policy, ResearchSurface::Operator]),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

fn validate_request(request: &ResearchWorkflowSpec) -> Result<(), ExecutionAssuranceError> {
    if request.request_id.trim().is_empty()
        || request.run_id.trim().is_empty()
        || request.workflow_id.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.plan_schema != INPUT_SCHEMA
        || request.nodes.is_empty()
        || request.budget_units == 0
        || request.max_budget_units == 0
        || request.budget_units > request.max_budget_units
        || !request.raw_data_local
        || !request.federated_summary_only
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(ExecutionAssuranceError::Invalid(
            "execution identity, schema, nodes, budget, locality, aggregate-only mode, or boundary is invalid".into(),
        ));
    }
    let mut ids = request
        .nodes
        .iter()
        .map(|node| node.node_id.clone())
        .collect::<Vec<_>>();
    ids.sort();
    if ids.windows(2).any(|pair| pair[0] == pair[1])
        || request.nodes.iter().any(|node| {
            node.node_id.trim().is_empty()
                || !canonical(&node.dependency_order)
                || node
                    .dependency_order
                    .iter()
                    .any(|dependency| dependency.trim().is_empty())
        })
    {
        return Err(ExecutionAssuranceError::Invalid(
            "execution node identifiers or dependencies are invalid".into(),
        ));
    }
    Ok(())
}

pub fn verify(request: &ResearchWorkflowSpec) -> Result<ExecutionRun, ExecutionAssuranceError> {
    validate_request(request)?;
    let mut nodes = request.nodes.clone();
    nodes.sort_by(|left, right| left.node_id.cmp(&right.node_id));
    let plan_order = nodes
        .iter()
        .map(|node| node.node_id.clone())
        .collect::<Vec<_>>();
    let plan_set = plan_order.iter().cloned().collect::<BTreeSet<_>>();
    let mut indegree = BTreeMap::<String, usize>::new();
    let mut dependents = BTreeMap::<String, Vec<String>>::new();
    for node in &nodes {
        indegree.insert(node.node_id.clone(), node.dependency_order.len());
        for dependency in &node.dependency_order {
            dependents
                .entry(dependency.clone())
                .or_default()
                .push(node.node_id.clone());
        }
    }
    for children in dependents.values_mut() {
        children.sort();
    }
    let mut queue = VecDeque::new();
    for node_id in &plan_order {
        if indegree[node_id] == 0 {
            queue.push_back(node_id.clone());
        }
    }
    let mut topological = Vec::new();
    while let Some(node_id) = queue.pop_front() {
        topological.push(node_id.clone());
        for child in dependents.get(&node_id).into_iter().flatten() {
            if let Some(value) = indegree.get_mut(child) {
                *value = value.saturating_sub(1);
                if *value == 0 {
                    queue.push_back(child.clone());
                }
            }
        }
    }
    let topological_set = topological.iter().cloned().collect::<BTreeSet<_>>();
    let cycle_order = plan_set
        .difference(&topological_set)
        .filter(|node_id| {
            nodes
                .iter()
                .find(|node| &node.node_id == *node_id)
                .is_some_and(|node| {
                    node.dependency_order
                        .iter()
                        .all(|dependency| plan_set.contains(dependency))
                })
        })
        .cloned()
        .collect::<Vec<_>>();
    let mut global_failed = BTreeSet::new();
    for (gate, failed) in [
        ("policy", !request.policy_allow),
        ("protected-closure", !request.protected_closure),
        ("raw-data-locality", !request.raw_data_local),
        ("aggregate-only", !request.federated_summary_only),
        ("adversarial-input", !request.adversarial_events.is_empty()),
    ] {
        if failed {
            global_failed.insert(gate.to_string());
        }
    }
    let mut completed = Vec::new();
    let mut unresolved = Vec::new();
    let mut blocked = Vec::new();
    let mut missing_dependencies = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut semantic_loss = Vec::new();
    let mut decisions = Vec::new();
    let allowed_effects = BTreeSet::from(["compute-local", "read-local", "write-artifact"]);
    let mut spent = 0_u32;
    for node in &nodes {
        let mut failed = global_failed.clone();
        let mut pending = BTreeSet::<String>::new();
        if node
            .dependency_order
            .iter()
            .any(|dependency| !plan_set.contains(dependency))
        {
            pending.insert("missing-dependency".into());
            for dependency in node
                .dependency_order
                .iter()
                .filter(|dependency| !plan_set.contains(*dependency))
            {
                missing_dependencies.insert(format!("{}:{dependency}", node.node_id));
                omissions.insert(format!("{}:missing-dependency:{dependency}", node.node_id));
            }
        }
        if cycle_order.contains(&node.node_id) {
            failed.insert("dependency-cycle".into());
        }
        if !allowed_effects.contains(node.effect_kind.as_str()) {
            failed.insert("effect-not-local-allow-listed".into());
        }
        if node.replay_identity != request.replay_identity {
            failed.insert("replay-identity".into());
        }
        if node.artifact_digest.is_none() {
            pending.insert("artifact-digest-missing".into());
            omissions.insert(format!("{}:artifact-digest-missing", node.node_id));
        }
        if node.provenance_digest.is_none() {
            pending.insert("provenance-missing".into());
            omissions.insert(format!("{}:provenance-missing", node.node_id));
        }
        if !node.omissions.is_empty() {
            pending.insert("node-omissions".into());
            omissions.extend(
                node.omissions
                    .iter()
                    .map(|item| format!("{}:{item}", node.node_id)),
            );
        }
        if !node.uncertainty.is_empty() {
            pending.insert("node-uncertainty".into());
            uncertainty.extend(
                node.uncertainty
                    .iter()
                    .map(|item| format!("{}:{item}", node.node_id)),
            );
        }
        match node.evidence_state {
            EvidenceState::Contradicted => {
                failed.insert("contradicted-evidence".into());
                negative.insert(format!("{}:contradicted", node.node_id));
            }
            EvidenceState::Unknown | EvidenceState::Speculative => {
                pending.insert("evidence-state-not-qualified".into());
                uncertainty.insert(format!("{}:evidence-state", node.node_id));
            }
            EvidenceState::Proven | EvidenceState::Supported => {}
        }
        if node.estimated_cost > request.budget_units.saturating_sub(spent) {
            pending.insert("budget-ceiling".into());
            omissions.insert(format!("{}:budget-ceiling", node.node_id));
        } else {
            spent = spent.saturating_add(node.estimated_cost);
        }
        negative.insert(format!("{}:execution-not-started", node.node_id));
        let disposition = if !failed.is_empty() {
            blocked.push(node.node_id.clone());
            "blocked"
        } else if !pending.is_empty() {
            unresolved.push(node.node_id.clone());
            "unresolved"
        } else {
            completed.push(node.node_id.clone());
            "completed"
        };
        decisions.push(json!({
            "node_id": node.node_id,
            "effect_kind": node.effect_kind,
            "disposition": disposition,
            "failed_gates": failed.clone().into_iter().collect::<Vec<_>>(),
            "conditional_gates": pending.into_iter().collect::<Vec<_>>(),
        }));
        if !failed.is_empty() {
            semantic_loss.push(SemanticLoss {
                field: format!("node:{}", node.node_id),
                reason: "execution node cannot be released after a failed plan or safety gate"
                    .into(),
                severity: LossSeverity::DecisionRelevant,
            });
        }
    }
    completed.sort();
    unresolved.sort();
    blocked.sort();
    let mut compensation = BTreeSet::new();
    for node_id in unresolved.iter().chain(blocked.iter()) {
        compensation.insert(format!("retain:{}:no-dispatch", node_id));
    }
    let disposition = if !global_failed.is_empty() || !blocked.is_empty() {
        ExecutionDisposition::Blocked
    } else if !unresolved.is_empty() || completed.len() != plan_order.len() {
        ExecutionDisposition::Unresolved
    } else {
        ExecutionDisposition::Qualified
    };
    let checkpoint_digest = ContentHash::of_value(&json!({
        "run_id": request.run_id,
        "workflow_id": request.workflow_id,
        "plan_order": plan_order,
        "topological_order": topological,
        "replay_identity": request.replay_identity,
    }))
    .map_err(|error| ExecutionAssuranceError::Artifact(error.to_string()))?;
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "run_id": request.run_id,
        "workflow_id": request.workflow_id,
        "scope": request.scope,
        "disposition": disposition,
        "plan_order": plan_order,
        "topological_order": topological,
        "completed_order": completed,
        "unresolved_order": unresolved,
        "blocked_order": blocked,
        "cycle_order": cycle_order,
        "missing_dependency_order": missing_dependencies,
        "compensation_order": compensation,
        "decisions": decisions,
        "checkpoint_digest": checkpoint_digest,
        "replay_identity": request.replay_identity,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let run_digest = ContentHash::of_value(&payload)
        .map_err(|error| ExecutionAssuranceError::Artifact(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("execution-run:{}", request.run_id),
        "application/vnd.aurora.execution-run+json",
        &payload,
        semantic_loss.clone(),
        vec![ProvenanceLink {
            source_id: request.run_id.clone(),
            relation: "computational-execution-assurance".into(),
            digest: run_digest.clone(),
        }],
    )
    .map_err(|error| ExecutionAssuranceError::Artifact(error.to_string()))?;
    let effect_receipts = if matches!(disposition, ExecutionDisposition::Qualified) {
        vec![format!("verify:execution-plan:{}", request.run_id)]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let receipt = ExecutionRun {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        run_id: request.run_id.clone(),
        workflow_id: request.workflow_id.clone(),
        scope: request.scope.clone(),
        disposition,
        plan_order,
        topological_order: topological,
        completed_order: completed,
        unresolved_order: unresolved,
        blocked_order: blocked,
        cycle_order,
        missing_dependency_order: missing_dependencies.into_iter().collect(),
        compensation_order: compensation.into_iter().collect(),
        decisions,
        checkpoint_digest,
        replay_identity: request.replay_identity.clone(),
        run_digest,
        semantic_loss,
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        artifact,
        effect_receipts,
        raw_data_local: request.raw_data_local,
        federation_export: "aggregate-digest-only".into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

pub fn verify_json(value: &Value) -> Result<Value, ExecutionAssuranceError> {
    let request: ResearchWorkflowSpec = serde_json::from_value(value.clone())
        .map_err(|error| ExecutionAssuranceError::Invalid(error.to_string()))?;
    serde_json::to_value(verify(&request)?)
        .map_err(|error| ExecutionAssuranceError::Artifact(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash() -> ContentHash {
        ContentHash::of_bytes(b"computational-execution-assurance")
    }

    fn node(id: &str, dependencies: Vec<&str>, state: EvidenceState) -> ExecutionNode {
        ExecutionNode {
            node_id: id.into(),
            dependency_order: dependencies.into_iter().map(String::from).collect(),
            effect_kind: "compute-local".into(),
            evidence_state: state,
            replay_identity: hash(),
            artifact_digest: Some(hash()),
            provenance_digest: Some(hash()),
            estimated_cost: 2,
            omissions: Vec::new(),
            uncertainty: Vec::new(),
        }
    }

    fn request() -> ResearchWorkflowSpec {
        ResearchWorkflowSpec {
            request_id: "request:execution".into(),
            run_id: "run:execution".into(),
            workflow_id: "workflow:execution".into(),
            plan_schema: INPUT_SCHEMA.into(),
            scope: "organoid-neuroscience".into(),
            nodes: vec![
                node("step-a", Vec::new(), EvidenceState::Supported),
                node("step-b", vec!["step-a"], EvidenceState::Proven),
            ],
            replay_identity: hash(),
            budget_units: 10,
            max_budget_units: 10,
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            federated_summary_only: true,
            adversarial_events: Vec::new(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn complete_graph_qualifies_and_replays() {
        let run = verify(&request()).unwrap();
        assert_eq!(run.disposition, ExecutionDisposition::Qualified);
        assert_eq!(run.digest().unwrap(), run.digest().unwrap());
        assert_eq!(run.topological_order, vec!["step-a", "step-b"]);
    }

    #[test]
    fn missing_dependency_and_unknown_are_unresolved() {
        let mut value = request();
        value.nodes[1].dependency_order = vec!["missing".into()];
        value.nodes[1].evidence_state = EvidenceState::Unknown;
        let run = verify(&value).unwrap();
        assert_eq!(run.disposition, ExecutionDisposition::Unresolved);
        assert!(run
            .missing_dependency_order
            .iter()
            .any(|item| item.contains("missing")));
    }

    #[test]
    fn cycle_and_external_effect_block_release() {
        let mut value = request();
        value.nodes[0].dependency_order = vec!["step-b".into()];
        value.nodes[1].effect_kind = "network".into();
        let run = verify(&value).unwrap();
        assert_eq!(run.disposition, ExecutionDisposition::Blocked);
        assert!(run.effect_receipts.contains(&"block:unsafe-release".into()));
    }

    #[test]
    fn policy_and_replay_failure_are_fail_closed() {
        let mut value = request();
        value.policy_allow = false;
        value.nodes[0].replay_identity = ContentHash::of_bytes(b"other");
        let run = verify(&value).unwrap();
        assert_eq!(run.disposition, ExecutionDisposition::Blocked);
        assert!(run
            .semantic_loss
            .iter()
            .any(|item| item.field == "node:step-a"));
    }

    #[test]
    fn manifest_is_a1_and_no_dispatch() {
        assert_eq!(capability_manifest().autonomy_tier, AutonomyTier::A1);
        assert!(capability_manifest()
            .surfaces
            .contains(&ResearchSurface::Cli));
    }
}

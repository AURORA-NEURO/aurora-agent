//! Multimodal computational-execution interoperability gateway.
//!
//! Atlas feature: `AFA-graph-P12-F22`.
//!
//! The gateway validates a typed research execution graph and emits an exchange-ready run
//! manifest. It never dispatches a job, opens a provider connection, or exports raw experimental
//! bytes; a separate governed executor may consume the qualified manifest.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, LossSeverity, ProvenanceLink, ResearchSurface, SemanticLoss, TypedPort,
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-graph-P12-F22";
pub const CONTRACT_VERSION: &str =
    "graph-multimodal-computational-execution-interoperability-gateway/1.0";
pub const INPUT_SCHEMA: &str = "ResearchWorkflowSpec2@1";
pub const OUTPUT_SCHEMA: &str = "ExecutionRun6@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionNode {
    pub node_id: String,
    pub study_id: String,
    pub modality_order: Vec<String>,
    pub dependency_order: Vec<String>,
    pub evidence_state: EvidenceState,
    pub artifact_digest: Option<ContentHash>,
    pub provenance_digest: Option<ContentHash>,
    pub replay_identity: ContentHash,
    pub effect: String,
    pub estimated_cost: u32,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchWorkflowSpec {
    pub request_id: String,
    pub workflow_id: String,
    pub scope: String,
    pub semantic_profile: String,
    pub schema_version: String,
    pub nodes: Vec<ExecutionNode>,
    pub required_study_order: Vec<String>,
    pub required_modality_order: Vec<String>,
    pub expected_comparability_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub budget_units: u32,
    pub max_budget_units: u32,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub permitted_effect_order: Vec<String>,
    pub adversarial_events: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionRun {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub scope: String,
    pub semantic_profile: String,
    pub disposition: String,
    pub plan_order: Vec<String>,
    pub topological_order: Vec<String>,
    pub completed_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub cycle_order: Vec<String>,
    pub missing_dependency_order: Vec<String>,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub exchange_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub checkpoint_digest: ContentHash,
    pub run_digest: ContentHash,
    pub semantic_loss: Vec<SemanticLoss>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GatewayError {
    #[error("invalid computational execution gateway input: {0}")]
    Invalid(String),
    #[error("computational execution gateway artifact failed: {0}")]
    Artifact(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

impl ExecutionRun {
    pub fn validate(&self) -> Result<(), GatewayError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.plan_order.is_empty()
            || self.topological_order.len() > self.plan_order.len()
            || self.study_order.is_empty()
            || self.modality_order.is_empty()
            || !self.raw_data_local
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.effect_receipts.is_empty()
        {
            return Err(GatewayError::Invalid("run identity, plan, study/modality closure, locality, boundary, or effects are incomplete".into()));
        }
        for values in [
            &self.plan_order,
            &self.completed_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.cycle_order,
            &self.missing_dependency_order,
            &self.study_order,
            &self.modality_order,
            &self.exchange_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(GatewayError::Invalid(
                    "execution run ordering is not canonical".into(),
                ));
            }
        }
        let partition = self
            .completed_order
            .iter()
            .chain(self.unresolved_order.iter())
            .chain(self.blocked_order.iter())
            .cloned()
            .collect::<Vec<_>>();
        if partition.len() != self.plan_order.len()
            || partition.iter().collect::<BTreeSet<_>>().len() != partition.len()
            || partition.iter().collect::<BTreeSet<_>>()
                != self.plan_order.iter().collect::<BTreeSet<_>>()
        {
            return Err(GatewayError::Invalid(
                "execution states do not partition plan nodes".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("exchange:permitted-artifacts:") && effect != "block:unsafe-release"
        }) {
            return Err(GatewayError::Invalid(
                "execution effect is outside permitted-artifact exchange gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| GatewayError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, GatewayError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|error| GatewayError::Artifact(error.to_string()))?,
        )
        .map_err(|error| GatewayError::Artifact(error.to_string()))
    }
}

pub fn capability_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "graph".into(), consumers: BTreeSet::from(["benchmark curator".into(), "execution gateway".into(), "workflow operator".into()]), behavior: "validates multimodal research execution graphs and emits replayable permitted-artifact exchange manifests without dispatching jobs".into(), value: "connects graph projections to interoperable execution contracts while preserving comparability, omissions, provenance, and local raw-data boundaries".into(), inputs: vec![TypedPort { name: "research_workflow_spec".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "execution_run".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: BTreeSet::from([Effect::ReadLocalData, Effect::WriteLocalArtifact, Effect::FederationExport]), permissions: BTreeSet::from(["connect:approved-endpoints".into()]), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "mcp-2025-06-18".into(), state: EvidenceState::Supported, locator: Some("https://modelcontextprotocol.io/specification/2025-06-18/basic/index".into()) }, EvidenceReference { source_id: "cwl".into(), state: EvidenceState::Supported, locator: Some("https://www.commonwl.org/specification/".into()) }, EvidenceReference { source_id: "ga4gh-wes".into(), state: EvidenceState::Supported, locator: Some("https://ga4gh.github.io/workflow-execution-service-schemas/docs/".into()) }], authority_requirements: vec![AuthorityRequirement { role: "execution-gateway-operator".into(), reason: "approve permitted-artifact exchange before dispatch".into() }], autonomy_tier: AutonomyTier::A2, surfaces: BTreeSet::from([ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Protocol, ResearchSurface::Policy, ResearchSurface::Operator]), boundary: PRECLINICAL_BOUNDARY.into() }
}

fn validate_spec(spec: &ResearchWorkflowSpec) -> Result<(), GatewayError> {
    if spec.schema_version != INPUT_SCHEMA
        || spec.request_id.trim().is_empty()
        || spec.workflow_id.trim().is_empty()
        || spec.scope.trim().is_empty()
        || spec.semantic_profile.trim().is_empty()
        || spec.nodes.is_empty()
        || spec.required_study_order.is_empty()
        || spec.required_modality_order.is_empty()
        || spec.budget_units == 0
        || spec.max_budget_units == 0
        || spec.budget_units > spec.max_budget_units
        || !spec.raw_data_local
        || spec.boundary != PRECLINICAL_BOUNDARY
        || !canonical(&spec.required_study_order)
        || !canonical(&spec.required_modality_order)
        || !canonical(&spec.permitted_effect_order)
    {
        return Err(GatewayError::Invalid("workflow identity, stages, bounds, locality, boundary, or canonical declarations are invalid".into()));
    }
    let mut ids = BTreeSet::new();
    let mut studies = BTreeSet::new();
    for node in &spec.nodes {
        if node.node_id.trim().is_empty()
            || !ids.insert(node.node_id.clone())
            || node.study_id.trim().is_empty()
            || node.modality_order.is_empty()
            || !canonical(&node.modality_order)
            || node.artifact_digest.is_none()
            || node.provenance_digest.is_none()
            || node.effect.trim().is_empty()
            || node.estimated_cost == 0
            || !spec.permitted_effect_order.contains(&node.effect)
        {
            return Err(GatewayError::Invalid("node identity, modality, artifact/provenance, cost, or permitted effect is invalid".into()));
        }
        studies.insert(node.study_id.clone());
    }
    if !spec
        .required_study_order
        .iter()
        .all(|study| studies.contains(study))
    {
        return Err(GatewayError::Invalid(
            "required study is absent from graph".into(),
        ));
    }
    Ok(())
}

pub fn admit(spec: &ResearchWorkflowSpec) -> Result<ExecutionRun, GatewayError> {
    validate_spec(spec)?;
    let nodes = spec
        .nodes
        .iter()
        .map(|node| (node.node_id.clone(), node))
        .collect::<BTreeMap<_, _>>();
    let mut plan_order = nodes.keys().cloned().collect::<Vec<_>>();
    plan_order.sort();
    let mut indegree = nodes
        .keys()
        .map(|id| (id.clone(), 0_usize))
        .collect::<BTreeMap<_, _>>();
    let mut edges: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut missing = BTreeSet::new();
    for node in &spec.nodes {
        for dependency in &node.dependency_order {
            if !nodes.contains_key(dependency) {
                missing.insert(format!("{}:{dependency}", node.node_id));
            } else {
                *indegree.get_mut(&node.node_id).expect("validated node") += 1;
                edges
                    .entry(dependency.clone())
                    .or_default()
                    .push(node.node_id.clone());
            }
        }
    }
    let mut queue = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(id, _)| id.clone())
        .collect::<VecDeque<_>>();
    let mut topological = Vec::new();
    while let Some(id) = queue.pop_front() {
        topological.push(id.clone());
        if let Some(children) = edges.get(&id) {
            for child in children {
                let degree = indegree.get_mut(child).expect("edge child");
                *degree -= 1;
                if *degree == 0 {
                    queue.push_back(child.clone());
                }
            }
        }
    }
    let cycle = indegree
        .iter()
        .filter(|(_, degree)| **degree > 0)
        .map(|(id, _)| id.clone())
        .collect::<Vec<_>>();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut semantic_loss = Vec::new();
    for item in &missing {
        omissions.insert(format!("missing-dependency:{item}"));
    }
    for node in &spec.nodes {
        if node.evidence_state == EvidenceState::Contradicted {
            semantic_loss.push(SemanticLoss {
                field: format!("node:{}", node.node_id),
                reason: "contradicted execution evidence cannot be admitted".into(),
                severity: LossSeverity::DecisionRelevant,
            });
        }
        if matches!(
            node.evidence_state,
            EvidenceState::Unknown | EvidenceState::Speculative
        ) {
            uncertainty.insert(format!("{}:evidence-state", node.node_id));
        }
        for item in &node.omissions {
            omissions.insert(format!("{}:{}", node.node_id, item));
        }
        for item in &node.uncertainty {
            uncertainty.insert(format!("{}:{}", node.node_id, item));
        }
        negative.insert(format!("{}:negative-result-not-observed", node.node_id));
    }
    let global_block = !spec.policy_allow
        || !spec.protected_closure
        || !spec.signed_approval
        || !spec.adversarial_events.is_empty();
    if !spec.policy_allow {
        omissions.insert("workflow:policy-denied".into());
    }
    if !spec.protected_closure {
        omissions.insert("workflow:protected-closure-incomplete".into());
    }
    if !spec.signed_approval {
        omissions.insert("workflow:signed-approval-missing".into());
    }
    for event in &spec.adversarial_events {
        omissions.insert(format!("workflow:adversarial:{event}"));
    }
    if !missing.is_empty() || !cycle.is_empty() {
        uncertainty.insert("workflow:graph-closure-incomplete".into());
    }
    let mut completed = Vec::new();
    let mut unresolved = Vec::new();
    let mut blocked = Vec::new();
    let mut spent = 0_u32;
    for node_id in &plan_order {
        let node = nodes[node_id];
        let dependency_blocked = node.dependency_order.iter().any(|dependency| {
            nodes
                .get(dependency)
                .is_some_and(|dependency| dependency.evidence_state == EvidenceState::Contradicted)
        });
        let dependency_uncertain = node.dependency_order.iter().any(|dependency| {
            nodes.get(dependency).is_some_and(|dependency| {
                matches!(
                    dependency.evidence_state,
                    EvidenceState::Unknown | EvidenceState::Speculative
                )
            })
        });
        let hard = global_block
            || cycle.contains(node_id)
            || node.evidence_state == EvidenceState::Contradicted
            || dependency_blocked;
        let conditional = missing
            .iter()
            .any(|item| item.starts_with(&format!("{}:", node_id)))
            || matches!(
                node.evidence_state,
                EvidenceState::Unknown | EvidenceState::Speculative
            )
            || dependency_uncertain
            || !node.omissions.is_empty()
            || !node.uncertainty.is_empty();
        if hard {
            blocked.push(node_id.clone());
        } else if conditional || node.estimated_cost > spec.budget_units.saturating_sub(spent) {
            unresolved.push(node_id.clone());
            if node.estimated_cost > spec.budget_units.saturating_sub(spent) {
                omissions.insert(format!("{}:budget-ceiling", node_id));
            }
        } else {
            spent = spent.saturating_add(node.estimated_cost);
            completed.push(node_id.clone());
        }
    }
    let disposition = if global_block || !cycle.is_empty() || !missing.is_empty() {
        "blocked"
    } else if !unresolved.is_empty() || !missing.is_empty() || !uncertainty.is_empty() {
        "unresolved"
    } else {
        "qualified"
    };
    let mut exchange = Vec::new();
    if disposition == "qualified" {
        exchange = completed
            .iter()
            .map(|id| format!("exchange:permitted-artifacts:{id}"))
            .collect();
    }
    let study_order = spec.required_study_order.clone();
    let modality_order = spec.required_modality_order.clone();
    let checkpoint_digest = ContentHash::of_value(&json!({"workflow_id": spec.workflow_id, "topological_order": topological, "replay_identity": spec.replay_identity})).map_err(|error| GatewayError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": OUTPUT_SCHEMA, "request_id": spec.request_id, "workflow_id": spec.workflow_id, "plan_order": plan_order, "topological_order": topological, "completed_order": completed, "unresolved_order": unresolved, "blocked_order": blocked, "cycle_order": cycle, "missing_dependency_order": missing, "study_order": study_order, "modality_order": modality_order, "exchange_order": exchange, "checkpoint_digest": checkpoint_digest, "replay_identity": spec.replay_identity, "disposition": disposition});
    let run_digest = ContentHash::of_value(&payload)
        .map_err(|error| GatewayError::Artifact(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("execution-run:{}", spec.workflow_id),
        "application/vnd.aurora.execution-run+json",
        &payload,
        semantic_loss.clone(),
        vec![ProvenanceLink {
            source_id: spec.workflow_id.clone(),
            relation: "graph-execution-gateway".into(),
            digest: run_digest.clone(),
        }],
    )
    .map_err(|error| GatewayError::Artifact(error.to_string()))?;
    let receipt = ExecutionRun {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: spec.request_id.clone(),
        workflow_id: spec.workflow_id.clone(),
        scope: spec.scope.clone(),
        semantic_profile: spec.semantic_profile.clone(),
        disposition: disposition.into(),
        plan_order,
        topological_order: topological,
        completed_order: completed,
        unresolved_order: unresolved,
        blocked_order: blocked,
        cycle_order: cycle,
        missing_dependency_order: missing.into_iter().collect(),
        study_order,
        modality_order,
        exchange_order: exchange,
        replay_identity: spec.replay_identity.clone(),
        checkpoint_digest,
        run_digest,
        semantic_loss,
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        artifact,
        effect_receipts: if disposition == "qualified" {
            vec![format!("exchange:permitted-artifacts:{}", spec.workflow_id)]
        } else {
            vec!["block:unsafe-release".into()]
        },
        raw_data_local: spec.raw_data_local,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash() -> ContentHash {
        ContentHash::of_bytes(b"graph-gateway")
    }
    fn node(id: &str, deps: Vec<String>, state: EvidenceState) -> ExecutionNode {
        ExecutionNode {
            node_id: id.into(),
            study_id: "study-a".into(),
            modality_order: vec!["imaging".into(), "omics".into()],
            dependency_order: deps,
            evidence_state: state,
            artifact_digest: Some(hash()),
            provenance_digest: Some(hash()),
            replay_identity: hash(),
            effect: "compute:local".into(),
            estimated_cost: 2,
            omissions: Vec::new(),
            uncertainty: Vec::new(),
        }
    }
    fn spec() -> ResearchWorkflowSpec {
        ResearchWorkflowSpec {
            request_id: "request:graph".into(),
            workflow_id: "workflow:graph".into(),
            scope: "organoid".into(),
            semantic_profile: "ome-ngff+anndata".into(),
            schema_version: INPUT_SCHEMA.into(),
            nodes: vec![
                node("node-b", vec!["node-a".into()], EvidenceState::Supported),
                node("node-a", Vec::new(), EvidenceState::Proven),
            ],
            required_study_order: vec!["study-a".into()],
            required_modality_order: vec!["imaging".into(), "omics".into()],
            expected_comparability_digest: hash(),
            replay_identity: hash(),
            budget_units: 10,
            max_budget_units: 10,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            permitted_effect_order: vec!["compute:local".into()],
            adversarial_events: Vec::new(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn qualified_graph_is_topological_and_exchange_ready() {
        let run = admit(&spec()).unwrap();
        assert_eq!(run.disposition, "qualified");
        assert_eq!(run.topological_order, vec!["node-a", "node-b"]);
        assert!(run.effect_receipts[0].starts_with("exchange:permitted-artifacts:"));
    }
    #[test]
    fn cycle_is_blocked_with_explicit_nodes() {
        let mut value = spec();
        value.nodes[0].dependency_order = vec!["node-a".into()];
        value.nodes[1].dependency_order = vec!["node-b".into()];
        let run = admit(&value).unwrap();
        assert_eq!(run.disposition, "blocked");
        assert_eq!(run.cycle_order.len(), 2);
    }
    #[test]
    fn missing_dependency_is_unresolved() {
        let mut value = spec();
        value.nodes[0].dependency_order = vec!["missing".into()];
        let run = admit(&value).unwrap();
        assert_eq!(run.disposition, "blocked");
        assert!(!run.missing_dependency_order.is_empty());
    }
    #[test]
    fn unknown_and_contradicted_nodes_never_complete() {
        let mut value = spec();
        value.nodes[0].evidence_state = EvidenceState::Unknown;
        value.nodes[1].evidence_state = EvidenceState::Contradicted;
        let run = admit(&value).unwrap();
        assert!(run.blocked_order.contains(&"node-a".into()));
        assert!(run.blocked_order.contains(&"node-b".into()));
    }
    #[test]
    fn policy_and_approval_fail_closed() {
        let mut value = spec();
        value.policy_allow = false;
        value.signed_approval = false;
        let run = admit(&value).unwrap();
        assert_eq!(run.disposition, "blocked");
        assert_eq!(run.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn manifest_is_a2_and_interoperable() {
        let manifest = capability_manifest();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A2);
        assert!(manifest.surfaces.contains(&ResearchSurface::Protocol));
    }
}

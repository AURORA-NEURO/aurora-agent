//! Deterministic, policy-bound execution of a typed research workflow graph.
//!
//! Atlas feature: `AFA-runtime-P12-F10`.
//!
//! [`ResearchExecutionSession`] is the narrow effect door for one run. This module adds the
//! product-level orchestration around that door: it validates a complete workflow plan, computes
//! one deterministic topological order, preflights authority/evidence/budget requirements, and
//! then appends every admitted node to the same replayable execution run. A dry run executes the
//! exact same preflight and ordering logic but emits no effect or tape entry.
//!
//! The executor intentionally accepts declarative actions rather than closures. That keeps the
//! product boundary serializable for MCP, SDK, and future workflow gateways, and makes an
//! execution claim falsifiable from the resulting `ExecutionRun` and replay bundle.

use crate::research_run::{ResearchExecutionSession, ResearchRuntimeError};
use crate::{Effect, EffectOutcome, EffectRequest};
use bioprism_foundation::{
    AuthorityRequirement, AutonomyGrant, AutonomyTier, CapabilityManifest, Determinism,
    Effect as ResearchEffect, EvidenceReference, EvidenceState, ExecutionRun, ExecutionStatus,
    PolicyDecision, PolicyReceipt, ProvenanceLink, ResearchContractError, ResearchSurface,
    ResearchWorkflowSpec, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::{ContentHash, RunId};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// Stable atlas identity for this implementation slice.
pub const FEATURE_ID: &str = "AFA-runtime-P12-F10";
pub const FEATURE_CONTRACT_VERSION: &str = "0.1.0";

/// Whether the workflow should only be admitted and ordered or actually append effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowExecutionMode {
    DryRun,
    Execute,
}

/// Product-level result state. Validation failures are returned as typed errors and never
/// represented as a successful receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowExecutionStatus {
    DryRun,
    Succeeded,
}

/// One declarative node action. `evidence_payload` is metadata or a content-addressed witness;
/// raw experimental data remains outside this transport object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowAction {
    pub node_id: String,
    pub event_type: String,
    pub effect: ResearchEffect,
    pub resource: String,
    pub cost: f64,
    #[serde(default)]
    pub evidence_payload: Option<Value>,
}

/// Complete input to the workflow execution product surface.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowExecutionRequest {
    pub workflow: ResearchWorkflowSpec,
    pub manifest: CapabilityManifest,
    pub grant: AutonomyGrant,
    pub policy: PolicyReceipt,
    pub run_id: RunId,
    pub actions: Vec<WorkflowAction>,
    pub mode: WorkflowExecutionMode,
}

/// Deterministic receipt joining ordered nodes, the typed execution run, budget accounting, and
/// a content-addressed artifact. The replay bundle remains available from the local execution
/// boundary; this compact receipt is the cross-language transport contract.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowExecutionReceipt {
    pub schema_version: String,
    pub feature_id: String,
    pub workflow_id: String,
    pub mode: WorkflowExecutionMode,
    pub status: WorkflowExecutionStatus,
    pub ordered_nodes: Vec<String>,
    pub completed_nodes: Vec<String>,
    pub run: ExecutionRun,
    pub run_digest: ContentHash,
    pub remaining_budget: BTreeMap<String, f64>,
    pub artifact: TypedResearchArtifact,
    pub reasons: Vec<String>,
    pub boundary: String,
}

impl WorkflowExecutionReceipt {
    pub fn digest(&self) -> Result<ContentHash, WorkflowExecutionError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| WorkflowExecutionError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| WorkflowExecutionError::Serialization(error.to_string()))
    }

    pub fn validate(&self) -> Result<(), WorkflowExecutionError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION {
            return Err(WorkflowExecutionError::Contract(
                ResearchContractError::SchemaVersion {
                    expected: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
                    found: self.schema_version.clone(),
                },
            ));
        }
        if self.feature_id != FEATURE_ID {
            return Err(WorkflowExecutionError::InvalidRequest(
                "workflow execution feature id mismatch".into(),
            ));
        }
        if self.workflow_id.trim().is_empty() || self.ordered_nodes.is_empty() {
            return Err(WorkflowExecutionError::InvalidRequest(
                "workflow execution receipt needs workflow and ordered nodes".into(),
            ));
        }
        if self.completed_nodes.len() > self.ordered_nodes.len()
            || !self
                .completed_nodes
                .iter()
                .all(|node| self.ordered_nodes.contains(node))
        {
            return Err(WorkflowExecutionError::InvalidRequest(
                "completed nodes are not a subset of the ordered plan".into(),
            ));
        }
        let mut unique_nodes = BTreeSet::new();
        if self
            .ordered_nodes
            .iter()
            .any(|node| node.trim().is_empty() || !unique_nodes.insert(node))
        {
            return Err(WorkflowExecutionError::InvalidRequest(
                "ordered workflow nodes must be non-empty and unique".into(),
            ));
        }
        match self.status {
            WorkflowExecutionStatus::DryRun if !self.completed_nodes.is_empty() => {
                return Err(WorkflowExecutionError::InvalidRequest(
                    "dry-run workflow receipts cannot contain completed nodes".into(),
                ));
            }
            WorkflowExecutionStatus::Succeeded if self.completed_nodes != self.ordered_nodes => {
                return Err(WorkflowExecutionError::InvalidRequest(
                    "successful workflow receipts must complete the ordered plan".into(),
                ));
            }
            _ => {}
        }
        if self.run.workflow_id != self.workflow_id {
            return Err(WorkflowExecutionError::InvalidRequest(
                "execution run and receipt workflow ids differ".into(),
            ));
        }
        self.run.validate()?;
        let expected_status = match self.status {
            WorkflowExecutionStatus::DryRun => ExecutionStatus::Planned,
            WorkflowExecutionStatus::Succeeded => ExecutionStatus::Succeeded,
        };
        if self.run.status != expected_status {
            return Err(WorkflowExecutionError::InvalidRequest(
                "receipt status and execution run status differ".into(),
            ));
        }
        if self.run.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.run.boundary != PRECLINICAL_BOUNDARY
            || self.run.events.len()
                != match self.status {
                    WorkflowExecutionStatus::DryRun => 0,
                    WorkflowExecutionStatus::Succeeded => self.ordered_nodes.len(),
                }
            || self
                .run
                .events
                .iter()
                .enumerate()
                .any(|(sequence, event)| event.sequence != sequence as u64)
            || self.run_digest == ContentHash::of_bytes(b"")
        {
            return Err(WorkflowExecutionError::InvalidRequest(
                "workflow execution run evidence is incomplete or inconsistent with its status"
                    .into(),
            ));
        }
        if self.reasons.is_empty() {
            return Err(WorkflowExecutionError::InvalidRequest(
                "workflow execution receipt needs a reason".into(),
            ));
        }
        if self.boundary != PRECLINICAL_BOUNDARY {
            return Err(WorkflowExecutionError::Contract(
                ResearchContractError::BoundaryMismatch {
                    capability: self.workflow_id.clone(),
                },
            ));
        }
        if self.artifact.artifact_id != format!("{}:{}", FEATURE_ID, self.workflow_id)
            || self.artifact.content_type != "application/vnd.aurora.workflow-execution+json"
            || !self.artifact.semantic_loss.is_empty()
            || self.artifact.provenance
                != vec![ProvenanceLink {
                    source_id: "execution-run".into(),
                    relation: "derived-from".into(),
                    digest: self.run_digest.clone(),
                }]
        {
            return Err(WorkflowExecutionError::InvalidRequest(
                "workflow execution artifact is not bound to its run".into(),
            ));
        }
        self.artifact.validate_metadata()?;
        let payload = json!({
            "feature_id": FEATURE_ID,
            "workflow_id": self.workflow_id,
            "mode": self.mode,
            "status": self.status,
            "ordered_nodes": self.ordered_nodes,
            "completed_nodes": self.completed_nodes,
            "run_digest": self.run_digest,
            "remaining_budget": self.remaining_budget,
            "reasons": self.reasons,
            "boundary": PRECLINICAL_BOUNDARY,
        });
        self.artifact
            .verify_payload(&payload)
            .map_err(WorkflowExecutionError::Contract)?;
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum WorkflowExecutionError {
    #[error("research contract rejected workflow execution: {0}")]
    Contract(#[from] ResearchContractError),
    #[error("workflow execution runtime rejected the run: {0}")]
    Runtime(#[from] ResearchRuntimeError),
    #[error("invalid workflow execution request: {0}")]
    InvalidRequest(String),
    #[error("workflow action is missing for node {0}")]
    MissingAction(String),
    #[error("workflow action is duplicated for node {0}")]
    DuplicateAction(String),
    #[error("workflow action references unknown node {0}")]
    UnknownNode(String),
    #[error("workflow action for node {node} has no matching approval")]
    MissingApproval { node: String },
    #[error("workflow action for node {node} requires evidence payload")]
    MissingEvidence { node: String },
    #[error("workflow resource {resource} is not present in the autonomy grant")]
    MissingBudget { resource: String },
    #[error("workflow resource {resource} is not declared by the workflow budget")]
    MissingWorkflowBudget { resource: String },
    #[error("workflow resource {resource} exceeds its grant budget: requested {requested}, available {available}")]
    BudgetExceeded {
        resource: String,
        requested: f64,
        available: f64,
    },
    #[error("workflow graph could not be deterministically ordered")]
    CyclicGraph,
    #[error("cannot serialize workflow execution: {0}")]
    Serialization(String),
}

/// Capability manifest for the MCP/SDK/API workflow execution surface.
pub fn workflow_execution_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: FEATURE_CONTRACT_VERSION.into(),
        owner_crate: "runtime".into(),
        consumers: [
            "laboratory automation engineer".into(),
            "workflow operator".into(),
            "downstream execution gateway".into(),
        ]
        .into(),
        behavior: "preflights, orders, and executes a bounded typed research workflow with replayable effects".into(),
        value: "turns a workflow graph into a deterministic, authority- and budget-bound execution receipt without hiding omissions or failed effects".into(),
        inputs: vec![TypedPort {
            name: "workflow_execution_request".into(),
            schema: "WorkflowExecutionRequest@1".into(),
            required: true,
        }],
        outputs: vec![TypedPort {
            name: "workflow_execution_receipt".into(),
            schema: "WorkflowExecutionReceipt@1".into(),
            required: true,
        }],
        effects: [
            ResearchEffect::ReadLocalData,
            ResearchEffect::WriteLocalArtifact,
            ResearchEffect::ExecuteLocalComputation,
            ResearchEffect::ExternalDataAccess,
            ResearchEffect::FederationExport,
            ResearchEffect::InstrumentExecution,
        ]
        .into(),
        permissions: ["invoke:declared-tools".into(), "write:local-research-artifact".into()]
            .into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference {
            source_id: "mcp-2025-06-18".into(),
            state: EvidenceState::Supported,
            locator: Some("https://modelcontextprotocol.io/specification/2025-06-18/basic/index".into()),
        }],
        authority_requirements: vec![AuthorityRequirement {
            role: "institutional workflow approver".into(),
            reason: "A2 workflow execution may access external research tools or produce durable artifacts".into(),
        }],
        autonomy_tier: AutonomyTier::A2,
        surfaces: [
            ResearchSurface::Cli,
            ResearchSurface::Api,
            ResearchSurface::Sdk,
            ResearchSurface::McpTool,
            ResearchSurface::Operator,
        ]
        .into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

/// Execute or dry-run a complete workflow graph using one policy- and authority-bound session.
pub fn execute_workflow(
    request: &WorkflowExecutionRequest,
) -> Result<WorkflowExecutionReceipt, WorkflowExecutionError> {
    let (ordered_nodes, actions, mut remaining_budget) = preflight(request)?;
    let mut session = ResearchExecutionSession::new(
        request.manifest.clone(),
        request.workflow.clone(),
        request.grant.clone(),
        request.policy.clone(),
        request.run_id.clone(),
    )?;

    let status = match request.mode {
        WorkflowExecutionMode::DryRun => WorkflowExecutionStatus::DryRun,
        WorkflowExecutionMode::Execute => {
            let mut completed = Vec::new();
            let mut checkpointed = BTreeSet::new();
            for node_id in &ordered_nodes {
                let Some(action) = actions.get(node_id) else {
                    return Err(WorkflowExecutionError::MissingAction(node_id.clone()));
                };
                if let Some(available) = remaining_budget.get_mut(&action.resource) {
                    *available -= action.cost;
                    if available.abs() < 1e-12 {
                        *available = 0.0;
                    }
                }
                let payload = action.evidence_payload.clone().unwrap_or(Value::Null);
                session.append_effect(
                    action.event_type.clone(),
                    action.effect,
                    Effect::performed(
                        EffectRequest::ServiceCall {
                            service: "aurora-research-workflow".into(),
                            operation: action.event_type.clone(),
                            request: payload,
                        },
                        EffectOutcome::new(json!({
                            "accepted": true,
                            "node_id": action.node_id,
                        })),
                    ),
                    action.evidence_payload.as_ref(),
                )?;
                completed.push(node_id.clone());
                for checkpoint in &request.workflow.checkpoints {
                    if !checkpointed.contains(&checkpoint.checkpoint_id)
                        && checkpoint
                            .after_nodes
                            .iter()
                            .all(|id| completed.iter().any(|done| done == id))
                    {
                        session.checkpoint(checkpoint.checkpoint_id.clone())?;
                        checkpointed.insert(checkpoint.checkpoint_id.clone());
                    }
                }
            }
            session.finish(ExecutionStatus::Succeeded)?;
            WorkflowExecutionStatus::Succeeded
        }
    };

    let completed_nodes = if status == WorkflowExecutionStatus::Succeeded {
        ordered_nodes.clone()
    } else {
        Vec::new()
    };
    let bundle = session.bundle()?;
    let run_digest = bundle.digest()?;
    let reasons = match status {
        WorkflowExecutionStatus::DryRun => {
            vec!["workflow preflight passed; dry-run emitted no effects or tape entries".into()]
        }
        WorkflowExecutionStatus::Succeeded => {
            vec!["all ordered workflow actions were admitted, recorded, and replay-bound".into()]
        }
    };
    let payload = json!({
        "feature_id": FEATURE_ID,
        "workflow_id": request.workflow.workflow_id,
        "mode": request.mode,
        "status": status,
        "ordered_nodes": ordered_nodes,
        "completed_nodes": completed_nodes,
        "run_digest": run_digest,
        "remaining_budget": remaining_budget,
        "reasons": reasons,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!("{}:{}", FEATURE_ID, request.workflow.workflow_id),
        "application/vnd.aurora.workflow-execution+json",
        &payload,
        Vec::new(),
        vec![ProvenanceLink {
            source_id: "execution-run".into(),
            relation: "derived-from".into(),
            digest: run_digest.clone(),
        }],
    )?;
    let receipt = WorkflowExecutionReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        workflow_id: request.workflow.workflow_id.clone(),
        mode: request.mode,
        status,
        ordered_nodes,
        completed_nodes,
        run: bundle.run.clone(),
        run_digest,
        remaining_budget,
        artifact,
        reasons,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

type PreflightState = (
    Vec<String>,
    BTreeMap<String, WorkflowAction>,
    BTreeMap<String, f64>,
);

fn preflight(request: &WorkflowExecutionRequest) -> Result<PreflightState, WorkflowExecutionError> {
    if request.manifest.capability_id != FEATURE_ID {
        return Err(WorkflowExecutionError::InvalidRequest(
            "manifest capability id must match workflow execution feature".into(),
        ));
    }
    request.workflow.validate()?;
    request.manifest.validate()?;
    request.grant.validate()?;
    request.policy.validate()?;
    if matches!(
        request.policy.decision,
        PolicyDecision::Deny
            | PolicyDecision::Redact
            | PolicyDecision::ApprovalRequired
            | PolicyDecision::Unresolved
    ) {
        return Err(WorkflowExecutionError::InvalidRequest(
            "policy decision does not admit workflow execution".into(),
        ));
    }
    if request.workflow.nodes.is_empty() {
        return Err(WorkflowExecutionError::InvalidRequest(
            "workflow must contain at least one node".into(),
        ));
    }
    if request.workflow.autonomy_tier > request.grant.autonomy_tier {
        return Err(WorkflowExecutionError::InvalidRequest(
            "workflow autonomy tier exceeds grant".into(),
        ));
    }

    let node_ids: BTreeSet<String> = request
        .workflow
        .nodes
        .iter()
        .map(|node| node.node_id.clone())
        .collect();
    let mut actions = BTreeMap::new();
    let mut requested = BTreeMap::<String, f64>::new();
    for action in &request.actions {
        if !node_ids.contains(&action.node_id) {
            return Err(WorkflowExecutionError::UnknownNode(action.node_id.clone()));
        }
        if actions
            .insert(action.node_id.clone(), action.clone())
            .is_some()
        {
            return Err(WorkflowExecutionError::DuplicateAction(
                action.node_id.clone(),
            ));
        }
        if action.event_type.trim().is_empty() || action.resource.trim().is_empty() {
            return Err(WorkflowExecutionError::InvalidRequest(
                "workflow actions require event_type and resource".into(),
            ));
        }
        if !action.cost.is_finite() || action.cost < 0.0 {
            return Err(WorkflowExecutionError::InvalidRequest(format!(
                "workflow action {} has invalid cost",
                action.node_id
            )));
        }
        if !request.manifest.effects.contains(&action.effect) {
            return Err(WorkflowExecutionError::InvalidRequest(format!(
                "workflow action {} requests an undeclared effect",
                action.node_id
            )));
        }
        if action.effect == ResearchEffect::InstrumentExecution && action.evidence_payload.is_none()
        {
            return Err(WorkflowExecutionError::MissingEvidence {
                node: action.node_id.clone(),
            });
        }
        let requested_amount = requested.entry(action.resource.clone()).or_default();
        *requested_amount += action.cost;
        if !requested_amount.is_finite() {
            return Err(WorkflowExecutionError::InvalidRequest(format!(
                "workflow action costs overflow for resource {}",
                action.resource
            )));
        }
    }
    for node in &request.workflow.nodes {
        if !actions.contains_key(&node.node_id) {
            return Err(WorkflowExecutionError::MissingAction(node.node_id.clone()));
        }
        if node.requires_approval {
            let Some(action) = actions.get(&node.node_id) else {
                return Err(WorkflowExecutionError::MissingAction(node.node_id.clone()));
            };
            if !request.workflow.approvals.iter().any(|approval| {
                approval.actor == node.actor && approval.action == action.event_type
            }) {
                return Err(WorkflowExecutionError::MissingApproval {
                    node: node.node_id.clone(),
                });
            }
        }
    }
    let remaining_budget = request.grant.resource_budget.clone();
    let workflow_budget: BTreeMap<String, f64> = request
        .workflow
        .budgets
        .iter()
        .map(|budget| (budget.resource.clone(), budget.amount))
        .collect();
    for (resource, amount) in requested {
        let declared = workflow_budget.get(&resource).copied().ok_or_else(|| {
            WorkflowExecutionError::MissingWorkflowBudget {
                resource: resource.clone(),
            }
        })?;
        if amount > declared {
            return Err(WorkflowExecutionError::BudgetExceeded {
                resource: resource.clone(),
                requested: amount,
                available: declared,
            });
        }
        let available = remaining_budget.get(&resource).copied().ok_or_else(|| {
            WorkflowExecutionError::MissingBudget {
                resource: resource.clone(),
            }
        })?;
        if amount > available {
            return Err(WorkflowExecutionError::BudgetExceeded {
                resource,
                requested: amount,
                available,
            });
        }
    }

    let ordered_nodes = topological_order(&request.workflow)?;
    // Ensure the grant has the same effect action names that the session will check.
    for action in actions.values() {
        let name = effect_name(action.effect);
        if !request.grant.permitted_actions.contains("*")
            && !request.grant.permitted_actions.contains(name)
        {
            return Err(WorkflowExecutionError::InvalidRequest(format!(
                "grant does not permit effect {} for node {}",
                name, action.node_id
            )));
        }
    }
    Ok((ordered_nodes, actions, remaining_budget))
}

fn topological_order(
    workflow: &ResearchWorkflowSpec,
) -> Result<Vec<String>, WorkflowExecutionError> {
    let mut indegree: BTreeMap<String, usize> = workflow
        .nodes
        .iter()
        .map(|node| (node.node_id.clone(), 0))
        .collect();
    let mut adjacency: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for edge in &workflow.edges {
        adjacency
            .entry(edge.from.clone())
            .or_default()
            .insert(edge.to.clone());
    }
    for successors in adjacency.values() {
        for successor in successors {
            *indegree
                .get_mut(successor)
                .ok_or(WorkflowExecutionError::CyclicGraph)? += 1;
        }
    }
    let mut ready: BTreeSet<String> = indegree
        .iter()
        .filter_map(|(node, degree)| (*degree == 0).then_some(node.clone()))
        .collect();
    let mut order = Vec::with_capacity(indegree.len());
    while let Some(node) = ready.iter().next().cloned() {
        ready.remove(&node);
        order.push(node.clone());
        if let Some(successors) = adjacency.get(&node) {
            for successor in successors {
                let Some(degree) = indegree.get_mut(successor) else {
                    return Err(WorkflowExecutionError::CyclicGraph);
                };
                *degree -= 1;
                if *degree == 0 {
                    ready.insert(successor.clone());
                }
            }
        }
    }
    if order.len() != indegree.len() {
        return Err(WorkflowExecutionError::CyclicGraph);
    }
    Ok(order)
}

fn effect_name(effect: ResearchEffect) -> &'static str {
    match effect {
        ResearchEffect::ReadLocalData => "read_local_data",
        ResearchEffect::WriteLocalArtifact => "write_local_artifact",
        ResearchEffect::ExecuteLocalComputation => "execute_local_computation",
        ResearchEffect::ExternalDataAccess => "external_data_access",
        ResearchEffect::FederationExport => "federation_export",
        ResearchEffect::InstrumentExecution => "instrument_execution",
        ResearchEffect::ConsumeMaterial => "consume_material",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bioprism_foundation::{
        ApprovalRequirement, Compensation, ExecutionStatus, PolicyDecision, ResourceBudget,
        WorkflowCheckpoint, WorkflowEdge, WorkflowNode,
    };
    use serde_json::json;

    fn request(mode: WorkflowExecutionMode) -> WorkflowExecutionRequest {
        let workflow = ResearchWorkflowSpec {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            workflow_id: "workflow:multimodal-demo".into(),
            intent: "replayable preclinical computation".into(),
            nodes: vec![
                WorkflowNode {
                    node_id: "b-compute".into(),
                    capability_id: FEATURE_ID.into(),
                    actor: "operator".into(),
                    requires_approval: false,
                },
                WorkflowNode {
                    node_id: "a-read".into(),
                    capability_id: FEATURE_ID.into(),
                    actor: "operator".into(),
                    requires_approval: false,
                },
            ],
            edges: vec![WorkflowEdge {
                from: "a-read".into(),
                to: "b-compute".into(),
            }],
            checkpoints: vec![WorkflowCheckpoint {
                checkpoint_id: "after-read".into(),
                after_nodes: ["a-read".into()].into(),
            }],
            budgets: vec![ResourceBudget {
                resource: "cpu_seconds".into(),
                amount: 10.0,
            }],
            compensations: vec![Compensation {
                effect: "write_local_artifact".into(),
                action: "retain_partial_artifact".into(),
            }],
            approvals: Vec::new(),
            autonomy_tier: AutonomyTier::A2,
            boundary: PRECLINICAL_BOUNDARY.into(),
        };
        let manifest = workflow_execution_manifest();
        let grant = AutonomyGrant {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            actor: "operator".into(),
            permitted_actions: ["read_local_data".into(), "execute_local_computation".into()]
                .into(),
            resource_budget: [("cpu_seconds".into(), 10.0)].into(),
            scope: "study:demo".into(),
            expires_at: "2099-01-01T00:00:00Z".into(),
            revoked: false,
            autonomy_tier: AutonomyTier::A2,
            approval_reference: Some("approval:demo".into()),
            boundary: PRECLINICAL_BOUNDARY.into(),
        };
        let policy = PolicyReceipt {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            receipt_id: "policy:demo".into(),
            decision: PolicyDecision::Allow,
            reasons: vec!["local preclinical workflow is within scope".into()],
            evaluated_artifacts: Vec::new(),
            authority_reference: Some("approval:demo".into()),
            boundary: PRECLINICAL_BOUNDARY.into(),
        };
        WorkflowExecutionRequest {
            workflow,
            manifest,
            grant,
            policy,
            run_id: RunId::parse("workflow-run-demo").unwrap(),
            actions: vec![
                WorkflowAction {
                    node_id: "b-compute".into(),
                    event_type: "compute:multimodal".into(),
                    effect: ResearchEffect::ExecuteLocalComputation,
                    resource: "cpu_seconds".into(),
                    cost: 3.0,
                    evidence_payload: Some(json!({"input_digest": "abc"})),
                },
                WorkflowAction {
                    node_id: "a-read".into(),
                    event_type: "read:study".into(),
                    effect: ResearchEffect::ReadLocalData,
                    resource: "cpu_seconds".into(),
                    cost: 1.0,
                    evidence_payload: None,
                },
            ],
            mode,
        }
    }

    #[test]
    fn deterministic_order_and_replay_bound_success() {
        let receipt = execute_workflow(&request(WorkflowExecutionMode::Execute)).unwrap();
        assert_eq!(receipt.status, WorkflowExecutionStatus::Succeeded);
        assert_eq!(receipt.ordered_nodes, ["a-read", "b-compute"]);
        assert_eq!(receipt.completed_nodes, receipt.ordered_nodes);
        assert_eq!(receipt.run.status, ExecutionStatus::Succeeded);
        assert_eq!(receipt.run.events.len(), 2);
        assert_eq!(receipt.run.checkpoints.len(), 1);
        assert!(receipt.digest().is_ok());
    }

    #[test]
    fn dry_run_has_identical_order_but_no_effects() {
        let receipt = execute_workflow(&request(WorkflowExecutionMode::DryRun)).unwrap();
        assert_eq!(receipt.status, WorkflowExecutionStatus::DryRun);
        assert_eq!(receipt.ordered_nodes, ["a-read", "b-compute"]);
        assert!(receipt.completed_nodes.is_empty());
        assert_eq!(receipt.run.status, ExecutionStatus::Planned);
        assert!(receipt.run.events.is_empty());
    }

    #[test]
    fn budget_is_preflighted_before_any_effect() {
        let mut request = request(WorkflowExecutionMode::Execute);
        request
            .grant
            .resource_budget
            .insert("cpu_seconds".into(), 2.0);
        let error = execute_workflow(&request).unwrap_err();
        assert!(matches!(
            error,
            WorkflowExecutionError::BudgetExceeded { .. }
        ));
    }

    #[test]
    fn budget_preflight_does_not_allow_tolerance_based_overspend() {
        let mut request = request(WorkflowExecutionMode::Execute);
        request.workflow.budgets[0].amount = 4.0;
        request
            .grant
            .resource_budget
            .insert("cpu_seconds".into(), 4.0);
        request.actions[1].cost = 2.0000000000001;

        assert!(matches!(
            execute_workflow(&request).unwrap_err(),
            WorkflowExecutionError::BudgetExceeded { .. }
        ));
    }

    #[test]
    fn overflowing_budget_totals_are_rejected_before_execution() {
        let mut request = request(WorkflowExecutionMode::DryRun);
        request.actions[0].cost = f64::MAX;
        request.actions[1].cost = f64::MAX;

        assert!(matches!(
            execute_workflow(&request).unwrap_err(),
            WorkflowExecutionError::InvalidRequest(_)
        ));
    }

    #[test]
    fn missing_node_action_and_approval_are_fail_closed() {
        let mut missing = request(WorkflowExecutionMode::Execute);
        missing.actions.pop();
        assert!(matches!(
            execute_workflow(&missing).unwrap_err(),
            WorkflowExecutionError::MissingAction(_)
        ));

        let mut approval = request(WorkflowExecutionMode::Execute);
        approval.workflow.nodes[0].requires_approval = true;
        approval.workflow.approvals = vec![ApprovalRequirement {
            approval_id: "approval:wrong".into(),
            actor: "other-operator".into(),
            action: "other-action".into(),
        }];
        assert!(matches!(
            execute_workflow(&approval).unwrap_err(),
            WorkflowExecutionError::MissingApproval { .. }
        ));
    }

    #[test]
    fn instrument_action_requires_evidence_before_execution() {
        let mut request = request(WorkflowExecutionMode::DryRun);
        request.actions[0].effect = ResearchEffect::InstrumentExecution;
        request.actions[0].evidence_payload = None;
        assert!(matches!(
            execute_workflow(&request).unwrap_err(),
            WorkflowExecutionError::MissingEvidence { .. }
        ));
    }

    #[test]
    fn receipt_rejects_tampered_artifact_payload() {
        let mut receipt = execute_workflow(&request(WorkflowExecutionMode::Execute)).unwrap();
        receipt.artifact.content_hash = ContentHash::of_bytes(b"tampered");
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn receipt_rejects_tampered_execution_run() {
        let mut receipt = execute_workflow(&request(WorkflowExecutionMode::Execute)).unwrap();
        receipt.run.events[0].event_type.clear();
        assert!(receipt.validate().is_err());
    }
}

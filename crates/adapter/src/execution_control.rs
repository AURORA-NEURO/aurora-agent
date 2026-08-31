//! Federated computational-execution control plane for typed research workflows.
//!
//! Atlas feature: `AFA-adapter-P12-F31`.
//!
//! This product admits a [`ResearchWorkflowSpec`] to an institution-local executor. It validates
//! the graph, computes one deterministic topological order, binds the plan to an [`ExecutionRun`],
//! and emits an effect receipt for every authorized local-computation node. The control plane does
//! not run code, contact instruments, or export raw data: a planned run is never represented as a
//! completed scientific computation. Missing approval, policy denial, malformed closure, and
//! replay identity remain machine-checkable outcomes.

use bioprism_foundation::{
    ApprovalRequirement, Compensation, Effect, ExecutionEvent, ExecutionRun, ExecutionStatus,
    ResearchWorkflowSpec, ResourceBudget, SemanticLoss, TypedResearchArtifact, WorkflowCheckpoint,
    WorkflowEdge, WorkflowNode, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::{ContentHash, RunId};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P12-F31";
pub const CONTRACT_VERSION: &str = "computational-execution-control-plane/1.0";
const MAX_TEXT_BYTES: usize = 512;
const MAX_NODES: usize = 8192;
const MAX_ITEMS: usize = 16384;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionAdmissionMode {
    DryRun,
    Admit,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComputationalExecutionRequest {
    pub request_id: String,
    pub workflow: ResearchWorkflowSpec,
    pub run_id: RunId,
    pub mode: ExecutionAdmissionMode,
    pub policy_allow: bool,
    pub authorization_reference: Option<String>,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionControlDecision {
    DryRun,
    Admitted,
    ApprovalRequired,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorizedExecutionEffect {
    pub node_id: String,
    pub effect: Effect,
    pub authorized: bool,
    pub executed: bool,
    pub payload_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComputationalExecutionReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub input: ComputationalExecutionRequest,
    pub input_digest: ContentHash,
    pub request_id: String,
    pub workflow_id: String,
    pub run_id: RunId,
    pub mode: ExecutionAdmissionMode,
    pub policy_allow: bool,
    pub approval_required: bool,
    pub authorization_reference: Option<String>,
    pub decision: ExecutionControlDecision,
    pub ordered_nodes: Vec<String>,
    pub admitted_nodes: Vec<String>,
    pub run: ExecutionRun,
    pub run_digest: ContentHash,
    pub authorized_effects: Vec<AuthorizedExecutionEffect>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub semantic_loss: Vec<SemanticLoss>,
    pub reasons: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub effects_executed: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

impl ComputationalExecutionReceipt {
    pub fn validate(&self) -> Result<(), ExecutionControlError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
        {
            return Err(ExecutionControlError::Contract(
                "computational execution contract identity mismatch".into(),
            ));
        }
        if self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.ordered_nodes.is_empty()
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.effects_executed
            || self.reasons.is_empty()
        {
            return Err(ExecutionControlError::InvalidRequest(
                "execution identity, ordered graph, locality, non-execution, boundary, and reasons are required".into(),
            ));
        }
        validate_text("request_id", &self.request_id)?;
        validate_text("workflow_id", &self.workflow_id)?;
        validate_text("boundary", &self.boundary)?;
        if let Some(reference) = &self.authorization_reference {
            validate_text("authorization_reference", reference)?;
        }
        if self.ordered_nodes.len() > MAX_NODES || self.admitted_nodes.len() > MAX_NODES {
            return Err(ExecutionControlError::InvalidRequest(
                "execution node count is outside its bounded contract".into(),
            ));
        }
        validate_unique_strings("ordered_nodes", &self.ordered_nodes)?;
        validate_unique_strings("admitted_nodes", &self.admitted_nodes)?;
        if self
            .admitted_nodes
            .iter()
            .any(|node| !self.ordered_nodes.contains(node))
            || !is_subsequence(&self.ordered_nodes, &self.admitted_nodes)
        {
            return Err(ExecutionControlError::InvalidRequest(
                "execution node identities must be unique and admitted nodes must be planned nodes"
                    .into(),
            ));
        }
        if self.run.workflow_id != self.workflow_id
            || self.run.run_id != self.run_id
            || self.run.status != ExecutionStatus::Planned
        {
            return Err(ExecutionControlError::InvalidRequest(
                "execution run identity or planned status does not match receipt".into(),
            ));
        }
        let expected_decision = if !self.policy_allow {
            ExecutionControlDecision::Blocked
        } else if self.approval_required && self.authorization_reference.is_none() {
            ExecutionControlDecision::ApprovalRequired
        } else if self.mode == ExecutionAdmissionMode::DryRun {
            ExecutionControlDecision::DryRun
        } else {
            ExecutionControlDecision::Admitted
        };
        if self.decision != expected_decision {
            return Err(ExecutionControlError::InvalidRequest(
                "execution decision does not match its mode, policy, and approval gates".into(),
            ));
        }
        if self.run.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.run.boundary != PRECLINICAL_BOUNDARY
            || self.run.plan_hash == ContentHash::of_bytes(b"")
        {
            return Err(ExecutionControlError::InvalidRequest(
                "planned execution run metadata is incomplete".into(),
            ));
        }
        let expected_replay_identity = ContentHash::of_bytes(
            format!("{}:{}", self.workflow_id, self.run.plan_hash).as_bytes(),
        );
        if self.run.replay_identity != expected_replay_identity {
            return Err(ExecutionControlError::InvalidRequest(
                "planned execution replay identity does not match the workflow plan".into(),
            ));
        }
        if self.decision == ExecutionControlDecision::Admitted
            && self.admitted_nodes != self.ordered_nodes
        {
            return Err(ExecutionControlError::InvalidRequest(
                "admitted execution must authorize the complete topological plan".into(),
            ));
        }
        if self.decision == ExecutionControlDecision::Admitted
            && self.authorized_effects.len() != self.admitted_nodes.len()
        {
            return Err(ExecutionControlError::InvalidRequest(
                "every admitted node needs one authorized effect receipt".into(),
            ));
        }
        if self.decision != ExecutionControlDecision::Admitted
            && (!self.authorized_effects.is_empty() || !self.admitted_nodes.is_empty())
        {
            return Err(ExecutionControlError::InvalidRequest(
                "non-admitted execution cannot contain admitted nodes or authorized effects".into(),
            ));
        }
        if self.authorized_effects.iter().any(|effect| {
            !effect.authorized
                || effect.executed
                || effect.effect != Effect::ExecuteLocalComputation
        }) {
            return Err(ExecutionControlError::InvalidRequest(
                "authorized execution effects must be local, not executed, and replay-bound".into(),
            ));
        }
        if self.authorized_effects.len() > MAX_NODES
            || self
                .authorized_effects
                .iter()
                .zip(&self.admitted_nodes)
                .any(|(effect, node)| &effect.node_id != node)
        {
            return Err(ExecutionControlError::InvalidRequest(
                "authorized effects must align with admitted node order".into(),
            ));
        }
        if self.omissions != canonical_omissions(self.decision)
            || self.uncertainty != canonical_uncertainty(self.decision)
            || self.reasons != canonical_reasons(self.decision)
            || self.semantic_loss != canonical_semantic_loss(self.decision)
        {
            return Err(ExecutionControlError::InvalidRequest(
                "execution outcome notes are not bound to the admission decision".into(),
            ));
        }
        for effect in &self.authorized_effects {
            validate_text("authorized_effect.node_id", &effect.node_id)?;
            if effect.payload_digest == ContentHash::of_bytes(b"") {
                return Err(ExecutionControlError::InvalidRequest(
                    "authorized effect payload digest is required".into(),
                ));
            }
            let payload = json!({
                "workflow_id": self.workflow_id,
                "node_id": effect.node_id,
                "effect": "execute_local_computation",
                "executed": false,
            });
            let expected_payload_digest = ContentHash::of_value(&payload)
                .map_err(|error| ExecutionControlError::Serialization(error.to_string()))?;
            if effect.payload_digest != expected_payload_digest {
                return Err(ExecutionControlError::InvalidRequest(
                    "authorized effect payload digest does not match its node".into(),
                ));
            }
        }
        let expected_checkpoint_count = if self.decision == ExecutionControlDecision::Admitted {
            1
        } else {
            0
        };
        if self.run.events.len() != self.authorized_effects.len()
            || self.run.checkpoints.len() != expected_checkpoint_count
        {
            return Err(ExecutionControlError::InvalidRequest(
                "planned run events and admission checkpoint do not match the decision".into(),
            ));
        }
        for (sequence, (event, effect)) in self
            .run
            .events
            .iter()
            .zip(&self.authorized_effects)
            .enumerate()
        {
            if event.sequence != sequence as u64
                || event.event_type != "local-computation-admitted"
                || event.effect != Some(Effect::ExecuteLocalComputation)
                || event.payload_hash.as_ref() != Some(&effect.payload_digest)
            {
                return Err(ExecutionControlError::InvalidRequest(
                    "planned run event is not bound to its authorized local effect".into(),
                ));
            }
        }
        if self.decision == ExecutionControlDecision::Admitted {
            let checkpoint = self.run.checkpoints.first().ok_or_else(|| {
                ExecutionControlError::InvalidRequest(
                    "admitted execution is missing its admission checkpoint".into(),
                )
            })?;
            let event_value = serde_json::to_value(&self.run.events)
                .map_err(|error| ExecutionControlError::Serialization(error.to_string()))?;
            let expected_replay_hash = ContentHash::of_value(&event_value)
                .map_err(|error| ExecutionControlError::Serialization(error.to_string()))?;
            if checkpoint.checkpoint_id != "admission-boundary"
                || checkpoint.event_sequence != self.run.events.len() as u64
                || checkpoint.replay_hash != expected_replay_hash
            {
                return Err(ExecutionControlError::InvalidRequest(
                    "admission checkpoint is not bound to the planned events".into(),
                ));
            }
        }
        if self.run_digest == ContentHash::of_bytes(b"") {
            return Err(ExecutionControlError::InvalidRequest(
                "execution run digest is required".into(),
            ));
        }
        let expected_run_digest = ContentHash::of_value(
            &serde_json::to_value(&self.run)
                .map_err(|error| ExecutionControlError::Serialization(error.to_string()))?,
        )
        .map_err(|error| ExecutionControlError::Serialization(error.to_string()))?;
        if self.run_digest != expected_run_digest {
            return Err(ExecutionControlError::InvalidRequest(
                "execution run digest does not match the planned run".into(),
            ));
        }
        validate_sorted_strings("omissions", &self.omissions)?;
        validate_sorted_strings("uncertainty", &self.uncertainty)?;
        validate_sorted_strings("reasons", &self.reasons)?;
        for loss in &self.semantic_loss {
            validate_text("semantic_loss.field", &loss.field)?;
            validate_text("semantic_loss.reason", &loss.reason)?;
        }
        if self.semantic_loss.windows(2).any(|pair| {
            (
                pair[0].field.as_str(),
                pair[0].reason.as_str(),
                pair[0].severity,
            ) >= (
                pair[1].field.as_str(),
                pair[1].reason.as_str(),
                pair[1].severity,
            )
        }) {
            return Err(ExecutionControlError::InvalidRequest(
                "execution semantic-loss ordering is not canonical".into(),
            ));
        }
        if self.artifact.artifact_id != format!("computational-execution:{}", self.request_id)
            || self.artifact.content_type
                != "application/vnd.aurora.computational-execution-control+json"
            || self.artifact.semantic_loss != self.semantic_loss
        {
            return Err(ExecutionControlError::Contract(
                "execution artifact is not bound to the planned receipt".into(),
            ));
        }
        if self.artifact.provenance
            != vec![bioprism_foundation::ProvenanceLink {
                source_id: self.workflow_id.clone(),
                relation: "planned-from-workflow-spec".into(),
                digest: self.run.plan_hash.clone(),
            }]
        {
            return Err(ExecutionControlError::Contract(
                "execution artifact provenance is not bound to the workflow plan".into(),
            ));
        }
        let payload = execution_payload(self);
        self.artifact
            .verify_payload(&payload)
            .map_err(|error| ExecutionControlError::Contract(error.to_string()))?;
        self.artifact
            .validate_metadata()
            .map_err(|error| ExecutionControlError::Contract(error.to_string()))?;
        validate_request(&self.input)?;
        if self.input_digest != execution_input_digest(&self.input)? {
            return Err(ExecutionControlError::Contract(
                "execution retained input digest does not match the request".into(),
            ));
        }
        let expected = build_computational_execution(&self.input)?;
        if self != &expected {
            return Err(ExecutionControlError::Contract(
                "execution receipt is not derived from its retained request".into(),
            ));
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, ExecutionControlError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ExecutionControlError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ExecutionControlError::Serialization(error.to_string()))
    }
}

fn validate_text(field: &str, value: &str) -> Result<(), ExecutionControlError> {
    if value.is_empty() || value.trim() != value {
        return Err(ExecutionControlError::InvalidRequest(format!(
            "{field} must be non-empty and trimmed"
        )));
    }
    if value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(ExecutionControlError::InvalidRequest(format!(
            "{field} is outside its bounded text contract"
        )));
    }
    Ok(())
}

fn execution_input_digest(
    request: &ComputationalExecutionRequest,
) -> Result<ContentHash, ExecutionControlError> {
    let canonical = canonical_computational_execution_request(request);
    let value = serde_json::to_value(canonical)
        .map_err(|error| ExecutionControlError::Serialization(error.to_string()))?;
    ContentHash::of_value(&value)
        .map_err(|error| ExecutionControlError::Serialization(error.to_string()))
}

fn canonical_computational_execution_request(
    request: &ComputationalExecutionRequest,
) -> ComputationalExecutionRequest {
    let mut canonical = request.clone();
    canonical
        .workflow
        .nodes
        .sort_by(|left, right| workflow_node_key(left).cmp(&workflow_node_key(right)));
    canonical
        .workflow
        .edges
        .sort_by(|left, right| workflow_edge_key(left).cmp(&workflow_edge_key(right)));
    canonical
        .workflow
        .checkpoints
        .sort_by(|left, right| workflow_checkpoint_key(left).cmp(&workflow_checkpoint_key(right)));
    canonical.workflow.budgets.sort_by(|left, right| {
        workflow_budget_key(left)
            .cmp(workflow_budget_key(right))
            .then_with(|| left.amount.total_cmp(&right.amount))
    });
    canonical.workflow.compensations.sort_by(|left, right| {
        workflow_compensation_key(left).cmp(&workflow_compensation_key(right))
    });
    canonical
        .workflow
        .approvals
        .sort_by(|left, right| workflow_approval_key(left).cmp(&workflow_approval_key(right)));
    canonical
}

fn workflow_node_key(node: &WorkflowNode) -> (&str, &str, &str, bool) {
    (
        node.node_id.as_str(),
        node.capability_id.as_str(),
        node.actor.as_str(),
        node.requires_approval,
    )
}

fn workflow_edge_key(edge: &WorkflowEdge) -> (&str, &str) {
    (edge.from.as_str(), edge.to.as_str())
}

fn workflow_checkpoint_key(checkpoint: &WorkflowCheckpoint) -> (&str, &BTreeSet<String>) {
    (checkpoint.checkpoint_id.as_str(), &checkpoint.after_nodes)
}

fn workflow_budget_key(budget: &ResourceBudget) -> &str {
    budget.resource.as_str()
}

fn workflow_compensation_key(compensation: &Compensation) -> (&str, &str) {
    (compensation.effect.as_str(), compensation.action.as_str())
}

fn workflow_approval_key(approval: &ApprovalRequirement) -> (&str, &str, &str) {
    (
        approval.approval_id.as_str(),
        approval.actor.as_str(),
        approval.action.as_str(),
    )
}

fn validate_unique_strings(field: &str, values: &[String]) -> Result<(), ExecutionControlError> {
    if values.len() > MAX_ITEMS {
        return Err(ExecutionControlError::InvalidRequest(format!(
            "{field} exceeds its item bound"
        )));
    }
    let mut unique = BTreeSet::new();
    for value in values {
        validate_text(field, value)?;
        if !unique.insert(value) {
            return Err(ExecutionControlError::InvalidRequest(format!(
                "{field} contains duplicate values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_strings(field: &str, values: &[String]) -> Result<(), ExecutionControlError> {
    validate_unique_strings(field, values)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ExecutionControlError::InvalidRequest(format!(
            "{field} ordering is not canonical"
        )));
    }
    Ok(())
}

fn is_subsequence(ordered: &[String], selected: &[String]) -> bool {
    let mut cursor = 0;
    for node in ordered {
        if selected
            .get(cursor)
            .is_some_and(|selected_node| selected_node == node)
        {
            cursor += 1;
        }
    }
    cursor == selected.len()
}

fn execution_payload(receipt: &ComputationalExecutionReceipt) -> serde_json::Value {
    execution_payload_from_parts(
        &receipt.request_id,
        &receipt.workflow_id,
        &receipt.run_id,
        receipt.mode,
        receipt.policy_allow,
        receipt.approval_required,
        receipt.authorization_reference.as_ref(),
        receipt.decision,
        &receipt.ordered_nodes,
        &receipt.admitted_nodes,
        &receipt.run,
        &receipt.run_digest,
        &receipt.authorized_effects,
        &receipt.omissions,
        &receipt.uncertainty,
        &receipt.semantic_loss,
        &receipt.reasons,
        receipt.effects_executed,
        receipt.raw_data_local,
        &receipt.boundary,
    )
}

fn canonical_reasons(decision: ExecutionControlDecision) -> Vec<String> {
    vec![match decision {
        ExecutionControlDecision::DryRun => {
            "graph and budget checks passed in dry-run; no effect was authorized"
        }
        ExecutionControlDecision::Admitted => {
            "workflow graph, locality, policy, authority, and replay gates passed; local executor remains responsible for actual computation"
        }
        ExecutionControlDecision::ApprovalRequired => {
            "workflow autonomy tier requires an independent approval reference"
        }
        ExecutionControlDecision::Blocked => "policy denied computational execution admission",
    }
    .into()]
}

fn canonical_omissions(decision: ExecutionControlDecision) -> Vec<String> {
    if decision == ExecutionControlDecision::ApprovalRequired {
        vec!["no local-computation effect was authorized before approval".into()]
    } else {
        Vec::new()
    }
}

fn canonical_uncertainty(decision: ExecutionControlDecision) -> Vec<String> {
    if decision == ExecutionControlDecision::Blocked {
        vec!["a denied policy is not evidence about scientific result quality".into()]
    } else {
        Vec::new()
    }
}

fn canonical_semantic_loss(decision: ExecutionControlDecision) -> Vec<SemanticLoss> {
    match decision {
        ExecutionControlDecision::ApprovalRequired => vec![SemanticLoss {
            field: "authority".into(),
            reason: "execution admission cannot infer an operator approval from workflow intent"
                .into(),
            severity: bioprism_foundation::LossSeverity::DecisionRelevant,
        }],
        ExecutionControlDecision::Blocked => vec![SemanticLoss {
            field: "policy".into(),
            reason: "policy denial prevents any execution-effect conclusion".into(),
            severity: bioprism_foundation::LossSeverity::DecisionRelevant,
        }],
        ExecutionControlDecision::DryRun | ExecutionControlDecision::Admitted => Vec::new(),
    }
}

#[allow(clippy::too_many_arguments)]
fn execution_payload_from_parts(
    request_id: &str,
    workflow_id: &str,
    run_id: &RunId,
    mode: ExecutionAdmissionMode,
    policy_allow: bool,
    approval_required: bool,
    authorization_reference: Option<&String>,
    decision: ExecutionControlDecision,
    ordered_nodes: &[String],
    admitted_nodes: &[String],
    run: &ExecutionRun,
    run_digest: &ContentHash,
    authorized_effects: &[AuthorizedExecutionEffect],
    omissions: &[String],
    uncertainty: &[String],
    semantic_loss: &[SemanticLoss],
    reasons: &[String],
    effects_executed: bool,
    raw_data_local: bool,
    boundary: &str,
) -> serde_json::Value {
    json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request_id,
        "workflow_id": workflow_id,
        "run_id": run_id,
        "mode": mode,
        "policy_allow": policy_allow,
        "approval_required": approval_required,
        "authorization_reference": authorization_reference,
        "decision": decision,
        "ordered_nodes": ordered_nodes,
        "admitted_nodes": admitted_nodes,
        "run": run,
        "run_digest": run_digest,
        "authorized_effects": authorized_effects,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "semantic_loss": semantic_loss,
        "reasons": reasons,
        "effects_executed": effects_executed,
        "raw_data_local": raw_data_local,
        "boundary": boundary,
    })
}

#[derive(Debug, Error)]
pub enum ExecutionControlError {
    #[error("invalid computational execution request: {0}")]
    InvalidRequest(String),
    #[error("computational execution contract rejected: {0}")]
    Contract(String),
    #[error("computational execution serialization failed: {0}")]
    Serialization(String),
}

pub fn admit_computational_execution(
    request: &ComputationalExecutionRequest,
) -> Result<ComputationalExecutionReceipt, ExecutionControlError> {
    let receipt = build_computational_execution(request)?;
    receipt.validate()?;
    Ok(receipt)
}

fn build_computational_execution(
    request: &ComputationalExecutionRequest,
) -> Result<ComputationalExecutionReceipt, ExecutionControlError> {
    let canonical_request = canonical_computational_execution_request(request);
    let request = &canonical_request;
    validate_request(request)?;
    let ordered_nodes = topological_order(&request.workflow)?;
    let workflow_value = serde_json::to_value(&request.workflow)
        .map_err(|error| ExecutionControlError::Serialization(error.to_string()))?;
    let plan_hash = ContentHash::of_value(&workflow_value)
        .map_err(|error| ExecutionControlError::Serialization(error.to_string()))?;
    let mut run = ExecutionRun::planned(
        request.run_id.clone(),
        request.workflow.workflow_id.clone(),
        plan_hash,
    )
    .map_err(|error| ExecutionControlError::Contract(error.to_string()))?;
    let policy_ready = request.policy_allow;
    let approval_ready = !request.workflow.autonomy_tier.requires_approval()
        || request
            .authorization_reference
            .as_ref()
            .is_some_and(|reference| !reference.trim().is_empty());
    let decision = if !policy_ready {
        ExecutionControlDecision::Blocked
    } else if !approval_ready {
        ExecutionControlDecision::ApprovalRequired
    } else if request.mode == ExecutionAdmissionMode::DryRun {
        ExecutionControlDecision::DryRun
    } else {
        ExecutionControlDecision::Admitted
    };
    let admitted_nodes = if matches!(decision, ExecutionControlDecision::Admitted) {
        ordered_nodes.clone()
    } else {
        Vec::new()
    };
    let mut authorized_effects = Vec::new();
    if decision == ExecutionControlDecision::Admitted {
        for (sequence, node_id) in admitted_nodes.iter().enumerate() {
            let payload = json!({
                "workflow_id": request.workflow.workflow_id,
                "node_id": node_id,
                "effect": "execute_local_computation",
                "executed": false,
            });
            let payload_hash = ContentHash::of_value(&payload)
                .map_err(|error| ExecutionControlError::Serialization(error.to_string()))?;
            run.append_event(ExecutionEvent {
                sequence: sequence as u64,
                event_type: "local-computation-admitted".into(),
                effect: Some(Effect::ExecuteLocalComputation),
                payload_hash: Some(payload_hash.clone()),
            })
            .map_err(|error| ExecutionControlError::Contract(error.to_string()))?;
            authorized_effects.push(AuthorizedExecutionEffect {
                node_id: node_id.clone(),
                effect: Effect::ExecuteLocalComputation,
                authorized: true,
                executed: false,
                payload_digest: payload_hash,
            });
        }
        if !admitted_nodes.is_empty() {
            run.checkpoint("admission-boundary")
                .map_err(|error| ExecutionControlError::Contract(error.to_string()))?;
            run.status = ExecutionStatus::Planned;
        }
    }
    let omissions = canonical_omissions(decision);
    let uncertainty = canonical_uncertainty(decision);
    let semantic_loss = canonical_semantic_loss(decision);
    let reasons = canonical_reasons(decision);
    let run_value = serde_json::to_value(&run)
        .map_err(|error| ExecutionControlError::Serialization(error.to_string()))?;
    let run_digest = ContentHash::of_value(&run_value)
        .map_err(|error| ExecutionControlError::Serialization(error.to_string()))?;
    let payload = execution_payload_from_parts(
        &request.request_id,
        &request.workflow.workflow_id,
        &request.run_id,
        request.mode,
        request.policy_allow,
        request.workflow.autonomy_tier.requires_approval(),
        request.authorization_reference.as_ref(),
        decision,
        &ordered_nodes,
        &admitted_nodes,
        &run,
        &run_digest,
        &authorized_effects,
        &omissions,
        &uncertainty,
        &semantic_loss,
        &reasons,
        false,
        request.raw_data_local,
        &request.boundary,
    );
    let artifact = TypedResearchArtifact::from_payload(
        format!("computational-execution:{}", request.request_id),
        "application/vnd.aurora.computational-execution-control+json",
        &payload,
        semantic_loss.clone(),
        vec![bioprism_foundation::ProvenanceLink {
            source_id: request.workflow.workflow_id.clone(),
            relation: "planned-from-workflow-spec".into(),
            digest: run.plan_hash.clone(),
        }],
    )
    .map_err(|error| ExecutionControlError::Contract(error.to_string()))?;
    let receipt = ComputationalExecutionReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        input: request.clone(),
        input_digest: execution_input_digest(request)?,
        request_id: request.request_id.clone(),
        workflow_id: request.workflow.workflow_id.clone(),
        run_id: request.run_id.clone(),
        mode: request.mode,
        policy_allow: request.policy_allow,
        approval_required: request.workflow.autonomy_tier.requires_approval(),
        authorization_reference: request.authorization_reference.clone(),
        decision,
        ordered_nodes,
        admitted_nodes,
        run,
        run_digest,
        authorized_effects,
        omissions,
        uncertainty,
        semantic_loss,
        reasons,
        artifact,
        effects_executed: false,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    Ok(receipt)
}

fn validate_request(request: &ComputationalExecutionRequest) -> Result<(), ExecutionControlError> {
    if request.request_id.trim().is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
    {
        return Err(ExecutionControlError::InvalidRequest(
            "request identity, locality, and preclinical boundary are required".into(),
        ));
    }
    validate_text("request_id", &request.request_id)?;
    validate_text("boundary", &request.boundary)?;
    if let Some(reference) = &request.authorization_reference {
        validate_text("authorization_reference", reference)?;
    }
    request
        .workflow
        .validate()
        .map_err(|error| ExecutionControlError::Contract(error.to_string()))?;
    if request.workflow.nodes.is_empty() || request.workflow.nodes.len() > MAX_NODES {
        return Err(ExecutionControlError::InvalidRequest(
            "workflow must contain a bounded non-empty node set".into(),
        ));
    }
    Ok(())
}

fn topological_order(
    workflow: &ResearchWorkflowSpec,
) -> Result<Vec<String>, ExecutionControlError> {
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
            *indegree.get_mut(successor).ok_or_else(|| {
                ExecutionControlError::InvalidRequest(
                    "workflow edge references an unknown node".into(),
                )
            })? += 1;
        }
    }
    let mut ready = indegree
        .iter()
        .filter_map(|(node, degree)| (*degree == 0).then_some(node.clone()))
        .collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(indegree.len());
    while let Some(node) = ready.iter().next().cloned() {
        ready.remove(&node);
        order.push(node.clone());
        if let Some(successors) = adjacency.get(&node) {
            for successor in successors {
                let degree = indegree.get_mut(successor).ok_or_else(|| {
                    ExecutionControlError::InvalidRequest(
                        "workflow edge references an unknown node".into(),
                    )
                })?;
                *degree -= 1;
                if *degree == 0 {
                    ready.insert(successor.clone());
                }
            }
        }
    }
    if order.len() != indegree.len() {
        return Err(ExecutionControlError::InvalidRequest(
            "workflow graph is cyclic and cannot be admitted".into(),
        ));
    }
    Ok(order)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bioprism_foundation::{
        ApprovalRequirement, AutonomyTier, Compensation, ResourceBudget, WorkflowCheckpoint,
        WorkflowEdge, WorkflowNode,
    };

    fn request(mode: ExecutionAdmissionMode) -> ComputationalExecutionRequest {
        let workflow = ResearchWorkflowSpec {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            workflow_id: "workflow:execution-control".into(),
            intent: "replayable local computation".into(),
            nodes: vec![
                WorkflowNode {
                    node_id: "b-compute".into(),
                    capability_id: "compute".into(),
                    actor: "executor".into(),
                    requires_approval: false,
                },
                WorkflowNode {
                    node_id: "a-read".into(),
                    capability_id: "read".into(),
                    actor: "executor".into(),
                    requires_approval: false,
                },
            ],
            edges: vec![WorkflowEdge {
                from: "a-read".into(),
                to: "b-compute".into(),
            }],
            checkpoints: vec![WorkflowCheckpoint {
                checkpoint_id: "checkpoint:all".into(),
                after_nodes: ["b-compute".into()].into(),
            }],
            budgets: vec![ResourceBudget {
                resource: "cpu".into(),
                amount: 10.0,
            }],
            compensations: vec![Compensation {
                effect: "execute_local_computation".into(),
                action: "stop".into(),
            }],
            approvals: vec![ApprovalRequirement {
                approval_id: "approval:execution".into(),
                actor: "operator".into(),
                action: "execute_local_computation".into(),
            }],
            autonomy_tier: AutonomyTier::A1,
            boundary: PRECLINICAL_BOUNDARY.into(),
        };
        ComputationalExecutionRequest {
            request_id: "request:execution-control".into(),
            workflow,
            run_id: RunId::parse("run:execution-control").unwrap(),
            mode,
            policy_allow: true,
            authorization_reference: None,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn topological_admission_is_deterministic_and_not_execution() {
        let first = admit_computational_execution(&request(ExecutionAdmissionMode::Admit)).unwrap();
        let second =
            admit_computational_execution(&request(ExecutionAdmissionMode::Admit)).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.ordered_nodes, ["a-read", "b-compute"]);
        assert_eq!(first.run.status, ExecutionStatus::Planned);
        assert!(!first.effects_executed);
        assert!(first.digest().is_ok());
    }

    #[test]
    fn declarative_workflow_collection_order_does_not_change_receipt_identity() {
        let mut reordered = request(ExecutionAdmissionMode::Admit);
        reordered.workflow.nodes.reverse();
        reordered.workflow.edges.reverse();
        reordered.workflow.checkpoints.reverse();
        reordered.workflow.budgets.reverse();
        reordered.workflow.compensations.reverse();
        reordered.workflow.approvals.reverse();

        let canonical =
            admit_computational_execution(&request(ExecutionAdmissionMode::Admit)).unwrap();
        let reordered = admit_computational_execution(&reordered).unwrap();

        assert_eq!(canonical, reordered);
        assert_eq!(canonical.input_digest, reordered.input_digest);
    }

    #[test]
    fn dry_run_has_no_authorized_effects() {
        let receipt =
            admit_computational_execution(&request(ExecutionAdmissionMode::DryRun)).unwrap();
        assert_eq!(receipt.decision, ExecutionControlDecision::DryRun);
        assert!(receipt.authorized_effects.is_empty());
    }

    #[test]
    fn denied_policy_blocks_without_effects() {
        let mut request = request(ExecutionAdmissionMode::Admit);
        request.policy_allow = false;
        let receipt = admit_computational_execution(&request).unwrap();
        assert_eq!(receipt.decision, ExecutionControlDecision::Blocked);
        assert!(receipt.authorized_effects.is_empty());
    }

    #[test]
    fn high_autonomy_without_authority_requires_approval() {
        let mut request = request(ExecutionAdmissionMode::Admit);
        request.workflow.autonomy_tier = AutonomyTier::A2;
        let receipt = admit_computational_execution(&request).unwrap();
        assert_eq!(receipt.decision, ExecutionControlDecision::ApprovalRequired);
        assert!(receipt.authorized_effects.is_empty());
    }

    #[test]
    fn whitespace_authority_reference_cannot_authorize_high_autonomy() {
        let mut request = request(ExecutionAdmissionMode::Admit);
        request.workflow.autonomy_tier = AutonomyTier::A2;
        request.authorization_reference = Some("   ".into());
        let error = admit_computational_execution(&request).unwrap_err();
        assert!(error.to_string().contains("authorization_reference"));
    }

    #[test]
    fn receipt_rejects_a_tampered_run_digest() {
        let mut receipt =
            admit_computational_execution(&request(ExecutionAdmissionMode::Admit)).unwrap();
        receipt.run_digest = ContentHash::of_bytes(b"tampered");
        let error = receipt.validate().unwrap_err();
        assert!(error.to_string().contains("run digest"));
    }

    #[test]
    fn receipt_rejects_a_tampered_effect_digest() {
        let mut receipt =
            admit_computational_execution(&request(ExecutionAdmissionMode::Admit)).unwrap();
        receipt.authorized_effects[0].payload_digest = ContentHash::of_bytes(b"tampered");
        let error = receipt.validate().unwrap_err();
        assert!(error.to_string().contains("effect payload digest"));
    }

    #[test]
    fn receipt_rejects_effects_out_of_admitted_node_order() {
        let mut receipt =
            admit_computational_execution(&request(ExecutionAdmissionMode::Admit)).unwrap();
        receipt.authorized_effects.swap(0, 1);
        let error = receipt.validate().unwrap_err();
        assert!(error.to_string().contains("align with admitted node order"));
    }

    #[test]
    fn approval_reference_allows_high_autonomy_admission() {
        let mut request = request(ExecutionAdmissionMode::Admit);
        request.workflow.autonomy_tier = AutonomyTier::A2;
        request.authorization_reference = Some("approval:operator".into());
        let receipt = admit_computational_execution(&request).unwrap();
        assert_eq!(receipt.decision, ExecutionControlDecision::Admitted);
        assert_eq!(receipt.authorized_effects.len(), 2);
    }

    #[test]
    fn receipt_rejects_a_tampered_planned_event() {
        let mut receipt =
            admit_computational_execution(&request(ExecutionAdmissionMode::Admit)).unwrap();
        receipt.run.events[0].event_type = "unexpected-event".into();
        let error = receipt.validate().unwrap_err();
        assert!(error.to_string().contains("planned run event"));
    }

    #[test]
    fn execution_artifact_payload_is_verified() {
        let mut receipt =
            admit_computational_execution(&request(ExecutionAdmissionMode::Admit)).unwrap();
        receipt.artifact.content_hash = ContentHash::of_bytes(b"tampered");
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn retained_request_tampering_is_rejected() {
        let mut receipt =
            admit_computational_execution(&request(ExecutionAdmissionMode::Admit)).unwrap();
        receipt.input.request_id = "request:tampered".into();
        assert!(receipt.validate().is_err());
    }
}

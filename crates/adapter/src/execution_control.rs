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
    Effect, ExecutionEvent, ExecutionRun, ExecutionStatus, ResearchWorkflowSpec, SemanticLoss,
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::{ContentHash, RunId};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P12-F31";
pub const CONTRACT_VERSION: &str = "computational-execution-control-plane/1.0";

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
    pub request_id: String,
    pub workflow_id: String,
    pub run_id: RunId,
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
        if self.ordered_nodes.windows(2).any(|pair| pair[0] == pair[1])
            || self
                .admitted_nodes
                .iter()
                .any(|node| !self.ordered_nodes.contains(node))
            || self
                .admitted_nodes
                .windows(2)
                .any(|pair| pair[0] == pair[1])
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
        if self.decision == ExecutionControlDecision::Admitted
            && self.authorized_effects.len() != self.admitted_nodes.len()
        {
            return Err(ExecutionControlError::InvalidRequest(
                "every admitted node needs one authorized effect receipt".into(),
            ));
        }
        if self.decision != ExecutionControlDecision::Admitted
            && !self.authorized_effects.is_empty()
        {
            return Err(ExecutionControlError::InvalidRequest(
                "non-admitted execution cannot contain authorized effects".into(),
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
        self.artifact
            .validate_metadata()
            .map_err(|error| ExecutionControlError::Contract(error.to_string()))?;
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
        || request.authorization_reference.is_some();
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
    let mut omissions = Vec::new();
    let mut uncertainty = Vec::new();
    let mut semantic_loss = Vec::new();
    let mut reasons = Vec::new();
    match decision {
        ExecutionControlDecision::DryRun => reasons.push("graph and budget checks passed in dry-run; no effect was authorized".into()),
        ExecutionControlDecision::Admitted => reasons.push("workflow graph, locality, policy, authority, and replay gates passed; local executor remains responsible for actual computation".into()),
        ExecutionControlDecision::ApprovalRequired => {
            reasons.push("workflow autonomy tier requires an independent approval reference".into());
            omissions.push("no local-computation effect was authorized before approval".into());
            semantic_loss.push(SemanticLoss {
                field: "authority".into(),
                reason: "execution admission cannot infer an operator approval from workflow intent".into(),
                severity: bioprism_foundation::LossSeverity::DecisionRelevant,
            });
        }
        ExecutionControlDecision::Blocked => {
            reasons.push("policy denied computational execution admission".into());
            uncertainty.push("a denied policy is not evidence about scientific result quality".into());
            semantic_loss.push(SemanticLoss {
                field: "policy".into(),
                reason: "policy denial prevents any execution-effect conclusion".into(),
                severity: bioprism_foundation::LossSeverity::DecisionRelevant,
            });
        }
    }
    let run_value = serde_json::to_value(&run)
        .map_err(|error| ExecutionControlError::Serialization(error.to_string()))?;
    let run_digest = ContentHash::of_value(&run_value)
        .map_err(|error| ExecutionControlError::Serialization(error.to_string()))?;
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "workflow_id": request.workflow.workflow_id,
        "run_id": request.run_id,
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
        "effects_executed": false,
        "raw_data_local": true,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!("computational-execution:{}", request.request_id),
        "application/vnd.aurora.computational-execution-control+json",
        &payload,
        semantic_loss.clone(),
        Vec::new(),
    )
    .map_err(|error| ExecutionControlError::Contract(error.to_string()))?;
    let receipt = ComputationalExecutionReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        workflow_id: request.workflow.workflow_id.clone(),
        run_id: request.run_id.clone(),
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
    receipt.validate()?;
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
    request
        .workflow
        .validate()
        .map_err(|error| ExecutionControlError::Contract(error.to_string()))?;
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
                let degree = indegree
                    .get_mut(successor)
                    .expect("validated workflow edge");
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
}

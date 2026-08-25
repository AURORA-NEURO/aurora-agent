//! Prospective high-throughput federation workflow fabric.
//!
//! Atlas feature: `AFA-adapter-P20-F15`.
//!
//! This fabric plans policy-bounded institutional exchanges using the foundation
//! [`FederationEnvelope`]. It produces no remote effect: task ordering, checkpoint and
//! compensation metadata, budget closure, signature evidence, partition state, and local-only
//! guarantees are evaluated before a workflow can be scheduled.

use bioprism_foundation::{
    FederationEnvelope, LossSeverity, SemanticLoss, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P20-F15";
pub const CONTRACT_VERSION: &str = "prospective-federation-workflow-fabric/1.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederationTask {
    pub task_id: String,
    pub action: String,
    pub input_digest: ContentHash,
    pub resource: String,
    pub budget_units: u64,
    pub checkpoint_id: String,
    pub compensation_action: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederationRequest {
    pub schema_version: String,
    pub workflow_id: String,
    pub origin: String,
    pub destination: String,
    pub purpose: String,
    pub tasks: Vec<FederationTask>,
    pub permitted_budget_units: u64,
    pub policy_allow: bool,
    pub authority_reference: Option<String>,
    pub signature: Option<String>,
    pub raw_data_local: bool,
    pub network_partition: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederationWorkflowDecision {
    Scheduled,
    ApprovalRequired,
    LocalOnly,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederationWorkflowReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub workflow_id: String,
    pub decision: FederationWorkflowDecision,
    pub task_order: Vec<String>,
    pub checkpoint_order: Vec<String>,
    pub compensation_order: Vec<String>,
    pub total_budget_units: u64,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub semantic_loss: Vec<SemanticLoss>,
    pub reasons: Vec<String>,
    pub effect_receipt: String,
    pub envelope: FederationEnvelope,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

impl FederationWorkflowReceipt {
    pub fn validate(&self) -> Result<(), FederationWorkflowError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
        {
            return Err(FederationWorkflowError::Contract(
                "federation workflow identity mismatch".into(),
            ));
        }
        if self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.workflow_id.trim().is_empty()
            || self.task_order.is_empty()
            || self.checkpoint_order.is_empty()
            || self.compensation_order.is_empty()
            || self.reasons.is_empty()
            || self.effect_receipt.trim().is_empty()
        {
            return Err(FederationWorkflowError::InvalidRequest("workflow identity, tasks, checkpoints, compensation, locality, reasons, effects, and boundary are required".into()));
        }
        for values in [
            &self.task_order,
            &self.checkpoint_order,
            &self.compensation_order,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(FederationWorkflowError::InvalidRequest(
                    "workflow ordering is not canonical".into(),
                ));
            }
        }
        self.envelope
            .validate()
            .map_err(|error| FederationWorkflowError::Contract(error.to_string()))?;
        self.artifact
            .validate_metadata()
            .map_err(|error| FederationWorkflowError::Contract(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, FederationWorkflowError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| FederationWorkflowError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| FederationWorkflowError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum FederationWorkflowError {
    #[error("invalid federation workflow request: {0}")]
    InvalidRequest(String),
    #[error("federation workflow contract rejected: {0}")]
    Contract(String),
    #[error("federation workflow serialization failed: {0}")]
    Serialization(String),
}

pub fn schedule_federation_workflow(
    request: &FederationRequest,
) -> Result<FederationWorkflowReceipt, FederationWorkflowError> {
    validate_request(request)?;
    let mut tasks = request.tasks.clone();
    tasks.sort_by(|left, right| left.task_id.cmp(&right.task_id));
    let task_order = tasks
        .iter()
        .map(|task| task.task_id.clone())
        .collect::<Vec<_>>();
    let checkpoint_order = tasks
        .iter()
        .map(|task| task.checkpoint_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let compensation_order = tasks
        .iter()
        .map(|task| task.compensation_action.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let total_budget_units = tasks.iter().map(|task| task.budget_units).sum::<u64>();
    let mut omissions = Vec::new();
    let mut uncertainty = Vec::new();
    let mut reasons = vec![format!(
        "{} federation tasks ordered with checkpoints and compensations",
        tasks.len()
    )];
    let mut semantic_loss = Vec::new();
    if request.network_partition {
        omissions.push("network partition prevents confirmation of destination admission".into());
        semantic_loss.push(SemanticLoss {
            field: "network_partition".into(),
            reason: "unconfirmed remote state cannot be promoted to scheduled success".into(),
            severity: LossSeverity::DecisionRelevant,
        });
    }
    let decision = if !request.policy_allow {
        FederationWorkflowDecision::Blocked
    } else if request.network_partition {
        FederationWorkflowDecision::Partial
    } else if request.authority_reference.is_none()
        || request.signature.as_deref().unwrap_or("").trim().is_empty()
    {
        uncertainty.push("authority reference or federation signature is incomplete".into());
        FederationWorkflowDecision::ApprovalRequired
    } else {
        FederationWorkflowDecision::Scheduled
    };
    if decision == FederationWorkflowDecision::Partial {
        reasons
            .push("partitioned work remains local-only until destination acknowledgement".into());
    }
    if decision == FederationWorkflowDecision::Blocked {
        reasons.push("policy denied federation scheduling".into());
    }
    if decision == FederationWorkflowDecision::ApprovalRequired {
        reasons.push("workflow requires authority and signed federation approval".into());
    }
    let effect_receipt = if decision == FederationWorkflowDecision::Scheduled {
        "schedule:research-work_no_remote_execution"
    } else {
        "retain_local_checkpoint_and_block_remote_schedule"
    };
    let payload = json!({ "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "workflow_id": request.workflow_id, "decision": decision, "task_order": task_order, "checkpoint_order": checkpoint_order, "compensation_order": compensation_order, "total_budget_units": total_budget_units, "omissions": omissions, "uncertainty": uncertainty, "semantic_loss": semantic_loss, "reasons": reasons, "effect_receipt": effect_receipt, "raw_data_local": true, "boundary": PRECLINICAL_BOUNDARY });
    let artifact = TypedResearchArtifact::from_payload(
        format!("federation-workflow:{}", request.workflow_id),
        "application/vnd.aurora.federation-workflow-receipt+json",
        &payload,
        semantic_loss.clone(),
        Vec::new(),
    )
    .map_err(|error| FederationWorkflowError::Contract(error.to_string()))?;
    let export = TypedResearchArtifact::from_payload(
        format!("federation-export:{}", request.workflow_id),
        "application/vnd.aurora.permitted-federation-export+json",
        &payload,
        semantic_loss.clone(),
        Vec::new(),
    )
    .map_err(|error| FederationWorkflowError::Contract(error.to_string()))?;
    let envelope = FederationEnvelope {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        envelope_id: format!("envelope:{}", request.workflow_id),
        origin: request.origin.clone(),
        purpose: request.purpose.clone(),
        export,
        policy_constraints: vec![
            "preclinical-research-only".into(),
            "raw-data-local".into(),
            "policy-approved-artifacts-only".into(),
        ],
        integrity_evidence: vec![request.tasks[0].input_digest.clone()],
        localization_statement:
            "raw data remains institution-local; only permitted metadata may cross federation"
                .into(),
        raw_data_local: true,
        signature: request
            .signature
            .clone()
            .or_else(|| Some("pending-approval".into())),
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let receipt = FederationWorkflowReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        workflow_id: request.workflow_id.clone(),
        decision,
        task_order,
        checkpoint_order,
        compensation_order,
        total_budget_units,
        omissions,
        uncertainty,
        semantic_loss,
        reasons,
        effect_receipt: effect_receipt.into(),
        envelope,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &FederationRequest) -> Result<(), FederationWorkflowError> {
    if request.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
        || request.workflow_id.trim().is_empty()
        || request.origin.trim().is_empty()
        || request.destination.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.tasks.is_empty()
        || !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(FederationWorkflowError::InvalidRequest(
            "workflow identity, tasks, locality, purpose, and boundary are required".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    let mut checkpoints = BTreeSet::new();
    let mut compensation = BTreeSet::new();
    let mut total = 0u64;
    for task in &request.tasks {
        if task.task_id.trim().is_empty()
            || task.action.trim().is_empty()
            || task.resource.trim().is_empty()
            || task.checkpoint_id.trim().is_empty()
            || task.compensation_action.trim().is_empty()
            || task.budget_units == 0
        {
            return Err(FederationWorkflowError::InvalidRequest(
                "task action, resource, checkpoint, compensation, and positive budget are required"
                    .into(),
            ));
        }
        if !ids.insert(task.task_id.clone())
            || !checkpoints.insert(task.checkpoint_id.clone())
            || !compensation.insert(task.compensation_action.clone())
        {
            return Err(FederationWorkflowError::InvalidRequest(
                "task, checkpoint, and compensation identities must be unique".into(),
            ));
        }
        total = total.checked_add(task.budget_units).ok_or_else(|| {
            FederationWorkflowError::InvalidRequest("workflow budget overflow".into())
        })?;
    }
    if total > request.permitted_budget_units {
        return Err(FederationWorkflowError::InvalidRequest(
            "workflow budget exceeds permitted budget".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> FederationRequest {
        FederationRequest {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            workflow_id: "workflow:qc".into(),
            origin: "institution:a".into(),
            destination: "institution:b".into(),
            purpose: "federated preclinical QC".into(),
            tasks: vec![
                FederationTask {
                    task_id: "task:b".into(),
                    action: "summarize".into(),
                    input_digest: ContentHash::of_bytes(b"b"),
                    resource: "cpu".into(),
                    budget_units: 20,
                    checkpoint_id: "checkpoint:b".into(),
                    compensation_action: "retain-b".into(),
                },
                FederationTask {
                    task_id: "task:a".into(),
                    action: "harmonize".into(),
                    input_digest: ContentHash::of_bytes(b"a"),
                    resource: "cpu".into(),
                    budget_units: 10,
                    checkpoint_id: "checkpoint:a".into(),
                    compensation_action: "retain-a".into(),
                },
            ],
            permitted_budget_units: 100,
            policy_allow: true,
            authority_reference: Some("authority:1".into()),
            signature: Some("signature:1".into()),
            raw_data_local: true,
            network_partition: false,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn workflow_is_deterministic_under_task_order() {
        let mut reversed = request();
        reversed.tasks.reverse();
        let first = schedule_federation_workflow(&request()).unwrap();
        let second = schedule_federation_workflow(&reversed).unwrap();
        assert_eq!(first.digest().unwrap(), second.digest().unwrap());
        assert_eq!(first.decision, FederationWorkflowDecision::Scheduled);
    }
    #[test]
    fn partition_is_partial_and_retains_omission() {
        let mut request = request();
        request.network_partition = true;
        let receipt = schedule_federation_workflow(&request).unwrap();
        assert_eq!(receipt.decision, FederationWorkflowDecision::Partial);
        assert!(!receipt.omissions.is_empty());
    }
    #[test]
    fn budget_overrun_is_rejected() {
        let mut request = request();
        request.permitted_budget_units = 1;
        assert!(schedule_federation_workflow(&request).is_err());
    }
    #[test]
    fn missing_signature_requires_approval() {
        let mut request = request();
        request.signature = None;
        let receipt = schedule_federation_workflow(&request).unwrap();
        assert_eq!(
            receipt.decision,
            FederationWorkflowDecision::ApprovalRequired
        );
    }
}

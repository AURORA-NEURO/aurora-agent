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
const MAX_TEXT_BYTES: usize = 512;
const MAX_ITEMS: usize = 16_384;

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
    pub origin: String,
    pub destination: String,
    pub purpose: String,
    pub permitted_budget_units: u64,
    pub policy_allow: bool,
    pub authority_reference: Option<String>,
    pub signature: Option<String>,
    pub network_partition: bool,
    pub decision: FederationWorkflowDecision,
    pub task_order: Vec<String>,
    pub checkpoint_order: Vec<String>,
    pub compensation_order: Vec<String>,
    pub input_digest_order: Vec<ContentHash>,
    pub total_budget_units: u64,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub semantic_loss: Vec<SemanticLoss>,
    pub reasons: Vec<String>,
    pub effect_receipt: String,
    pub task_digest: ContentHash,
    pub envelope_digest: ContentHash,
    pub envelope: FederationEnvelope,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

fn validate_text(field: &str, value: &str) -> Result<(), FederationWorkflowError> {
    if value.is_empty() || value.trim() != value {
        return Err(FederationWorkflowError::InvalidRequest(format!(
            "{field} must be non-empty and trimmed"
        )));
    }
    if value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(FederationWorkflowError::InvalidRequest(format!(
            "{field} is outside its bounded text contract"
        )));
    }
    Ok(())
}

fn validate_unique_strings(field: &str, values: &[String]) -> Result<(), FederationWorkflowError> {
    if values.len() > MAX_ITEMS {
        return Err(FederationWorkflowError::InvalidRequest(format!(
            "{field} exceeds its item bound"
        )));
    }
    let mut unique = BTreeSet::new();
    for value in values {
        validate_text(field, value)?;
        if !unique.insert(value) {
            return Err(FederationWorkflowError::InvalidRequest(format!(
                "{field} contains duplicate values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_strings(field: &str, values: &[String]) -> Result<(), FederationWorkflowError> {
    validate_unique_strings(field, values)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(FederationWorkflowError::InvalidRequest(format!(
            "{field} ordering is not canonical"
        )));
    }
    Ok(())
}

fn validate_digest(field: &str, digest: &ContentHash) -> Result<(), FederationWorkflowError> {
    if digest.as_str().len() != 64
        || !digest
            .as_str()
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err(FederationWorkflowError::InvalidRequest(format!(
            "{field} must be a 64-character hex digest"
        )));
    }
    Ok(())
}

fn validate_digests(field: &str, digests: &[ContentHash]) -> Result<(), FederationWorkflowError> {
    if digests.len() > MAX_ITEMS {
        return Err(FederationWorkflowError::InvalidRequest(format!(
            "{field} exceeds its item bound"
        )));
    }
    for digest in digests {
        validate_digest(field, digest)?;
    }
    Ok(())
}

impl FederationWorkflowReceipt {
    pub fn validate(&self) -> Result<(), FederationWorkflowError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.workflow_id.trim().is_empty()
            || self.origin.trim().is_empty()
            || self.destination.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.task_order.is_empty()
            || self.checkpoint_order.is_empty()
            || self.compensation_order.is_empty()
            || self.input_digest_order.is_empty()
            || self.reasons.is_empty()
            || self.effect_receipt.trim().is_empty()
        {
            return Err(FederationWorkflowError::Contract(
                "federation workflow identity mismatch".into(),
            ));
        }
        validate_text("workflow_id", &self.workflow_id)?;
        validate_text("origin", &self.origin)?;
        validate_text("destination", &self.destination)?;
        validate_text("purpose", &self.purpose)?;
        validate_text("boundary", &self.boundary)?;
        validate_sorted_strings("task_order", &self.task_order)?;
        validate_sorted_strings("checkpoint_order", &self.checkpoint_order)?;
        validate_sorted_strings("compensation_order", &self.compensation_order)?;
        validate_digests("input_digest_order", &self.input_digest_order)?;
        if self.task_order.len() != self.checkpoint_order.len()
            || self.task_order.len() != self.compensation_order.len()
            || self.task_order.len() != self.input_digest_order.len()
        {
            return Err(FederationWorkflowError::InvalidRequest(
                "workflow task, checkpoint, compensation, and input closures differ".into(),
            ));
        }
        validate_unique_strings("omissions", &self.omissions)?;
        validate_unique_strings("uncertainty", &self.uncertainty)?;
        validate_unique_strings("reasons", &self.reasons)?;
        validate_text("effect_receipt", &self.effect_receipt)?;
        if let Some(authority) = &self.authority_reference {
            validate_text("authority_reference", authority)?;
        }
        if let Some(signature) = &self.signature {
            validate_text("signature", signature)?;
        }
        validate_digest("task_digest", &self.task_digest)?;
        validate_digest("envelope_digest", &self.envelope_digest)?;
        if self.total_budget_units > self.permitted_budget_units {
            return Err(FederationWorkflowError::InvalidRequest(
                "receipt budget exceeds permitted budget".into(),
            ));
        }
        let expected_decision = if !self.policy_allow {
            FederationWorkflowDecision::Blocked
        } else if self.network_partition {
            FederationWorkflowDecision::Partial
        } else if self.authority_reference.is_none()
            || self.signature.as_deref().unwrap_or("").trim().is_empty()
        {
            FederationWorkflowDecision::ApprovalRequired
        } else {
            FederationWorkflowDecision::Scheduled
        };
        if self.decision != expected_decision {
            return Err(FederationWorkflowError::InvalidRequest(
                "workflow decision does not match policy, partition, and authority state".into(),
            ));
        }
        let expected_omissions = if self.network_partition {
            vec!["network partition prevents confirmation of destination admission".to_string()]
        } else {
            Vec::new()
        };
        if self.omissions != expected_omissions {
            return Err(FederationWorkflowError::InvalidRequest(
                "workflow omissions do not match network partition state".into(),
            ));
        }
        let expected_uncertainty = if !self.policy_allow
            || self.network_partition
            || (self.authority_reference.is_some()
                && !self.signature.as_deref().unwrap_or("").trim().is_empty())
        {
            Vec::new()
        } else {
            vec!["authority reference or federation signature is incomplete".to_string()]
        };
        if self.uncertainty != expected_uncertainty {
            return Err(FederationWorkflowError::InvalidRequest(
                "workflow uncertainty does not match authority state".into(),
            ));
        }
        let expected_loss = if self.network_partition {
            vec![SemanticLoss {
                field: "network_partition".into(),
                reason: "unconfirmed remote state cannot be promoted to scheduled success".into(),
                severity: LossSeverity::DecisionRelevant,
            }]
        } else {
            Vec::new()
        };
        if self.semantic_loss != expected_loss {
            return Err(FederationWorkflowError::Contract(
                "workflow semantic-loss closure does not match partition state".into(),
            ));
        }
        let mut expected_reasons = vec![format!(
            "{} federation tasks ordered with checkpoints and compensations",
            self.task_order.len()
        )];
        if self.decision == FederationWorkflowDecision::Partial {
            expected_reasons.push(
                "partitioned work remains local-only until destination acknowledgement".into(),
            );
        }
        if self.decision == FederationWorkflowDecision::Blocked {
            expected_reasons.push("policy denied federation scheduling".into());
        }
        if self.decision == FederationWorkflowDecision::ApprovalRequired {
            expected_reasons
                .push("workflow requires authority and signed federation approval".into());
        }
        expected_reasons.sort();
        if self.reasons != expected_reasons {
            return Err(FederationWorkflowError::InvalidRequest(
                "workflow reasons are not bound to decision state".into(),
            ));
        }
        let expected_effect = if self.decision == FederationWorkflowDecision::Scheduled {
            "schedule:research-work_no_remote_execution"
        } else {
            "retain_local_checkpoint_and_block_remote_schedule"
        };
        if self.effect_receipt != expected_effect {
            return Err(FederationWorkflowError::InvalidRequest(
                "workflow effect does not match decision".into(),
            ));
        }
        self.envelope
            .validate()
            .map_err(|error| FederationWorkflowError::Contract(error.to_string()))?;
        if self.envelope.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.envelope.envelope_id != format!("envelope:{}", self.workflow_id)
            || self.envelope.origin != self.origin
            || self.envelope.purpose != self.purpose
            || self.envelope.integrity_evidence != self.input_digest_order
            || self.envelope.policy_constraints
                != vec![
                    "preclinical-research-only".to_string(),
                    "raw-data-local".to_string(),
                    "policy-approved-artifacts-only".to_string(),
                ]
            || self.envelope.localization_statement
                != "raw data remains institution-local; only permitted metadata may cross federation"
            || self.envelope.raw_data_local != self.raw_data_local
            || self.envelope.signature != self.signature.clone().or_else(|| Some("pending-approval".into()))
            || self.envelope.boundary != PRECLINICAL_BOUNDARY
        {
            return Err(FederationWorkflowError::Contract(
                "federation envelope is not bound to workflow state".into(),
            ));
        }
        let expected_envelope = ContentHash::of_value(&json!({
            "workflow_id": self.workflow_id,
            "origin": self.origin,
            "destination": self.destination,
            "purpose": self.purpose,
            "policy_constraints": self.envelope.policy_constraints,
            "integrity_evidence": self.input_digest_order,
            "localization_statement": self.envelope.localization_statement,
            "raw_data_local": self.raw_data_local,
            "signature": self.envelope.signature,
            "task_digest": self.task_digest,
        }))
        .map_err(|error| FederationWorkflowError::Serialization(error.to_string()))?;
        if self.envelope_digest != expected_envelope {
            return Err(FederationWorkflowError::Contract(
                "envelope digest does not match workflow envelope".into(),
            ));
        }
        if self.envelope.export.artifact_id != format!("federation-export:{}", self.workflow_id)
            || self.envelope.export.content_type
                != "application/vnd.aurora.permitted-federation-export+json"
            || self.envelope.export.semantic_loss != self.semantic_loss
            || !self.envelope.export.provenance.is_empty()
        {
            return Err(FederationWorkflowError::Contract(
                "federation export is not bound to workflow state".into(),
            ));
        }
        let payload = json!({
            "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
            "contract_version": CONTRACT_VERSION,
            "feature_id": FEATURE_ID,
            "workflow_id": self.workflow_id,
            "origin": self.origin,
            "destination": self.destination,
            "purpose": self.purpose,
            "permitted_budget_units": self.permitted_budget_units,
            "policy_allow": self.policy_allow,
            "authority_reference": self.authority_reference,
            "signature": self.signature,
            "network_partition": self.network_partition,
            "decision": self.decision,
            "task_order": self.task_order,
            "checkpoint_order": self.checkpoint_order,
            "compensation_order": self.compensation_order,
            "input_digest_order": self.input_digest_order,
            "total_budget_units": self.total_budget_units,
            "omissions": self.omissions,
            "uncertainty": self.uncertainty,
            "semantic_loss": self.semantic_loss,
            "reasons": self.reasons,
            "effect_receipt": self.effect_receipt,
            "task_digest": self.task_digest,
            "envelope_digest": self.envelope_digest,
            "raw_data_local": self.raw_data_local,
            "boundary": PRECLINICAL_BOUNDARY,
        });
        self.artifact
            .verify_payload(&payload)
            .map_err(|error| FederationWorkflowError::Contract(error.to_string()))?;
        self.artifact
            .validate_metadata()
            .map_err(|error| FederationWorkflowError::Contract(error.to_string()))?;
        if self.artifact.artifact_id != format!("federation-workflow:{}", self.workflow_id)
            || self.artifact.content_type
                != "application/vnd.aurora.federation-workflow-receipt+json"
            || self.artifact.semantic_loss != self.semantic_loss
            || !self.artifact.provenance.is_empty()
        {
            return Err(FederationWorkflowError::Contract(
                "workflow receipt artifact is not bound to workflow state".into(),
            ));
        }
        self.envelope
            .export
            .verify_payload(&payload)
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
    let input_digest_order = tasks
        .iter()
        .map(|task| task.input_digest.clone())
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
    reasons.sort();
    let effect_receipt = if decision == FederationWorkflowDecision::Scheduled {
        "schedule:research-work_no_remote_execution"
    } else {
        "retain_local_checkpoint_and_block_remote_schedule"
    };
    let task_digest = ContentHash::of_value(
        &serde_json::to_value(&tasks)
            .map_err(|error| FederationWorkflowError::Serialization(error.to_string()))?,
    )
    .map_err(|error| FederationWorkflowError::Serialization(error.to_string()))?;
    let envelope_signature = request
        .signature
        .clone()
        .or_else(|| Some("pending-approval".into()));
    let envelope_digest = ContentHash::of_value(&json!({
        "workflow_id": request.workflow_id,
        "origin": request.origin,
        "destination": request.destination,
        "purpose": request.purpose,
        "policy_constraints": [
            "preclinical-research-only",
            "raw-data-local",
            "policy-approved-artifacts-only"
        ],
        "integrity_evidence": input_digest_order.clone(),
        "localization_statement": "raw data remains institution-local; only permitted metadata may cross federation",
        "raw_data_local": request.raw_data_local,
        "signature": envelope_signature,
        "task_digest": task_digest,
    }))
    .map_err(|error| FederationWorkflowError::Serialization(error.to_string()))?;
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "workflow_id": request.workflow_id,
        "origin": request.origin,
        "destination": request.destination,
        "purpose": request.purpose,
        "permitted_budget_units": request.permitted_budget_units,
        "policy_allow": request.policy_allow,
        "authority_reference": request.authority_reference,
        "signature": request.signature,
        "network_partition": request.network_partition,
        "decision": decision,
        "task_order": task_order,
        "checkpoint_order": checkpoint_order,
        "compensation_order": compensation_order,
        "input_digest_order": input_digest_order,
        "total_budget_units": total_budget_units,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "semantic_loss": semantic_loss,
        "reasons": reasons,
        "effect_receipt": effect_receipt,
        "task_digest": task_digest,
        "envelope_digest": envelope_digest,
        "raw_data_local": request.raw_data_local,
        "boundary": PRECLINICAL_BOUNDARY,
    });
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
        integrity_evidence: input_digest_order.clone(),
        localization_statement:
            "raw data remains institution-local; only permitted metadata may cross federation"
                .into(),
        raw_data_local: request.raw_data_local,
        signature: envelope_signature,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let receipt = FederationWorkflowReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        workflow_id: request.workflow_id.clone(),
        origin: request.origin.clone(),
        destination: request.destination.clone(),
        purpose: request.purpose.clone(),
        permitted_budget_units: request.permitted_budget_units,
        policy_allow: request.policy_allow,
        authority_reference: request.authority_reference.clone(),
        signature: request.signature.clone(),
        network_partition: request.network_partition,
        decision,
        task_order,
        checkpoint_order,
        compensation_order,
        input_digest_order,
        total_budget_units,
        omissions,
        uncertainty,
        semantic_loss,
        reasons,
        effect_receipt: effect_receipt.into(),
        task_digest,
        envelope_digest,
        envelope,
        artifact,
        raw_data_local: request.raw_data_local,
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
        || request.tasks.len() > MAX_ITEMS
        || !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(FederationWorkflowError::InvalidRequest(
            "workflow identity, tasks, locality, purpose, and boundary are required".into(),
        ));
    }
    validate_text("workflow_id", &request.workflow_id)?;
    validate_text("origin", &request.origin)?;
    validate_text("destination", &request.destination)?;
    validate_text("purpose", &request.purpose)?;
    validate_text("boundary", &request.boundary)?;
    if !request.origin.starts_with("institution:")
        || !request.destination.starts_with("institution:")
    {
        return Err(FederationWorkflowError::InvalidRequest(
            "federation endpoints must use the institution namespace".into(),
        ));
    }
    if let Some(authority) = &request.authority_reference {
        validate_text("authority_reference", authority)?;
    }
    if let Some(signature) = &request.signature {
        validate_text("signature", signature)?;
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
        validate_text("task.task_id", &task.task_id)?;
        validate_text("task.action", &task.action)?;
        validate_text("task.resource", &task.resource)?;
        validate_text("task.checkpoint_id", &task.checkpoint_id)?;
        validate_text("task.compensation_action", &task.compensation_action)?;
        validate_digest("task.input_digest", &task.input_digest)?;
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

    #[test]
    fn task_digest_binds_task_details() {
        let mut receipt = schedule_federation_workflow(&request()).unwrap();
        receipt.task_digest = ContentHash::of_bytes(b"tampered-task");
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn export_payload_tampering_is_rejected() {
        let mut receipt = schedule_federation_workflow(&request()).unwrap();
        receipt.envelope.export.content_hash = ContentHash::of_bytes(b"tampered-export");
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn invalid_institution_namespace_is_rejected() {
        let mut value = request();
        value.destination = "remote-endpoint".into();
        assert!(schedule_federation_workflow(&value).is_err());
    }

    #[test]
    fn partition_keeps_remote_schedule_blocked() {
        let mut value = request();
        value.network_partition = true;
        let receipt = schedule_federation_workflow(&value).unwrap();
        assert_eq!(
            receipt.effect_receipt,
            "retain_local_checkpoint_and_block_remote_schedule"
        );
    }
}

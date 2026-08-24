//! High-throughput deterministic workflow batch ledger.
//!
//! Atlas feature: `AFA-runtime-P12-F11`.

use crate::workflow_execution::{
    execute_workflow, WorkflowExecutionMode, WorkflowExecutionRequest, WorkflowExecutionStatus,
};
use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-runtime-P12-F11";
pub const FEATURE_VERSION: &str = "0.1.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowBatchMode {
    DryRun,
    Execute,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowBatchRequest {
    pub workflows: Vec<WorkflowExecutionRequest>,
    pub mode: WorkflowBatchMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowBatchDisposition {
    Succeeded,
    DryRun,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowBatchEntry {
    pub workflow_id: String,
    pub disposition: WorkflowBatchDisposition,
    pub receipt_digest: Option<ContentHash>,
    pub ordered_nodes: Vec<String>,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowBatchReceipt {
    pub schema_version: String,
    pub feature_id: String,
    pub total_workflows: usize,
    pub succeeded_workflows: usize,
    pub dry_run_workflows: usize,
    pub blocked_workflows: usize,
    pub entries: Vec<WorkflowBatchEntry>,
    pub artifact: TypedResearchArtifact,
    pub boundary: String,
}

impl WorkflowBatchReceipt {
    pub fn validate(&self) -> Result<(), WorkflowBatchError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
        {
            return Err(WorkflowBatchError::InvalidField(
                "schema, feature, or boundary".into(),
            ));
        }
        if self.total_workflows == 0
            || self.total_workflows != self.entries.len()
            || self.succeeded_workflows + self.dry_run_workflows + self.blocked_workflows
                != self.total_workflows
            || self
                .entries
                .iter()
                .any(|entry| entry.workflow_id.trim().is_empty() || entry.reasons.is_empty())
        {
            return Err(WorkflowBatchError::InvalidField(
                "workflow counts, identity, or reasons".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| WorkflowBatchError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, WorkflowBatchError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| WorkflowBatchError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| WorkflowBatchError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum WorkflowBatchError {
    #[error("invalid workflow batch field: {0}")]
    InvalidField(String),
    #[error("duplicate workflow id {0}")]
    DuplicateWorkflow(String),
    #[error("workflow execution error: {0}")]
    Execution(String),
    #[error("artifact error: {0}")]
    Artifact(String),
    #[error("serialization error: {0}")]
    Serialization(String),
}

pub fn workflow_batch_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: FEATURE_VERSION.into(),
        owner_crate: "runtime".into(),
        consumers: ["research workflow operator".into(), "throughput scheduler".into()].into(),
        behavior: "preflights and executes a canonical batch of typed research workflows while retaining per-workflow ordering, status, digest, and failure reasons".into(),
        value: "turns prospective high-throughput workflow submission into an auditable ledger without hiding blocked runs".into(),
        inputs: vec![TypedPort { name: "workflow_batch_request".into(), schema: "WorkflowBatchRequest@1".into(), required: true }],
        outputs: vec![TypedPort { name: "workflow_batch_receipt".into(), schema: "WorkflowBatchReceipt@1".into(), required: true }],
        effects: [Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(),
        permissions: ["invoke:declared-tools".into(), "write:local-research-artifact".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "mcp-2025-06-18".into(), state: EvidenceState::Supported, locator: Some("https://modelcontextprotocol.io/specification/2025-06-18/basic/index".into()) }],
        authority_requirements: Vec::new(),
        autonomy_tier: AutonomyTier::A2,
        surfaces: [ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn execute_workflow_batch(
    request: &WorkflowBatchRequest,
) -> Result<WorkflowBatchReceipt, WorkflowBatchError> {
    validate_request(request)?;
    let mut workflows = request.workflows.clone();
    workflows.sort_by(|left, right| left.workflow.workflow_id.cmp(&right.workflow.workflow_id));
    let mut entries = Vec::with_capacity(workflows.len());
    for mut workflow in workflows {
        workflow.mode = match request.mode {
            WorkflowBatchMode::DryRun => WorkflowExecutionMode::DryRun,
            WorkflowBatchMode::Execute => WorkflowExecutionMode::Execute,
        };
        let workflow_id = workflow.workflow.workflow_id.clone();
        match execute_workflow(&workflow) {
            Ok(receipt) => {
                let disposition = match receipt.status {
                    WorkflowExecutionStatus::DryRun => WorkflowBatchDisposition::DryRun,
                    WorkflowExecutionStatus::Succeeded => WorkflowBatchDisposition::Succeeded,
                };
                let digest = receipt
                    .digest()
                    .map_err(|error| WorkflowBatchError::Execution(error.to_string()))?;
                entries.push(WorkflowBatchEntry {
                    workflow_id,
                    disposition,
                    receipt_digest: Some(digest),
                    ordered_nodes: receipt.ordered_nodes,
                    reasons: receipt.reasons,
                });
            }
            Err(error) => entries.push(WorkflowBatchEntry {
                workflow_id,
                disposition: WorkflowBatchDisposition::Blocked,
                receipt_digest: None,
                ordered_nodes: Vec::new(),
                reasons: vec![error.to_string()],
            }),
        }
    }
    let succeeded_workflows = entries
        .iter()
        .filter(|entry| entry.disposition == WorkflowBatchDisposition::Succeeded)
        .count();
    let dry_run_workflows = entries
        .iter()
        .filter(|entry| entry.disposition == WorkflowBatchDisposition::DryRun)
        .count();
    let blocked_workflows = entries
        .iter()
        .filter(|entry| entry.disposition == WorkflowBatchDisposition::Blocked)
        .count();
    let payload = json!({ "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "feature_id": FEATURE_ID, "total_workflows": entries.len(), "succeeded_workflows": succeeded_workflows, "dry_run_workflows": dry_run_workflows, "blocked_workflows": blocked_workflows, "entries": entries, "boundary": PRECLINICAL_BOUNDARY });
    let artifact = TypedResearchArtifact::from_payload(
        "workflow-batch",
        "application/vnd.aurora.workflow-batch+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| WorkflowBatchError::Artifact(error.to_string()))?;
    let receipt = WorkflowBatchReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        total_workflows: entries.len(),
        succeeded_workflows,
        dry_run_workflows,
        blocked_workflows,
        entries: serde_json::from_value(payload["entries"].clone())
            .map_err(|error| WorkflowBatchError::Serialization(error.to_string()))?,
        artifact,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &WorkflowBatchRequest) -> Result<(), WorkflowBatchError> {
    if request.workflows.is_empty() {
        return Err(WorkflowBatchError::InvalidField(
            "at least one workflow is required".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for workflow in &request.workflows {
        if workflow.workflow.workflow_id.trim().is_empty()
            || !ids.insert(workflow.workflow.workflow_id.clone())
        {
            return Err(WorkflowBatchError::DuplicateWorkflow(
                workflow.workflow.workflow_id.clone(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_batch_is_rejected() {
        assert!(execute_workflow_batch(&WorkflowBatchRequest {
            workflows: Vec::new(),
            mode: WorkflowBatchMode::DryRun
        })
        .is_err());
    }
}

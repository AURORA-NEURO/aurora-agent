//! Dependency-closed execution manifest for reproducible glioma computation.
//!
//! P09-F14 compiles a computation DAG. This feature adds the production admission contract around
//! that DAG: every task has a pinned tool, typed input/output contract, local resource envelope,
//! retry policy, and expected output. Cycles, unpinned tools, undeclared effects, and non-local
//! execution are rejected before a scheduler or worker can be called.

use super::workflow::{
    compile_glioma_computation_workflow, GliomaComputationWorkflow, GliomaComputationWorkflowError,
    GliomaComputationWorkflowRequest,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P09-F05";
pub const OUTPUT_SCHEMA: &str = "GliomaWorkflowExecutionManifest1@1";
pub const MAX_TOOL_PINS: usize = 2_048;
pub const MAX_TASK_CONTRACTS: usize = 2_048;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowToolPin {
    pub task_id: String,
    pub tool_id: String,
    pub version: String,
    pub build_digest: ContentHash,
    pub source: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowEffect {
    ReadLocalArtifact,
    ExecuteLocalComputation,
    WriteLocalArtifact,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowTaskContract {
    pub task_id: String,
    pub input_schema_order: Vec<String>,
    pub output_schema: String,
    pub effects: Vec<WorkflowEffect>,
    pub deterministic: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowResourceClass {
    Cpu,
    Gpu,
    MemoryBound,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowResourceBinding {
    pub task_id: String,
    pub class: WorkflowResourceClass,
    pub cpu_milli: u32,
    pub memory_mb: u32,
    pub accelerator_count: u16,
    pub site_local: bool,
    pub max_concurrency: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowRetryPolicy {
    pub max_retries: u8,
    pub retryable_error_code_order: Vec<String>,
    pub backoff_ticks: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpectedWorkflowOutput {
    pub task_id: String,
    pub output_schema: String,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnsupportedWorkflowStep {
    pub step_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowExecutionManifestRequest {
    pub workflow: GliomaComputationWorkflowRequest,
    pub tool_pins: Vec<WorkflowToolPin>,
    pub task_contracts: Vec<WorkflowTaskContract>,
    pub resource_bindings: Vec<WorkflowResourceBinding>,
    pub retry_policy: WorkflowRetryPolicy,
    pub expected_outputs: Vec<ExpectedWorkflowOutput>,
    pub unsupported_steps: Vec<UnsupportedWorkflowStep>,
    pub replay_identity: ContentHash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowExecutionManifestDisposition {
    Ready,
    ResourceBlocked,
    Unsupported,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowExecutionManifest {
    pub feature_id: String,
    pub output_schema: String,
    pub workflow: GliomaComputationWorkflow,
    pub task_order: Vec<String>,
    pub tool_pins: Vec<WorkflowToolPin>,
    pub task_contracts: Vec<WorkflowTaskContract>,
    pub resource_bindings: Vec<WorkflowResourceBinding>,
    pub retry_policy: WorkflowRetryPolicy,
    pub expected_output_order: Vec<String>,
    pub unsupported_step_order: Vec<String>,
    pub disposition: WorkflowExecutionManifestDisposition,
    pub uncertainty: Vec<String>,
    pub next_action: String,
    pub replay_identity: ContentHash,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WorkflowExecutionManifestError {
    #[error("workflow execution manifest request is invalid: {0}")]
    InvalidRequest(String),
    #[error("workflow compilation failed: {0}")]
    Workflow(#[from] GliomaComputationWorkflowError),
    #[error("workflow execution manifest is invalid: {0}")]
    InvalidOutput(String),
    #[error("workflow execution manifest digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn safe_text(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 512
}

fn digest_input(manifest: &WorkflowExecutionManifest) -> serde_json::Value {
    serde_json::json!({
        "feature_id": manifest.feature_id,
        "output_schema": manifest.output_schema,
        "workflow": manifest.workflow,
        "task_order": manifest.task_order,
        "tool_pins": manifest.tool_pins,
        "task_contracts": manifest.task_contracts,
        "resource_bindings": manifest.resource_bindings,
        "retry_policy": manifest.retry_policy,
        "expected_output_order": manifest.expected_output_order,
        "unsupported_step_order": manifest.unsupported_step_order,
        "disposition": manifest.disposition,
        "uncertainty": manifest.uncertainty,
        "next_action": manifest.next_action,
        "replay_identity": manifest.replay_identity,
    })
}

fn validate_request(
    request: &WorkflowExecutionManifestRequest,
    workflow: &GliomaComputationWorkflow,
) -> Result<(), WorkflowExecutionManifestError> {
    if request.replay_identity != request.workflow.replay_identity
        || request.replay_identity.as_str().len() != 64
        || request.tool_pins.len() != workflow.candidates.len()
        || request.task_contracts.len() != workflow.candidates.len()
        || request.resource_bindings.len() != workflow.candidates.len()
        || request.tool_pins.len() > MAX_TOOL_PINS
        || request.task_contracts.len() > MAX_TASK_CONTRACTS
        || request.retry_policy.backoff_ticks == 0
        || request
            .retry_policy
            .retryable_error_code_order
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || request
            .retry_policy
            .retryable_error_code_order
            .iter()
            .any(|value| !safe_text(value))
        || request.expected_outputs.is_empty()
        || request
            .expected_outputs
            .iter()
            .any(|output| !safe_text(&output.task_id) || !safe_text(&output.output_schema))
        || request
            .unsupported_steps
            .iter()
            .any(|step| !safe_text(&step.step_id) || !safe_text(&step.reason))
    {
        return Err(WorkflowExecutionManifestError::InvalidRequest(
            "manifest identity, one contract/pin/resource per compiled task, bounded retry policy, and expected outputs are required".into(),
        ));
    }
    let task_ids = workflow
        .candidates
        .iter()
        .map(|candidate| candidate.candidate_id.clone())
        .collect::<BTreeSet<_>>();
    let mut pin_ids = BTreeSet::new();
    for pin in &request.tool_pins {
        if !task_ids.contains(&pin.task_id)
            || !safe_text(&pin.tool_id)
            || !safe_text(&pin.version)
            || !safe_text(&pin.source)
            || pin.build_digest.as_str().len() != 64
            || !pin_ids.insert(pin.task_id.clone())
        {
            return Err(WorkflowExecutionManifestError::InvalidRequest(
                "tool pins must cover each task exactly once with a pinned version/build/source"
                    .into(),
            ));
        }
    }
    let mut candidate_map = BTreeMap::new();
    for candidate in &workflow.candidates {
        candidate_map.insert(candidate.candidate_id.clone(), candidate);
    }
    let mut contract_ids = BTreeSet::new();
    for contract in &request.task_contracts {
        let candidate = candidate_map.get(&contract.task_id).ok_or_else(|| {
            WorkflowExecutionManifestError::InvalidRequest(
                "task contract references an unknown compiled task".into(),
            )
        })?;
        if !contract_ids.insert(contract.task_id.clone())
            || !safe_text(&contract.output_schema)
            || contract.output_schema != candidate.task.output_schema
            || contract
                .input_schema_order
                .iter()
                .any(|schema| !safe_text(schema))
            || contract
                .input_schema_order
                .windows(2)
                .any(|pair| pair[0] > pair[1])
            || contract.effects.is_empty()
            || !contract
                .effects
                .contains(&WorkflowEffect::ExecuteLocalComputation)
            || !contract
                .effects
                .contains(&WorkflowEffect::WriteLocalArtifact)
            || (!candidate.task.input_artifact_ids.is_empty()
                && !contract
                    .effects
                    .contains(&WorkflowEffect::ReadLocalArtifact))
        {
            return Err(WorkflowExecutionManifestError::InvalidRequest(
                "task contracts must match output schemas, declare sorted inputs, and declare local computation/read/write effects".into(),
            ));
        }
    }
    let mut resource_ids = BTreeSet::new();
    for resource in &request.resource_bindings {
        if !task_ids.contains(&resource.task_id)
            || !resource_ids.insert(resource.task_id.clone())
            || resource.cpu_milli == 0
            || resource.memory_mb == 0
            || resource.max_concurrency == 0
            || !resource.site_local
            || (resource.class == WorkflowResourceClass::Gpu && resource.accelerator_count == 0)
        {
            return Err(WorkflowExecutionManifestError::InvalidRequest(
                "resource bindings must be unique, positive, local, and class-compatible".into(),
            ));
        }
    }
    let mut expected_ids = BTreeSet::new();
    for output in &request.expected_outputs {
        let candidate = candidate_map.get(&output.task_id).ok_or_else(|| {
            WorkflowExecutionManifestError::InvalidRequest(
                "expected output references an unknown compiled task".into(),
            )
        })?;
        if !expected_ids.insert(output.task_id.clone())
            || output.output_schema != candidate.task.output_schema
        {
            return Err(WorkflowExecutionManifestError::InvalidRequest(
                "expected output identities and schemas must match compiled tasks".into(),
            ));
        }
    }
    let mut unsupported_ids = BTreeSet::new();
    for step in &request.unsupported_steps {
        if !unsupported_ids.insert(step.step_id.clone()) {
            return Err(WorkflowExecutionManifestError::InvalidRequest(
                "unsupported workflow steps must be unique".into(),
            ));
        }
    }
    Ok(())
}

fn validate_output(
    manifest: &WorkflowExecutionManifest,
) -> Result<(), WorkflowExecutionManifestError> {
    manifest.workflow.validate()?;
    if manifest.feature_id != FEATURE_ID
        || manifest.output_schema != OUTPUT_SCHEMA
        || manifest.task_order != manifest.workflow.dependency_order
        || manifest
            .task_order
            .windows(2)
            .any(|pair| pair[0] == pair[1])
        || manifest.tool_pins.len() != manifest.task_order.len()
        || manifest.task_contracts.len() != manifest.task_order.len()
        || manifest.resource_bindings.len() != manifest.task_order.len()
        || !canonical(&manifest.expected_output_order)
        || !canonical(&manifest.unsupported_step_order)
        || manifest.expected_output_order.is_empty()
        || manifest.retry_policy.backoff_ticks == 0
        || manifest
            .uncertainty
            .iter()
            .any(|item| item.trim().is_empty())
        || manifest.next_action.trim().is_empty()
        || (manifest.disposition == WorkflowExecutionManifestDisposition::Ready
            && (!manifest.unsupported_step_order.is_empty()
                || !manifest.workflow.within_declared_resources))
    {
        return Err(WorkflowExecutionManifestError::InvalidOutput(
            "manifest identity, graph binding, coverage, ordering, disposition, or action is invalid".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(manifest))
        .map_err(|error| WorkflowExecutionManifestError::Digest(error.to_string()))?;
    if expected != manifest.digest {
        return Err(WorkflowExecutionManifestError::InvalidOutput(
            "manifest digest is not content-addressed".into(),
        ));
    }
    Ok(())
}

impl WorkflowExecutionManifest {
    pub fn validate(&self) -> Result<(), WorkflowExecutionManifestError> {
        validate_output(self)
    }
}

/// Compile the workflow plus its pinned tools, effects, resources, and expected outputs.
pub fn compile_glioma_workflow_execution_manifest(
    request: &WorkflowExecutionManifestRequest,
) -> Result<WorkflowExecutionManifest, WorkflowExecutionManifestError> {
    let workflow = compile_glioma_computation_workflow(&request.workflow)?;
    validate_request(request, &workflow)?;
    let mut tool_pins = request.tool_pins.clone();
    tool_pins.sort_by(|left, right| left.task_id.cmp(&right.task_id));
    let mut task_contracts = request.task_contracts.clone();
    task_contracts.sort_by(|left, right| left.task_id.cmp(&right.task_id));
    let mut resource_bindings = request.resource_bindings.clone();
    resource_bindings.sort_by(|left, right| left.task_id.cmp(&right.task_id));
    let mut expected_outputs = request
        .expected_outputs
        .iter()
        .map(|output| output.task_id.clone())
        .collect::<Vec<_>>();
    expected_outputs.sort();
    let mut unsupported_steps = request
        .unsupported_steps
        .iter()
        .map(|step| step.step_id.clone())
        .collect::<Vec<_>>();
    unsupported_steps.sort();
    let disposition = if !unsupported_steps.is_empty() {
        WorkflowExecutionManifestDisposition::Unsupported
    } else if !workflow.within_declared_resources {
        WorkflowExecutionManifestDisposition::ResourceBlocked
    } else if expected_outputs.is_empty() {
        WorkflowExecutionManifestDisposition::Unresolved
    } else {
        WorkflowExecutionManifestDisposition::Ready
    };
    let mut uncertainty = workflow.uncertainty.clone();
    if !unsupported_steps.is_empty() {
        uncertainty
            .push("unsupported workflow steps remain outside the computation executor".into());
    }
    uncertainty.sort();
    uncertainty.dedup();
    let next_action = match disposition {
        WorkflowExecutionManifestDisposition::Ready => {
            "submit the dependency-closed manifest to the caller-owned local scheduler".into()
        }
        WorkflowExecutionManifestDisposition::ResourceBlocked => {
            "increase the approved resource envelope or reduce the workflow before scheduling"
                .into()
        }
        WorkflowExecutionManifestDisposition::Unsupported => {
            "replace or explicitly authorize every unsupported step before scheduling".into()
        }
        WorkflowExecutionManifestDisposition::Unresolved => {
            "declare at least one expected output before scheduling".into()
        }
    };
    let mut manifest = WorkflowExecutionManifest {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        task_order: workflow.dependency_order.clone(),
        workflow,
        tool_pins,
        task_contracts,
        resource_bindings,
        retry_policy: request.retry_policy.clone(),
        expected_output_order: expected_outputs,
        unsupported_step_order: unsupported_steps,
        disposition,
        uncertainty,
        next_action,
        replay_identity: request.replay_identity.clone(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-workflow-execution-manifest"),
    };
    manifest.digest = ContentHash::of_value(&digest_input(&manifest))
        .map_err(|error| WorkflowExecutionManifestError::Digest(error.to_string()))?;
    validate_output(&manifest)?;
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p09_reproducible_computation::workflow::tests::request as workflow_request;

    fn manifest_request() -> WorkflowExecutionManifestRequest {
        let workflow = compile_glioma_computation_workflow(&workflow_request()).unwrap();
        let tool_pins = workflow
            .candidates
            .iter()
            .map(|candidate| WorkflowToolPin {
                task_id: candidate.candidate_id.clone(),
                tool_id: format!("aurora-{}", candidate.task.operation as u8),
                version: "1.0.0".into(),
                build_digest: ContentHash::of_bytes(candidate.candidate_id.as_bytes()),
                source: "local-registry".into(),
            })
            .collect::<Vec<_>>();
        let task_contracts = workflow
            .candidates
            .iter()
            .map(|candidate| WorkflowTaskContract {
                task_id: candidate.candidate_id.clone(),
                input_schema_order: candidate
                    .task
                    .input_artifact_ids
                    .iter()
                    .map(|_| "GliomaLocalArtifactRef1@1".into())
                    .collect(),
                output_schema: candidate.task.output_schema.clone(),
                effects: vec![
                    WorkflowEffect::ReadLocalArtifact,
                    WorkflowEffect::ExecuteLocalComputation,
                    WorkflowEffect::WriteLocalArtifact,
                ],
                deterministic: true,
            })
            .collect::<Vec<_>>();
        let resource_bindings = workflow
            .candidates
            .iter()
            .map(|candidate| WorkflowResourceBinding {
                task_id: candidate.candidate_id.clone(),
                class: WorkflowResourceClass::Cpu,
                cpu_milli: 1_000,
                memory_mb: 512,
                accelerator_count: 0,
                site_local: true,
                max_concurrency: 1,
            })
            .collect::<Vec<_>>();
        let expected_outputs = workflow
            .requested_terminal_order
            .iter()
            .map(|task_id| ExpectedWorkflowOutput {
                task_id: task_id.clone(),
                output_schema: workflow
                    .candidates
                    .iter()
                    .find(|candidate| &candidate.candidate_id == task_id)
                    .map(|candidate| candidate.task.output_schema.clone())
                    .expect("terminal task exists"),
                required: true,
            })
            .collect();
        WorkflowExecutionManifestRequest {
            workflow: workflow_request(),
            tool_pins,
            task_contracts,
            resource_bindings,
            retry_policy: WorkflowRetryPolicy {
                max_retries: 1,
                retryable_error_code_order: vec!["transient".into(), "worker_busy".into()],
                backoff_ticks: 2,
            },
            expected_outputs,
            unsupported_steps: Vec::new(),
            replay_identity: workflow.replay_identity.clone(),
        }
    }

    #[test]
    fn manifest_is_ready_and_replay_stable() {
        let request = manifest_request();
        let first = compile_glioma_workflow_execution_manifest(&request).unwrap();
        let second = compile_glioma_workflow_execution_manifest(&request).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            first.disposition,
            WorkflowExecutionManifestDisposition::Ready
        );
        first.validate().unwrap();
        assert_eq!(first.task_order, first.workflow.dependency_order);
    }

    #[test]
    fn unpinned_tool_is_rejected_before_admission() {
        let mut request = manifest_request();
        request.tool_pins[0].version.clear();
        assert!(matches!(
            compile_glioma_workflow_execution_manifest(&request),
            Err(WorkflowExecutionManifestError::InvalidRequest(_))
        ));
    }

    #[test]
    fn unsupported_step_is_preserved_as_blocked_not_scheduled() {
        let mut request = manifest_request();
        request.unsupported_steps.push(UnsupportedWorkflowStep {
            step_id: "external-proprietary-segmentation".into(),
            reason: "tool is not available in the pinned local environment".into(),
        });
        let manifest = compile_glioma_workflow_execution_manifest(&request).unwrap();
        assert_eq!(
            manifest.disposition,
            WorkflowExecutionManifestDisposition::Unsupported
        );
        assert_eq!(
            manifest.unsupported_step_order,
            vec!["external-proprietary-segmentation"]
        );
        assert!(manifest.next_action.contains("unsupported"));
    }

    #[test]
    fn non_local_resource_is_rejected() {
        let mut request = manifest_request();
        request.resource_bindings[0].site_local = false;
        assert!(compile_glioma_workflow_execution_manifest(&request).is_err());
    }
}

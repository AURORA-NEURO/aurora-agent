//! Versioned, idempotent task handoff for autonomous preclinical glioma computation.
//!
//! The API is a pre-dispatch product boundary. It accepts a typed local task, a qualified
//! environment lock, a policy grant, and a resource budget, then returns a replayable handle.
//! It never executes code or moves raw data; institution-local workers consume the handle after
//! this gate. Duplicate submissions converge, while a reused key with changed scientific inputs
//! fails closed.

use super::execution_environment_lock::{
    ComputeEnvironmentLock, ComputeEnvironmentLockDisposition,
};
use crate::glioma_engine::LocalArtifactRef;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P09-F21";
pub const OUTPUT_SCHEMA: &str = "GliomaReproducibleTaskExchange1@1";
pub const MAX_TASKS_PER_EXCHANGE: usize = 1;
pub const MAX_INPUTS: usize = 256;
pub const MAX_PRIOR_SUBMISSIONS: usize = 4_096;
pub const MAX_TEXT_LEN: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReproducibleTaskInput {
    pub artifact: LocalArtifactRef,
    pub schema: String,
    pub authorized: bool,
    pub local_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReproducibleTaskSpec {
    pub task_id: String,
    pub workflow_id: String,
    pub replay_identity: ContentHash,
    pub input_schema_order: Vec<String>,
    pub inputs: Vec<ReproducibleTaskInput>,
    pub output_schema: String,
    pub estimated_cost_units: u64,
    pub estimated_duration_ticks: u64,
    pub deterministic: bool,
    pub locality_required: bool,
    pub effects_local_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReproducibleTaskPolicyGrant {
    pub grant_id: String,
    pub site_id: String,
    pub approved: bool,
    pub allow_local_compute: bool,
    pub allow_external_effects: bool,
    pub expires_at_tick: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReproducibleTaskResourceBudget {
    pub max_cost_units: u64,
    pub max_duration_ticks: u64,
    pub max_memory_mb: u32,
    pub max_accelerator_count: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReproducibleTaskSubmission {
    pub idempotency_key: String,
    pub task: ReproducibleTaskSpec,
    pub environment_lock: ComputeEnvironmentLock,
    pub policy: ReproducibleTaskPolicyGrant,
    pub budget: ReproducibleTaskResourceBudget,
    pub current_tick: u64,
    pub prior_submissions: Vec<ReproducibleTaskExchangeRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReproducibleTaskExchangeStatus {
    Ready,
    Duplicate,
    Blocked,
    ReplayMismatch,
    Unauthorized,
    OverBudget,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReproducibleTaskResultState {
    NotStarted,
    ExistingHandle,
    Partial,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReproducibleExecutionHandle {
    pub handle_id: String,
    pub idempotency_key: String,
    pub request_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub event_cursor: ContentHash,
    pub local_only: bool,
    pub expires_at_tick: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReproducibleTaskExchangeRecord {
    pub idempotency_key: String,
    pub request_digest: ContentHash,
    pub handle: ReproducibleExecutionHandle,
    pub status: ReproducibleTaskExchangeStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReproducibleTaskExchange {
    pub feature_id: String,
    pub output_schema: String,
    pub task_id: String,
    pub workflow_id: String,
    pub status: ReproducibleTaskExchangeStatus,
    pub result_state: ReproducibleTaskResultState,
    pub handle: ReproducibleExecutionHandle,
    pub request_digest: ContentHash,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub next_action: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReproducibleTaskApiError {
    #[error("reproducible task submission is invalid: {0}")]
    InvalidRequest(String),
    #[error("reproducible task exchange is invalid: {0}")]
    InvalidOutput(String),
    #[error("reproducible task digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique_non_empty(values: &[String], max: usize) -> bool {
    let mut seen = BTreeSet::new();
    values.len() <= max
        && values.iter().all(|value| {
            !value.trim().is_empty() && value.len() <= MAX_TEXT_LEN && seen.insert(value)
        })
}

fn valid_hash(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

fn request_digest_input(request: &ReproducibleTaskSubmission) -> serde_json::Value {
    serde_json::json!({
        "idempotency_key": request.idempotency_key,
        "task": request.task,
        "environment_lock_digest": request.environment_lock.digest,
        "policy": request.policy,
        "budget": request.budget,
        "current_tick": request.current_tick,
    })
}

fn digest_input(exchange: &ReproducibleTaskExchange) -> serde_json::Value {
    serde_json::json!({
        "feature_id": exchange.feature_id,
        "output_schema": exchange.output_schema,
        "task_id": exchange.task_id,
        "workflow_id": exchange.workflow_id,
        "status": exchange.status,
        "result_state": exchange.result_state,
        "handle": exchange.handle,
        "request_digest": exchange.request_digest,
        "negative_evidence": exchange.negative_evidence,
        "uncertainty": exchange.uncertainty,
        "next_action": exchange.next_action,
    })
}

fn validate_task(task: &ReproducibleTaskSpec) -> Result<(), ReproducibleTaskApiError> {
    if task.task_id.trim().is_empty()
        || task.workflow_id.trim().is_empty()
        || task.task_id.len() > MAX_TEXT_LEN
        || task.workflow_id.len() > MAX_TEXT_LEN
        || !valid_hash(&task.replay_identity)
        || task.input_schema_order.is_empty()
        || !unique_non_empty(&task.input_schema_order, MAX_INPUTS)
        || !canonical(&task.input_schema_order)
        || task.inputs.is_empty()
        || task.inputs.len() > MAX_INPUTS
        || task.output_schema.trim().is_empty()
        || task.output_schema.len() > MAX_TEXT_LEN
        || task.estimated_cost_units == 0
        || task.estimated_duration_ticks == 0
        || !task.effects_local_only
    {
        return Err(ReproducibleTaskApiError::InvalidRequest(
            "task identity, replay identity, canonical input schemas, bounded local inputs, output schema, cost, duration, and local effects are required".into(),
        ));
    }
    for input in &task.inputs {
        if input.schema.trim().is_empty()
            || input.schema.len() > MAX_TEXT_LEN
            || !input.authorized
            || (task.locality_required && !input.local_only)
            || input.artifact.validate().is_err()
            || !input.artifact.local_only
            || input.artifact.contains_human_data
            || input.artifact.contains_direct_identifiers
        {
            return Err(ReproducibleTaskApiError::InvalidRequest(
                "inputs must be authorized local artifacts without human data or direct identifiers".into(),
            ));
        }
    }
    Ok(())
}

fn validate_policy(policy: &ReproducibleTaskPolicyGrant) -> Result<(), ReproducibleTaskApiError> {
    if policy.grant_id.trim().is_empty()
        || policy.site_id.trim().is_empty()
        || policy.grant_id.len() > MAX_TEXT_LEN
        || policy.site_id.len() > MAX_TEXT_LEN
        || policy.expires_at_tick == 0
        || policy.allow_external_effects
    {
        return Err(ReproducibleTaskApiError::InvalidRequest(
            "policy grant must be bounded, expiring, local-only, and explicitly identified".into(),
        ));
    }
    Ok(())
}

fn validate_budget(
    budget: &ReproducibleTaskResourceBudget,
) -> Result<(), ReproducibleTaskApiError> {
    if budget.max_cost_units == 0 || budget.max_duration_ticks == 0 || budget.max_memory_mb == 0 {
        return Err(ReproducibleTaskApiError::InvalidRequest(
            "positive local cost, duration, and memory budgets are required".into(),
        ));
    }
    Ok(())
}

fn validate_submission(
    request: &ReproducibleTaskSubmission,
) -> Result<(), ReproducibleTaskApiError> {
    if request.idempotency_key.trim().is_empty()
        || request.idempotency_key.len() > MAX_TEXT_LEN
        || request.current_tick == 0
        || request.prior_submissions.len() > MAX_PRIOR_SUBMISSIONS
        || request
            .prior_submissions
            .windows(2)
            .any(|pair| pair[0].idempotency_key >= pair[1].idempotency_key)
    {
        return Err(ReproducibleTaskApiError::InvalidRequest(
            "bounded idempotency key, current tick, and canonical prior submissions are required"
                .into(),
        ));
    }
    validate_task(&request.task)?;
    validate_policy(&request.policy)?;
    validate_budget(&request.budget)?;
    if request.environment_lock.feature_id != "GAF-GLIOMA-P09-F06"
        || request.environment_lock.validate().is_err()
        || request.environment_lock.disposition != ComputeEnvironmentLockDisposition::Qualified
        || request.environment_lock.digest.as_str().len() != 64
    {
        return Err(ReproducibleTaskApiError::InvalidRequest(
            "a valid qualified P09-F06 environment lock is required before task admission".into(),
        ));
    }
    let mut keys = BTreeSet::new();
    for prior in &request.prior_submissions {
        if !keys.insert(prior.idempotency_key.clone())
            || prior.idempotency_key.trim().is_empty()
            || !valid_hash(&prior.request_digest)
            || prior.handle.idempotency_key != prior.idempotency_key
            || !valid_hash(&prior.handle.request_digest)
        {
            return Err(ReproducibleTaskApiError::InvalidRequest(
                "prior submission records must have unique keys and valid replay handles".into(),
            ));
        }
    }
    Ok(())
}

impl ReproducibleTaskExchange {
    pub fn validate(&self) -> Result<(), ReproducibleTaskApiError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.task_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || !valid_hash(&self.request_digest)
            || self.handle.idempotency_key.trim().is_empty()
            || self.handle.request_digest != self.request_digest
            || !valid_hash(&self.handle.event_cursor)
            || !valid_hash(&self.handle.replay_identity)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.next_action.trim().is_empty()
        {
            return Err(ReproducibleTaskApiError::InvalidOutput(
                "exchange identity, handle binding, evidence ordering, and next action are invalid"
                    .into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ReproducibleTaskApiError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ReproducibleTaskApiError::InvalidOutput(
                "task exchange digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Admit a typed local computation task and return an idempotent replayable handle.
pub fn submit_glioma_reproducible_task(
    request: &ReproducibleTaskSubmission,
) -> Result<ReproducibleTaskExchange, ReproducibleTaskApiError> {
    validate_submission(request)?;
    let request_digest = ContentHash::of_value(&request_digest_input(request))
        .map_err(|error| ReproducibleTaskApiError::Digest(error.to_string()))?;
    let prior = request
        .prior_submissions
        .iter()
        .find(|prior| prior.idempotency_key == request.idempotency_key);
    let mut status = ReproducibleTaskExchangeStatus::Ready;
    let mut result_state = ReproducibleTaskResultState::NotStarted;
    let mut negative_evidence = Vec::new();
    let uncertainty = Vec::new();
    let handle_id = format!("glioma-task:{}", request.idempotency_key);
    let event_cursor = ContentHash::of_value(&serde_json::json!({
        "task_id": request.task.task_id,
        "workflow_id": request.task.workflow_id,
        "request_digest": request_digest,
    }))
    .map_err(|error| ReproducibleTaskApiError::Digest(error.to_string()))?;
    let mut handle = ReproducibleExecutionHandle {
        handle_id,
        idempotency_key: request.idempotency_key.clone(),
        request_digest: request_digest.clone(),
        replay_identity: request.task.replay_identity.clone(),
        event_cursor,
        local_only: true,
        expires_at_tick: request.policy.expires_at_tick,
    };
    if let Some(prior) = prior {
        if prior.request_digest == request_digest {
            status = ReproducibleTaskExchangeStatus::Duplicate;
            result_state = ReproducibleTaskResultState::ExistingHandle;
            handle = prior.handle.clone();
        } else {
            status = ReproducibleTaskExchangeStatus::ReplayMismatch;
            negative_evidence.push("idempotency-key-reused-with-different-task-contract".into());
        }
    } else if !request.policy.approved || !request.policy.allow_local_compute {
        status = ReproducibleTaskExchangeStatus::Unauthorized;
        negative_evidence.push("policy-grant-does-not-authorize-local-computation".into());
    } else if request.current_tick >= request.policy.expires_at_tick {
        status = ReproducibleTaskExchangeStatus::Blocked;
        negative_evidence.push("policy-grant-expired-before-dispatch".into());
    } else if request.task.estimated_cost_units > request.budget.max_cost_units
        || request.task.estimated_duration_ticks > request.budget.max_duration_ticks
    {
        status = ReproducibleTaskExchangeStatus::OverBudget;
        negative_evidence.push("task-exceeds-declared-local-resource-budget".into());
    } else if request.environment_lock.disposition != ComputeEnvironmentLockDisposition::Qualified {
        status = ReproducibleTaskExchangeStatus::Blocked;
        negative_evidence.push("environment-lock-is-not-qualified".into());
    }
    negative_evidence.sort();
    let next_action = match status {
        ReproducibleTaskExchangeStatus::Ready => "pass_handle_to_institution_local_executor",
        ReproducibleTaskExchangeStatus::Duplicate => "reuse_existing_handle_without_resubmission",
        ReproducibleTaskExchangeStatus::ReplayMismatch => {
            "stop_and_issue_new_idempotency_key_after_review"
        }
        ReproducibleTaskExchangeStatus::Unauthorized => "obtain_or_correct_local_policy_grant",
        ReproducibleTaskExchangeStatus::OverBudget => "replan_task_or_increase_bounded_budget",
        ReproducibleTaskExchangeStatus::Blocked => "repair_preflight_blocker_before_dispatch",
        ReproducibleTaskExchangeStatus::Rejected => "operator_review_required",
    };
    let mut exchange = ReproducibleTaskExchange {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        task_id: request.task.task_id.clone(),
        workflow_id: request.task.workflow_id.clone(),
        status,
        result_state,
        handle,
        request_digest,
        negative_evidence,
        uncertainty,
        next_action: next_action.into(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-reproducible-task-exchange"),
    };
    exchange.digest = ContentHash::of_value(&digest_input(&exchange))
        .map_err(|error| ReproducibleTaskApiError::Digest(error.to_string()))?;
    exchange.validate()?;
    Ok(exchange)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p09_reproducible_computation::execution_environment_lock::{
        ComputeEnvironmentDependency, ComputeEnvironmentLockRequest,
        EnvironmentArchitectureProfile, EnvironmentDependencyKind,
    };

    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }

    fn lock() -> ComputeEnvironmentLock {
        let dependency = ComputeEnvironmentDependency {
            name: "numpy".into(),
            kind: EnvironmentDependencyKind::Library,
            version_constraint: "=2.1.0".into(),
            resolved_version: "2.1.0".into(),
            source: "registry://trusted".into(),
            source_digest: hash("source"),
            build_digest: hash("build"),
            runtime_abi: "abi-v1".into(),
            required: true,
            available: true,
            portable: true,
            metadata_signed: true,
            source_mutable: false,
            compromised: false,
            compatible_architecture_order: vec!["x86_64".into()],
            contains_human_data: false,
            contains_clinical_decision: false,
        };
        let request = ComputeEnvironmentLockRequest {
            objective: "qualify task api test environment".into(),
            workflow_manifest_digest: hash("workflow"),
            workflow_task_order: vec!["model".into()],
            architecture: EnvironmentArchitectureProfile {
                os_family: "linux".into(),
                os_version: "6.8".into(),
                architecture: "x86_64".into(),
                abi: "gnu".into(),
                cpu_feature_order: vec!["avx2".into()],
                accelerator_order: vec!["none".into()],
                host_digest: hash("host"),
            },
            dependencies: vec![dependency],
            trusted_source_order: vec!["registry://trusted".into()],
            require_signed_metadata: true,
            require_portable_dependencies: true,
        };
        super::super::execution_environment_lock::lock_glioma_compute_environment(&request)
            .expect("qualified lock")
    }

    fn submission(key: &str) -> ReproducibleTaskSubmission {
        ReproducibleTaskSubmission {
            idempotency_key: key.into(),
            task: ReproducibleTaskSpec {
                task_id: "normalize".into(),
                workflow_id: "glioma-workflow".into(),
                replay_identity: hash("replay"),
                input_schema_order: vec!["image-stack".into()],
                inputs: vec![ReproducibleTaskInput {
                    artifact: LocalArtifactRef {
                        artifact_id: "local-input".into(),
                        content_hash: hash("input"),
                        content_type: "image-stack".into(),
                        local_only: true,
                        contains_human_data: false,
                        contains_direct_identifiers: false,
                    },
                    schema: "image-stack".into(),
                    authorized: true,
                    local_only: true,
                }],
                output_schema: "normalized-image-stack".into(),
                estimated_cost_units: 2,
                estimated_duration_ticks: 5,
                deterministic: true,
                locality_required: true,
                effects_local_only: true,
            },
            environment_lock: lock(),
            policy: ReproducibleTaskPolicyGrant {
                grant_id: "grant-1".into(),
                site_id: "site-a".into(),
                approved: true,
                allow_local_compute: true,
                allow_external_effects: false,
                expires_at_tick: 100,
            },
            budget: ReproducibleTaskResourceBudget {
                max_cost_units: 10,
                max_duration_ticks: 20,
                max_memory_mb: 1024,
                max_accelerator_count: 0,
            },
            current_tick: 1,
            prior_submissions: Vec::new(),
        }
    }

    #[test]
    fn qualified_task_returns_replayable_ready_handle() {
        let exchange = submit_glioma_reproducible_task(&submission("key-1")).expect("admit");
        assert_eq!(exchange.status, ReproducibleTaskExchangeStatus::Ready);
        assert_eq!(
            exchange.result_state,
            ReproducibleTaskResultState::NotStarted
        );
        assert!(exchange.handle.local_only);
    }

    #[test]
    fn duplicate_submission_is_idempotent_and_conflict_is_replay_mismatch() {
        let first_request = submission("key-1");
        let first = submit_glioma_reproducible_task(&first_request).expect("first");
        let mut duplicate = submission("key-1");
        duplicate.prior_submissions = vec![ReproducibleTaskExchangeRecord {
            idempotency_key: "key-1".into(),
            request_digest: first.request_digest.clone(),
            handle: first.handle.clone(),
            status: first.status,
        }];
        let second = submit_glioma_reproducible_task(&duplicate).expect("duplicate");
        assert_eq!(second.status, ReproducibleTaskExchangeStatus::Duplicate);
        assert_eq!(second.handle.handle_id, first.handle.handle_id);
        let mut conflict = duplicate;
        conflict.task.output_schema = "different-output".into();
        let mismatch = submit_glioma_reproducible_task(&conflict).expect("mismatch");
        assert_eq!(
            mismatch.status,
            ReproducibleTaskExchangeStatus::ReplayMismatch
        );
    }

    #[test]
    fn unauthorized_or_over_budget_work_never_becomes_ready() {
        let mut unauthorized = submission("key-unauthorized");
        unauthorized.policy.approved = false;
        let denied = submit_glioma_reproducible_task(&unauthorized).expect("denied");
        assert_eq!(denied.status, ReproducibleTaskExchangeStatus::Unauthorized);
        let mut over_budget = submission("key-budget");
        over_budget.budget.max_cost_units = 1;
        let bounded = submit_glioma_reproducible_task(&over_budget).expect("budget");
        assert_eq!(bounded.status, ReproducibleTaskExchangeStatus::OverBudget);
    }

    #[test]
    fn unauthorized_artifact_is_rejected_before_dispatch() {
        let mut request = submission("key-artifact");
        request.task.inputs[0].authorized = false;
        assert!(matches!(
            submit_glioma_reproducible_task(&request),
            Err(ReproducibleTaskApiError::InvalidRequest(_))
        ));
    }
}

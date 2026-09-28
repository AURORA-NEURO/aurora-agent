//! Bind institution-local cross-program stage receipts to P04 multi-study action assignments.
//!
//! P04-F31 adapts validated outputs from the shared glioma engine to the explicit local outcome
//! contract consumed by P04-F06. It binds each receipt to the exact workflow task, study, and
//! stage kind, retains only a bounded digest-addressed snapshot, and fails closed when an engine
//! stage is negative, partial, or blocked. A stage's negative label is not treated as a biological
//! negative result because the generic engine receipt cannot distinguish dry-run output from a
//! measured null result.

use super::context_replay::{DecisionContextActionOutcome, DecisionContextActionOutcomeStatus};
use super::multi_study_context_artifact::MultiStudyDecisionContextArtifact;
use super::multi_study_workflow::{MAX_TASKS, MultiStudyTaskDisposition, MultiStudyWorkflowPlan};
use crate::glioma_engine::{
    CONTRACT_VERSION as ENGINE_CONTRACT_VERSION, FEATURE_ID as ENGINE_FEATURE_ID,
    GliomaExecutionReceipt, GliomaStageDisposition,
};
use bioprism_foundation::PRECLINICAL_BOUNDARY;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = super::adaptive_context_scheduler::FEATURE_ID;
pub const OUTPUT_SCHEMA: &str = "GliomaMultiStudyExecutionReceipt1@1";
pub const MAX_SOURCE_RECEIPTS: usize = 256;
pub const MAX_BINDINGS: usize = MAX_TASKS;
pub const MAX_REQUEST_BYTES: usize = 16_000_000;
pub const MAX_REPORT_BYTES: usize = 24_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiStudyExecutionReceiptBinding {
    pub task_id: String,
    pub execution_digest: ContentHash,
    pub stage_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MultiStudyExecutionReceiptRequest {
    pub workflow_plan: MultiStudyWorkflowPlan,
    pub context_artifact: MultiStudyDecisionContextArtifact,
    pub source_receipts: Vec<GliomaExecutionReceipt>,
    pub bindings: Vec<MultiStudyExecutionReceiptBinding>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultiStudyExecutionReceiptDisposition {
    Complete,
    Partial,
    Awaiting,
    NotExecutable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiStudyExecutionStageSnapshot {
    pub stage_id: String,
    pub attempts: u8,
    pub disposition: GliomaStageDisposition,
    pub artifact_digest: Option<ContentHash>,
    pub error_digest: Option<ContentHash>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiStudyExecutionSourceSnapshot {
    pub source_feature_id: String,
    pub source_contract_version: String,
    pub research_id: String,
    pub study_id: String,
    pub source_plan_digest: ContentHash,
    pub execution_digest: ContentHash,
    pub disposition: String,
    pub boundary: String,
    pub stage_order: Vec<MultiStudyExecutionStageSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiStudyExecutionTaskSnapshot {
    pub task_id: String,
    pub action_id: String,
    pub study_id: String,
    pub stage_id: String,
    pub disposition: MultiStudyTaskDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiStudyExecutionActionOutcome {
    pub task_id: String,
    pub action_id: String,
    pub study_id: String,
    pub stage_id: String,
    pub source_feature_id: String,
    pub source_plan_digest: ContentHash,
    pub source_execution_digest: ContentHash,
    pub source_disposition: GliomaStageDisposition,
    pub source_artifact_digest: Option<ContentHash>,
    pub source_error_digest: Option<ContentHash>,
    pub status: DecisionContextActionOutcomeStatus,
    pub result_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiStudyExecutionReceiptReport {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub epoch: u32,
    pub source_context_digest: ContentHash,
    pub workflow_plan_digest: ContentHash,
    pub task_order: Vec<String>,
    pub task_snapshots: Vec<MultiStudyExecutionTaskSnapshot>,
    pub source_execution_order: Vec<MultiStudyExecutionSourceSnapshot>,
    pub binding_order: Vec<MultiStudyExecutionReceiptBinding>,
    pub source_execution_digest_order: Vec<ContentHash>,
    pub outcome_order: Vec<MultiStudyExecutionActionOutcome>,
    pub missing_task_order: Vec<String>,
    pub approval_hold_task_order: Vec<String>,
    pub disposition: MultiStudyExecutionReceiptDisposition,
    pub next_route: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MultiStudyExecutionReceiptError {
    #[error("multi-study execution receipt request is invalid: {0}")]
    InvalidRequest(String),
    #[error("multi-study execution receipt report is invalid: {0}")]
    InvalidOutput(String),
    #[error("multi-study execution receipt digest failed: {0}")]
    Digest(String),
}

struct DerivedMultiStudyExecutionReceiptFields {
    source_execution_digest_order: Vec<ContentHash>,
    outcome_order: Vec<MultiStudyExecutionActionOutcome>,
    missing_task_order: Vec<String>,
    approval_hold_task_order: Vec<String>,
    disposition: MultiStudyExecutionReceiptDisposition,
    next_route: String,
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn ranked_unique(values: &[String]) -> bool {
    let mut seen = BTreeSet::new();
    values
        .iter()
        .all(|value| !value.trim().is_empty() && seen.insert(value))
}

fn digest_input(report: &MultiStudyExecutionReceiptReport) -> serde_json::Value {
    json!({
        "feature_id": report.feature_id,
        "output_schema": report.output_schema,
        "objective": report.objective,
        "epoch": report.epoch,
        "source_context_digest": report.source_context_digest,
        "workflow_plan_digest": report.workflow_plan_digest,
        "task_order": report.task_order,
        "task_snapshots": report.task_snapshots,
        "source_execution_order": report.source_execution_order,
        "binding_order": report.binding_order,
        "source_execution_digest_order": report.source_execution_digest_order,
        "outcome_order": report.outcome_order,
        "missing_task_order": report.missing_task_order,
        "approval_hold_task_order": report.approval_hold_task_order,
        "disposition": report.disposition,
        "next_route": report.next_route,
    })
}

fn fail_request(message: impl Into<String>) -> MultiStudyExecutionReceiptError {
    MultiStudyExecutionReceiptError::InvalidRequest(message.into())
}

fn stage_status(stage: &MultiStudyExecutionStageSnapshot) -> DecisionContextActionOutcomeStatus {
    match stage.disposition {
        GliomaStageDisposition::Completed => DecisionContextActionOutcomeStatus::Completed,
        // The generic execution receipt does not distinguish a measured negative result from a
        // simulation marker, so those dispositions remain explicitly unknown to P04.
        GliomaStageDisposition::Negative | GliomaStageDisposition::Partial => {
            DecisionContextActionOutcomeStatus::Unknown
        }
        GliomaStageDisposition::Blocked if stage.error_digest.is_some() => {
            DecisionContextActionOutcomeStatus::Failed
        }
        GliomaStageDisposition::Blocked => DecisionContextActionOutcomeStatus::Blocked,
    }
}

fn result_digest(
    source: &MultiStudyExecutionSourceSnapshot,
    stage: &MultiStudyExecutionStageSnapshot,
) -> Result<ContentHash, MultiStudyExecutionReceiptError> {
    if let Some(digest) = &stage.artifact_digest {
        return Ok(digest.clone());
    }
    ContentHash::of_value(&json!({
        "execution_digest": source.execution_digest,
        "stage_id": stage.stage_id,
        "disposition": stage.disposition,
        "error_digest": stage.error_digest,
    }))
    .map_err(|error| MultiStudyExecutionReceiptError::Digest(error.to_string()))
}

fn snapshot_source(receipt: &GliomaExecutionReceipt) -> MultiStudyExecutionSourceSnapshot {
    let mut stage_order = receipt
        .stages
        .iter()
        .map(|stage| MultiStudyExecutionStageSnapshot {
            stage_id: stage.stage_id.clone(),
            attempts: stage.attempts,
            disposition: stage.disposition,
            artifact_digest: stage.artifact_digest.clone(),
            error_digest: stage
                .error
                .as_deref()
                .map(|error| ContentHash::of_bytes(error.as_bytes())),
        })
        .collect::<Vec<_>>();
    stage_order.sort_by(|left, right| left.stage_id.cmp(&right.stage_id));
    MultiStudyExecutionSourceSnapshot {
        source_feature_id: receipt.feature_id.clone(),
        source_contract_version: receipt.contract_version.clone(),
        research_id: receipt.research_id.clone(),
        study_id: receipt.study_id.clone(),
        source_plan_digest: receipt.plan_digest.clone(),
        execution_digest: receipt.execution_digest.clone(),
        disposition: receipt.disposition.clone(),
        boundary: receipt.boundary.clone(),
        stage_order,
    }
}

fn validate_request(
    request: &MultiStudyExecutionReceiptRequest,
) -> Result<(), MultiStudyExecutionReceiptError> {
    request
        .workflow_plan
        .validate()
        .map_err(|error| fail_request(format!("workflow plan is invalid: {error}")))?;
    request
        .context_artifact
        .validate()
        .map_err(|error| fail_request(format!("context artifact is invalid: {error}")))?;
    if request.workflow_plan.objective != request.context_artifact.objective
        || request.workflow_plan.epoch != request.context_artifact.epoch
        || request.workflow_plan.source_artifact_digest != request.context_artifact.digest
        || request.workflow_plan.study_order != request.context_artifact.study_order
        || request.workflow_plan.eligible_study_order
            != request
                .workflow_plan
                .eligible_study_order
                .iter()
                .filter(|study_id| {
                    request
                        .context_artifact
                        .eligible_study_order
                        .contains(study_id)
                })
                .cloned()
                .collect::<Vec<_>>()
        || request.source_receipts.len() > MAX_SOURCE_RECEIPTS
        || request.bindings.len() > MAX_BINDINGS
    {
        return Err(fail_request(
            "workflow/context identities, study lineage, receipt count, or binding count are invalid",
        ));
    }
    let request_bytes = serde_json::to_vec(request)
        .map_err(|error| fail_request(error.to_string()))?
        .len();
    if request_bytes > MAX_REQUEST_BYTES {
        return Err(fail_request(format!(
            "request is {request_bytes} bytes, above the {MAX_REQUEST_BYTES}-byte limit"
        )));
    }

    let task_by_id = request
        .workflow_plan
        .tasks
        .iter()
        .map(|task| (task.task_id.as_str(), task))
        .collect::<BTreeMap<_, _>>();
    if request
        .workflow_plan
        .tasks
        .iter()
        .map(|task| (task.action_id.as_str(), task.study_id.as_str()))
        .collect::<BTreeSet<_>>()
        .len()
        != request.workflow_plan.tasks.len()
    {
        return Err(fail_request(
            "workflow plan contains duplicate action/study task assignments",
        ));
    }
    let action_by_id = request
        .context_artifact
        .actions
        .iter()
        .map(|action| (action.action.action_id.as_str(), action))
        .collect::<BTreeMap<_, _>>();
    let mut source_by_digest = BTreeMap::new();
    for receipt in &request.source_receipts {
        receipt.validate().map_err(|error| {
            fail_request(format!("source execution receipt is invalid: {error}"))
        })?;
        if receipt.feature_id != ENGINE_FEATURE_ID
            || receipt.contract_version != ENGINE_CONTRACT_VERSION
            || receipt.research_id.trim().is_empty()
            || receipt.study_id.trim().is_empty()
            || source_by_digest
                .insert(receipt.execution_digest.as_str(), receipt)
                .is_some()
        {
            return Err(fail_request(
                "source execution receipts must have unique digests and exact engine identity",
            ));
        }
        let stage_ids = receipt
            .stages
            .iter()
            .map(|stage| stage.stage_id.as_str())
            .collect::<BTreeSet<_>>();
        if stage_ids.len() != receipt.stages.len() {
            return Err(fail_request(format!(
                "source execution receipt {} has duplicate or non-canonical stages",
                receipt.execution_digest
            )));
        }
        if receipt.stages.iter().any(|stage| {
            stage.stage_id.trim().is_empty()
                || (stage.attempts == 0 && stage.disposition != GliomaStageDisposition::Blocked)
                || stage
                    .error
                    .as_ref()
                    .is_some_and(|error| error.trim().is_empty())
                || (stage.disposition != GliomaStageDisposition::Blocked
                    && stage.artifact_digest.is_none())
        }) {
            return Err(fail_request(format!(
                "source execution receipt {} contains an invalid stage snapshot",
                receipt.execution_digest
            )));
        }
    }

    let mut task_bindings = BTreeSet::new();
    let mut receipt_stages_used = BTreeSet::new();
    let mut source_receipts_used = BTreeSet::new();
    for binding in &request.bindings {
        let Some(task) = task_by_id.get(binding.task_id.as_str()) else {
            return Err(fail_request(format!(
                "binding references unknown workflow task {}",
                binding.task_id
            )));
        };
        if !task_bindings.insert(binding.task_id.as_str())
            || task.disposition != MultiStudyTaskDisposition::Scheduled
            || binding.stage_id.trim().is_empty()
            || task.task_id != format!("{}@{}", task.action_id, task.study_id)
        {
            return Err(fail_request(format!(
                "task {} is duplicated, approval-held, or has an empty stage binding",
                binding.task_id
            )));
        }
        let Some(action) = action_by_id.get(task.action_id.as_str()) else {
            return Err(fail_request(format!(
                "workflow task {} references an unknown context action",
                task.task_id
            )));
        };
        let expected_stage_id = action.action.stage_kind.stage_id();
        if binding.stage_id != expected_stage_id
            || !action.study_order.contains(&task.study_id)
            || request.context_artifact.study_group.get(&task.study_id)
                != Some(&task.independent_group)
            || !request
                .workflow_plan
                .eligible_study_order
                .contains(&task.study_id)
        {
            return Err(fail_request(format!(
                "task {} does not match its source action stage and study lineage",
                task.task_id
            )));
        }
        let Some(receipt) = source_by_digest.get(binding.execution_digest.as_str()) else {
            return Err(fail_request(format!(
                "binding for task {} references an unretained execution receipt",
                task.task_id
            )));
        };
        if receipt.study_id != task.study_id {
            return Err(fail_request(format!(
                "execution receipt study does not match task {}",
                task.task_id
            )));
        }
        let stage = receipt
            .stages
            .iter()
            .find(|stage| stage.stage_id == binding.stage_id)
            .ok_or_else(|| {
                fail_request(format!(
                    "execution receipt {} does not contain stage {}",
                    receipt.execution_digest, binding.stage_id
                ))
            })?;
        if stage.attempts == 0
            || (stage.disposition != GliomaStageDisposition::Blocked
                && stage.artifact_digest.is_none())
            || !receipt_stages_used
                .insert((binding.execution_digest.as_str(), binding.stage_id.as_str()))
        {
            return Err(fail_request(format!(
                "execution stage {} has no usable output or is reused across task bindings",
                binding.stage_id
            )));
        }
        source_receipts_used.insert(binding.execution_digest.as_str());
    }
    if source_receipts_used.len() != request.source_receipts.len() {
        return Err(fail_request(
            "every retained source execution receipt must be used by at least one task binding",
        ));
    }
    Ok(())
}

fn derive_report_fields(
    task_order: &[String],
    task_snapshots: &[MultiStudyExecutionTaskSnapshot],
    source_execution_order: &[MultiStudyExecutionSourceSnapshot],
    binding_order: &[MultiStudyExecutionReceiptBinding],
) -> Result<DerivedMultiStudyExecutionReceiptFields, MultiStudyExecutionReceiptError> {
    let task_by_id = task_snapshots
        .iter()
        .map(|task| (task.task_id.as_str(), task))
        .collect::<BTreeMap<_, _>>();
    let source_by_digest = source_execution_order
        .iter()
        .map(|source| (source.execution_digest.as_str(), source))
        .collect::<BTreeMap<_, _>>();
    let mut outcomes = Vec::with_capacity(binding_order.len());
    let mut bound_tasks = BTreeSet::new();
    let mut used_stages = BTreeSet::new();
    let mut used_receipts = BTreeSet::new();
    for binding in binding_order {
        let task = task_by_id.get(binding.task_id.as_str()).ok_or_else(|| {
            MultiStudyExecutionReceiptError::InvalidOutput(
                "binding references an unknown task snapshot".into(),
            )
        })?;
        let source = source_by_digest
            .get(binding.execution_digest.as_str())
            .ok_or_else(|| {
                MultiStudyExecutionReceiptError::InvalidOutput(
                    "binding references an unknown source receipt snapshot".into(),
                )
            })?;
        if !bound_tasks.insert(binding.task_id.as_str())
            || task.disposition != MultiStudyTaskDisposition::Scheduled
            || task.study_id != source.study_id
            || task.stage_id != binding.stage_id
        {
            return Err(MultiStudyExecutionReceiptError::InvalidOutput(
                "task, stage, study, and source receipt bindings are inconsistent".into(),
            ));
        }
        let stage = source
            .stage_order
            .iter()
            .find(|stage| stage.stage_id == binding.stage_id)
            .ok_or_else(|| {
                MultiStudyExecutionReceiptError::InvalidOutput(
                    "binding references a stage absent from the source receipt".into(),
                )
            })?;
        if !used_stages.insert((binding.execution_digest.as_str(), binding.stage_id.as_str())) {
            return Err(MultiStudyExecutionReceiptError::InvalidOutput(
                "one source stage cannot satisfy multiple action tasks".into(),
            ));
        }
        used_receipts.insert(binding.execution_digest.as_str());
        outcomes.push(MultiStudyExecutionActionOutcome {
            task_id: task.task_id.clone(),
            action_id: task.action_id.clone(),
            study_id: task.study_id.clone(),
            stage_id: stage.stage_id.clone(),
            source_feature_id: source.source_feature_id.clone(),
            source_plan_digest: source.source_plan_digest.clone(),
            source_execution_digest: source.execution_digest.clone(),
            source_disposition: stage.disposition,
            source_artifact_digest: stage.artifact_digest.clone(),
            source_error_digest: stage.error_digest.clone(),
            status: stage_status(stage),
            result_digest: result_digest(source, stage)?,
        });
    }
    outcomes.sort_by(|left, right| {
        left.action_id
            .cmp(&right.action_id)
            .then_with(|| left.study_id.cmp(&right.study_id))
    });
    let mut source_execution_digest_order = used_receipts
        .iter()
        .filter_map(|digest| {
            source_by_digest
                .get(digest)
                .map(|source| source.execution_digest.clone())
        })
        .collect::<Vec<_>>();
    source_execution_digest_order.sort();

    let scheduled_tasks = task_snapshots
        .iter()
        .filter(|task| task.disposition == MultiStudyTaskDisposition::Scheduled)
        .collect::<Vec<_>>();
    let mut missing_task_order = scheduled_tasks
        .iter()
        .filter(|task| !bound_tasks.contains(task.task_id.as_str()))
        .map(|task| task.task_id.clone())
        .collect::<Vec<_>>();
    missing_task_order.sort();
    let mut approval_hold_task_order = task_snapshots
        .iter()
        .filter(|task| task.disposition == MultiStudyTaskDisposition::ApprovalRequired)
        .map(|task| task.task_id.clone())
        .collect::<Vec<_>>();
    approval_hold_task_order.sort();
    let disposition = if scheduled_tasks.is_empty() {
        MultiStudyExecutionReceiptDisposition::NotExecutable
    } else if outcomes.is_empty() {
        MultiStudyExecutionReceiptDisposition::Awaiting
    } else if !missing_task_order.is_empty() {
        MultiStudyExecutionReceiptDisposition::Partial
    } else {
        MultiStudyExecutionReceiptDisposition::Complete
    };
    let next_route = if outcomes
        .iter()
        .any(|outcome| outcome.status != DecisionContextActionOutcomeStatus::Completed)
    {
        "researcher_review"
    } else if !missing_task_order.is_empty() {
        "execution_receipt_collection"
    } else if disposition == MultiStudyExecutionReceiptDisposition::Complete {
        "multi_study_context_outcome_assimilation"
    } else if !approval_hold_task_order.is_empty() {
        "researcher_approval_gate"
    } else {
        "glioma_researcher_workbench"
    };
    if task_order.len() != task_snapshots.len()
        || task_order
            .iter()
            .zip(task_snapshots)
            .any(|(task_id, task)| task_id != &task.task_id)
    {
        return Err(MultiStudyExecutionReceiptError::InvalidOutput(
            "task order does not match its retained snapshots".into(),
        ));
    }
    Ok(DerivedMultiStudyExecutionReceiptFields {
        source_execution_digest_order,
        outcome_order: outcomes,
        missing_task_order,
        approval_hold_task_order,
        disposition,
        next_route: next_route.into(),
    })
}

impl MultiStudyExecutionReceiptReport {
    /// Convert this study's digest-bound outcomes into the F06 local outcome input shape.
    pub fn decision_context_outcomes_for_study(
        &self,
        study_id: &str,
    ) -> Result<Vec<DecisionContextActionOutcome>, MultiStudyExecutionReceiptError> {
        self.validate()?;
        Ok(self
            .outcome_order
            .iter()
            .filter(|outcome| outcome.study_id == study_id)
            .map(|outcome| DecisionContextActionOutcome {
                action_id: outcome.action_id.clone(),
                status: outcome.status,
                result_digest: outcome.result_digest.clone(),
            })
            .collect())
    }

    /// Replays the retained, value-free source snapshots and task bindings.
    pub fn validate(&self) -> Result<(), MultiStudyExecutionReceiptError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.epoch == 0
            || self.source_context_digest.as_str().len() != 64
            || self.workflow_plan_digest.as_str().len() != 64
            || !ranked_unique(&self.task_order)
            || !self
                .task_snapshots
                .iter()
                .map(|task| task.task_id.clone())
                .eq(self.task_order.iter().cloned())
            || !canonical(&self.source_execution_digest_order)
            || !canonical(
                &self
                    .source_execution_order
                    .iter()
                    .map(|source| source.execution_digest.clone())
                    .collect::<Vec<_>>(),
            )
            || !canonical(
                &self
                    .binding_order
                    .iter()
                    .map(|binding| binding.task_id.clone())
                    .collect::<Vec<_>>(),
            )
            || !canonical(
                &self
                    .outcome_order
                    .iter()
                    .map(|outcome| (outcome.action_id.clone(), outcome.study_id.clone()))
                    .collect::<Vec<_>>(),
            )
            || !canonical(&self.missing_task_order)
            || !canonical(&self.approval_hold_task_order)
            || self.digest.as_str().len() != 64
            || self.task_snapshots.len() > MAX_TASKS
            || self.source_execution_order.len() > MAX_SOURCE_RECEIPTS
            || self.binding_order.len() > MAX_BINDINGS
            || self.outcome_order.len() > MAX_BINDINGS
        {
            return Err(MultiStudyExecutionReceiptError::InvalidOutput(
                "identity, canonical order, row bounds, or digest fields are invalid".into(),
            ));
        }
        if self.task_snapshots.iter().any(|task| {
            task.task_id.trim().is_empty()
                || task.action_id.trim().is_empty()
                || task.study_id.trim().is_empty()
                || task.stage_id.trim().is_empty()
        }) || self.source_execution_order.iter().any(|source| {
            source.source_feature_id != ENGINE_FEATURE_ID
                || source.source_contract_version != ENGINE_CONTRACT_VERSION
                || source.research_id.trim().is_empty()
                || source.study_id.trim().is_empty()
                || source.source_plan_digest.as_str().len() != 64
                || source.execution_digest.as_str().len() != 64
                || source.boundary != PRECLINICAL_BOUNDARY
                || !matches!(
                    source.disposition.as_str(),
                    "succeeded" | "partial" | "failed"
                )
                || !canonical(
                    &source
                        .stage_order
                        .iter()
                        .map(|stage| stage.stage_id.clone())
                        .collect::<Vec<_>>(),
                )
                || source.stage_order.iter().any(|stage| {
                    stage.stage_id.trim().is_empty()
                        || (stage.attempts == 0
                            && stage.disposition != GliomaStageDisposition::Blocked)
                        || stage
                            .artifact_digest
                            .as_ref()
                            .is_some_and(|digest| digest.as_str().len() != 64)
                        || stage
                            .error_digest
                            .as_ref()
                            .is_some_and(|digest| digest.as_str().len() != 64)
                        || (stage.disposition != GliomaStageDisposition::Blocked
                            && stage.artifact_digest.is_none())
                })
        }) {
            return Err(MultiStudyExecutionReceiptError::InvalidOutput(
                "task or source stage snapshots are malformed".into(),
            ));
        }
        if self.outcome_order.iter().any(|outcome| {
            outcome.task_id.trim().is_empty()
                || outcome.action_id.trim().is_empty()
                || outcome.study_id.trim().is_empty()
                || outcome.stage_id.trim().is_empty()
                || outcome.source_feature_id != ENGINE_FEATURE_ID
                || outcome.source_plan_digest.as_str().len() != 64
                || outcome.source_execution_digest.as_str().len() != 64
                || outcome.result_digest.as_str().len() != 64
        }) {
            return Err(MultiStudyExecutionReceiptError::InvalidOutput(
                "execution outcome snapshots are malformed".into(),
            ));
        }
        let derived = derive_report_fields(
            &self.task_order,
            &self.task_snapshots,
            &self.source_execution_order,
            &self.binding_order,
        )?;
        if derived.source_execution_digest_order != self.source_execution_digest_order
            || derived.source_execution_digest_order
                != self
                    .source_execution_order
                    .iter()
                    .map(|source| source.execution_digest.clone())
                    .collect::<Vec<_>>()
            || derived.outcome_order != self.outcome_order
            || derived.missing_task_order != self.missing_task_order
            || derived.approval_hold_task_order != self.approval_hold_task_order
            || derived.disposition != self.disposition
            || derived.next_route != self.next_route
        {
            return Err(MultiStudyExecutionReceiptError::InvalidOutput(
                "report does not replay from the retained source receipts and task bindings".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| MultiStudyExecutionReceiptError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(MultiStudyExecutionReceiptError::Digest(
                "multi-study execution receipt digest does not match canonical content".into(),
            ));
        }
        let report_bytes = serde_json::to_vec(self)
            .map_err(|error| MultiStudyExecutionReceiptError::Digest(error.to_string()))?
            .len();
        if report_bytes > MAX_REPORT_BYTES {
            return Err(MultiStudyExecutionReceiptError::InvalidOutput(format!(
                "report is {report_bytes} bytes, above the {MAX_REPORT_BYTES}-byte limit"
            )));
        }
        Ok(())
    }
}

/// Validate cross-program engine receipts and bind exact stages to their P04 action tasks.
pub fn reconcile_glioma_multi_study_execution_receipts(
    request: &MultiStudyExecutionReceiptRequest,
) -> Result<MultiStudyExecutionReceiptReport, MultiStudyExecutionReceiptError> {
    validate_request(request)?;
    let task_order = request.workflow_plan.task_order.clone();
    let task_by_id = request
        .workflow_plan
        .tasks
        .iter()
        .map(|task| (task.task_id.as_str(), task))
        .collect::<BTreeMap<_, _>>();
    let action_by_id = request
        .context_artifact
        .actions
        .iter()
        .map(|action| (action.action.action_id.as_str(), action))
        .collect::<BTreeMap<_, _>>();
    let task_snapshots = task_order
        .iter()
        .map(|task_id| {
            let task = task_by_id[task_id.as_str()];
            let action = action_by_id[task.action_id.as_str()];
            MultiStudyExecutionTaskSnapshot {
                task_id: task.task_id.clone(),
                action_id: task.action_id.clone(),
                study_id: task.study_id.clone(),
                stage_id: action.action.stage_kind.stage_id().into(),
                disposition: task.disposition,
            }
        })
        .collect::<Vec<_>>();
    let mut source_execution_order = request
        .source_receipts
        .iter()
        .map(snapshot_source)
        .collect::<Vec<_>>();
    source_execution_order
        .sort_by(|left, right| left.execution_digest.cmp(&right.execution_digest));
    let mut binding_order = request.bindings.clone();
    binding_order.sort_by(|left, right| left.task_id.cmp(&right.task_id));
    let derived = derive_report_fields(
        &task_order,
        &task_snapshots,
        &source_execution_order,
        &binding_order,
    )?;
    let mut report = MultiStudyExecutionReceiptReport {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.workflow_plan.objective.clone(),
        epoch: request.workflow_plan.epoch,
        source_context_digest: request.context_artifact.digest.clone(),
        workflow_plan_digest: request.workflow_plan.digest.clone(),
        task_order,
        task_snapshots,
        source_execution_order,
        binding_order,
        source_execution_digest_order: derived.source_execution_digest_order,
        outcome_order: derived.outcome_order,
        missing_task_order: derived.missing_task_order,
        approval_hold_task_order: derived.approval_hold_task_order,
        disposition: derived.disposition,
        next_route: derived.next_route,
        digest: ContentHash::of_bytes(b"unsealed-glioma-multi-study-execution-receipt"),
    };
    report.digest = ContentHash::of_value(&digest_input(&report))
        .map_err(|error| MultiStudyExecutionReceiptError::Digest(error.to_string()))?;
    report.validate()?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p04_decision_context::multi_study_workflow;
    use crate::glioma_engine::{
        GliomaModality, GliomaModelSystem, GliomaResearchIntent, LocalArtifactRef,
        dry_run_glioma_research,
    };
    use bioprism_foundation::{AutonomyTier, PRECLINICAL_BOUNDARY};
    use bioprism_onco::OutputUse;
    use std::collections::BTreeSet;

    fn digest(label: &str) -> ContentHash {
        ContentHash::of_bytes(label.as_bytes())
    }

    fn engine_receipt() -> GliomaExecutionReceipt {
        dry_run_glioma_research(&GliomaResearchIntent {
            research_id: "cross-program-receipt-test".into(),
            study_id: "study-a".into(),
            objective: "map molecular mechanisms in a preclinical glioma model".into(),
            output_uses: BTreeSet::from([OutputUse::CohortAnalysis, OutputUse::MethodDevelopment]),
            model_systems: BTreeSet::from([
                GliomaModelSystem::Organoid,
                GliomaModelSystem::InSilico,
            ]),
            modalities: BTreeSet::from([
                GliomaModality::Literature,
                GliomaModality::Genomics,
                GliomaModality::Transcriptomics,
                GliomaModality::Imaging,
                GliomaModality::Computational,
                GliomaModality::Replication,
            ]),
            input_artifacts: vec![LocalArtifactRef {
                artifact_id: "artifact:receipt-test".into(),
                content_hash: digest("receipt-test-artifact"),
                content_type: "application/vnd.glioma.input+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            }],
            requested_autonomy: AutonomyTier::A1,
            approval_reference: None,
            budget_units: 300,
            max_retries: 1,
            allow_instrument_execution: false,
            allow_federation: false,
            raw_data_local: true,
            aggregate_only: true,
            replay_identity: digest("receipt-test-replay"),
            boundary: PRECLINICAL_BOUNDARY.into(),
        })
        .unwrap()
    }

    #[test]
    fn exact_engine_stage_is_bound_and_dry_run_negative_fails_closed_as_unknown() {
        let workflow_request = multi_study_workflow::tests::request();
        let plan =
            multi_study_workflow::plan_glioma_multi_study_workflow(&workflow_request).unwrap();
        let receipt = engine_receipt();
        let request = MultiStudyExecutionReceiptRequest {
            workflow_plan: plan,
            context_artifact: workflow_request.artifact,
            bindings: vec![MultiStudyExecutionReceiptBinding {
                task_id: "action-a@study-a".into(),
                execution_digest: receipt.execution_digest.clone(),
                stage_id: "experiment-design".into(),
            }],
            source_receipts: vec![receipt],
        };

        let report = reconcile_glioma_multi_study_execution_receipts(&request).unwrap();

        assert_eq!(
            report.disposition,
            MultiStudyExecutionReceiptDisposition::Partial
        );
        assert_eq!(report.missing_task_order.len(), 3);
        assert_eq!(report.outcome_order.len(), 1);
        assert_eq!(
            report.outcome_order[0].source_disposition,
            GliomaStageDisposition::Negative
        );
        assert_eq!(
            report.outcome_order[0].status,
            DecisionContextActionOutcomeStatus::Unknown
        );
        assert!(report.outcome_order[0].source_artifact_digest.is_some());
        let local = report
            .decision_context_outcomes_for_study("study-a")
            .unwrap();
        assert_eq!(local[0].action_id, "action-a");
        assert_eq!(local[0].status, DecisionContextActionOutcomeStatus::Unknown);
        report.validate().unwrap();
    }

    #[test]
    fn receipt_cannot_be_reused_for_a_different_action_stage() {
        let workflow_request = multi_study_workflow::tests::request();
        let plan =
            multi_study_workflow::plan_glioma_multi_study_workflow(&workflow_request).unwrap();
        let receipt = engine_receipt();
        let request = MultiStudyExecutionReceiptRequest {
            workflow_plan: plan,
            context_artifact: workflow_request.artifact,
            bindings: vec![MultiStudyExecutionReceiptBinding {
                task_id: "action-a@study-a".into(),
                execution_digest: receipt.execution_digest.clone(),
                stage_id: "mechanism-exploration".into(),
            }],
            source_receipts: vec![receipt],
        };

        let error = reconcile_glioma_multi_study_execution_receipts(&request).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("does not match its source action stage")
        );
    }

    #[test]
    fn approval_held_workflow_task_cannot_accept_an_execution_receipt() {
        let mut workflow_request = multi_study_workflow::tests::request();
        workflow_request.artifact.actions[0].action.autonomy_tier = AutonomyTier::A2;
        workflow_request.artifact.actions[1].action.autonomy_tier = AutonomyTier::A2;
        multi_study_workflow::tests::reseal(&mut workflow_request.artifact);
        for budget in &mut workflow_request.study_budgets {
            budget.autonomy_ceiling = AutonomyTier::A2;
            budget.approved_autonomy = AutonomyTier::A1;
        }
        let plan =
            multi_study_workflow::plan_glioma_multi_study_workflow(&workflow_request).unwrap();
        let receipt = engine_receipt();
        let request = MultiStudyExecutionReceiptRequest {
            workflow_plan: plan,
            context_artifact: workflow_request.artifact,
            bindings: vec![MultiStudyExecutionReceiptBinding {
                task_id: "action-a@study-a".into(),
                execution_digest: receipt.execution_digest.clone(),
                stage_id: "experiment-design".into(),
            }],
            source_receipts: vec![receipt],
        };

        let error = reconcile_glioma_multi_study_execution_receipts(&request).unwrap_err();
        assert!(error.to_string().contains("approval-held"));
    }

    #[test]
    fn re_digested_outcome_tampering_fails_snapshot_replay() {
        let workflow_request = multi_study_workflow::tests::request();
        let plan =
            multi_study_workflow::plan_glioma_multi_study_workflow(&workflow_request).unwrap();
        let receipt = engine_receipt();
        let request = MultiStudyExecutionReceiptRequest {
            workflow_plan: plan,
            context_artifact: workflow_request.artifact,
            bindings: vec![MultiStudyExecutionReceiptBinding {
                task_id: "action-a@study-a".into(),
                execution_digest: receipt.execution_digest.clone(),
                stage_id: "experiment-design".into(),
            }],
            source_receipts: vec![receipt],
        };
        let mut report = reconcile_glioma_multi_study_execution_receipts(&request).unwrap();
        report.outcome_order[0].status = DecisionContextActionOutcomeStatus::Completed;
        report.digest = ContentHash::of_value(&digest_input(&report)).unwrap();

        let error = report.validate().unwrap_err();
        assert!(error.to_string().contains("does not replay"));
    }
}

//! Checkpoint-aware recovery and resume planning for P02 knowledge workflows.
//!
//! Long-running glioma evidence workflows must survive adapter crashes, partial outputs, and
//! operator pauses without silently replaying completed work.  This feature validates a local
//! workflow plus an explicit execution snapshot, then emits deterministic resume/replay/hold
//! actions. It does not execute a step or infer a result from a missing artifact.

use super::workflow_compile::{LocalResearchWorkflow, LocalWorkflowStep};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P02-F26";
pub const OUTPUT_SCHEMA: &str = "GliomaWorkflowRecoveryPlan1@1";
pub const MAX_OBSERVATIONS: usize = 4_096;
pub const MAX_ACTIONS: usize = 4_096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowObservedStatus {
    Completed,
    FailedRetryable,
    FailedTerminal,
    Running,
    Skipped,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRecoveryDecision {
    ResumeFromCheckpoint,
    ReplayIdempotentStep,
    CompensateThenRetry,
    HoldForOperator,
    AlreadyComplete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRecoveryDisposition {
    Ready,
    Partial,
    Held,
    Complete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowStepObservation {
    pub action_id: String,
    pub status: WorkflowObservedStatus,
    pub attempts: u8,
    pub checkpoint_hash: Option<ContentHash>,
    pub output_artifact_hash: Option<ContentHash>,
    pub failure_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowRecoveryRequest {
    pub objective: String,
    pub workflow: LocalResearchWorkflow,
    pub observations: Vec<WorkflowStepObservation>,
    pub max_retry_attempts: u8,
    pub allow_resume_after_failure: bool,
    pub require_checkpoint_artifact: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowRecoveryAction {
    pub action_id: String,
    pub step_id: String,
    pub wave: usize,
    pub dependency_order: Vec<String>,
    pub decision: WorkflowRecoveryDecision,
    pub next_attempt: u8,
    pub checkpoint_hash: Option<ContentHash>,
    pub output_artifact_hash: Option<ContentHash>,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowRecoveryPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub source_workflow_digest: ContentHash,
    pub action_order: Vec<String>,
    pub actions: Vec<WorkflowRecoveryAction>,
    pub completed_order: Vec<String>,
    pub resume_order: Vec<String>,
    pub replay_order: Vec<String>,
    pub hold_order: Vec<String>,
    pub omitted_order: Vec<String>,
    pub uncertainty: Vec<String>,
    pub resume_wave: usize,
    pub disposition: WorkflowRecoveryDisposition,
    pub next_step: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WorkflowRecoveryError {
    #[error("workflow recovery request is invalid: {0}")]
    InvalidRequest(String),
    #[error("workflow recovery output is invalid: {0}")]
    InvalidOutput(String),
    #[error("workflow recovery digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &WorkflowRecoveryPlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "source_workflow_digest": output.source_workflow_digest,
        "action_order": output.action_order,
        "actions": output.actions,
        "completed_order": output.completed_order,
        "resume_order": output.resume_order,
        "replay_order": output.replay_order,
        "hold_order": output.hold_order,
        "omitted_order": output.omitted_order,
        "uncertainty": output.uncertainty,
        "resume_wave": output.resume_wave,
        "disposition": output.disposition,
        "next_step": output.next_step,
    })
}

impl WorkflowRecoveryPlan {
    pub fn validate(&self) -> Result<(), WorkflowRecoveryError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.source_workflow_digest.as_str().len() != 64
            || !canonical(&self.action_order)
            || !canonical(&self.completed_order)
            || !canonical(&self.resume_order)
            || !canonical(&self.replay_order)
            || !canonical(&self.hold_order)
            || !canonical(&self.omitted_order)
            || !canonical(&self.uncertainty)
            || !canonical(
                &self
                    .actions
                    .iter()
                    .map(|action| action.action_id.clone())
                    .collect::<Vec<_>>(),
            )
            || self.actions.iter().any(|action| {
                action.action_id.trim().is_empty()
                    || action.step_id.trim().is_empty()
                    || !canonical(&action.dependency_order)
                    || action.rationale.trim().is_empty()
                    || action.next_attempt == 0
                    || action.wave == 0
            })
            || self.next_step.trim().is_empty()
            || self.digest.as_str().len() != 64
        {
            return Err(WorkflowRecoveryError::InvalidOutput(
                "identity, ordering, action, wave, rationale, or digest fields are invalid".into(),
            ));
        }
        let action_ids = self
            .actions
            .iter()
            .map(|action| action.action_id.clone())
            .collect::<BTreeSet<_>>();
        let action_order = self.action_order.iter().cloned().collect::<BTreeSet<_>>();
        let classified = self
            .resume_order
            .iter()
            .chain(self.replay_order.iter())
            .chain(self.hold_order.iter())
            .chain(self.omitted_order.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        if action_order != action_ids
            || action_order.len() != self.action_order.len()
            || classified.len()
                != self.resume_order.len()
                    + self.replay_order.len()
                    + self.hold_order.len()
                    + self.omitted_order.len()
            || !classified.is_subset(&action_ids)
            || self
                .completed_order
                .iter()
                .any(|id| action_ids.contains(id))
        {
            return Err(WorkflowRecoveryError::InvalidOutput(
                "action identity or recovery partitions are inconsistent".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| WorkflowRecoveryError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(WorkflowRecoveryError::Digest(
                "recovery digest does not match canonical content".into(),
            ));
        }
        Ok(())
    }
}

fn dependency_ready(step: &LocalWorkflowStep, accepted: &BTreeSet<String>) -> bool {
    step.dependency_order
        .iter()
        .all(|dependency| accepted.contains(dependency))
}

/// Build a deterministic resume/replay plan from a workflow and an explicit execution snapshot.
pub fn plan_glioma_workflow_recovery(
    request: &WorkflowRecoveryRequest,
) -> Result<WorkflowRecoveryPlan, WorkflowRecoveryError> {
    if request.objective.trim().is_empty()
        || request.max_retry_attempts == 0
        || request.max_retry_attempts > 16
        || request.observations.len() > MAX_OBSERVATIONS
    {
        return Err(WorkflowRecoveryError::InvalidRequest(
            "objective, retry bound, or observation bound is invalid".into(),
        ));
    }
    request
        .workflow
        .validate()
        .map_err(|error| WorkflowRecoveryError::InvalidRequest(error.to_string()))?;
    if request.workflow.objective != request.objective {
        return Err(WorkflowRecoveryError::InvalidRequest(
            "recovery objective must bind to workflow objective".into(),
        ));
    }
    let mut observations = BTreeMap::new();
    for observation in &request.observations {
        if observation.action_id.trim().is_empty()
            || observation.attempts > request.max_retry_attempts
            || observations
                .insert(observation.action_id.clone(), observation)
                .is_some()
        {
            return Err(WorkflowRecoveryError::InvalidRequest(
                "observations must have unique action ids and bounded attempts".into(),
            ));
        }
    }
    let steps = request
        .workflow
        .steps
        .iter()
        .map(|step| (step.action_id.clone(), step))
        .collect::<BTreeMap<_, _>>();
    let mut completed = BTreeSet::new();
    let mut actions = Vec::new();
    let mut hold = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for (action_id, _step) in &steps {
        if let Some(observation) = observations.get(action_id) {
            let checkpoint_valid = observation.checkpoint_hash.is_some()
                && (!request.require_checkpoint_artifact
                    || observation.output_artifact_hash.is_some());
            if observation.status == WorkflowObservedStatus::Completed && checkpoint_valid {
                completed.insert(action_id.clone());
                continue;
            }
        }
    }
    for (action_id, step) in &steps {
        if completed.contains(action_id) {
            continue;
        }
        let observation = observations.get(action_id);
        let dependencies_done = dependency_ready(step, &completed);
        let (decision, next_attempt, checkpoint_hash, output_artifact_hash, rationale) =
            match observation {
                None => (
                    if dependencies_done {
                        WorkflowRecoveryDecision::ReplayIdempotentStep
                    } else {
                        WorkflowRecoveryDecision::HoldForOperator
                    },
                    1,
                    None,
                    None,
                    if dependencies_done {
                        "step has no observation and is replayable in dependency order"
                    } else {
                        "step is unobserved but a dependency is not complete"
                    },
                ),
                Some(observation) => match observation.status {
                    WorkflowObservedStatus::Completed => (
                        if dependencies_done {
                            WorkflowRecoveryDecision::ReplayIdempotentStep
                        } else {
                            WorkflowRecoveryDecision::HoldForOperator
                        },
                        observation.attempts.saturating_add(1),
                        observation.checkpoint_hash.clone(),
                        observation.output_artifact_hash.clone(),
                        if checkpoint_valid(observation, request.require_checkpoint_artifact) {
                            "completed status lacks a trusted checkpoint binding; replay is required"
                        } else {
                            "completed status is missing a required checkpoint or output artifact"
                        },
                    ),
                    WorkflowObservedStatus::FailedRetryable => {
                        let can_retry = request.allow_resume_after_failure
                            && observation.attempts < request.max_retry_attempts
                            && dependencies_done;
                        (
                            if can_retry {
                                WorkflowRecoveryDecision::CompensateThenRetry
                            } else {
                                WorkflowRecoveryDecision::HoldForOperator
                            },
                            observation.attempts.saturating_add(1),
                            observation.checkpoint_hash.clone(),
                            observation.output_artifact_hash.clone(),
                            if can_retry {
                                "retryable failure has a bounded compensation and retry budget"
                            } else {
                                "retryable failure is held because policy, dependency, or retry budget is unresolved"
                            },
                        )
                    }
                    WorkflowObservedStatus::FailedTerminal
                    | WorkflowObservedStatus::Running
                    | WorkflowObservedStatus::Skipped
                    | WorkflowObservedStatus::Unknown => (
                        WorkflowRecoveryDecision::HoldForOperator,
                        observation.attempts.saturating_add(1).max(1),
                        observation.checkpoint_hash.clone(),
                        observation.output_artifact_hash.clone(),
                        "execution state is not safe to replay without an operator decision",
                    ),
                },
            };
        let wave = if dependencies_done {
            step.wave.max(1)
        } else {
            hold.insert(action_id.clone());
            uncertainty.insert(format!("{action_id}: dependency closure is incomplete"));
            0
        };
        actions.push(WorkflowRecoveryAction {
            action_id: action_id.clone(),
            step_id: step.step_id.clone(),
            wave: wave.max(1),
            dependency_order: step.dependency_order.clone(),
            decision,
            next_attempt,
            checkpoint_hash,
            output_artifact_hash,
            rationale: rationale.into(),
        });
    }
    actions.sort_by(|left, right| left.action_id.cmp(&right.action_id));
    let action_order = actions
        .iter()
        .map(|action| action.action_id.clone())
        .collect::<Vec<_>>();
    let completed_order = completed.into_iter().collect::<Vec<_>>();
    let mut resume_order = BTreeSet::new();
    let mut replay_order = BTreeSet::new();
    for action in &actions {
        match action.decision {
            WorkflowRecoveryDecision::ResumeFromCheckpoint
            | WorkflowRecoveryDecision::CompensateThenRetry => {
                resume_order.insert(action.action_id.clone());
            }
            WorkflowRecoveryDecision::ReplayIdempotentStep => {
                replay_order.insert(action.action_id.clone());
            }
            WorkflowRecoveryDecision::HoldForOperator
            | WorkflowRecoveryDecision::AlreadyComplete => {}
        }
    }
    let resume_order = resume_order.into_iter().collect::<Vec<_>>();
    let replay_order = replay_order.into_iter().collect::<Vec<_>>();
    let hold_order = actions
        .iter()
        .filter(|action| action.decision == WorkflowRecoveryDecision::HoldForOperator)
        .map(|action| action.action_id.clone())
        .collect::<Vec<_>>();
    let resume_wave = actions
        .iter()
        .filter(|action| {
            resume_order.binary_search(&action.action_id).is_ok()
                || replay_order.binary_search(&action.action_id).is_ok()
        })
        .map(|action| action.wave)
        .min()
        .unwrap_or(0);
    let disposition = if actions.is_empty() {
        WorkflowRecoveryDisposition::Complete
    } else if !hold_order.is_empty() {
        WorkflowRecoveryDisposition::Held
    } else if !resume_order.is_empty() || !replay_order.is_empty() {
        WorkflowRecoveryDisposition::Ready
    } else {
        WorkflowRecoveryDisposition::Partial
    };
    let next_step = match disposition {
        WorkflowRecoveryDisposition::Complete => {
            "publish the completed workflow state and downstream knowledge artifact"
        }
        WorkflowRecoveryDisposition::Ready => {
            "dispatch only the dependency-ready resume/replay actions to the local executor"
        }
        WorkflowRecoveryDisposition::Partial => {
            "reconcile partial observations before scheduling another wave"
        }
        WorkflowRecoveryDisposition::Held => {
            "obtain operator disposition for terminal, running, or dependency-blocked steps"
        }
    };
    let mut output = WorkflowRecoveryPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        source_workflow_digest: request.workflow.digest.clone(),
        action_order,
        actions,
        completed_order,
        resume_order,
        replay_order,
        hold_order,
        omitted_order: Vec::new(),
        uncertainty: uncertainty.into_iter().collect(),
        resume_wave,
        disposition,
        next_step: next_step.into(),
        digest: ContentHash::of_value(&serde_json::Value::Null)
            .map_err(|error| WorkflowRecoveryError::Digest(error.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| WorkflowRecoveryError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

fn checkpoint_valid(observation: &WorkflowStepObservation, require_artifact: bool) -> bool {
    observation.checkpoint_hash.is_some()
        && (!require_artifact || observation.output_artifact_hash.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workflow() -> LocalResearchWorkflow {
        let steps = vec![
            LocalWorkflowStep {
                step_id: "step-a".into(),
                action_id: "action-a".into(),
                claim_key: "claim-a".into(),
                dependency_order: vec![],
                wave: 1,
                retry_limit: 2,
                checkpoint_after: true,
                compensation_kind: "preserve".into(),
                expected_artifact_kind: "typed".into(),
            },
            LocalWorkflowStep {
                step_id: "step-b".into(),
                action_id: "action-b".into(),
                claim_key: "claim-b".into(),
                dependency_order: vec!["action-a".into()],
                wave: 2,
                retry_limit: 2,
                checkpoint_after: true,
                compensation_kind: "restore".into(),
                expected_artifact_kind: "typed".into(),
            },
        ];
        let mut output = LocalResearchWorkflow {
            feature_id: "GAF-GLIOMA-P02-F13".into(),
            output_schema: "GliomaLocalResearchWorkflow1@1".into(),
            objective: "recover workflow".into(),
            source_plan_digest: ContentHash::of_bytes(b"plan"),
            step_order: vec!["action-a".into(), "action-b".into()],
            parallel_waves: vec![vec!["action-a".into()], vec!["action-b".into()]],
            checkpoint_order: vec!["action-a".into(), "action-b".into()],
            compensation_order: vec!["action-a".into(), "action-b".into()],
            steps,
            critical_path_steps: 2,
            total_cost_milli: 100,
            omitted_order: vec![],
            uncertainty: vec![],
            disposition: super::super::workflow_compile::LocalWorkflowDisposition::Ready,
            next_step: "execute".into(),
            digest: ContentHash::of_value(&serde_json::Value::Null).unwrap(),
        };
        output.digest = ContentHash::of_value(
            &super::super::workflow_compile::digest_input_for_recovery(&output),
        )
        .unwrap();
        output
    }

    #[test]
    fn resumes_after_a_valid_checkpoint_without_replaying_completed_work() {
        let workflow = workflow();
        let request = WorkflowRecoveryRequest {
            objective: "recover workflow".into(),
            workflow,
            observations: vec![WorkflowStepObservation {
                action_id: "action-a".into(),
                status: WorkflowObservedStatus::Completed,
                attempts: 1,
                checkpoint_hash: Some(ContentHash::of_bytes(b"checkpoint-a")),
                output_artifact_hash: Some(ContentHash::of_bytes(b"artifact-a")),
                failure_code: None,
            }],
            max_retry_attempts: 3,
            allow_resume_after_failure: true,
            require_checkpoint_artifact: true,
        };
        let output = plan_glioma_workflow_recovery(&request).unwrap();
        assert_eq!(output.completed_order, vec!["action-a"]);
        assert_eq!(output.replay_order, vec!["action-b"]);
        assert!(!output.replay_order.contains(&"action-a".to_string()));
    }

    #[test]
    fn terminal_failure_is_held_for_operator() {
        let mut request = WorkflowRecoveryRequest {
            objective: "recover workflow".into(),
            workflow: workflow(),
            observations: vec![],
            max_retry_attempts: 3,
            allow_resume_after_failure: true,
            require_checkpoint_artifact: false,
        };
        request.observations.push(WorkflowStepObservation {
            action_id: "action-a".into(),
            status: WorkflowObservedStatus::FailedTerminal,
            attempts: 2,
            checkpoint_hash: None,
            output_artifact_hash: None,
            failure_code: Some("adapter_revoked".into()),
        });
        let output = plan_glioma_workflow_recovery(&request).unwrap();
        assert!(output.hold_order.contains(&"action-a".to_string()));
        assert_eq!(output.disposition, WorkflowRecoveryDisposition::Held);
    }

    #[test]
    fn retryable_failure_is_compensated_and_bounded() {
        let request = WorkflowRecoveryRequest {
            objective: "recover workflow".into(),
            workflow: workflow(),
            observations: vec![WorkflowStepObservation {
                action_id: "action-a".into(),
                status: WorkflowObservedStatus::FailedRetryable,
                attempts: 1,
                checkpoint_hash: Some(ContentHash::of_bytes(b"checkpoint-a")),
                output_artifact_hash: None,
                failure_code: Some("timeout".into()),
            }],
            max_retry_attempts: 3,
            allow_resume_after_failure: true,
            require_checkpoint_artifact: false,
        };
        let output = plan_glioma_workflow_recovery(&request).unwrap();
        assert_eq!(output.resume_order, vec!["action-a"]);
        assert_eq!(
            output.actions[0].decision,
            WorkflowRecoveryDecision::CompensateThenRetry
        );
    }
}

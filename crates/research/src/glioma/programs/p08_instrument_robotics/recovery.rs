//! Deterministic recovery planning for institution-local glioma instrument runs.
//!
//! Instrument execution can stop after a partial plate, a transient gateway failure, a changed
//! interlock, or a negative but valid result.  This feature converts the signed execution record
//! into a bounded recovery plan.  It never reconnects to hardware, retries an operation itself,
//! or treats a recovery recommendation as biological evidence; an institution-owned gateway must
//! separately admit and execute any returned action.

use super::execution::{
    InstrumentExecutionDisposition, InstrumentExecutionRun, InstrumentExecutionStopReason,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P08-F06";
pub const OUTPUT_SCHEMA: &str = "GliomaInstrumentRecovery1@1";
pub const MAX_ACTIONS: usize = 1_024;
pub const MAX_RETRY_ATTEMPTS: u8 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstrumentRecoveryDecision {
    NoAction,
    VerifyArtifact,
    Resume,
    Retry,
    Recalibrate,
    Compensate,
    HumanReview,
    Abort,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstrumentRecoveryPriority {
    Immediate,
    Deferred,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentRecoveryRequest {
    pub objective: String,
    pub run: InstrumentExecutionRun,
    pub allow_retry: bool,
    pub allow_recalibration: bool,
    pub max_retry_attempts: u8,
    pub require_human_for_partial: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentRecoveryAction {
    pub action_id: String,
    pub source_action_id: String,
    pub decision: InstrumentRecoveryDecision,
    pub priority: InstrumentRecoveryPriority,
    pub rationale: String,
    pub preserves_negative_evidence: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstrumentRecoveryDisposition {
    Ready,
    Partial,
    Blocked,
    NoRecovery,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentRecoveryPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub run_digest: ContentHash,
    pub instrument_id: String,
    pub action_order: Vec<String>,
    pub actions: Vec<InstrumentRecoveryAction>,
    pub recovery_order: Vec<String>,
    pub retry_order: Vec<String>,
    pub resume_order: Vec<String>,
    pub recalibration_order: Vec<String>,
    pub compensation_order: Vec<String>,
    pub human_review_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: InstrumentRecoveryDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum InstrumentRecoveryError {
    #[error("instrument recovery request is invalid: {0}")]
    InvalidRequest(String),
    #[error("instrument recovery output is invalid: {0}")]
    InvalidOutput(String),
    #[error("instrument recovery digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique_nonempty(values: &[String]) -> bool {
    values.iter().all(|value| !value.trim().is_empty())
        && values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

fn digest_input(plan: &InstrumentRecoveryPlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": plan.feature_id,
        "output_schema": plan.output_schema,
        "objective": plan.objective,
        "run_digest": plan.run_digest,
        "instrument_id": plan.instrument_id,
        "action_order": plan.action_order,
        "actions": plan.actions,
        "recovery_order": plan.recovery_order,
        "retry_order": plan.retry_order,
        "resume_order": plan.resume_order,
        "recalibration_order": plan.recalibration_order,
        "compensation_order": plan.compensation_order,
        "human_review_order": plan.human_review_order,
        "blocked_order": plan.blocked_order,
        "negative_evidence": plan.negative_evidence,
        "uncertainty": plan.uncertainty,
        "disposition": plan.disposition,
    })
}

impl InstrumentRecoveryPlan {
    pub fn validate(&self) -> Result<(), InstrumentRecoveryError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.instrument_id.trim().is_empty()
            || self.run_digest.as_str().len() != 64
            || !canonical(&self.action_order)
            || !unique_nonempty(&self.action_order)
            || self.action_order.len() != self.actions.len()
            || !canonical(&self.recovery_order)
            || !canonical(&self.retry_order)
            || !canonical(&self.resume_order)
            || !canonical(&self.recalibration_order)
            || !canonical(&self.compensation_order)
            || !canonical(&self.human_review_order)
            || !canonical(&self.blocked_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.actions.iter().any(|action| {
                action.action_id.trim().is_empty()
                    || action.source_action_id.trim().is_empty()
                    || action.rationale.trim().is_empty()
            })
        {
            return Err(InstrumentRecoveryError::InvalidOutput(
                "identity, action ordering, partitions, and rationale invariants are invalid"
                    .into(),
            ));
        }
        let action_ids = self
            .actions
            .iter()
            .map(|action| action.action_id.clone())
            .collect::<BTreeSet<_>>();
        if action_ids != self.action_order.iter().cloned().collect::<BTreeSet<_>>()
            || self.actions.iter().any(|action| {
                action.source_action_id.trim().is_empty()
                    || action.action_id != format!("recovery:{}", action.source_action_id)
            })
        {
            return Err(InstrumentRecoveryError::InvalidOutput(
                "recovery action identities do not reconcile with source actions".into(),
            ));
        }
        for partition in [
            &self.retry_order,
            &self.resume_order,
            &self.recalibration_order,
            &self.compensation_order,
            &self.human_review_order,
            &self.blocked_order,
        ] {
            if partition.iter().any(|id| !action_ids.contains(id)) {
                return Err(InstrumentRecoveryError::InvalidOutput(
                    "recovery partitions contain unknown actions".into(),
                ));
            }
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| InstrumentRecoveryError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(InstrumentRecoveryError::Digest(
                "instrument recovery digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(request: &InstrumentRecoveryRequest) -> Result<(), InstrumentRecoveryError> {
    if request.objective.trim().is_empty()
        || request.objective.trim() != request.run.objective.trim()
        || request.run.action_order.is_empty()
        || request.run.action_order.len() > MAX_ACTIONS
        || request.max_retry_attempts == 0
        || request.max_retry_attempts > MAX_RETRY_ATTEMPTS
    {
        return Err(InstrumentRecoveryError::InvalidRequest(
            "objective binding, bounded execution run, and retry limits are required".into(),
        ));
    }
    request
        .run
        .validate()
        .map_err(|error| InstrumentRecoveryError::InvalidRequest(error.to_string()))?;
    Ok(())
}

fn priority(decision: InstrumentRecoveryDecision) -> InstrumentRecoveryPriority {
    match decision {
        InstrumentRecoveryDecision::Retry
        | InstrumentRecoveryDecision::Resume
        | InstrumentRecoveryDecision::Recalibrate => InstrumentRecoveryPriority::Immediate,
        InstrumentRecoveryDecision::HumanReview | InstrumentRecoveryDecision::Abort => {
            InstrumentRecoveryPriority::Blocked
        }
        InstrumentRecoveryDecision::NoAction
        | InstrumentRecoveryDecision::VerifyArtifact
        | InstrumentRecoveryDecision::Compensate => InstrumentRecoveryPriority::Deferred,
    }
}

/// Convert a validated instrument execution run into a bounded, deterministic recovery plan.
pub fn plan_glioma_instrument_recovery(
    request: &InstrumentRecoveryRequest,
) -> Result<InstrumentRecoveryPlan, InstrumentRecoveryError> {
    validate_request(request)?;
    let mut by_action = BTreeMap::new();
    for result in &request.run.results {
        let (decision, rationale) = match result.disposition {
            InstrumentExecutionDisposition::Completed => {
                if result.artifact.is_some() {
                    (
                        InstrumentRecoveryDecision::NoAction,
                        "completed result has a local artifact; no recovery is required".into(),
                    )
                } else {
                    (
                        InstrumentRecoveryDecision::HumanReview,
                        "completed result lacks a local artifact and cannot be replayed safely"
                            .into(),
                    )
                }
            }
            InstrumentExecutionDisposition::Negative => {
                if result.artifact.is_some() {
                    (
                        InstrumentRecoveryDecision::VerifyArtifact,
                        "negative result is preserved and requires artifact verification, not automatic retry".into(),
                    )
                } else {
                    (
                        InstrumentRecoveryDecision::HumanReview,
                        "negative result lacks a local artifact; preserve the claim and require review".into(),
                    )
                }
            }
            InstrumentExecutionDisposition::Partial => {
                if request.require_human_for_partial {
                    (
                        InstrumentRecoveryDecision::HumanReview,
                        "partial physical execution requires explicit operator review before compensation or resume".into(),
                    )
                } else if request.allow_retry && result.attempt_count < request.max_retry_attempts {
                    (
                        InstrumentRecoveryDecision::Resume,
                        "partial execution may resume only from the gateway checkpoint under a fresh preflight".into(),
                    )
                } else {
                    (
                        InstrumentRecoveryDecision::Compensate,
                        "partial execution exceeded the retry budget and requires honest compensation".into(),
                    )
                }
            }
            InstrumentExecutionDisposition::Failed => {
                if request.allow_retry && result.attempt_count < request.max_retry_attempts {
                    (
                        InstrumentRecoveryDecision::Retry,
                        "retryable gateway failure remains within the bounded retry budget".into(),
                    )
                } else {
                    (
                        InstrumentRecoveryDecision::HumanReview,
                        "gateway failure cannot be retried within policy limits".into(),
                    )
                }
            }
            InstrumentExecutionDisposition::Blocked => {
                if request.allow_recalibration
                    && result
                        .uncertainty
                        .iter()
                        .any(|item| item.contains("calibrat") || item.contains("interlock"))
                {
                    (
                        InstrumentRecoveryDecision::Recalibrate,
                        "execution was blocked by a calibration/interlock condition; refresh local readiness before re-planning".into(),
                    )
                } else {
                    (
                        InstrumentRecoveryDecision::Abort,
                        "execution was blocked without a qualified local recovery path".into(),
                    )
                }
            }
            InstrumentExecutionDisposition::Unresolved => (
                InstrumentRecoveryDecision::HumanReview,
                "unresolved gateway state cannot be inferred as success or failure".into(),
            ),
        };
        let action_id = format!("recovery:{}", result.action_id);
        by_action.insert(
            action_id.clone(),
            InstrumentRecoveryAction {
                action_id,
                source_action_id: result.action_id.clone(),
                decision,
                priority: priority(decision),
                rationale,
                preserves_negative_evidence: result.disposition
                    == InstrumentExecutionDisposition::Negative
                    || !result.negative_evidence.is_empty(),
            },
        );
    }
    let actions = by_action.into_values().collect::<Vec<_>>();
    let action_order = actions
        .iter()
        .map(|action| action.action_id.clone())
        .collect::<Vec<_>>();
    let mut recovery_order = actions
        .iter()
        .filter(|action| action.decision != InstrumentRecoveryDecision::NoAction)
        .map(|action| action.action_id.clone())
        .collect::<Vec<_>>();
    recovery_order.sort_by_key(|id| {
        let action = actions
            .iter()
            .find(|action| &action.action_id == id)
            .unwrap();
        (action.priority as u8, id.clone())
    });
    let partition = |decision: InstrumentRecoveryDecision| {
        actions
            .iter()
            .filter(|action| action.decision == decision)
            .map(|action| action.action_id.clone())
            .collect::<Vec<_>>()
    };
    let retry_order = partition(InstrumentRecoveryDecision::Retry);
    let resume_order = partition(InstrumentRecoveryDecision::Resume);
    let recalibration_order = partition(InstrumentRecoveryDecision::Recalibrate);
    let compensation_order = partition(InstrumentRecoveryDecision::Compensate);
    let human_review_order = partition(InstrumentRecoveryDecision::HumanReview);
    let blocked_order = actions
        .iter()
        .filter(|action| {
            matches!(
                action.decision,
                InstrumentRecoveryDecision::HumanReview | InstrumentRecoveryDecision::Abort
            )
        })
        .map(|action| action.action_id.clone())
        .collect::<Vec<_>>();
    let mut negative_evidence = request.run.negative_evidence.clone();
    negative_evidence.extend(
        actions
            .iter()
            .filter(|action| action.preserves_negative_evidence)
            .map(|action| format!("{}:negative-evidence-preserved", action.source_action_id)),
    );
    negative_evidence.sort();
    negative_evidence.dedup();
    let mut uncertainty = request.run.uncertainty.clone();
    if matches!(
        request.run.stop_reason,
        InstrumentExecutionStopReason::InterlockChanged
            | InstrumentExecutionStopReason::EmergencyStopFailed
            | InstrumentExecutionStopReason::UnresolvedResult
    ) {
        uncertainty.push(format!("stop-reason:{:?}", request.run.stop_reason));
    }
    uncertainty.sort();
    uncertainty.dedup();
    let disposition = if !blocked_order.is_empty() {
        InstrumentRecoveryDisposition::Blocked
    } else if recovery_order.is_empty() {
        InstrumentRecoveryDisposition::NoRecovery
    } else if actions.iter().any(|action| {
        matches!(
            action.decision,
            InstrumentRecoveryDecision::Compensate | InstrumentRecoveryDecision::VerifyArtifact
        )
    }) {
        InstrumentRecoveryDisposition::Partial
    } else {
        InstrumentRecoveryDisposition::Ready
    };
    let mut plan = InstrumentRecoveryPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        run_digest: request.run.digest.clone(),
        instrument_id: request.run.instrument_id.clone(),
        action_order,
        actions,
        recovery_order,
        retry_order,
        resume_order,
        recalibration_order,
        compensation_order,
        human_review_order,
        blocked_order,
        negative_evidence,
        uncertainty,
        disposition,
        digest: ContentHash::of_bytes(b"placeholder"),
    };
    plan.digest = ContentHash::of_value(&digest_input(&plan))
        .map_err(|error| InstrumentRecoveryError::Digest(error.to_string()))?;
    plan.validate()?;
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p08_instrument_robotics::execution::{
        InstrumentExecutionResult, InstrumentExecutionStopReason,
    };
    use crate::glioma_engine::LocalArtifactRef;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn artifact(action_id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: format!("artifact:{action_id}"),
            content_hash: hash(action_id),
            content_type: "application/vnd.aurora.glioma.instrument-operation+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn run(
        results: Vec<InstrumentExecutionResult>,
        stop_reason: InstrumentExecutionStopReason,
    ) -> InstrumentExecutionRun {
        let mut output = InstrumentExecutionRun {
            feature_id: super::super::execution::FEATURE_ID.into(),
            output_schema: super::super::execution::OUTPUT_SCHEMA.into(),
            objective: "recovery test".into(),
            plan_digest: hash("plan"),
            instrument_id: "instrument-1".into(),
            action_order: results
                .iter()
                .map(|result| result.action_id.clone())
                .collect(),
            results,
            completed_order: Vec::new(),
            negative_order: Vec::new(),
            partial_order: Vec::new(),
            failed_order: Vec::new(),
            unresolved_order: Vec::new(),
            skipped_order: Vec::new(),
            retry_count: 0,
            emergency_stop_requested: false,
            emergency_stop_succeeded: false,
            uncertainty: Vec::new(),
            negative_evidence: Vec::new(),
            disposition: InstrumentExecutionDisposition::Partial,
            stop_reason,
            digest: hash("placeholder"),
        };
        for result in &output.results {
            match result.disposition {
                InstrumentExecutionDisposition::Completed => {
                    output.completed_order.push(result.action_id.clone())
                }
                InstrumentExecutionDisposition::Negative => {
                    output.negative_order.push(result.action_id.clone())
                }
                InstrumentExecutionDisposition::Partial => {
                    output.partial_order.push(result.action_id.clone())
                }
                InstrumentExecutionDisposition::Failed => {
                    output.failed_order.push(result.action_id.clone())
                }
                InstrumentExecutionDisposition::Blocked => {
                    output.skipped_order.push(result.action_id.clone())
                }
                InstrumentExecutionDisposition::Unresolved => {
                    output.unresolved_order.push(result.action_id.clone())
                }
            }
        }
        output.completed_order.sort();
        output.negative_order.sort();
        output.partial_order.sort();
        output.failed_order.sort();
        output.skipped_order.sort();
        output.unresolved_order.sort();
        output.digest =
            ContentHash::of_value(&super::super::execution::digest_input(&output)).unwrap();
        output
    }

    fn result(
        action_id: &str,
        disposition: InstrumentExecutionDisposition,
        attempt_count: u8,
        with_artifact: bool,
    ) -> InstrumentExecutionResult {
        InstrumentExecutionResult {
            action_id: action_id.into(),
            disposition,
            attempt_count,
            started_tick: Some(1),
            completed_tick: Some(2),
            artifact: with_artifact.then(|| artifact(action_id)),
            note: "gateway result".into(),
            uncertainty: Vec::new(),
            negative_evidence: Vec::new(),
        }
    }

    #[test]
    fn recovery_retries_failed_and_preserves_negative_results() {
        let output = plan_glioma_instrument_recovery(&InstrumentRecoveryRequest {
            objective: "recovery test".into(),
            run: run(
                vec![
                    result("a1", InstrumentExecutionDisposition::Failed, 1, false),
                    result("a2", InstrumentExecutionDisposition::Negative, 1, true),
                ],
                InstrumentExecutionStopReason::ExecutorFailed,
            ),
            allow_retry: true,
            allow_recalibration: false,
            max_retry_attempts: 3,
            require_human_for_partial: false,
        })
        .unwrap();
        assert_eq!(output.retry_order, vec!["recovery:a1"]);
        assert_eq!(
            output.actions[1].decision,
            InstrumentRecoveryDecision::VerifyArtifact
        );
        assert!(output
            .negative_evidence
            .iter()
            .any(|item| item == "a2:negative-evidence-preserved"));
        output.validate().unwrap();
    }

    #[test]
    fn partial_execution_can_be_blocked_for_human_review() {
        let output = plan_glioma_instrument_recovery(&InstrumentRecoveryRequest {
            objective: "recovery test".into(),
            run: run(
                vec![result(
                    "a1",
                    InstrumentExecutionDisposition::Partial,
                    1,
                    true,
                )],
                InstrumentExecutionStopReason::PartialResult,
            ),
            allow_retry: true,
            allow_recalibration: false,
            max_retry_attempts: 3,
            require_human_for_partial: true,
        })
        .unwrap();
        assert_eq!(output.disposition, InstrumentRecoveryDisposition::Blocked);
        assert_eq!(output.human_review_order, vec!["recovery:a1"]);
        output.validate().unwrap();
    }
}

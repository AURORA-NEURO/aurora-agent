//! Guarded execution of an admitted preclinical glioma instrument plan.
//!
//! P08 preflight is intentionally planning-only. This module provides the next production seam:
//! an institution-owned gateway can execute an admitted plan while the controller rechecks
//! authorization and interlocks before every operation, bounds retries, and requests an emergency
//! stop on any unsafe or partial outcome. The research crate never opens a device connection and
//! never treats a successful instrument operation as scientific evidence.

use super::preflight::{
    InstrumentAction, InstrumentAuthorization, InstrumentInterlockSnapshot,
    InstrumentPreflightDisposition, InstrumentPreflightPlan,
};
use crate::glioma_engine::LocalArtifactRef;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P08-F11";
pub const OUTPUT_SCHEMA: &str = "GliomaInstrumentExecution1@1";
pub const MAX_RETRIES: u8 = 8;
pub const MAX_ACTIONS: usize = 1_024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentExecutionRequest {
    pub objective: String,
    pub plan: InstrumentPreflightPlan,
    pub actions: Vec<InstrumentAction>,
    pub authorization: InstrumentAuthorization,
    pub live_interlocks: InstrumentInterlockSnapshot,
    pub current_tick: u64,
    pub minimum_waste_capacity_milli: u64,
    pub max_retries: u8,
    pub require_artifacts: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstrumentExecutionDisposition {
    Completed,
    Negative,
    Partial,
    Failed,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstrumentExecutionStopReason {
    Completed,
    PreflightBlocked,
    AuthorizationInvalid,
    InterlockChanged,
    ExecutorFailed,
    EmergencyStopFailed,
    PartialResult,
    UnresolvedResult,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentExecutionResult {
    pub action_id: String,
    pub disposition: InstrumentExecutionDisposition,
    pub attempt_count: u8,
    pub started_tick: Option<u64>,
    pub completed_tick: Option<u64>,
    pub artifact: Option<LocalArtifactRef>,
    pub note: String,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstrumentExecutionFailure {
    pub reason: String,
    pub retryable: bool,
}

/// Institution-local gateways implement this seam. The gateway owns transport, hardware
/// authentication, emergency-stop wiring, and physical safety interlocks; the controller owns
/// ordering, retry bounds, artifact validation, and fail-closed classification.
pub trait InstrumentExecutor {
    fn observe_interlocks(
        &mut self,
    ) -> Result<InstrumentInterlockSnapshot, InstrumentExecutionFailure>;

    fn verify_authorization(
        &mut self,
        authorization: &InstrumentAuthorization,
    ) -> Result<(), InstrumentExecutionFailure>;

    fn execute_action(
        &mut self,
        action: &InstrumentAction,
        attempt: u8,
    ) -> Result<InstrumentExecutionResult, InstrumentExecutionFailure>;

    fn emergency_stop(&mut self) -> Result<(), InstrumentExecutionFailure>;
}

/// A deterministic executor for sandbox and protocol tests. It emits local synthetic artifacts
/// and never contacts hardware, consumes material, or creates biological evidence.
#[derive(Debug)]
pub struct DryRunInstrumentExecutor {
    pub interlocks: InstrumentInterlockSnapshot,
    pub emergency_stop_called: bool,
}

impl InstrumentExecutor for DryRunInstrumentExecutor {
    fn observe_interlocks(
        &mut self,
    ) -> Result<InstrumentInterlockSnapshot, InstrumentExecutionFailure> {
        Ok(self.interlocks.clone())
    }

    fn verify_authorization(
        &mut self,
        _authorization: &InstrumentAuthorization,
    ) -> Result<(), InstrumentExecutionFailure> {
        Ok(())
    }

    fn execute_action(
        &mut self,
        action: &InstrumentAction,
        attempt: u8,
    ) -> Result<InstrumentExecutionResult, InstrumentExecutionFailure> {
        let content_hash = ContentHash::of_value(&serde_json::json!({
            "action_id": action.action_id,
            "instrument_id": action.instrument_id,
            "operation": action.operation,
            "attempt": attempt,
            "simulation_only": true,
        }))
        .map_err(|error| InstrumentExecutionFailure {
            reason: format!("dry-run artifact digest failed: {error}"),
            retryable: false,
        })?;
        Ok(InstrumentExecutionResult {
            action_id: action.action_id.clone(),
            disposition: InstrumentExecutionDisposition::Completed,
            attempt_count: attempt,
            started_tick: Some(action.requested_start_tick),
            completed_tick: Some(
                action
                    .requested_start_tick
                    .saturating_add(action.duration_ticks),
            ),
            artifact: Some(LocalArtifactRef {
                artifact_id: format!("dry-run-instrument:{}", action.action_id),
                content_hash,
                content_type: "application/vnd.aurora.glioma.instrument-operation+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            }),
            note:
                "dry-run instrument operation completed; no hardware or biological effect occurred"
                    .into(),
            uncertainty: vec!["simulation-only-operation".into()],
            negative_evidence: vec!["synthetic-operation-is-not-biological-evidence".into()],
        })
    }

    fn emergency_stop(&mut self) -> Result<(), InstrumentExecutionFailure> {
        self.emergency_stop_called = true;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentExecutionRun {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub plan_digest: ContentHash,
    pub instrument_id: String,
    pub action_order: Vec<String>,
    pub results: Vec<InstrumentExecutionResult>,
    pub completed_order: Vec<String>,
    pub negative_order: Vec<String>,
    pub partial_order: Vec<String>,
    pub failed_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub skipped_order: Vec<String>,
    pub retry_count: u32,
    pub emergency_stop_requested: bool,
    pub emergency_stop_succeeded: bool,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub disposition: InstrumentExecutionDisposition,
    pub stop_reason: InstrumentExecutionStopReason,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum InstrumentExecutionError {
    #[error("instrument execution request is invalid: {0}")]
    InvalidRequest(String),
    #[error("instrument preflight is not executable: {0}")]
    PreflightBlocked(String),
    #[error("instrument gateway returned an invalid result: {0}")]
    InvalidGatewayResult(String),
    #[error("instrument execution output is invalid: {0}")]
    InvalidOutput(String),
    #[error("instrument execution digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(run: &InstrumentExecutionRun) -> serde_json::Value {
    serde_json::json!({
        "feature_id": run.feature_id,
        "output_schema": run.output_schema,
        "objective": run.objective,
        "plan_digest": run.plan_digest,
        "instrument_id": run.instrument_id,
        "action_order": run.action_order,
        "results": run.results,
        "completed_order": run.completed_order,
        "negative_order": run.negative_order,
        "partial_order": run.partial_order,
        "failed_order": run.failed_order,
        "unresolved_order": run.unresolved_order,
        "skipped_order": run.skipped_order,
        "retry_count": run.retry_count,
        "emergency_stop_requested": run.emergency_stop_requested,
        "emergency_stop_succeeded": run.emergency_stop_succeeded,
        "uncertainty": run.uncertainty,
        "negative_evidence": run.negative_evidence,
        "disposition": run.disposition,
        "stop_reason": run.stop_reason,
    })
}

impl InstrumentExecutionRun {
    pub fn validate(&self) -> Result<(), InstrumentExecutionError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.instrument_id.trim().is_empty()
            || self.plan_digest.as_str().len() != 64
            || self.action_order.len() != self.results.len()
            || self.action_order.len() > MAX_ACTIONS
            || self.action_order.windows(2).any(|pair| pair[0] == pair[1])
            || !canonical(&self.completed_order)
            || !canonical(&self.negative_order)
            || !canonical(&self.partial_order)
            || !canonical(&self.failed_order)
            || !canonical(&self.skipped_order)
            || !canonical(&self.uncertainty)
            || !canonical(&self.negative_evidence)
            || self.results.iter().any(|result| {
                result.action_id.trim().is_empty()
                    || result.note.trim().is_empty()
                    || result.attempt_count == 0
                    || result.uncertainty.iter().any(|item| item.trim().is_empty())
                    || result
                        .negative_evidence
                        .iter()
                        .any(|item| item.trim().is_empty())
                    || result
                        .artifact
                        .as_ref()
                        .is_some_and(|artifact| artifact.validate().is_err())
            })
        {
            return Err(InstrumentExecutionError::InvalidOutput(
                "identity, action results, ordering, artifact, or partition invariants are invalid"
                    .into(),
            ));
        }
        let action_ids = self.action_order.iter().cloned().collect::<BTreeSet<_>>();
        let result_ids = self
            .results
            .iter()
            .map(|result| result.action_id.clone())
            .collect::<BTreeSet<_>>();
        if action_ids != result_ids {
            return Err(InstrumentExecutionError::InvalidOutput(
                "action and result identities do not reconcile".into(),
            ));
        }
        for (order, disposition) in [
            (
                &self.completed_order,
                InstrumentExecutionDisposition::Completed,
            ),
            (
                &self.negative_order,
                InstrumentExecutionDisposition::Negative,
            ),
            (&self.partial_order, InstrumentExecutionDisposition::Partial),
            (&self.failed_order, InstrumentExecutionDisposition::Failed),
            (
                &self.unresolved_order,
                InstrumentExecutionDisposition::Unresolved,
            ),
            (&self.skipped_order, InstrumentExecutionDisposition::Blocked),
        ] {
            let expected = self
                .results
                .iter()
                .filter(|result| result.disposition == disposition)
                .map(|result| result.action_id.clone())
                .collect::<BTreeSet<_>>();
            if order.iter().cloned().collect::<BTreeSet<_>>() != expected {
                return Err(InstrumentExecutionError::InvalidOutput(
                    "instrument result disposition partitions do not reconcile".into(),
                ));
            }
        }
        if self.emergency_stop_succeeded && !self.emergency_stop_requested {
            return Err(InstrumentExecutionError::InvalidOutput(
                "emergency-stop success requires a requested stop".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| InstrumentExecutionError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(InstrumentExecutionError::InvalidOutput(
                "instrument execution digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(request: &InstrumentExecutionRequest) -> Result<(), InstrumentExecutionError> {
    if request.objective.trim().is_empty()
        || request.actions.is_empty()
        || request.actions.len() > MAX_ACTIONS
        || request.current_tick > super::preflight::MAX_TICK
        || request.live_interlocks.observed_tick < request.current_tick
        || request.live_interlocks.calibration_valid_until_tick < request.current_tick
        || request.live_interlocks.waste_capacity_milli < request.minimum_waste_capacity_milli
        || request.max_retries > MAX_RETRIES
        || request.authorization.authorization_id.trim().is_empty()
        || request.authorization.operator_id.trim().is_empty()
        || request.authorization.instrument_scope.trim().is_empty()
        || request.authorization.approval_digest.as_str().len() != 64
        || request.authorization.revoked
        || request.authorization.issued_tick > request.current_tick
        || request.authorization.expires_tick <= request.current_tick
        || request.authorization.expires_tick > super::preflight::MAX_TICK
    {
        return Err(InstrumentExecutionError::InvalidRequest(
            "bounded actions, live interlocks, authorization, current tick, retries, and local waste capacity are required"
                .into(),
        ));
    }
    Ok(())
}

fn safe_interlocks(
    snapshot: &InstrumentInterlockSnapshot,
    request: &InstrumentExecutionRequest,
) -> bool {
    snapshot.observed_tick >= request.current_tick
        && snapshot.emergency_stop_clear
        && snapshot.guard_closed
        && snapshot.deck_clear
        && snapshot.consumables_available
        && snapshot.calibration_valid_until_tick >= snapshot.observed_tick
        && snapshot.waste_capacity_milli >= request.minimum_waste_capacity_milli
        && snapshot
            .temperature_milli
            .zip(snapshot.minimum_temperature_milli)
            .zip(snapshot.maximum_temperature_milli)
            .is_none_or(|((temperature, minimum), maximum)| {
                temperature >= minimum && temperature <= maximum
            })
}

fn gateway_result(
    action: &InstrumentAction,
    mut result: InstrumentExecutionResult,
    attempt: u8,
    require_artifacts: bool,
) -> Result<InstrumentExecutionResult, InstrumentExecutionError> {
    if result.action_id != action.action_id
        || result.note.trim().is_empty()
        || result.attempt_count == 0
        || matches!(
            result.disposition,
            InstrumentExecutionDisposition::Blocked | InstrumentExecutionDisposition::Failed
        )
        || result.uncertainty.iter().any(|item| item.trim().is_empty())
        || result
            .negative_evidence
            .iter()
            .any(|item| item.trim().is_empty())
    {
        return Err(InstrumentExecutionError::InvalidGatewayResult(format!(
            "gateway returned an invalid result for action {}",
            action.action_id
        )));
    }
    if require_artifacts
        && result.artifact.is_none()
        && result.disposition != InstrumentExecutionDisposition::Unresolved
    {
        return Err(InstrumentExecutionError::InvalidGatewayResult(format!(
            "gateway returned no required local artifact for action {}",
            action.action_id
        )));
    }
    if let Some(artifact) = &result.artifact {
        artifact
            .validate()
            .map_err(|error| InstrumentExecutionError::InvalidGatewayResult(error.to_string()))?;
    }
    result.attempt_count = attempt;
    Ok(result)
}

/// Execute an admitted instrument plan through a caller-owned, hardware-specific gateway.
pub fn execute_glioma_instrument_plan<E: InstrumentExecutor>(
    request: &InstrumentExecutionRequest,
    executor: &mut E,
) -> Result<InstrumentExecutionRun, InstrumentExecutionError> {
    validate_request(request)?;
    request
        .plan
        .validate()
        .map_err(|error| InstrumentExecutionError::PreflightBlocked(error.to_string()))?;
    if request.objective.trim() != request.plan.objective.trim()
        || request.authorization.authorization_id != request.plan.authorization_id
        || request.authorization.instrument_scope != request.plan.instrument_id
        || request.plan.disposition != InstrumentPreflightDisposition::Admitted
        || !request.plan.dispatch_permitted
        || request.plan.admitted_order != request.plan.action_order
    {
        return Err(InstrumentExecutionError::PreflightBlocked(
            "objective, authorization, instrument scope, or preflight admission does not permit execution"
                .into(),
        ));
    }
    let action_ids = request
        .actions
        .iter()
        .map(|action| action.action_id.clone())
        .collect::<BTreeSet<_>>();
    if action_ids.len() != request.actions.len()
        || action_ids != request.plan.action_order.iter().cloned().collect()
        || request.actions.iter().any(|action| {
            action.instrument_id != request.plan.instrument_id
                || action.model_system != request.plan.model_system
        })
    {
        return Err(InstrumentExecutionError::InvalidRequest(
            "action identities, instrument, model system, and admitted plan order must reconcile"
                .into(),
        ));
    }
    let action_map = request
        .actions
        .iter()
        .map(|action| (action.action_id.clone(), action))
        .collect::<BTreeMap<_, _>>();
    let mut results = Vec::new();
    let mut completed = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut partial = BTreeSet::new();
    let mut failed = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut skipped = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative_evidence = BTreeSet::new();
    let mut retry_count = 0_u32;
    let mut emergency_stop_requested = false;
    let mut emergency_stop_succeeded = false;
    let mut stop_reason = InstrumentExecutionStopReason::Completed;
    let mut disposition = InstrumentExecutionDisposition::Completed;
    for action_id in &request.plan.action_order {
        let action = action_map[action_id];
        let interlocks = match executor.observe_interlocks() {
            Ok(snapshot) => snapshot,
            Err(error) => {
                emergency_stop_requested = true;
                emergency_stop_succeeded = executor.emergency_stop().is_ok();
                stop_reason = if emergency_stop_succeeded {
                    InstrumentExecutionStopReason::ExecutorFailed
                } else {
                    InstrumentExecutionStopReason::EmergencyStopFailed
                };
                disposition = InstrumentExecutionDisposition::Failed;
                failed.insert(action_id.clone());
                uncertainty.insert(format!("interlock-observation-failed:{}", error.reason));
                results.push(InstrumentExecutionResult {
                    action_id: action_id.clone(),
                    disposition: InstrumentExecutionDisposition::Failed,
                    attempt_count: 1,
                    started_tick: None,
                    completed_tick: None,
                    artifact: None,
                    note: format!(
                        "instrument gateway interlock observation failed before dispatch: {}",
                        error.reason
                    ),
                    uncertainty: vec!["interlock-observation-failed".into()],
                    negative_evidence: Vec::new(),
                });
                for remaining in request.plan.action_order.iter().skip(results.len()) {
                    skipped.insert(remaining.clone());
                    results.push(InstrumentExecutionResult {
                        action_id: remaining.clone(),
                        disposition: InstrumentExecutionDisposition::Blocked,
                        attempt_count: 1,
                        started_tick: None,
                        completed_tick: None,
                        artifact: None,
                        note: "skipped after interlock observation failed before dispatch".into(),
                        uncertainty: vec!["interlock-observation-failed".into()],
                        negative_evidence: Vec::new(),
                    });
                }
                break;
            }
        };
        if !safe_interlocks(&interlocks, request) {
            emergency_stop_requested = true;
            emergency_stop_succeeded = executor.emergency_stop().is_ok();
            stop_reason = if emergency_stop_succeeded {
                InstrumentExecutionStopReason::InterlockChanged
            } else {
                InstrumentExecutionStopReason::EmergencyStopFailed
            };
            disposition = InstrumentExecutionDisposition::Blocked;
            for remaining in request.plan.action_order.iter().skip(results.len()) {
                skipped.insert(remaining.clone());
                results.push(InstrumentExecutionResult {
                    action_id: remaining.clone(),
                    disposition: InstrumentExecutionDisposition::Blocked,
                    attempt_count: 1,
                    started_tick: None,
                    completed_tick: None,
                    artifact: None,
                    note: "skipped after a live interlock failed before dispatch".into(),
                    uncertainty: vec!["interlock-recheck-failed".into()],
                    negative_evidence: Vec::new(),
                });
            }
            uncertainty.insert("live-interlock-recheck-failed".into());
            break;
        }
        if executor
            .verify_authorization(&request.authorization)
            .is_err()
        {
            emergency_stop_requested = true;
            emergency_stop_succeeded = executor.emergency_stop().is_ok();
            stop_reason = if emergency_stop_succeeded {
                InstrumentExecutionStopReason::AuthorizationInvalid
            } else {
                InstrumentExecutionStopReason::EmergencyStopFailed
            };
            disposition = InstrumentExecutionDisposition::Blocked;
            for remaining in request.plan.action_order.iter().skip(results.len()) {
                skipped.insert(remaining.clone());
                results.push(InstrumentExecutionResult {
                    action_id: remaining.clone(),
                    disposition: InstrumentExecutionDisposition::Blocked,
                    attempt_count: 1,
                    started_tick: None,
                    completed_tick: None,
                    artifact: None,
                    note: "skipped after gateway authorization recheck failed".into(),
                    uncertainty: vec!["authorization-recheck-failed".into()],
                    negative_evidence: Vec::new(),
                });
            }
            uncertainty.insert("authorization-recheck-failed".into());
            break;
        }
        let mut accepted = None;
        for attempt in 1..=request.max_retries.saturating_add(1) {
            match executor.execute_action(action, attempt) {
                Ok(result) => {
                    let result =
                        gateway_result(action, result, attempt, request.require_artifacts)?;
                    if result.disposition == InstrumentExecutionDisposition::Partial {
                        partial.insert(action_id.clone());
                        disposition = InstrumentExecutionDisposition::Partial;
                        stop_reason = InstrumentExecutionStopReason::PartialResult;
                        uncertainty.extend(result.uncertainty.iter().cloned());
                        negative_evidence.extend(result.negative_evidence.iter().cloned());
                        accepted = Some(result);
                        break;
                    }
                    if result.disposition == InstrumentExecutionDisposition::Negative {
                        negative.insert(action_id.clone());
                    } else if result.disposition == InstrumentExecutionDisposition::Unresolved {
                        unresolved.insert(action_id.clone());
                    } else {
                        completed.insert(action_id.clone());
                    }
                    uncertainty.extend(result.uncertainty.iter().cloned());
                    negative_evidence.extend(result.negative_evidence.iter().cloned());
                    accepted = Some(result);
                    break;
                }
                Err(error) if error.retryable && attempt <= request.max_retries => {
                    retry_count = retry_count.saturating_add(1);
                }
                Err(error) => {
                    failed.insert(action_id.clone());
                    uncertainty.insert(format!("executor-failed:{}", error.reason));
                    stop_reason = InstrumentExecutionStopReason::ExecutorFailed;
                    disposition = InstrumentExecutionDisposition::Failed;
                    accepted = Some(InstrumentExecutionResult {
                        action_id: action_id.clone(),
                        disposition: InstrumentExecutionDisposition::Failed,
                        attempt_count: attempt,
                        started_tick: None,
                        completed_tick: None,
                        artifact: None,
                        note: format!("instrument gateway failed: {}", error.reason),
                        uncertainty: vec!["executor-failure".into()],
                        negative_evidence: Vec::new(),
                    });
                    break;
                }
            }
        }
        let result = accepted.expect("bounded retry loop always accepts a result");
        if result.disposition == InstrumentExecutionDisposition::Partial {
            results.push(result);
            emergency_stop_requested = true;
            emergency_stop_succeeded = executor.emergency_stop().is_ok();
            if !emergency_stop_succeeded {
                stop_reason = InstrumentExecutionStopReason::EmergencyStopFailed;
            }
            for remaining in request.plan.action_order.iter().skip(results.len()) {
                skipped.insert(remaining.clone());
                results.push(InstrumentExecutionResult {
                    action_id: remaining.clone(),
                    disposition: InstrumentExecutionDisposition::Blocked,
                    attempt_count: 1,
                    started_tick: None,
                    completed_tick: None,
                    artifact: None,
                    note: "skipped after a partial instrument effect".into(),
                    uncertainty: vec!["partial-effect-halt".into()],
                    negative_evidence: Vec::new(),
                });
            }
            break;
        }
        if result.disposition == InstrumentExecutionDisposition::Unresolved {
            unresolved.insert(action_id.clone());
            results.push(result);
            disposition = InstrumentExecutionDisposition::Unresolved;
            stop_reason = InstrumentExecutionStopReason::UnresolvedResult;
            emergency_stop_requested = true;
            emergency_stop_succeeded = executor.emergency_stop().is_ok();
            if !emergency_stop_succeeded {
                stop_reason = InstrumentExecutionStopReason::EmergencyStopFailed;
            }
            for remaining in request.plan.action_order.iter().skip(results.len()) {
                skipped.insert(remaining.clone());
                results.push(InstrumentExecutionResult {
                    action_id: remaining.clone(),
                    disposition: InstrumentExecutionDisposition::Blocked,
                    attempt_count: 1,
                    started_tick: None,
                    completed_tick: None,
                    artifact: None,
                    note: "skipped after an unresolved instrument result".into(),
                    uncertainty: vec!["unresolved-result-halt".into()],
                    negative_evidence: Vec::new(),
                });
            }
            break;
        }
        if result.disposition == InstrumentExecutionDisposition::Failed {
            results.push(result);
            emergency_stop_requested = true;
            emergency_stop_succeeded = executor.emergency_stop().is_ok();
            if !emergency_stop_succeeded {
                stop_reason = InstrumentExecutionStopReason::EmergencyStopFailed;
            }
            for remaining in request.plan.action_order.iter().skip(results.len()) {
                skipped.insert(remaining.clone());
                results.push(InstrumentExecutionResult {
                    action_id: remaining.clone(),
                    disposition: InstrumentExecutionDisposition::Blocked,
                    attempt_count: 1,
                    started_tick: None,
                    completed_tick: None,
                    artifact: None,
                    note: "skipped after an instrument gateway failure".into(),
                    uncertainty: vec!["executor-failure-halt".into()],
                    negative_evidence: Vec::new(),
                });
            }
            break;
        }
        results.push(result);
    }
    let mut run = InstrumentExecutionRun {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        plan_digest: request.plan.digest.clone(),
        instrument_id: request.plan.instrument_id.clone(),
        action_order: request.plan.action_order.clone(),
        results,
        completed_order: completed.into_iter().collect(),
        negative_order: negative.into_iter().collect(),
        partial_order: partial.into_iter().collect(),
        failed_order: failed.into_iter().collect(),
        unresolved_order: unresolved.into_iter().collect(),
        skipped_order: skipped.into_iter().collect(),
        retry_count,
        emergency_stop_requested,
        emergency_stop_succeeded,
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative_evidence.into_iter().collect(),
        disposition,
        stop_reason,
        digest: ContentHash::of_bytes(b"unsealed-glioma-instrument-execution"),
    };
    if run.disposition == InstrumentExecutionDisposition::Completed
        && run.completed_order.len() != run.action_order.len()
    {
        run.disposition = InstrumentExecutionDisposition::Negative;
    }
    run.digest = ContentHash::of_value(&digest_input(&run))
        .map_err(|error| InstrumentExecutionError::Digest(error.to_string()))?;
    run.validate()?;
    Ok(run)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p08_instrument_robotics::calibration::{
        analyze_instrument_calibration, CalibrationRequest, CalibrationRun,
    };
    use crate::glioma::programs::p08_instrument_robotics::preflight::{
        preflight_glioma_instrument, InstrumentOperation, InstrumentParameter,
        InstrumentPreflightRequest,
    };
    use crate::glioma_engine::GliomaModelSystem;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_bytes(label.as_bytes())
    }

    fn calibration(
    ) -> crate::glioma::programs::p08_instrument_robotics::calibration::InstrumentCalibration {
        let runs = (1..=3)
            .map(|sequence_index| CalibrationRun {
                run_id: format!("cal-{sequence_index}"),
                sequence_index,
                batch_id: format!("batch-{sequence_index}"),
                instrument_id: "imager-1".into(),
                metric_name: "control".into(),
                model_system: GliomaModelSystem::Organoid,
                observed_milli: 500 + i64::from(sequence_index),
                expected_milli: 500,
                artifact: LocalArtifactRef {
                    artifact_id: format!("artifact-{sequence_index}"),
                    content_hash: hash(&format!("artifact-{sequence_index}")),
                    content_type: "application/vnd.aurora.glioma-control+json".into(),
                    local_only: true,
                    contains_human_data: false,
                    contains_direct_identifiers: false,
                },
            })
            .collect::<Vec<_>>();
        analyze_instrument_calibration(
            &CalibrationRequest {
                objective: "qualify imager".into(),
                instrument_id: "imager-1".into(),
                model_system: GliomaModelSystem::Organoid,
                metric_name: "control".into(),
                minimum_runs: 3,
                reference_run_count: 2,
                max_reference_mad_milli: 5,
                max_drift_milli: 20,
                max_slope_milli_per_tick: 10,
            },
            &runs,
        )
        .unwrap()
    }

    fn action() -> InstrumentAction {
        InstrumentAction {
            action_id: "acquire".into(),
            instrument_id: "imager-1".into(),
            operation: InstrumentOperation::AcquireImage,
            model_system: GliomaModelSystem::Organoid,
            requested_start_tick: 1,
            duration_ticks: 2,
            risk_milli: 100,
            requires_operator: false,
            output_schema: "Image1@1".into(),
            parameters: Vec::<InstrumentParameter>::new(),
        }
    }

    fn request() -> InstrumentExecutionRequest {
        let action = action();
        let interlocks = InstrumentInterlockSnapshot {
            observed_tick: 1,
            emergency_stop_clear: true,
            guard_closed: true,
            deck_clear: true,
            consumables_available: true,
            waste_capacity_milli: 100_000,
            temperature_milli: Some(37_000),
            minimum_temperature_milli: Some(36_000),
            maximum_temperature_milli: Some(38_000),
            calibration_valid_until_tick: 100,
            calibration_sequence_index: 3,
        };
        let authorization = InstrumentAuthorization {
            authorization_id: "approval-1".into(),
            operator_id: "operator-1".into(),
            instrument_scope: "imager-1".into(),
            approval_digest: ContentHash::of_bytes(b"approval"),
            issued_tick: 0,
            expires_tick: 100,
            revoked: false,
        };
        let preflight_request = InstrumentPreflightRequest {
            objective: "acquire image".into(),
            instrument_id: "imager-1".into(),
            model_system: GliomaModelSystem::Organoid,
            actions: vec![action.clone()],
            calibration: calibration(),
            interlocks: interlocks.clone(),
            authorization: authorization.clone(),
            current_tick: 1,
            maximum_total_risk_milli: 500,
            maximum_duration_ticks: 20,
            minimum_waste_capacity_milli: 100,
        };
        let plan = preflight_glioma_instrument(&preflight_request).unwrap();
        InstrumentExecutionRequest {
            objective: "acquire image".into(),
            plan,
            actions: vec![action],
            authorization,
            live_interlocks: interlocks,
            current_tick: 1,
            minimum_waste_capacity_milli: 100,
            max_retries: 1,
            require_artifacts: true,
        }
    }

    struct UnresolvedExecutor {
        interlocks: InstrumentInterlockSnapshot,
        emergency_stop_called: bool,
    }

    impl InstrumentExecutor for UnresolvedExecutor {
        fn observe_interlocks(
            &mut self,
        ) -> Result<InstrumentInterlockSnapshot, InstrumentExecutionFailure> {
            Ok(self.interlocks.clone())
        }

        fn verify_authorization(
            &mut self,
            _authorization: &InstrumentAuthorization,
        ) -> Result<(), InstrumentExecutionFailure> {
            Ok(())
        }

        fn execute_action(
            &mut self,
            action: &InstrumentAction,
            attempt: u8,
        ) -> Result<InstrumentExecutionResult, InstrumentExecutionFailure> {
            Ok(InstrumentExecutionResult {
                action_id: action.action_id.clone(),
                disposition: InstrumentExecutionDisposition::Unresolved,
                attempt_count: attempt,
                started_tick: Some(action.requested_start_tick),
                completed_tick: None,
                artifact: None,
                note: "instrument output was not measurable".into(),
                uncertainty: vec!["measurement-unavailable".into()],
                negative_evidence: vec!["no-measurable-output".into()],
            })
        }

        fn emergency_stop(&mut self) -> Result<(), InstrumentExecutionFailure> {
            self.emergency_stop_called = true;
            Ok(())
        }
    }

    #[test]
    fn executes_admitted_plan_with_rechecks() {
        let request = request();
        let mut executor = DryRunInstrumentExecutor {
            interlocks: request.live_interlocks.clone(),
            emergency_stop_called: false,
        };
        let run = execute_glioma_instrument_plan(&request, &mut executor).unwrap();
        assert_eq!(run.disposition, InstrumentExecutionDisposition::Completed);
        assert_eq!(run.completed_order, vec!["acquire"]);
        assert!(!executor.emergency_stop_called);
        run.validate().unwrap();
    }

    #[test]
    fn live_interlock_failure_halts_and_requests_stop() {
        let mut request = request();
        request.live_interlocks.guard_closed = false;
        let mut executor = DryRunInstrumentExecutor {
            interlocks: request.live_interlocks.clone(),
            emergency_stop_called: false,
        };
        let run = execute_glioma_instrument_plan(&request, &mut executor).unwrap();
        assert_eq!(run.disposition, InstrumentExecutionDisposition::Blocked);
        assert_eq!(
            run.stop_reason,
            InstrumentExecutionStopReason::InterlockChanged
        );
        assert!(run.emergency_stop_requested);
        assert!(run.emergency_stop_succeeded);
        assert!(executor.emergency_stop_called);
        assert_eq!(run.skipped_order, vec!["acquire"]);
    }

    #[test]
    fn revoked_or_blocked_preflight_never_dispatches() {
        let mut request = request();
        request.plan.dispatch_permitted = false;
        let mut executor = DryRunInstrumentExecutor {
            interlocks: request.live_interlocks.clone(),
            emergency_stop_called: false,
        };
        let error = execute_glioma_instrument_plan(&request, &mut executor).unwrap_err();
        assert!(matches!(
            error,
            InstrumentExecutionError::PreflightBlocked(_)
        ));
        assert!(!executor.emergency_stop_called);
    }

    #[test]
    fn unresolved_gateway_result_is_preserved_and_halts_safely() {
        let request = request();
        let mut executor = UnresolvedExecutor {
            interlocks: request.live_interlocks.clone(),
            emergency_stop_called: false,
        };
        let run = execute_glioma_instrument_plan(&request, &mut executor).unwrap();
        assert_eq!(run.disposition, InstrumentExecutionDisposition::Unresolved);
        assert_eq!(run.unresolved_order, vec!["acquire"]);
        assert_eq!(
            run.stop_reason,
            InstrumentExecutionStopReason::UnresolvedResult
        );
        assert!(run.emergency_stop_requested);
        assert!(executor.emergency_stop_called);
        run.validate().unwrap();
    }
}

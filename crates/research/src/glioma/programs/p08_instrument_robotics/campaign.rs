//! Safety-aware multi-run instrument campaigns for preclinical glioma workflows.
//!
//! P08 execution already guards one admitted plan. This controller composes several such plans
//! into a durable run-level operation: it executes them in declared order, carries every typed
//! result forward, and fail-closes the remaining queue after a partial, unresolved, blocked, or
//! failed physical effect. The controller never opens a device connection; the existing
//! institution-owned `InstrumentExecutor` remains the only effectful seam.

use super::execution::{
    execute_glioma_instrument_plan, InstrumentExecutionDisposition, InstrumentExecutionRequest,
    InstrumentExecutionRun, InstrumentExecutor,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P08-F12";
pub const OUTPUT_SCHEMA: &str = "GliomaInstrumentCampaign1@1";
pub const MAX_RUNS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentCampaignRunRequest {
    pub run_id: String,
    pub execution: InstrumentExecutionRequest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentCampaignRequest {
    pub objective: String,
    pub runs: Vec<InstrumentCampaignRunRequest>,
    pub max_runs: usize,
    pub stop_on_negative: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentCampaignFailure {
    pub run_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentCampaignRunResult {
    pub run_id: String,
    pub execution: InstrumentExecutionRun,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstrumentCampaignDisposition {
    Completed,
    Negative,
    Partial,
    Failed,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstrumentCampaignStopReason {
    Completed,
    NegativeResult,
    SafetyHalt,
    InvalidRun,
    MaxRuns,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentCampaign {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub run_order: Vec<String>,
    pub results: Vec<InstrumentCampaignRunResult>,
    pub failures: Vec<InstrumentCampaignFailure>,
    pub completed_run_order: Vec<String>,
    pub negative_run_order: Vec<String>,
    pub partial_run_order: Vec<String>,
    pub failed_run_order: Vec<String>,
    pub blocked_run_order: Vec<String>,
    pub unresolved_run_order: Vec<String>,
    pub retry_count: u32,
    pub emergency_stop_requested: bool,
    pub emergency_stop_succeeded: bool,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub simulation_only: bool,
    pub disposition: InstrumentCampaignDisposition,
    pub stop_reason: InstrumentCampaignStopReason,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum InstrumentCampaignError {
    #[error("instrument campaign request is invalid: {0}")]
    InvalidRequest(String),
    #[error("instrument campaign run failed: {0}")]
    Run(String),
    #[error("instrument campaign output is invalid: {0}")]
    InvalidOutput(String),
    #[error("instrument campaign digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(campaign: &InstrumentCampaign) -> serde_json::Value {
    serde_json::json!({
        "feature_id": campaign.feature_id,
        "output_schema": campaign.output_schema,
        "objective": campaign.objective,
        "run_order": campaign.run_order,
        "results": campaign.results,
        "failures": campaign.failures,
        "completed_run_order": campaign.completed_run_order,
        "negative_run_order": campaign.negative_run_order,
        "partial_run_order": campaign.partial_run_order,
        "failed_run_order": campaign.failed_run_order,
        "blocked_run_order": campaign.blocked_run_order,
        "unresolved_run_order": campaign.unresolved_run_order,
        "retry_count": campaign.retry_count,
        "emergency_stop_requested": campaign.emergency_stop_requested,
        "emergency_stop_succeeded": campaign.emergency_stop_succeeded,
        "uncertainty": campaign.uncertainty,
        "negative_evidence": campaign.negative_evidence,
        "simulation_only": campaign.simulation_only,
        "disposition": campaign.disposition,
        "stop_reason": campaign.stop_reason,
    })
}

fn validate_request(request: &InstrumentCampaignRequest) -> Result<(), InstrumentCampaignError> {
    if request.objective.trim().is_empty()
        || request.runs.is_empty()
        || request.runs.len() > MAX_RUNS
        || request.max_runs == 0
        || request.max_runs > MAX_RUNS
        || request.runs.len() > request.max_runs
    {
        return Err(InstrumentCampaignError::InvalidRequest(
            "objective, non-empty bounded runs, and a sufficient max-run bound are required".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for run in &request.runs {
        if run.run_id.trim().is_empty()
            || !ids.insert(run.run_id.clone())
            || run.execution.objective.trim().is_empty()
            || run.execution.plan.instrument_id.trim().is_empty()
        {
            return Err(InstrumentCampaignError::InvalidRequest(
                "run ids, execution objectives, and instrument bindings must be unique and typed"
                    .into(),
            ));
        }
    }
    Ok(())
}

impl InstrumentCampaign {
    pub fn validate(&self) -> Result<(), InstrumentCampaignError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.run_order.is_empty()
            || self.run_order.len() > MAX_RUNS
            || self.run_order.iter().any(|id| id.trim().is_empty())
            || self.run_order.windows(2).any(|pair| pair[0] == pair[1])
            || !canonical(&self.completed_run_order)
            || !canonical(&self.negative_run_order)
            || !canonical(&self.partial_run_order)
            || !canonical(&self.failed_run_order)
            || !canonical(&self.blocked_run_order)
            || !canonical(&self.unresolved_run_order)
            || !canonical(&self.uncertainty)
            || !canonical(&self.negative_evidence)
            || self
                .uncertainty
                .iter()
                .chain(self.negative_evidence.iter())
                .any(|item| item.trim().is_empty())
            || self.emergency_stop_succeeded && !self.emergency_stop_requested
            || self.digest.as_str().len() != 64
        {
            return Err(InstrumentCampaignError::InvalidOutput(
                "identity, run ordering, explanations, emergency-stop, or digest fields are invalid"
                    .into(),
            ));
        }
        let run_ids = self.run_order.iter().cloned().collect::<BTreeSet<_>>();
        let mut partitions = Vec::new();
        for order in [
            &self.completed_run_order,
            &self.negative_run_order,
            &self.partial_run_order,
            &self.failed_run_order,
            &self.blocked_run_order,
            &self.unresolved_run_order,
        ] {
            let set = order.iter().cloned().collect::<BTreeSet<_>>();
            if set.len() != order.len() || set.iter().any(|id| !run_ids.contains(id)) {
                return Err(InstrumentCampaignError::InvalidOutput(
                    "run disposition partition contains an unknown or duplicate id".into(),
                ));
            }
            partitions.push(set);
        }
        let mut union = BTreeSet::new();
        for set in &partitions {
            if set.iter().any(|id| !union.insert(id.clone())) {
                return Err(InstrumentCampaignError::InvalidOutput(
                    "run disposition partitions overlap".into(),
                ));
            }
        }
        if union != run_ids {
            return Err(InstrumentCampaignError::InvalidOutput(
                "run disposition partitions do not cover the campaign".into(),
            ));
        }
        let mut result_ids = BTreeSet::new();
        for result in &self.results {
            if !result_ids.insert(result.run_id.clone())
                || !run_ids.contains(&result.run_id)
                || result.execution.objective.trim().is_empty()
            {
                return Err(InstrumentCampaignError::InvalidOutput(
                    "execution result identity is invalid".into(),
                ));
            }
            result
                .execution
                .validate()
                .map_err(|error| InstrumentCampaignError::InvalidOutput(error.to_string()))?;
            let partition_matches = match result.execution.disposition {
                InstrumentExecutionDisposition::Completed => self
                    .completed_run_order
                    .binary_search(&result.run_id)
                    .is_ok(),
                InstrumentExecutionDisposition::Negative => self
                    .negative_run_order
                    .binary_search(&result.run_id)
                    .is_ok(),
                InstrumentExecutionDisposition::Partial => {
                    self.partial_run_order.binary_search(&result.run_id).is_ok()
                }
                InstrumentExecutionDisposition::Failed => {
                    self.failed_run_order.binary_search(&result.run_id).is_ok()
                }
                InstrumentExecutionDisposition::Blocked => {
                    self.blocked_run_order.binary_search(&result.run_id).is_ok()
                }
                InstrumentExecutionDisposition::Unresolved => self
                    .unresolved_run_order
                    .binary_search(&result.run_id)
                    .is_ok(),
            };
            if !partition_matches {
                return Err(InstrumentCampaignError::InvalidOutput(
                    "execution result disposition does not match its campaign partition".into(),
                ));
            }
        }
        let mut failure_ids = BTreeSet::new();
        for failure in &self.failures {
            if !failure_ids.insert(failure.run_id.clone())
                || !self.failed_run_order.binary_search(&failure.run_id).is_ok()
                || failure.reason.trim().is_empty()
            {
                return Err(InstrumentCampaignError::InvalidOutput(
                    "campaign failure identity or reason is invalid".into(),
                ));
            }
        }
        let blocked_ids = self
            .blocked_run_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let unexecuted_blocked = blocked_ids
            .difference(&result_ids)
            .cloned()
            .collect::<BTreeSet<_>>();
        if result_ids
            .intersection(&unexecuted_blocked)
            .next()
            .is_some()
            || result_ids.intersection(&failure_ids).next().is_some()
        {
            return Err(InstrumentCampaignError::InvalidOutput(
                "executed results overlap blocked or failed-request partitions".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| InstrumentCampaignError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(InstrumentCampaignError::InvalidOutput(
                "instrument campaign digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Execute admitted instrument plans in order and stop the queue after any unsafe or
/// scientifically unresolved physical outcome.
pub fn execute_glioma_instrument_campaign<E: InstrumentExecutor>(
    request: &InstrumentCampaignRequest,
    executor: &mut E,
) -> Result<InstrumentCampaign, InstrumentCampaignError> {
    validate_request(request)?;
    let run_order = request
        .runs
        .iter()
        .map(|run| run.run_id.clone())
        .collect::<Vec<_>>();
    let mut results = Vec::new();
    let mut failures = Vec::new();
    let mut completed = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut partial = BTreeSet::new();
    let mut failed = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative_evidence = BTreeSet::new();
    let mut retry_count = 0_u32;
    let mut emergency_stop_requested = false;
    let mut emergency_stop_succeeded = false;
    let mut stop_reason = InstrumentCampaignStopReason::Completed;

    for (index, run) in request.runs.iter().enumerate() {
        let execution = match execute_glioma_instrument_plan(&run.execution, executor) {
            Ok(execution) => execution,
            Err(error) => {
                failed.insert(run.run_id.clone());
                failures.push(InstrumentCampaignFailure {
                    run_id: run.run_id.clone(),
                    reason: error.to_string(),
                });
                uncertainty.insert(format!("run-invalid-or-refused:{}", run.run_id));
                stop_reason = InstrumentCampaignStopReason::InvalidRun;
                for remaining in request.runs.iter().skip(index + 1) {
                    blocked.insert(remaining.run_id.clone());
                }
                break;
            }
        };
        retry_count = retry_count.saturating_add(execution.retry_count);
        let stop_was_requested = emergency_stop_requested;
        emergency_stop_requested |= execution.emergency_stop_requested;
        if execution.emergency_stop_requested {
            emergency_stop_succeeded = if stop_was_requested {
                emergency_stop_succeeded && execution.emergency_stop_succeeded
            } else {
                execution.emergency_stop_succeeded
            };
        }
        uncertainty.extend(execution.uncertainty.iter().cloned());
        negative_evidence.extend(execution.negative_evidence.iter().cloned());
        results.push(InstrumentCampaignRunResult {
            run_id: run.run_id.clone(),
            execution: execution.clone(),
        });
        match execution.disposition {
            InstrumentExecutionDisposition::Completed => {
                completed.insert(run.run_id.clone());
            }
            InstrumentExecutionDisposition::Negative => {
                negative.insert(run.run_id.clone());
                if request.stop_on_negative {
                    stop_reason = InstrumentCampaignStopReason::NegativeResult;
                    for remaining in request.runs.iter().skip(index + 1) {
                        blocked.insert(remaining.run_id.clone());
                    }
                    break;
                }
            }
            InstrumentExecutionDisposition::Partial => {
                partial.insert(run.run_id.clone());
                stop_reason = InstrumentCampaignStopReason::SafetyHalt;
                for remaining in request.runs.iter().skip(index + 1) {
                    blocked.insert(remaining.run_id.clone());
                }
                break;
            }
            InstrumentExecutionDisposition::Failed => {
                failed.insert(run.run_id.clone());
                stop_reason = InstrumentCampaignStopReason::SafetyHalt;
                for remaining in request.runs.iter().skip(index + 1) {
                    blocked.insert(remaining.run_id.clone());
                }
                break;
            }
            InstrumentExecutionDisposition::Blocked => {
                blocked.insert(run.run_id.clone());
                stop_reason = InstrumentCampaignStopReason::SafetyHalt;
                for remaining in request.runs.iter().skip(index + 1) {
                    blocked.insert(remaining.run_id.clone());
                }
                break;
            }
            InstrumentExecutionDisposition::Unresolved => {
                unresolved.insert(run.run_id.clone());
                stop_reason = InstrumentCampaignStopReason::SafetyHalt;
                for remaining in request.runs.iter().skip(index + 1) {
                    blocked.insert(remaining.run_id.clone());
                }
                break;
            }
        }
    }
    if results.len() == request.runs.len() && stop_reason == InstrumentCampaignStopReason::Completed
    {
        stop_reason = InstrumentCampaignStopReason::Completed;
    } else if results.len() == request.max_runs
        && stop_reason == InstrumentCampaignStopReason::Completed
    {
        stop_reason = InstrumentCampaignStopReason::MaxRuns;
    }
    let disposition = if !failed.is_empty() {
        InstrumentCampaignDisposition::Failed
    } else if !partial.is_empty() {
        InstrumentCampaignDisposition::Partial
    } else if !unresolved.is_empty() {
        InstrumentCampaignDisposition::Unresolved
    } else if !blocked.is_empty() {
        InstrumentCampaignDisposition::Blocked
    } else if !negative.is_empty() {
        InstrumentCampaignDisposition::Negative
    } else {
        InstrumentCampaignDisposition::Completed
    };
    let mut output = InstrumentCampaign {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        run_order,
        results,
        failures,
        completed_run_order: completed.into_iter().collect(),
        negative_run_order: negative.into_iter().collect(),
        partial_run_order: partial.into_iter().collect(),
        failed_run_order: failed.into_iter().collect(),
        blocked_run_order: blocked.into_iter().collect(),
        unresolved_run_order: unresolved.into_iter().collect(),
        retry_count,
        emergency_stop_requested,
        emergency_stop_succeeded,
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative_evidence.into_iter().collect(),
        simulation_only: executor.simulation_only(),
        disposition,
        stop_reason,
        digest: ContentHash::of_bytes(b"unsealed-glioma-instrument-campaign"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| InstrumentCampaignError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p08_instrument_robotics::calibration::{
        analyze_instrument_calibration, CalibrationRequest, CalibrationRun,
    };
    use crate::glioma::programs::p08_instrument_robotics::execution::DryRunInstrumentExecutor;
    use crate::glioma::programs::p08_instrument_robotics::preflight::{
        preflight_glioma_instrument, InstrumentAction, InstrumentAuthorization,
        InstrumentInterlockSnapshot, InstrumentOperation, InstrumentParameter,
        InstrumentPreflightRequest,
    };
    use crate::glioma_engine::GliomaModelSystem;

    fn hash() -> ContentHash {
        ContentHash::of_bytes(b"approval").clone()
    }

    fn execution_request() -> InstrumentExecutionRequest {
        let artifact = hash();
        let calibration = analyze_instrument_calibration(
            &CalibrationRequest {
                objective: "qualify imaging controls".into(),
                instrument_id: "imager-1".into(),
                model_system: GliomaModelSystem::Organoid,
                metric_name: "control_intensity".into(),
                minimum_runs: 3,
                reference_run_count: 2,
                max_reference_mad_milli: 5,
                max_drift_milli: 20,
                max_slope_milli_per_tick: 10,
            },
            &[
                CalibrationRun {
                    run_id: "r1".into(),
                    sequence_index: 1,
                    batch_id: "b1".into(),
                    instrument_id: "imager-1".into(),
                    metric_name: "control_intensity".into(),
                    model_system: GliomaModelSystem::Organoid,
                    observed_milli: 500,
                    expected_milli: 500,
                    artifact: crate::glioma_engine::LocalArtifactRef {
                        artifact_id: "a1".into(),
                        content_hash: artifact.clone(),
                        content_type: "application/json".into(),
                        local_only: true,
                        contains_human_data: false,
                        contains_direct_identifiers: false,
                    },
                },
                CalibrationRun {
                    run_id: "r2".into(),
                    sequence_index: 2,
                    batch_id: "b2".into(),
                    instrument_id: "imager-1".into(),
                    metric_name: "control_intensity".into(),
                    model_system: GliomaModelSystem::Organoid,
                    observed_milli: 501,
                    expected_milli: 500,
                    artifact: crate::glioma_engine::LocalArtifactRef {
                        artifact_id: "a2".into(),
                        content_hash: artifact.clone(),
                        content_type: "application/json".into(),
                        local_only: true,
                        contains_human_data: false,
                        contains_direct_identifiers: false,
                    },
                },
                CalibrationRun {
                    run_id: "r3".into(),
                    sequence_index: 3,
                    batch_id: "b3".into(),
                    instrument_id: "imager-1".into(),
                    metric_name: "control_intensity".into(),
                    model_system: GliomaModelSystem::Organoid,
                    observed_milli: 502,
                    expected_milli: 500,
                    artifact: crate::glioma_engine::LocalArtifactRef {
                        artifact_id: "a3".into(),
                        content_hash: artifact.clone(),
                        content_type: "application/json".into(),
                        local_only: true,
                        contains_human_data: false,
                        contains_direct_identifiers: false,
                    },
                },
            ],
        )
        .unwrap();
        let actions = vec![InstrumentAction {
            action_id: "acquire".into(),
            instrument_id: "imager-1".into(),
            operation: InstrumentOperation::AcquireImage,
            model_system: GliomaModelSystem::Organoid,
            requested_start_tick: 1,
            duration_ticks: 2,
            risk_milli: 100,
            requires_operator: false,
            output_schema: "Image1@1".into(),
            parameters: vec![InstrumentParameter {
                name: "exposure".into(),
                value_milli: 100,
                unit: "millisecond_milli".into(),
                minimum_milli: Some(1),
                maximum_milli: Some(10_000),
            }],
        }];
        let preflight_request = InstrumentPreflightRequest {
            objective: "run organoid image".into(),
            instrument_id: "imager-1".into(),
            model_system: GliomaModelSystem::Organoid,
            actions: actions.clone(),
            calibration,
            interlocks: InstrumentInterlockSnapshot {
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
            },
            authorization: InstrumentAuthorization {
                authorization_id: "approval-1".into(),
                operator_id: "operator-1".into(),
                instrument_scope: "imager-1".into(),
                approval_digest: artifact,
                issued_tick: 0,
                expires_tick: 100,
                revoked: false,
            },
            current_tick: 1,
            maximum_total_risk_milli: 500,
            maximum_duration_ticks: 20,
            minimum_waste_capacity_milli: 100,
        };
        let plan = preflight_glioma_instrument(&preflight_request).unwrap();
        InstrumentExecutionRequest {
            objective: "run organoid image".into(),
            plan,
            actions,
            authorization: preflight_request.authorization,
            live_interlocks: preflight_request.interlocks,
            current_tick: 1,
            minimum_waste_capacity_milli: 100,
            max_retries: 1,
            require_artifacts: true,
        }
    }

    #[test]
    fn campaign_executes_admitted_runs_and_replays() {
        let execution = execution_request();
        let request = InstrumentCampaignRequest {
            objective: "run a two-stage organoid imaging campaign".into(),
            runs: vec![
                InstrumentCampaignRunRequest {
                    run_id: "stage-a".into(),
                    execution: execution.clone(),
                },
                InstrumentCampaignRunRequest {
                    run_id: "stage-b".into(),
                    execution,
                },
            ],
            max_runs: 2,
            stop_on_negative: true,
        };
        let mut first_executor = DryRunInstrumentExecutor {
            interlocks: request.runs[0].execution.live_interlocks.clone(),
            emergency_stop_called: false,
        };
        let mut second_executor = DryRunInstrumentExecutor {
            interlocks: request.runs[0].execution.live_interlocks.clone(),
            emergency_stop_called: false,
        };
        let first = execute_glioma_instrument_campaign(&request, &mut first_executor).unwrap();
        let second = execute_glioma_instrument_campaign(&request, &mut second_executor).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.disposition, InstrumentCampaignDisposition::Completed);
        assert_eq!(first.completed_run_order, vec!["stage-a", "stage-b"]);
        assert_eq!(first.results.len(), 2);
        first.validate().unwrap();
    }

    #[test]
    fn preflight_refusal_blocks_remaining_runs_without_dispatch() {
        let mut execution = execution_request();
        execution.plan.dispatch_permitted = false;
        let request = InstrumentCampaignRequest {
            objective: "blocked organoid campaign".into(),
            runs: vec![
                InstrumentCampaignRunRequest {
                    run_id: "blocked".into(),
                    execution,
                },
                InstrumentCampaignRunRequest {
                    run_id: "never".into(),
                    execution: execution_request(),
                },
            ],
            max_runs: 2,
            stop_on_negative: true,
        };
        let mut executor = DryRunInstrumentExecutor {
            interlocks: request.runs[0].execution.live_interlocks.clone(),
            emergency_stop_called: false,
        };
        let output = execute_glioma_instrument_campaign(&request, &mut executor).unwrap();
        assert_eq!(output.disposition, InstrumentCampaignDisposition::Failed);
        assert_eq!(output.failed_run_order, vec!["blocked"]);
        assert_eq!(output.blocked_run_order, vec!["never"]);
        assert!(output.results.is_empty());
        output.validate().unwrap();
    }
}

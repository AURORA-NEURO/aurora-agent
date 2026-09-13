//! Preflight-barrier and campaign orchestration for local glioma instruments.
//!
//! P08 already validates and executes one instrument plan and can run a sequence of plans. This
//! feature adds the missing program-level barrier: inspect every planned run first, refuse the
//! whole campaign when any run is not dispatch-permitted, then execute the admitted queue through
//! the caller-owned gateway. It prevents partial physical dispatch caused by discovering a later
//! preflight failure and keeps the safety handoff machine-readable.

use super::campaign::{
    execute_glioma_instrument_campaign, InstrumentCampaign, InstrumentCampaignDisposition,
    InstrumentCampaignError, InstrumentCampaignRequest,
};
use super::execution::{DryRunInstrumentExecutor, InstrumentExecutor};
use super::preflight::InstrumentPreflightDisposition;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P08-F24";
pub const OUTPUT_SCHEMA: &str = "GliomaInstrumentOperatingCycle1@1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstrumentExecutionMode {
    LocalSimulation,
    GovernedLocal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentPreflightSummary {
    pub run_id: String,
    pub instrument_id: String,
    pub disposition: InstrumentPreflightDisposition,
    pub dispatch_permitted: bool,
    pub action_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentOperatingCycleRequest {
    pub campaign: InstrumentCampaignRequest,
    pub require_all_admitted: bool,
    pub execution_mode: InstrumentExecutionMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstrumentOperatingCycleDisposition {
    Ready,
    Executed,
    Negative,
    Partial,
    Blocked,
    Failed,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentOperatingCycle {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub phase_order: Vec<String>,
    pub preflight: Vec<InstrumentPreflightSummary>,
    pub campaign: Option<InstrumentCampaign>,
    pub simulation_only: bool,
    pub execution_mode: InstrumentExecutionMode,
    pub next_operator_action: String,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: InstrumentOperatingCycleDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum InstrumentOperatingCycleError {
    #[error("instrument operating-cycle request is invalid: {0}")]
    InvalidRequest(String),
    #[error("instrument operating-cycle campaign failed: {0}")]
    Campaign(#[from] InstrumentCampaignError),
    #[error("instrument operating-cycle output is invalid: {0}")]
    InvalidOutput(String),
    #[error("instrument operating-cycle digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &InstrumentOperatingCycle) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "phase_order": output.phase_order,
        "preflight": output.preflight,
        "campaign": output.campaign,
        "simulation_only": output.simulation_only,
        "execution_mode": output.execution_mode,
        "next_operator_action": output.next_operator_action,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn summaries(
    request: &InstrumentCampaignRequest,
) -> Result<Vec<InstrumentPreflightSummary>, InstrumentOperatingCycleError> {
    request
        .runs
        .iter()
        .map(|run| {
            run.execution.plan.validate().map_err(|error| {
                InstrumentOperatingCycleError::InvalidRequest(error.to_string())
            })?;
            let plan = &run.execution.plan;
            let reasons = plan
                .decisions
                .iter()
                .flat_map(|decision| decision.reasons.clone())
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect();
            Ok(InstrumentPreflightSummary {
                run_id: run.run_id.clone(),
                instrument_id: plan.instrument_id.clone(),
                disposition: plan.disposition,
                dispatch_permitted: plan.dispatch_permitted,
                action_order: plan.action_order.clone(),
                blocked_order: plan.blocked_order.clone(),
                unresolved_order: plan.unresolved_order.clone(),
                reasons,
            })
        })
        .collect()
}

fn campaign_disposition(
    campaign: InstrumentCampaignDisposition,
) -> InstrumentOperatingCycleDisposition {
    match campaign {
        InstrumentCampaignDisposition::Completed => InstrumentOperatingCycleDisposition::Executed,
        InstrumentCampaignDisposition::Negative => InstrumentOperatingCycleDisposition::Negative,
        InstrumentCampaignDisposition::Partial => InstrumentOperatingCycleDisposition::Partial,
        InstrumentCampaignDisposition::Failed => InstrumentOperatingCycleDisposition::Failed,
        InstrumentCampaignDisposition::Blocked => InstrumentOperatingCycleDisposition::Blocked,
        InstrumentCampaignDisposition::Unresolved => {
            InstrumentOperatingCycleDisposition::Unresolved
        }
    }
}

impl InstrumentOperatingCycle {
    pub fn validate(&self) -> Result<(), InstrumentOperatingCycleError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.phase_order
                != [
                    "preflight_barrier".to_string(),
                    "instrument_campaign".to_string(),
                    "operator_handoff".to_string(),
                ]
            || self.preflight.is_empty()
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.simulation_only
                != matches!(
                    self.execution_mode,
                    InstrumentExecutionMode::LocalSimulation
                )
            || self.next_operator_action.trim().is_empty()
        {
            return Err(InstrumentOperatingCycleError::InvalidOutput(
                "identity, phase order, preflight barrier, execution mode, or ordering is invalid"
                    .into(),
            ));
        }
        for summary in &self.preflight {
            if summary.run_id.trim().is_empty()
                || summary.instrument_id.trim().is_empty()
                || summary.action_order.is_empty()
                || !canonical(&summary.blocked_order)
                || !canonical(&summary.unresolved_order)
                || !canonical(&summary.reasons)
            {
                return Err(InstrumentOperatingCycleError::InvalidOutput(
                    "preflight summaries are not canonical".into(),
                ));
            }
        }
        if let Some(campaign) = &self.campaign {
            campaign
                .validate()
                .map_err(|error| InstrumentOperatingCycleError::InvalidOutput(error.to_string()))?;
            if campaign.objective != self.objective {
                return Err(InstrumentOperatingCycleError::InvalidOutput(
                    "campaign objective is not bound to the operating cycle".into(),
                ));
            }
        } else if !matches!(
            self.disposition,
            InstrumentOperatingCycleDisposition::Blocked
        ) {
            return Err(InstrumentOperatingCycleError::InvalidOutput(
                "only a blocked preflight barrier may omit campaign execution".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| InstrumentOperatingCycleError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(InstrumentOperatingCycleError::InvalidOutput(
                "instrument operating-cycle digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Validate every preflight before executing any instrument run, then execute the admitted queue.
pub fn execute_glioma_instrument_operating_cycle<E: InstrumentExecutor>(
    request: &InstrumentOperatingCycleRequest,
    executor: &mut E,
) -> Result<InstrumentOperatingCycle, InstrumentOperatingCycleError> {
    if request.campaign.objective.trim().is_empty() || request.campaign.runs.is_empty() {
        return Err(InstrumentOperatingCycleError::InvalidRequest(
            "objective and instrument runs are required".into(),
        ));
    }
    let preflight = summaries(&request.campaign)?;
    let barrier_blocked = request.require_all_admitted
        && preflight.iter().any(|summary| {
            summary.disposition != InstrumentPreflightDisposition::Admitted
                || !summary.dispatch_permitted
        });
    let (campaign, disposition, next_operator_action, negative_evidence, uncertainty) =
        if barrier_blocked {
            (
                None,
                InstrumentOperatingCycleDisposition::Blocked,
                "resolve every blocked or unresolved preflight before authorizing dispatch".into(),
                preflight
                    .iter()
                    .flat_map(|summary| {
                        summary
                            .blocked_order
                            .iter()
                            .map(|id| format!("preflight-blocked:{id}"))
                    })
                    .collect::<Vec<_>>(),
                preflight
                    .iter()
                    .flat_map(|summary| {
                        summary
                            .unresolved_order
                            .iter()
                            .map(|id| format!("preflight-unresolved:{id}"))
                    })
                    .collect::<Vec<_>>(),
            )
        } else {
            let campaign = execute_glioma_instrument_campaign(&request.campaign, executor)?;
            let disposition = campaign_disposition(campaign.disposition);
            let next = if campaign.emergency_stop_requested {
                "review emergency-stop evidence before any further dispatch".into()
            } else if matches!(disposition, InstrumentOperatingCycleDisposition::Executed) {
                "adjudicate returned local artifacts before promoting them into research evidence"
                    .into()
            } else {
                "review the campaign stop reason and resolve the local instrument hold".into()
            };
            (
                Some(campaign.clone()),
                disposition,
                next,
                campaign.negative_evidence.clone(),
                campaign.uncertainty.clone(),
            )
        };
    let mut negative_evidence = negative_evidence;
    negative_evidence.sort();
    negative_evidence.dedup();
    let mut uncertainty = uncertainty;
    uncertainty.sort();
    uncertainty.dedup();
    let mut output = InstrumentOperatingCycle {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.campaign.objective.clone(),
        phase_order: vec![
            "preflight_barrier".into(),
            "instrument_campaign".into(),
            "operator_handoff".into(),
        ],
        preflight,
        campaign,
        simulation_only: matches!(
            request.execution_mode,
            InstrumentExecutionMode::LocalSimulation
        ),
        execution_mode: request.execution_mode,
        next_operator_action,
        negative_evidence,
        uncertainty,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-instrument-operating-cycle"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| InstrumentOperatingCycleError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

pub fn dry_run_instrument_executor_from_request(
    request: &InstrumentOperatingCycleRequest,
) -> Result<DryRunInstrumentExecutor, InstrumentOperatingCycleError> {
    let snapshot = request
        .campaign
        .runs
        .first()
        .ok_or_else(|| {
            InstrumentOperatingCycleError::InvalidRequest("instrument runs are required".into())
        })?
        .execution
        .live_interlocks
        .clone();
    Ok(DryRunInstrumentExecutor {
        interlocks: snapshot,
        emergency_stop_called: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p08_instrument_robotics::campaign::InstrumentCampaignRunRequest;
    use crate::glioma::programs::p08_instrument_robotics::execution::InstrumentExecutionRequest;
    use crate::glioma::programs::p08_instrument_robotics::preflight::{
        InstrumentActionDecision, InstrumentActionDisposition, InstrumentAuthorization,
        InstrumentInterlockSnapshot, InstrumentPreflightPlan,
    };
    use crate::glioma_engine::GliomaModelSystem;

    fn blocked_request() -> InstrumentOperatingCycleRequest {
        let plan_without_digest = serde_json::json!({
            "feature_id": "GAF-GLIOMA-P08-F10",
            "output_schema": "GliomaInstrumentPreflight1@1",
            "objective": "blocked instrument run",
            "instrument_id": "imager-1",
            "model_system": "organoid",
            "authorization_id": "approval-1",
            "action_order": ["acquire"],
            "admitted_order": [],
            "blocked_order": ["acquire"],
            "unresolved_order": [],
            "decisions": [{
                "action_id": "acquire",
                "disposition": "blocked",
                "scheduled_start_tick": null,
                "scheduled_end_tick": null,
                "reasons": ["interlock-closed"]
            }],
            "total_risk_milli": 0,
            "total_duration_ticks": 0,
            "required_interlocks": [],
            "compensation_order": [],
            "negative_evidence": [],
            "uncertainty": [],
            "dispatch_permitted": false,
            "disposition": "blocked"
        });
        let plan_digest = ContentHash::of_value(&plan_without_digest).unwrap();
        let plan = InstrumentPreflightPlan {
            feature_id: "GAF-GLIOMA-P08-F10".into(),
            output_schema: "GliomaInstrumentPreflight1@1".into(),
            objective: "blocked instrument run".into(),
            instrument_id: "imager-1".into(),
            model_system: GliomaModelSystem::Organoid,
            authorization_id: "approval-1".into(),
            action_order: vec!["acquire".into()],
            admitted_order: Vec::new(),
            blocked_order: vec!["acquire".into()],
            unresolved_order: Vec::new(),
            decisions: vec![InstrumentActionDecision {
                action_id: "acquire".into(),
                disposition: InstrumentActionDisposition::Blocked,
                scheduled_start_tick: None,
                scheduled_end_tick: None,
                reasons: vec!["interlock-closed".into()],
            }],
            total_risk_milli: 0,
            total_duration_ticks: 0,
            required_interlocks: Vec::new(),
            compensation_order: Vec::new(),
            negative_evidence: Vec::new(),
            uncertainty: Vec::new(),
            dispatch_permitted: false,
            disposition: InstrumentPreflightDisposition::Blocked,
            digest: plan_digest,
        };
        let run = InstrumentCampaignRunRequest {
            run_id: "blocked-run".into(),
            execution: InstrumentExecutionRequest {
                objective: "blocked instrument run".into(),
                plan,
                actions: Vec::new(),
                authorization: InstrumentAuthorization {
                    authorization_id: "approval-1".into(),
                    operator_id: "operator-1".into(),
                    instrument_scope: "imager-1".into(),
                    approval_digest: ContentHash::of_bytes(b"approval"),
                    issued_tick: 0,
                    expires_tick: 10,
                    revoked: false,
                },
                live_interlocks: InstrumentInterlockSnapshot {
                    observed_tick: 1,
                    emergency_stop_clear: true,
                    guard_closed: true,
                    deck_clear: true,
                    consumables_available: true,
                    waste_capacity_milli: 1_000,
                    temperature_milli: None,
                    minimum_temperature_milli: None,
                    maximum_temperature_milli: None,
                    calibration_valid_until_tick: 10,
                    calibration_sequence_index: 1,
                },
                current_tick: 1,
                minimum_waste_capacity_milli: 1,
                max_retries: 0,
                require_artifacts: false,
            },
        };
        InstrumentOperatingCycleRequest {
            campaign: InstrumentCampaignRequest {
                objective: "blocked instrument run".into(),
                runs: vec![run],
                max_runs: 1,
                stop_on_negative: true,
            },
            require_all_admitted: true,
            execution_mode: InstrumentExecutionMode::LocalSimulation,
        }
    }

    #[test]
    fn preflight_barrier_holds_before_dispatch_and_replays() {
        let request = blocked_request();
        let mut first = dry_run_instrument_executor_from_request(&request).unwrap();
        let mut second = dry_run_instrument_executor_from_request(&request).unwrap();
        let output = execute_glioma_instrument_operating_cycle(&request, &mut first).unwrap();
        let replay = execute_glioma_instrument_operating_cycle(&request, &mut second).unwrap();
        assert_eq!(output, replay);
        assert_eq!(
            output.disposition,
            InstrumentOperatingCycleDisposition::Blocked
        );
        assert!(output.campaign.is_none());
        assert_eq!(output.negative_evidence, vec!["preflight-blocked:acquire"]);
        output.validate().unwrap();
        assert!(!first.emergency_stop_called);
    }
}

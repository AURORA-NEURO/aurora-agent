//! Governed instrument-to-science loop for preclinical glioma assays.
//!
//! This feature closes the gap between a safe instrument run and a scientifically usable result.
//! It executes the existing preflight-barrier operating cycle through the caller-owned gateway,
//! then adjudicates each returned local assay summary against QC, effect, uncertainty, replicate,
//! and negative-control gates. Hardware completion is never promoted automatically: absent or
//! low-quality summaries remain unresolved/negative and become explicit next research actions.

use super::assay_adjudication::{
    adjudicate_glioma_assay_evidence, AssayEvidenceDisposition, AssayEvidenceObservation,
    AssayEvidenceRequest, InstrumentAssayEvidenceAssessment,
};
use super::execution::InstrumentExecutor;
use super::operating_cycle::{
    execute_glioma_instrument_operating_cycle, InstrumentOperatingCycle,
    InstrumentOperatingCycleDisposition, InstrumentOperatingCycleError,
    InstrumentOperatingCycleRequest,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P08-F16";
pub const OUTPUT_SCHEMA: &str = "GliomaInstrumentScienceLoop1@1";
pub const MAX_OBSERVATIONS: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentScienceLoopRequest {
    pub operating_cycle: InstrumentOperatingCycleRequest,
    pub evidence: AssayEvidenceRequest,
    pub stop_on_first_qualified_run: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstrumentScienceLoopDisposition {
    Qualified,
    Partial,
    Negative,
    Blocked,
    Failed,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentScienceLoop {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub phase_order: Vec<String>,
    pub operating_cycle: InstrumentOperatingCycle,
    pub assessment_run_order: Vec<String>,
    pub assessments: Vec<InstrumentAssayEvidenceAssessment>,
    pub qualified_action_order: Vec<String>,
    pub negative_action_order: Vec<String>,
    pub unresolved_action_order: Vec<String>,
    pub next_research_action_order: Vec<String>,
    pub simulation_only: bool,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: InstrumentScienceLoopDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum InstrumentScienceLoopError {
    #[error("instrument science-loop request is invalid: {0}")]
    InvalidRequest(String),
    #[error("instrument science-loop operating cycle failed: {0}")]
    OperatingCycle(#[from] InstrumentOperatingCycleError),
    #[error("instrument science-loop assay evidence failed: {0}")]
    AssayEvidence(String),
    #[error("instrument science-loop output is invalid: {0}")]
    InvalidOutput(String),
    #[error("instrument science-loop digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &InstrumentScienceLoop) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "phase_order": output.phase_order,
        "operating_cycle": output.operating_cycle,
        "assessment_run_order": output.assessment_run_order,
        "assessments": output.assessments,
        "qualified_action_order": output.qualified_action_order,
        "negative_action_order": output.negative_action_order,
        "unresolved_action_order": output.unresolved_action_order,
        "next_research_action_order": output.next_research_action_order,
        "simulation_only": output.simulation_only,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn validate_request(
    request: &InstrumentScienceLoopRequest,
    observations: &[AssayEvidenceObservation],
) -> Result<(), InstrumentScienceLoopError> {
    if request.operating_cycle.campaign.objective.trim().is_empty()
        || request.evidence.objective.trim().is_empty()
        || request.evidence.instrument_id.trim().is_empty()
        || observations.len() > MAX_OBSERVATIONS
        || request.operating_cycle.campaign.runs.is_empty()
        || request
            .operating_cycle
            .campaign
            .runs
            .iter()
            .any(|run| run.execution.plan.instrument_id != request.evidence.instrument_id)
    {
        return Err(InstrumentScienceLoopError::InvalidRequest(
            "non-empty objectives, runs, a bounded observation set, and one instrument binding are required".into(),
        ));
    }
    Ok(())
}

fn prefixed(run_id: &str, action_id: &str) -> String {
    format!("{run_id}:{action_id}")
}

impl InstrumentScienceLoop {
    pub fn validate(&self) -> Result<(), InstrumentScienceLoopError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.phase_order
                != [
                    "preflight_and_execution".to_string(),
                    "assay_evidence_adjudication".to_string(),
                    "research_action_handoff".to_string(),
                ]
            || !canonical(&self.assessment_run_order)
            || self.assessment_run_order.len() != self.assessments.len()
            || !canonical(&self.qualified_action_order)
            || !canonical(&self.negative_action_order)
            || !canonical(&self.unresolved_action_order)
            || !canonical(&self.next_research_action_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
        {
            return Err(InstrumentScienceLoopError::InvalidOutput(
                "identity, phase, assessment, partition, or ordering invariants are invalid".into(),
            ));
        }
        self.operating_cycle
            .validate()
            .map_err(|error| InstrumentScienceLoopError::InvalidOutput(error.to_string()))?;
        for assessment in &self.assessments {
            assessment
                .validate()
                .map_err(|error| InstrumentScienceLoopError::InvalidOutput(error.to_string()))?;
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| InstrumentScienceLoopError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(InstrumentScienceLoopError::InvalidOutput(
                "science-loop digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Execute the governed instrument cycle, then turn local assay summaries into research-grade
/// evidence or explicit next actions. The gateway remains the only physical-effect seam.
pub fn execute_glioma_instrument_science_loop<E: InstrumentExecutor>(
    request: &InstrumentScienceLoopRequest,
    observations: &[AssayEvidenceObservation],
    executor: &mut E,
) -> Result<InstrumentScienceLoop, InstrumentScienceLoopError> {
    validate_request(request, observations)?;
    let operating_cycle =
        execute_glioma_instrument_operating_cycle(&request.operating_cycle, executor)?;
    let mut assessments = Vec::new();
    let mut assessment_run_order = Vec::new();
    let mut qualified = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut next_actions = BTreeSet::new();
    let mut negative_evidence = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let observations_by_action = observations
        .iter()
        .map(|observation| (observation.action_id.clone(), observation))
        .collect::<BTreeMap<_, _>>();

    if let Some(campaign) = &operating_cycle.campaign {
        for run in &campaign.results {
            let run_observations = run
                .execution
                .action_order
                .iter()
                .filter_map(|action_id| observations_by_action.get(action_id).copied())
                .cloned()
                .collect::<Vec<_>>();
            let assessment = adjudicate_glioma_assay_evidence(
                &request.evidence,
                &run.execution,
                &run_observations,
            )
            .map_err(|error| InstrumentScienceLoopError::AssayEvidence(error.to_string()))?;
            assessment_run_order.push(run.run_id.clone());
            for action_id in &assessment.qualified_order {
                qualified.insert(prefixed(&run.run_id, action_id));
            }
            for action_id in &assessment.negative_order {
                negative.insert(prefixed(&run.run_id, action_id));
            }
            for action_id in &assessment.unresolved_order {
                unresolved.insert(prefixed(&run.run_id, action_id));
            }
            for action in &assessment.next_action_order {
                next_actions.insert(prefixed(&run.run_id, action));
            }
            negative_evidence.extend(assessment.negative_evidence.iter().cloned());
            uncertainty.extend(assessment.uncertainty.iter().cloned());
            let stop = request.stop_on_first_qualified_run
                && assessment.disposition == AssayEvidenceDisposition::Qualified;
            assessments.push(assessment);
            if stop {
                break;
            }
        }
    } else {
        uncertainty
            .insert("instrument-operating-cycle-did-not-produce-an-admitted-campaign".into());
        next_actions.insert("resolve-instrument-preflight-before-assay-adjudication".into());
    }
    assessment_run_order.sort();
    let disposition = if matches!(
        operating_cycle.disposition,
        InstrumentOperatingCycleDisposition::Blocked
    ) {
        InstrumentScienceLoopDisposition::Blocked
    } else if matches!(
        operating_cycle.disposition,
        InstrumentOperatingCycleDisposition::Failed
    ) {
        InstrumentScienceLoopDisposition::Failed
    } else if !qualified.is_empty() && negative.is_empty() && unresolved.is_empty() {
        InstrumentScienceLoopDisposition::Qualified
    } else if !qualified.is_empty() || !negative.is_empty() {
        InstrumentScienceLoopDisposition::Partial
    } else if !assessments.is_empty() {
        InstrumentScienceLoopDisposition::Unresolved
    } else {
        InstrumentScienceLoopDisposition::Blocked
    };
    if matches!(disposition, InstrumentScienceLoopDisposition::Qualified) {
        next_actions.insert("promote-qualified-assay-evidence-into-analysis".into());
    } else {
        next_actions.insert("retain-assay-hold-and-route-the-highest-information-follow-up".into());
    }
    let mut negative_evidence = negative_evidence.into_iter().collect::<Vec<_>>();
    negative_evidence.extend(operating_cycle.negative_evidence.iter().cloned());
    negative_evidence.sort();
    negative_evidence.dedup();
    let mut uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    uncertainty.extend(operating_cycle.uncertainty.iter().cloned());
    uncertainty.sort();
    uncertainty.dedup();
    let mut output = InstrumentScienceLoop {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.operating_cycle.campaign.objective.clone(),
        phase_order: vec![
            "preflight_and_execution".into(),
            "assay_evidence_adjudication".into(),
            "research_action_handoff".into(),
        ],
        operating_cycle,
        assessment_run_order,
        assessments,
        qualified_action_order: qualified.into_iter().collect(),
        negative_action_order: negative.into_iter().collect(),
        unresolved_action_order: unresolved.into_iter().collect(),
        next_research_action_order: next_actions.into_iter().collect(),
        simulation_only: request.operating_cycle.execution_mode
            == super::operating_cycle::InstrumentExecutionMode::LocalSimulation,
        negative_evidence,
        uncertainty,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-instrument-science-loop"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| InstrumentScienceLoopError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p08_instrument_robotics::operating_cycle::dry_run_instrument_executor_from_request;

    fn blocked_request() -> InstrumentScienceLoopRequest {
        let plan_without_digest = serde_json::json!({
            "feature_id": "GAF-GLIOMA-P08-F10",
            "output_schema": "GliomaInstrumentPreflight1@1",
            "objective": "blocked science loop",
            "instrument_id": "imager-1",
            "model_system": "organoid",
            "authorization_id": "approval-1",
            "action_order": ["acquire"],
            "admitted_order": [],
            "blocked_order": ["acquire"],
            "unresolved_order": [],
            "decisions": [{"action_id":"acquire","disposition":"blocked","scheduled_start_tick":null,"scheduled_end_tick":null,"reasons":["interlock-closed"]}],
            "total_risk_milli": 0,
            "total_duration_ticks": 0,
            "required_interlocks": [],
            "compensation_order": [],
            "negative_evidence": [],
            "uncertainty": [],
            "dispatch_permitted": false,
            "disposition": "blocked"
        });
        let digest = ContentHash::of_value(&plan_without_digest).unwrap();
        let plan = serde_json::json!({
            "feature_id": "GAF-GLIOMA-P08-F10",
            "output_schema": "GliomaInstrumentPreflight1@1",
            "objective": "blocked science loop",
            "instrument_id": "imager-1",
            "model_system": "organoid",
            "authorization_id": "approval-1",
            "action_order": ["acquire"],
            "admitted_order": [],
            "blocked_order": ["acquire"],
            "unresolved_order": [],
            "decisions": [{"action_id":"acquire","disposition":"blocked","scheduled_start_tick":null,"scheduled_end_tick":null,"reasons":["interlock-closed"]}],
            "total_risk_milli": 0,
            "total_duration_ticks": 0,
            "required_interlocks": [],
            "compensation_order": [],
            "negative_evidence": [],
            "uncertainty": [],
            "dispatch_permitted": false,
            "disposition": "blocked",
            "digest": digest
        });
        serde_json::from_value(serde_json::json!({
            "operating_cycle": {
                "campaign": {
                    "objective": "blocked science loop",
                    "runs": [{"run_id":"blocked-run","execution": {
                        "objective":"blocked science loop",
                        "plan": plan,
                        "actions": [],
                        "authorization":{"authorization_id":"approval-1","operator_id":"operator-1","instrument_scope":"imager-1","approval_digest":ContentHash::of_bytes(b"approval"),"issued_tick":0,"expires_tick":10,"revoked":false},
                        "live_interlocks":{"observed_tick":1,"emergency_stop_clear":true,"guard_closed":true,"deck_clear":true,"consumables_available":true,"waste_capacity_milli":1000,"temperature_milli":null,"minimum_temperature_milli":null,"maximum_temperature_milli":null,"calibration_valid_until_tick":10,"calibration_sequence_index":1},
                        "current_tick":1,"minimum_waste_capacity_milli":1,"max_retries":0,"require_artifacts":false
                    }}],
                    "max_runs":1,
                    "stop_on_negative":true
                },
                "require_all_admitted":true,
                "execution_mode":"local_simulation"
            },
            "evidence":{"objective":"blocked science loop","instrument_id":"imager-1","modality":"imaging","min_qc_milli":800,"min_effect_milli":100,"max_uncertainty_milli":100,"min_replicates":2,"require_negative_control":false,"max_negative_control_milli":0},
            "stop_on_first_qualified_run":true
        }))
        .unwrap()
    }

    #[test]
    fn blocked_preflight_never_reaches_assay_promotion() {
        let request = blocked_request();
        let mut executor =
            dry_run_instrument_executor_from_request(&request.operating_cycle).unwrap();
        let output = execute_glioma_instrument_science_loop(&request, &[], &mut executor).unwrap();
        assert_eq!(
            output.disposition,
            InstrumentScienceLoopDisposition::Blocked
        );
        assert!(output.assessments.is_empty());
        assert!(output
            .next_research_action_order
            .iter()
            .any(|action| action.contains("resolve-instrument-preflight")));
        assert!(output.simulation_only);
        output.validate().unwrap();
    }
}

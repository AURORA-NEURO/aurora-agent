//! Governed instrument-to-science loop for preclinical glioma assays.
//!
//! This feature closes the gap between a safe instrument run and a scientifically usable result.
//! It executes the existing preflight-barrier operating cycle through the caller-owned gateway,
//! then adjudicates each returned local assay summary against QC, effect, uncertainty, replicate,
//! and negative-control gates. Hardware completion is never promoted automatically: absent or
//! low-quality summaries remain unresolved/negative and become explicit next research actions.

use super::assay_adjudication::{
    AssayEvidenceDisposition, AssayEvidenceObservation, AssayEvidenceRequest,
    InstrumentAssayEvidenceAssessment, adjudicate_glioma_assay_evidence,
};
use super::execution::InstrumentExecutor;
use super::operating_cycle::{
    InstrumentOperatingCycle, InstrumentOperatingCycleDisposition, InstrumentOperatingCycleError,
    InstrumentOperatingCycleRequest, InstrumentPreflightSummary,
    execute_glioma_instrument_operating_cycle,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P08-F16";
pub const OUTPUT_SCHEMA: &str = "GliomaInstrumentScienceLoop1@2";
pub const INPUT_SCHEMA: &str = "GliomaInstrumentScienceLoopInput1@1";
pub const MAX_OBSERVATIONS: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentScienceLoopRequest {
    pub operating_cycle: InstrumentOperatingCycleRequest,
    pub evidence: AssayEvidenceRequest,
    pub stop_on_first_qualified_run: bool,
}

/// An assay summary tied to one exact instrument campaign run. Action IDs may repeat across runs,
/// so the run identity is mandatory at the science-loop boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentAssayEvidenceRunObservation {
    pub run_id: String,
    pub observation: AssayEvidenceObservation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentScienceLoopAssessment {
    pub run_id: String,
    pub assessment: InstrumentAssayEvidenceAssessment,
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
    /// Commitment to the exact request and run-scoped assay summaries supplied to this loop.
    pub input_digest: ContentHash,
    pub phase_order: Vec<String>,
    pub operating_cycle: InstrumentOperatingCycle,
    pub assessments: Vec<InstrumentScienceLoopAssessment>,
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
        "input_digest": output.input_digest,
        "phase_order": output.phase_order,
        "operating_cycle": output.operating_cycle,
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

fn request_digest(
    request: &InstrumentScienceLoopRequest,
    observations: &[InstrumentAssayEvidenceRunObservation],
) -> Result<ContentHash, InstrumentScienceLoopError> {
    #[derive(Serialize)]
    struct Input<'a> {
        input_schema: &'static str,
        request: &'a InstrumentScienceLoopRequest,
        observations: &'a [InstrumentAssayEvidenceRunObservation],
    }
    ContentHash::of_serializable(&Input {
        input_schema: INPUT_SCHEMA,
        request,
        observations,
    })
    .map_err(|error| InstrumentScienceLoopError::Digest(error.to_string()))
}

fn validate_request(
    request: &InstrumentScienceLoopRequest,
    observations: &[InstrumentAssayEvidenceRunObservation],
) -> Result<(), InstrumentScienceLoopError> {
    if request.operating_cycle.campaign.objective.trim().is_empty()
        || request.evidence.objective.trim().is_empty()
        || request.evidence.objective != request.operating_cycle.campaign.objective
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
            "matching non-empty objectives, runs, a bounded observation set, and one instrument binding are required".into(),
        ));
    }
    let mut actions_by_run = BTreeMap::new();
    for run in &request.operating_cycle.campaign.runs {
        if run.run_id.trim().is_empty()
            || actions_by_run
                .insert(
                    run.run_id.as_str(),
                    run.execution
                        .plan
                        .action_order
                        .iter()
                        .map(String::as_str)
                        .collect::<BTreeSet<_>>(),
                )
                .is_some()
        {
            return Err(InstrumentScienceLoopError::InvalidRequest(
                "instrument campaign run identities must be non-empty and unique".into(),
            ));
        }
    }
    let mut observations_by_action = BTreeSet::new();
    for input in observations {
        let Some(actions) = actions_by_run.get(input.run_id.as_str()) else {
            return Err(InstrumentScienceLoopError::InvalidRequest(
                "assay observations must bind to a declared instrument run".into(),
            ));
        };
        let observation = &input.observation;
        if observation.action_id.trim().is_empty()
            || !actions.contains(observation.action_id.as_str())
            || observation.artifact.validate().is_err()
            || observation.uncertainty_milli > super::assay_adjudication::MAX_UNCERTAINTY_MILLI
            || observation.qc_milli > 1_000
            || observation.replicate_count == 0
            || observation
                .negative_control_milli
                .is_some_and(|value| value > super::assay_adjudication::MAX_UNCERTAINTY_MILLI)
            || !observations_by_action
                .insert((input.run_id.as_str(), observation.action_id.as_str()))
        {
            return Err(InstrumentScienceLoopError::InvalidRequest(
                "run-scoped assay observations must be unique, execution-bound, local, and within QC/uncertainty bounds".into(),
            ));
        }
    }
    Ok(())
}

fn prefixed(run_id: &str, action_id: &str) -> String {
    format!("{run_id}:{action_id}")
}

#[cfg(test)]
fn canonicalize_assessment_pairs<T>(
    assessment_run_order: Vec<String>,
    assessments: Vec<T>,
) -> (Vec<String>, Vec<T>) {
    let mut assessment_pairs = assessment_run_order
        .into_iter()
        .zip(assessments)
        .collect::<Vec<_>>();
    assessment_pairs.sort_by(|left, right| left.0.cmp(&right.0));
    assessment_pairs.into_iter().unzip()
}

impl InstrumentScienceLoop {
    /// Verify the sealed output against the caller-retained source request and observations.
    /// This binds source data; it does not replay or authenticate instrument effects.
    pub fn validate_against(
        &self,
        request: &InstrumentScienceLoopRequest,
        observations: &[InstrumentAssayEvidenceRunObservation],
    ) -> Result<(), InstrumentScienceLoopError> {
        self.validate()?;
        validate_request(request, observations)?;
        if request_digest(request, observations)? != self.input_digest {
            return Err(InstrumentScienceLoopError::InvalidOutput(
                "science-loop input digest does not match the supplied request and observations"
                    .into(),
            ));
        }
        let expected_preflight = request
            .operating_cycle
            .campaign
            .runs
            .iter()
            .map(|run| {
                let plan = &run.execution.plan;
                let reasons = plan
                    .decisions
                    .iter()
                    .flat_map(|decision| decision.reasons.iter().cloned())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect();
                InstrumentPreflightSummary {
                    run_id: run.run_id.clone(),
                    instrument_id: plan.instrument_id.clone(),
                    disposition: plan.disposition,
                    dispatch_permitted: plan.dispatch_permitted,
                    action_order: plan.action_order.clone(),
                    blocked_order: plan.blocked_order.clone(),
                    unresolved_order: plan.unresolved_order.clone(),
                    reasons,
                }
            })
            .collect::<Vec<_>>();
        if self.operating_cycle.objective != request.operating_cycle.campaign.objective
            || self.operating_cycle.execution_mode != request.operating_cycle.execution_mode
            || self.operating_cycle.preflight != expected_preflight
        {
            return Err(InstrumentScienceLoopError::InvalidOutput(
                "operating-cycle preflight and mode do not bind to the supplied campaign request"
                    .into(),
            ));
        }
        if let Some(campaign) = &self.operating_cycle.campaign {
            let expected_run_order = request
                .operating_cycle
                .campaign
                .runs
                .iter()
                .map(|run| run.run_id.as_str())
                .collect::<Vec<_>>();
            if campaign
                .run_order
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
                != expected_run_order
                || campaign.results.iter().any(|result| {
                    request
                        .operating_cycle
                        .campaign
                        .runs
                        .iter()
                        .find(|run| run.run_id == result.run_id)
                        .is_none_or(|run| {
                            result.execution.plan_digest != run.execution.plan.digest
                                || result.execution.objective != run.execution.objective
                                || result.execution.instrument_id
                                    != run.execution.plan.instrument_id
                                || result.execution.action_order != run.execution.plan.action_order
                        })
                })
            {
                return Err(InstrumentScienceLoopError::InvalidOutput(
                    "executed campaign runs do not bind to the declared run order and preflight plans"
                        .into(),
                ));
            }
            for item in &self.assessments {
                let run = campaign
                    .results
                    .iter()
                    .find(|run| run.run_id == item.run_id)
                    .expect("validated science-loop assessments bind to campaign runs");
                let run_observations = observations
                    .iter()
                    .filter(|input| input.run_id == item.run_id)
                    .map(|input| input.observation.clone())
                    .collect::<Vec<_>>();
                let expected = adjudicate_glioma_assay_evidence(
                    &request.evidence,
                    &run.execution,
                    &run_observations,
                )
                .map_err(|error| InstrumentScienceLoopError::AssayEvidence(error.to_string()))?;
                if item.assessment != expected {
                    return Err(InstrumentScienceLoopError::InvalidOutput(
                        "assay assessment does not derive from the supplied run-scoped observations"
                            .into(),
                    ));
                }
            }
        }
        Ok(())
    }

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
            || self.input_digest.as_str().len() != 64
            || self
                .assessments
                .iter()
                .any(|item| item.run_id.trim().is_empty())
            || self
                .assessments
                .windows(2)
                .any(|pair| pair[0].run_id == pair[1].run_id)
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
        for item in &self.assessments {
            item.assessment
                .validate()
                .map_err(|error| InstrumentScienceLoopError::InvalidOutput(error.to_string()))?;
        }
        if let Some(campaign) = &self.operating_cycle.campaign {
            if self.assessments.len() > campaign.results.len()
                || self
                    .assessments
                    .iter()
                    .zip(&campaign.results)
                    .any(|(item, run)| {
                        item.run_id != run.run_id
                            || item.assessment.execution_digest != run.execution.digest
                            || item.assessment.objective != self.objective
                            || item.assessment.instrument_id != run.execution.instrument_id
                            || item.assessment.action_order != run.execution.action_order
                    })
                || self.assessments.len() < campaign.results.len()
                    && self.assessments.last().is_none_or(|item| {
                        item.assessment.disposition != AssayEvidenceDisposition::Qualified
                    })
            {
                return Err(InstrumentScienceLoopError::InvalidOutput(
                    "assessments must bind in execution order to the exact run and may stop early only after qualification".into(),
                ));
            }
        } else if !self.assessments.is_empty() {
            return Err(InstrumentScienceLoopError::InvalidOutput(
                "assessments cannot exist without an executed campaign".into(),
            ));
        }
        let mut expected_qualified = BTreeSet::new();
        let mut expected_negative = BTreeSet::new();
        let mut expected_unresolved = BTreeSet::new();
        let mut expected_next_actions = BTreeSet::new();
        let mut expected_negative_evidence = self
            .operating_cycle
            .negative_evidence
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut expected_uncertainty = self
            .operating_cycle
            .uncertainty
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        for item in &self.assessments {
            let assessment = &item.assessment;
            expected_qualified.extend(
                assessment
                    .qualified_order
                    .iter()
                    .map(|id| prefixed(&item.run_id, id)),
            );
            expected_negative.extend(
                assessment
                    .negative_order
                    .iter()
                    .map(|id| prefixed(&item.run_id, id)),
            );
            expected_unresolved.extend(
                assessment
                    .unresolved_order
                    .iter()
                    .map(|id| prefixed(&item.run_id, id)),
            );
            expected_next_actions.extend(
                assessment
                    .next_action_order
                    .iter()
                    .map(|id| prefixed(&item.run_id, id)),
            );
            expected_negative_evidence.extend(assessment.negative_evidence.iter().cloned());
            expected_uncertainty.extend(assessment.uncertainty.iter().cloned());
        }
        if self.operating_cycle.campaign.is_none() {
            expected_next_actions
                .insert("resolve-instrument-preflight-before-assay-adjudication".to_string());
            expected_uncertainty.insert(
                "instrument-operating-cycle-did-not-produce-an-admitted-campaign".to_string(),
            );
        }
        let expected_disposition = if self.operating_cycle.disposition
            == InstrumentOperatingCycleDisposition::Blocked
        {
            InstrumentScienceLoopDisposition::Blocked
        } else if self.operating_cycle.disposition == InstrumentOperatingCycleDisposition::Failed {
            InstrumentScienceLoopDisposition::Failed
        } else if !expected_qualified.is_empty()
            && expected_negative.is_empty()
            && expected_unresolved.is_empty()
        {
            InstrumentScienceLoopDisposition::Qualified
        } else if !expected_qualified.is_empty() || !expected_negative.is_empty() {
            InstrumentScienceLoopDisposition::Partial
        } else if !self.assessments.is_empty() {
            InstrumentScienceLoopDisposition::Unresolved
        } else {
            InstrumentScienceLoopDisposition::Blocked
        };
        expected_next_actions.insert(
            if expected_disposition == InstrumentScienceLoopDisposition::Qualified {
                "promote-qualified-assay-evidence-into-analysis".to_string()
            } else {
                "retain-assay-hold-and-route-the-highest-information-follow-up".to_string()
            },
        );
        expected_negative_evidence.extend(self.operating_cycle.negative_evidence.iter().cloned());
        expected_uncertainty.extend(self.operating_cycle.uncertainty.iter().cloned());
        if self.qualified_action_order != expected_qualified.into_iter().collect::<Vec<_>>()
            || self.negative_action_order != expected_negative.into_iter().collect::<Vec<_>>()
            || self.unresolved_action_order != expected_unresolved.into_iter().collect::<Vec<_>>()
            || self.next_research_action_order
                != expected_next_actions.into_iter().collect::<Vec<_>>()
            || self.negative_evidence != expected_negative_evidence.into_iter().collect::<Vec<_>>()
            || self.uncertainty != expected_uncertainty.into_iter().collect::<Vec<_>>()
            || self.disposition != expected_disposition
            || self.simulation_only != self.operating_cycle.simulation_only
        {
            return Err(InstrumentScienceLoopError::InvalidOutput(
                "science-loop partitions, evidence, disposition, or simulation state do not derive from the bound assessments".into(),
            ));
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
    observations: &[InstrumentAssayEvidenceRunObservation],
    executor: &mut E,
) -> Result<InstrumentScienceLoop, InstrumentScienceLoopError> {
    validate_request(request, observations)?;
    let input_digest = request_digest(request, observations)?;
    let operating_cycle =
        execute_glioma_instrument_operating_cycle(&request.operating_cycle, executor)?;
    let mut assessments = Vec::<InstrumentScienceLoopAssessment>::new();
    let mut qualified = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut next_actions = BTreeSet::new();
    let mut negative_evidence = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let observations_by_run_action = observations
        .iter()
        .map(|input| {
            (
                (input.run_id.clone(), input.observation.action_id.clone()),
                &input.observation,
            )
        })
        .collect::<BTreeMap<_, _>>();

    if let Some(campaign) = &operating_cycle.campaign {
        for run in &campaign.results {
            let run_observations = run
                .execution
                .action_order
                .iter()
                .filter_map(|action_id| {
                    observations_by_run_action
                        .get(&(run.run_id.clone(), action_id.clone()))
                        .copied()
                })
                .cloned()
                .collect::<Vec<_>>();
            let assessment = adjudicate_glioma_assay_evidence(
                &request.evidence,
                &run.execution,
                &run_observations,
            )
            .map_err(|error| InstrumentScienceLoopError::AssayEvidence(error.to_string()))?;
            let item = InstrumentScienceLoopAssessment {
                run_id: run.run_id.clone(),
                assessment,
            };
            for action_id in &item.assessment.qualified_order {
                qualified.insert(prefixed(&run.run_id, action_id));
            }
            for action_id in &item.assessment.negative_order {
                negative.insert(prefixed(&run.run_id, action_id));
            }
            for action_id in &item.assessment.unresolved_order {
                unresolved.insert(prefixed(&run.run_id, action_id));
            }
            for action in &item.assessment.next_action_order {
                next_actions.insert(prefixed(&run.run_id, action));
            }
            negative_evidence.extend(item.assessment.negative_evidence.iter().cloned());
            uncertainty.extend(item.assessment.uncertainty.iter().cloned());
            let stop = request.stop_on_first_qualified_run
                && item.assessment.disposition == AssayEvidenceDisposition::Qualified;
            assessments.push(item);
            if stop {
                break;
            }
        }
    } else {
        uncertainty
            .insert("instrument-operating-cycle-did-not-produce-an-admitted-campaign".into());
        next_actions.insert("resolve-instrument-preflight-before-assay-adjudication".into());
    }
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
        input_digest,
        phase_order: vec![
            "preflight_and_execution".into(),
            "assay_evidence_adjudication".into(),
            "research_action_handoff".into(),
        ],
        operating_cycle,
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
    output.validate_against(request, observations)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p08_instrument_robotics::calibration::{
        CalibrationRequest, CalibrationRun, analyze_instrument_calibration,
    };
    use crate::glioma::programs::p08_instrument_robotics::campaign::{
        InstrumentCampaignRequest, InstrumentCampaignRunRequest,
    };
    use crate::glioma::programs::p08_instrument_robotics::execution::InstrumentExecutionRequest;
    use crate::glioma::programs::p08_instrument_robotics::operating_cycle::{
        InstrumentExecutionMode, dry_run_instrument_executor_from_request,
    };
    use crate::glioma::programs::p08_instrument_robotics::preflight::{
        InstrumentAction, InstrumentAuthorization, InstrumentInterlockSnapshot,
        InstrumentOperation, InstrumentParameter, InstrumentPreflightRequest,
        preflight_glioma_instrument,
    };
    use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_bytes(label.as_bytes())
    }

    fn admitted_execution_request() -> InstrumentExecutionRequest {
        let calibration_runs = (1..=3)
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
                    artifact_id: format!("control-{sequence_index}"),
                    content_hash: hash(&format!("control-{sequence_index}")),
                    content_type: "application/vnd.aurora.glioma-control+json".into(),
                    local_only: true,
                    contains_human_data: false,
                    contains_direct_identifiers: false,
                },
            })
            .collect::<Vec<_>>();
        let calibration = analyze_instrument_calibration(
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
            &calibration_runs,
        )
        .unwrap();
        let action = InstrumentAction {
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
        };
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
            approval_digest: hash("approval"),
            issued_tick: 0,
            expires_tick: 100,
            revoked: false,
        };
        let plan = preflight_glioma_instrument(&InstrumentPreflightRequest {
            objective: "acquire image".into(),
            instrument_id: "imager-1".into(),
            model_system: GliomaModelSystem::Organoid,
            actions: vec![action.clone()],
            calibration,
            interlocks: interlocks.clone(),
            authorization: authorization.clone(),
            current_tick: 1,
            maximum_total_risk_milli: 500,
            maximum_duration_ticks: 20,
            minimum_waste_capacity_milli: 100,
        })
        .unwrap();
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

    fn two_run_request() -> InstrumentScienceLoopRequest {
        let execution = admitted_execution_request();
        InstrumentScienceLoopRequest {
            operating_cycle: InstrumentOperatingCycleRequest {
                campaign: InstrumentCampaignRequest {
                    objective: "compare two organoid instrument runs".into(),
                    runs: vec![
                        InstrumentCampaignRunRequest {
                            run_id: "run-z".into(),
                            execution: execution.clone(),
                        },
                        InstrumentCampaignRunRequest {
                            run_id: "run-a".into(),
                            execution,
                        },
                    ],
                    max_runs: 2,
                    stop_on_negative: false,
                },
                require_all_admitted: true,
                execution_mode: InstrumentExecutionMode::LocalSimulation,
            },
            evidence: AssayEvidenceRequest {
                objective: "compare two organoid instrument runs".into(),
                instrument_id: "imager-1".into(),
                modality: crate::glioma_engine::GliomaModality::Imaging,
                min_qc_milli: 800,
                min_effect_milli: 100,
                max_uncertainty_milli: 100,
                min_replicates: 2,
                require_negative_control: false,
                max_negative_control_milli: 0,
            },
            stop_on_first_qualified_run: false,
        }
    }

    fn two_run_observations() -> Vec<InstrumentAssayEvidenceRunObservation> {
        [
            ("run-z", "qualified-output", 500),
            ("run-a", "negative-output", 10),
        ]
        .into_iter()
        .map(
            |(run_id, artifact_id, signal_milli)| InstrumentAssayEvidenceRunObservation {
                run_id: run_id.into(),
                observation: AssayEvidenceObservation {
                    action_id: "acquire".into(),
                    artifact: LocalArtifactRef {
                        artifact_id: artifact_id.into(),
                        content_hash: hash(artifact_id),
                        content_type: "application/vnd.aurora.glioma.assay+json".into(),
                        local_only: true,
                        contains_human_data: false,
                        contains_direct_identifiers: false,
                    },
                    signal_milli,
                    baseline_milli: 0,
                    uncertainty_milli: 10,
                    qc_milli: 950,
                    replicate_count: 3,
                    negative_control_milli: None,
                },
            },
        )
        .collect()
    }

    fn blocked_request() -> InstrumentScienceLoopRequest {
        let empty_action_manifest_digest = ContentHash::of_value(&serde_json::json!([])).unwrap();
        let plan_without_digest = serde_json::json!({
            "feature_id": "GAF-GLIOMA-P08-F10",
            "output_schema": "GliomaInstrumentPreflight1@2",
            "objective": "blocked science loop",
            "instrument_id": "imager-1",
            "model_system": "organoid",
            "authorization_id": "approval-1",
            "action_manifest_digest": empty_action_manifest_digest.clone(),
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
            "output_schema": "GliomaInstrumentPreflight1@2",
            "objective": "blocked science loop",
            "instrument_id": "imager-1",
            "model_system": "organoid",
            "authorization_id": "approval-1",
            "action_manifest_digest": empty_action_manifest_digest,
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
    fn canonical_run_order_preserves_assessment_binding() {
        let (run_order, assessments) = canonicalize_assessment_pairs(
            vec!["run-z".into(), "run-a".into()],
            vec!["assessment-z", "assessment-a"],
        );
        assert_eq!(run_order, vec!["run-a", "run-z"]);
        assert_eq!(assessments, vec!["assessment-a", "assessment-z"]);
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
        assert!(
            output
                .next_research_action_order
                .iter()
                .any(|action| action.contains("resolve-instrument-preflight"))
        );
        assert!(output.simulation_only);
        output.validate().unwrap();
    }

    #[test]
    fn repeated_action_ids_keep_their_run_scoped_assessment_and_source_digest() {
        let request = two_run_request();
        let observations = two_run_observations();
        let mut executor =
            dry_run_instrument_executor_from_request(&request.operating_cycle).unwrap();
        let output =
            execute_glioma_instrument_science_loop(&request, &observations, &mut executor).unwrap();

        assert_eq!(output.output_schema, "GliomaInstrumentScienceLoop1@2");
        assert_eq!(
            output
                .assessments
                .iter()
                .map(|item| item.run_id.as_str())
                .collect::<Vec<_>>(),
            vec!["run-z", "run-a"]
        );
        assert_eq!(
            output.assessments[0].assessment.qualified_order,
            vec!["acquire"]
        );
        assert_eq!(
            output.assessments[1].assessment.negative_order,
            vec!["acquire"]
        );
        assert_eq!(
            output.assessments[0].assessment.records[0]
                .artifact
                .as_ref()
                .unwrap()
                .artifact_id,
            "qualified-output"
        );
        assert_eq!(
            output.assessments[1].assessment.records[0]
                .artifact
                .as_ref()
                .unwrap()
                .artifact_id,
            "negative-output"
        );
        output.validate_against(&request, &observations).unwrap();

        let mut changed_observations = observations.clone();
        changed_observations[0].run_id = "run-a".into();
        assert!(
            output
                .validate_against(&request, &changed_observations)
                .is_err()
        );

        let mut wrong_source = observations[0].observation.clone();
        wrong_source.artifact = observations[1].observation.artifact.clone();
        let first_execution =
            &output.operating_cycle.campaign.as_ref().unwrap().results[0].execution;
        let wrongly_bound_assessment =
            adjudicate_glioma_assay_evidence(&request.evidence, first_execution, &[wrong_source])
                .unwrap();
        let mut resealed = output.clone();
        resealed.assessments[0].assessment = wrongly_bound_assessment;
        resealed.digest = ContentHash::of_value(&digest_input(&resealed)).unwrap();
        resealed.validate().unwrap();
        assert!(resealed.validate_against(&request, &observations).is_err());
    }

    #[test]
    fn assay_observation_for_an_undeclared_run_is_refused_before_execution() {
        let request = blocked_request();
        let observations = vec![InstrumentAssayEvidenceRunObservation {
            run_id: "unlisted-run".into(),
            observation: two_run_observations().remove(0).observation,
        }];
        assert!(validate_request(&request, &observations).is_err());
    }
}

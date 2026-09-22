//! Evidence adjudication after guarded preclinical instrument execution.
//!
//! P08 execution only proves that a gateway completed (or failed) an operation. This feature
//! consumes that run plus typed, local assay summaries and decides whether a result is eligible for
//! downstream scientific analysis. It keeps low QC, missing measurements, null effects, negative
//! controls, and uncertainty visible; it never claims that a completed instrument operation is
//! evidence by itself.

use super::execution::{
    InstrumentExecutionDisposition, InstrumentExecutionResult, InstrumentExecutionRun,
};
use crate::glioma_engine::{GliomaModality, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P08-F20";
pub const OUTPUT_SCHEMA: &str = "GliomaInstrumentAssayEvidence1@1";
pub const MAX_OBSERVATIONS: usize = 4_096;
pub const MAX_UNCERTAINTY_MILLI: u64 = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssayEvidenceRequest {
    pub objective: String,
    pub instrument_id: String,
    pub modality: GliomaModality,
    pub min_qc_milli: u16,
    pub min_effect_milli: u64,
    pub max_uncertainty_milli: u64,
    pub min_replicates: u16,
    pub require_negative_control: bool,
    pub max_negative_control_milli: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssayEvidenceObservation {
    pub action_id: String,
    pub artifact: LocalArtifactRef,
    pub signal_milli: i64,
    pub baseline_milli: i64,
    pub uncertainty_milli: u64,
    pub qc_milli: u16,
    pub replicate_count: u16,
    pub negative_control_milli: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssayEvidenceRecord {
    pub action_id: String,
    pub effect_milli: i64,
    pub absolute_effect_milli: u64,
    pub qc_milli: u16,
    pub uncertainty_milli: u64,
    pub replicate_count: u16,
    pub negative_control_milli: Option<u64>,
    pub artifact: Option<LocalArtifactRef>,
    pub eligible: bool,
    pub reason_order: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssayEvidenceDisposition {
    Qualified,
    Partial,
    Negative,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentAssayEvidenceAssessment {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub instrument_id: String,
    pub modality: GliomaModality,
    pub execution_digest: ContentHash,
    pub action_order: Vec<String>,
    pub records: Vec<AssayEvidenceRecord>,
    pub qualified_order: Vec<String>,
    pub negative_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub next_action_order: Vec<String>,
    pub evidence_eligible: bool,
    pub limitations: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: AssayEvidenceDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AssayEvidenceError {
    #[error("assay evidence request is invalid: {0}")]
    InvalidRequest(String),
    #[error("assay evidence observation is invalid: {0}")]
    InvalidObservation(String),
    #[error("instrument execution cannot supply assay evidence: {0}")]
    InvalidExecution(String),
    #[error("assay evidence output is invalid: {0}")]
    InvalidOutput(String),
    #[error("assay evidence digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &InstrumentAssayEvidenceAssessment) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "instrument_id": output.instrument_id,
        "modality": output.modality,
        "execution_digest": output.execution_digest,
        "action_order": output.action_order,
        "records": output.records,
        "qualified_order": output.qualified_order,
        "negative_order": output.negative_order,
        "unresolved_order": output.unresolved_order,
        "next_action_order": output.next_action_order,
        "evidence_eligible": output.evidence_eligible,
        "limitations": output.limitations,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn validate_request(request: &AssayEvidenceRequest) -> Result<(), AssayEvidenceError> {
    if request.objective.trim().is_empty()
        || request.instrument_id.trim().is_empty()
        || request.min_qc_milli > 1_000
        || request.max_uncertainty_milli > MAX_UNCERTAINTY_MILLI
        || request.min_replicates == 0
        || request.max_negative_control_milli > MAX_UNCERTAINTY_MILLI
    {
        return Err(AssayEvidenceError::InvalidRequest(
            "objective, instrument, QC, uncertainty, replicate, or control bounds are invalid"
                .into(),
        ));
    }
    Ok(())
}

impl InstrumentAssayEvidenceAssessment {
    pub fn validate(&self) -> Result<(), AssayEvidenceError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.instrument_id.trim().is_empty()
            || !canonical(&self.action_order)
            || !canonical(&self.qualified_order)
            || !canonical(&self.negative_order)
            || !canonical(&self.unresolved_order)
            || !canonical(&self.next_action_order)
            || !canonical(&self.limitations)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.records.iter().any(|record| {
                record.action_id.trim().is_empty()
                    || record
                        .artifact
                        .as_ref()
                        .is_some_and(|artifact| artifact.validate().is_err())
                    || record.qc_milli > 1_000
                    || record.uncertainty_milli > MAX_UNCERTAINTY_MILLI
                    || record
                        .negative_control_milli
                        .is_some_and(|value| value > MAX_UNCERTAINTY_MILLI)
                    || (record.replicate_count == 0
                        && (record.artifact.is_some() || record.eligible))
                    || record.eligible == !record.reason_order.is_empty()
                    || record
                        .reason_order
                        .windows(2)
                        .any(|pair| pair[0] >= pair[1])
            })
        {
            return Err(AssayEvidenceError::InvalidOutput(
                "identity, ordering, artifact, or assay-record bounds are invalid".into(),
            ));
        }
        let actions = self.action_order.iter().cloned().collect::<BTreeSet<_>>();
        let records = self
            .records
            .iter()
            .map(|record| record.action_id.clone())
            .collect::<BTreeSet<_>>();
        let qualified = self
            .qualified_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let negative = self.negative_order.iter().cloned().collect::<BTreeSet<_>>();
        let unresolved = self
            .unresolved_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let eligible_records = self
            .records
            .iter()
            .filter(|record| record.eligible)
            .map(|record| record.action_id.clone())
            .collect::<BTreeSet<_>>();
        if actions != records
            || records.len() != self.records.len()
            || eligible_records != qualified
            || qualified.intersection(&negative).next().is_some()
            || qualified.intersection(&unresolved).next().is_some()
            || negative.intersection(&unresolved).next().is_some()
            || qualified
                .union(&negative)
                .chain(unresolved.iter())
                .any(|id| !actions.contains(id))
        {
            return Err(AssayEvidenceError::InvalidOutput(
                "action partitions do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| AssayEvidenceError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(AssayEvidenceError::InvalidOutput(
                "assay evidence digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn result_by_action(
    execution: &InstrumentExecutionRun,
) -> BTreeMap<String, &InstrumentExecutionResult> {
    execution
        .results
        .iter()
        .map(|result| (result.action_id.clone(), result))
        .collect()
}

/// Decide whether executed instrument operations produced assay evidence eligible for science.
/// Hardware completion alone can never qualify; every action needs a local, de-identified summary
/// that clears the requested QC, uncertainty, replicate, effect, and control gates.
pub fn adjudicate_glioma_assay_evidence(
    request: &AssayEvidenceRequest,
    execution: &InstrumentExecutionRun,
    observations: &[AssayEvidenceObservation],
) -> Result<InstrumentAssayEvidenceAssessment, AssayEvidenceError> {
    validate_request(request)?;
    if observations.len() > MAX_OBSERVATIONS {
        return Err(AssayEvidenceError::InvalidObservation(
            "observation count exceeds configured bound".into(),
        ));
    }
    execution
        .validate()
        .map_err(|error| AssayEvidenceError::InvalidExecution(error.to_string()))?;
    if execution.instrument_id != request.instrument_id {
        return Err(AssayEvidenceError::InvalidExecution(
            "execution instrument does not match the adjudication request".into(),
        ));
    }
    let mut by_action = BTreeMap::new();
    for observation in observations {
        if observation.action_id.trim().is_empty()
            || observation.artifact.validate().is_err()
            || observation.uncertainty_milli > MAX_UNCERTAINTY_MILLI
            || observation.qc_milli > 1_000
            || observation.replicate_count == 0
            || observation
                .negative_control_milli
                .is_some_and(|value| value > MAX_UNCERTAINTY_MILLI)
        {
            return Err(AssayEvidenceError::InvalidObservation(
                "observations require local artifacts and bounded QC/uncertainty/replicate values"
                    .into(),
            ));
        }
        if by_action
            .insert(observation.action_id.clone(), observation)
            .is_some()
        {
            return Err(AssayEvidenceError::InvalidObservation(
                "duplicate assay action observations are not admissible".into(),
            ));
        }
    }
    let execution_results = result_by_action(execution);
    if by_action
        .keys()
        .any(|action_id| !execution_results.contains_key(action_id))
    {
        return Err(AssayEvidenceError::InvalidObservation(
            "observation action is not present in the validated execution run".into(),
        ));
    }
    let mut action_order = execution.action_order.clone();
    action_order.sort();
    let mut records = Vec::with_capacity(action_order.len());
    let mut qualified = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut next_actions = BTreeSet::new();
    let mut negative_evidence = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for action_id in &action_order {
        let Some(result) = execution_results.get(action_id) else {
            unresolved.insert(action_id.clone());
            next_actions.insert(format!("acquire-execution-result:{action_id}"));
            uncertainty.insert(format!("missing-execution-result:{action_id}"));
            records.push(AssayEvidenceRecord {
                action_id: action_id.clone(),
                effect_milli: 0,
                absolute_effect_milli: 0,
                qc_milli: 0,
                uncertainty_milli: MAX_UNCERTAINTY_MILLI,
                replicate_count: 0,
                negative_control_milli: None,
                artifact: None,
                eligible: false,
                reason_order: vec!["missing-execution-result".into()],
            });
            continue;
        };
        let Some(observation) = by_action.get(action_id) else {
            unresolved.insert(action_id.clone());
            next_actions.insert(format!("acquire-assay-observation:{action_id}"));
            uncertainty.insert(format!("missing-assay-observation:{action_id}"));
            records.push(AssayEvidenceRecord {
                action_id: action_id.clone(),
                effect_milli: 0,
                absolute_effect_milli: 0,
                qc_milli: 0,
                uncertainty_milli: MAX_UNCERTAINTY_MILLI,
                replicate_count: 0,
                negative_control_milli: None,
                artifact: None,
                eligible: false,
                reason_order: vec!["missing-assay-observation".into()],
            });
            continue;
        };
        let effect = observation
            .signal_milli
            .saturating_sub(observation.baseline_milli);
        let absolute_effect = effect.unsigned_abs();
        let mut reasons = BTreeSet::new();
        if result.disposition != InstrumentExecutionDisposition::Completed
            || result.artifact.is_none()
        {
            reasons.insert("instrument-operation-did-not-complete".to_string());
        }
        if observation.qc_milli < request.min_qc_milli {
            reasons.insert("qc-below-floor".to_string());
        }
        if observation.uncertainty_milli > request.max_uncertainty_milli {
            reasons.insert("uncertainty-above-ceiling".to_string());
        }
        if observation.replicate_count < request.min_replicates {
            reasons.insert("replicate-floor-not-met".to_string());
        }
        if absolute_effect < request.min_effect_milli {
            reasons.insert("effect-below-declared-floor".to_string());
        }
        if request.require_negative_control {
            match observation.negative_control_milli {
                Some(value) if value <= request.max_negative_control_milli => {}
                Some(value) => {
                    reasons.insert("negative-control-exceeds-ceiling".to_string());
                    negative_evidence.insert(format!("negative-control:{action_id}:{value}"));
                }
                None => {
                    reasons.insert("negative-control-missing".to_string());
                    uncertainty.insert(format!("negative-control-missing:{action_id}"));
                }
            }
        }
        let reason_order = reasons.iter().cloned().collect::<Vec<_>>();
        let eligible = reasons.is_empty();
        if eligible {
            qualified.insert(action_id.clone());
        } else if reasons.iter().any(|reason| {
            reason == "effect-below-declared-floor" || reason == "negative-control-exceeds-ceiling"
        }) && reasons.len() == 1
        {
            negative.insert(action_id.clone());
            negative_evidence.insert(format!("null-or-negative-assay:{action_id}"));
        } else {
            unresolved.insert(action_id.clone());
            next_actions.insert(format!("review-or-repeat-assay:{action_id}"));
            if reasons.iter().any(|reason| reason == "qc-below-floor") {
                next_actions.insert(format!("recalibrate-instrument:{action_id}"));
            }
            uncertainty.extend(reasons.iter().map(|reason| format!("{action_id}:{reason}")));
        }
        records.push(AssayEvidenceRecord {
            action_id: action_id.clone(),
            effect_milli: effect,
            absolute_effect_milli: absolute_effect,
            qc_milli: observation.qc_milli,
            uncertainty_milli: observation.uncertainty_milli,
            replicate_count: observation.replicate_count,
            negative_control_milli: observation.negative_control_milli,
            artifact: Some(observation.artifact.clone()),
            eligible,
            reason_order,
        });
    }
    let disposition = if unresolved.len() == action_order.len() {
        AssayEvidenceDisposition::Unresolved
    } else if !qualified.is_empty() && unresolved.is_empty() && negative.is_empty() {
        AssayEvidenceDisposition::Qualified
    } else if !negative.is_empty() && qualified.is_empty() && unresolved.is_empty() {
        AssayEvidenceDisposition::Negative
    } else if !qualified.is_empty() || !negative.is_empty() {
        AssayEvidenceDisposition::Partial
    } else {
        AssayEvidenceDisposition::Negative
    };
    let mut limitations = BTreeSet::from([
        "instrument operation completion is not biological evidence without assay adjudication"
            .to_string(),
        "effect thresholds and QC floors are declared gates, not causal identification".to_string(),
    ]);
    if execution.disposition != InstrumentExecutionDisposition::Completed {
        limitations.insert(
            "execution run itself is non-complete; evidence cannot be globally promoted".into(),
        );
    }
    let evidence_eligible = disposition == AssayEvidenceDisposition::Qualified
        && execution.disposition == InstrumentExecutionDisposition::Completed;
    let mut output = InstrumentAssayEvidenceAssessment {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        instrument_id: request.instrument_id.clone(),
        modality: request.modality,
        execution_digest: execution.digest.clone(),
        action_order,
        records,
        qualified_order: qualified.into_iter().collect(),
        negative_order: negative.into_iter().collect(),
        unresolved_order: unresolved.into_iter().collect(),
        next_action_order: next_actions.into_iter().collect(),
        evidence_eligible,
        limitations: limitations.into_iter().collect(),
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-assay-evidence"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| AssayEvidenceError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(label: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: label.into(),
            content_hash: ContentHash::of_bytes(label.as_bytes()),
            content_type: "application/vnd.aurora.glioma.assay+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn execution_run() -> InstrumentExecutionRun {
        let result = InstrumentExecutionResult {
            action_id: "acquire".into(),
            disposition: InstrumentExecutionDisposition::Completed,
            attempt_count: 1,
            started_tick: Some(1),
            completed_tick: Some(2),
            artifact: Some(artifact("instrument-output")),
            note: "synthetic operation completed".into(),
            uncertainty: vec!["synthetic-operation".into()],
            negative_evidence: vec!["dry-run-is-not-biology".into()],
        };
        let mut run = InstrumentExecutionRun {
            feature_id: super::super::execution::FEATURE_ID.into(),
            output_schema: super::super::execution::OUTPUT_SCHEMA.into(),
            objective: "acquire assay image".into(),
            plan_digest: ContentHash::of_bytes(b"plan"),
            instrument_id: "imager-1".into(),
            action_order: vec!["acquire".into()],
            results: vec![result],
            completed_order: vec!["acquire".into()],
            negative_order: Vec::new(),
            partial_order: Vec::new(),
            failed_order: Vec::new(),
            unresolved_order: Vec::new(),
            skipped_order: Vec::new(),
            retry_count: 0,
            emergency_stop_requested: false,
            emergency_stop_succeeded: false,
            uncertainty: vec!["synthetic-operation".into()],
            negative_evidence: vec!["dry-run-is-not-biology".into()],
            disposition: InstrumentExecutionDisposition::Completed,
            stop_reason: super::super::execution::InstrumentExecutionStopReason::Completed,
            digest: ContentHash::of_bytes(b"unsealed-execution"),
        };
        run.digest = ContentHash::of_value(&serde_json::json!({
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
        }))
        .unwrap();
        run.validate().unwrap();
        run
    }

    fn request() -> AssayEvidenceRequest {
        AssayEvidenceRequest {
            objective: "qualify organoid imaging assay".into(),
            instrument_id: "imager-1".into(),
            modality: GliomaModality::Imaging,
            min_qc_milli: 900,
            min_effect_milli: 500,
            max_uncertainty_milli: 100,
            min_replicates: 2,
            require_negative_control: true,
            max_negative_control_milli: 100,
        }
    }

    #[test]
    fn qualifies_only_typed_assay_evidence_and_replays() {
        let execution = execution_run();
        let observations = vec![AssayEvidenceObservation {
            action_id: "acquire".into(),
            artifact: artifact("assay-summary"),
            signal_milli: 900,
            baseline_milli: 100,
            uncertainty_milli: 10,
            qc_milli: 950,
            replicate_count: 3,
            negative_control_milli: Some(0),
        }];
        let first =
            adjudicate_glioma_assay_evidence(&request(), &execution, &observations).unwrap();
        let second =
            adjudicate_glioma_assay_evidence(&request(), &execution, &observations).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.disposition, AssayEvidenceDisposition::Qualified);
        assert!(first.evidence_eligible);
        assert_eq!(first.qualified_order, vec!["acquire"]);
        assert!(first.records[0].eligible);
        first.validate().unwrap();
    }

    #[test]
    fn missing_observation_stays_unresolved_with_next_action() {
        let assessment =
            adjudicate_glioma_assay_evidence(&request(), &execution_run(), &[]).unwrap();
        assert_eq!(assessment.disposition, AssayEvidenceDisposition::Unresolved);
        assert!(!assessment.evidence_eligible);
        assert_eq!(assessment.unresolved_order, vec!["acquire"]);
        assert_eq!(
            assessment.next_action_order,
            vec!["acquire-assay-observation:acquire"]
        );
        assert_eq!(assessment.records[0].artifact, None);
    }

    #[test]
    fn null_effect_is_negative_not_promoted() {
        let observation = AssayEvidenceObservation {
            action_id: "acquire".into(),
            artifact: artifact("assay-summary"),
            signal_milli: 105,
            baseline_milli: 100,
            uncertainty_milli: 10,
            qc_milli: 950,
            replicate_count: 3,
            negative_control_milli: Some(0),
        };
        let assessment =
            adjudicate_glioma_assay_evidence(&request(), &execution_run(), &[observation]).unwrap();
        assert_eq!(assessment.disposition, AssayEvidenceDisposition::Negative);
        assert!(!assessment.evidence_eligible);
        assert_eq!(assessment.negative_order, vec!["acquire"]);
        assert!(assessment
            .negative_evidence
            .iter()
            .any(|item| item == "null-or-negative-assay:acquire"));
    }
}

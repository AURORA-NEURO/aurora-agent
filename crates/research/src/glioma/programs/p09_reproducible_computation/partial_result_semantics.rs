//! Typed partial-result semantics for reproducible preclinical glioma computation.
//!
//! Long-running research computation rarely ends as a clean all-or-nothing result. A budget
//! stop, worker crash, protected-closure block, or unavailable dependency must not be coerced
//! into a measured zero. This feature turns an execution run plus explicit field observations
//! into a replayable result bundle that keeps measured values, measured nulls, censored values,
//! failures, redaction, and unavailability distinct. It also computes a conservative downstream
//! operation gate so interpretation and publication cannot silently consume incomplete outputs.

use super::execution::{ComputationExecution, ComputationTaskDisposition};
use crate::glioma_engine::LocalArtifactRef;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P09-F08";
pub const OUTPUT_SCHEMA: &str = "GliomaPartialResultBundle1@1";
pub const MAX_FIELDS: usize = 4_096;
pub const MAX_NOTE_BYTES: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PartialResultFieldState {
    Measured,
    MeasuredNull,
    Censored,
    Interrupted,
    Failed,
    Unavailable,
    Redacted,
    Invalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PartialResultTermination {
    Completed,
    BudgetExhausted,
    Interrupted,
    Failed,
    Cancelled,
    ResourceUnavailable,
    PolicyBlocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PartialResultOperation {
    MissingnessReview,
    DescriptiveSummary,
    ModelFit,
    MechanismInference,
    Publication,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartialResultFieldSpec {
    pub field_id: String,
    pub task_id: String,
    pub output_schema: String,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartialResultFieldObservation {
    pub field_id: String,
    pub state: PartialResultFieldState,
    pub artifact: Option<LocalArtifactRef>,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartialResultPolicy {
    pub allow_partial_descriptive_summary: bool,
    pub allow_measured_nulls: bool,
    pub allow_censored_fields: bool,
    pub allow_publication: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartialResultBundleRequest {
    pub objective: String,
    pub replay_identity: ContentHash,
    pub execution: ComputationExecution,
    pub expected_fields: Vec<PartialResultFieldSpec>,
    pub observations: Vec<PartialResultFieldObservation>,
    pub termination: PartialResultTermination,
    pub policy: PartialResultPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartialResultFieldRecord {
    pub field_id: String,
    pub task_id: String,
    pub output_schema: String,
    pub required: bool,
    pub state: PartialResultFieldState,
    pub artifact: Option<LocalArtifactRef>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartialResultOperationDecision {
    pub operation: PartialResultOperation,
    pub allowed: bool,
    pub reason_order: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PartialResultDisposition {
    Complete,
    NullOnly,
    Partial,
    Failed,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartialResultBundle {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub replay_identity: ContentHash,
    pub termination: PartialResultTermination,
    pub fields: Vec<PartialResultFieldRecord>,
    pub measured_order: Vec<String>,
    pub measured_null_order: Vec<String>,
    pub censored_order: Vec<String>,
    pub interrupted_order: Vec<String>,
    pub failed_order: Vec<String>,
    pub unavailable_order: Vec<String>,
    pub redacted_order: Vec<String>,
    pub invalid_order: Vec<String>,
    pub operation_decisions: Vec<PartialResultOperationDecision>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub next_action: String,
    pub disposition: PartialResultDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PartialResultSemanticsError {
    #[error("partial-result request is invalid: {0}")]
    InvalidRequest(String),
    #[error("partial-result output is invalid: {0}")]
    InvalidOutput(String),
    #[error("partial-result digest failed: {0}")]
    Digest(String),
}

fn safe_text(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= MAX_NOTE_BYTES
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &PartialResultBundle) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "replay_identity": output.replay_identity,
        "termination": output.termination,
        "fields": output.fields,
        "measured_order": output.measured_order,
        "measured_null_order": output.measured_null_order,
        "censored_order": output.censored_order,
        "interrupted_order": output.interrupted_order,
        "failed_order": output.failed_order,
        "unavailable_order": output.unavailable_order,
        "redacted_order": output.redacted_order,
        "invalid_order": output.invalid_order,
        "operation_decisions": output.operation_decisions,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "next_action": output.next_action,
        "disposition": output.disposition,
    })
}

impl PartialResultBundle {
    pub fn validate(&self) -> Result<(), PartialResultSemanticsError> {
        let ids = self
            .fields
            .iter()
            .map(|field| field.field_id.clone())
            .collect::<BTreeSet<_>>();
        let all_orders = [
            &self.measured_order,
            &self.measured_null_order,
            &self.censored_order,
            &self.interrupted_order,
            &self.failed_order,
            &self.unavailable_order,
            &self.redacted_order,
            &self.invalid_order,
        ];
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !safe_text(&self.objective)
            || self.replay_identity.as_str().len() != 64
            || self.fields.is_empty()
            || self
                .fields
                .windows(2)
                .any(|pair| pair[0].field_id >= pair[1].field_id)
            || self.fields.iter().any(|field| {
                !safe_text(&field.field_id)
                    || !safe_text(&field.task_id)
                    || !safe_text(&field.output_schema)
                    || !safe_text(&field.reason)
                    || field
                        .artifact
                        .as_ref()
                        .is_some_and(|artifact| artifact.validate().is_err())
                    || (field.state == PartialResultFieldState::Measured
                        && field.artifact.is_none())
                    || (field.state != PartialResultFieldState::Measured
                        && field.artifact.is_some())
            })
            || all_orders.iter().any(|order| !canonical(order))
            || self
                .operation_decisions
                .windows(2)
                .any(|pair| pair[0].operation >= pair[1].operation)
            || self.operation_decisions.iter().any(|decision| {
                decision
                    .reason_order
                    .iter()
                    .any(|reason| !safe_text(reason))
                    || !canonical(&decision.reason_order)
            })
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || !safe_text(&self.next_action)
        {
            return Err(PartialResultSemanticsError::InvalidOutput(
                "identity, field ordering, state/artifact pairing, operation ordering, or evidence invariants are invalid".into(),
            ));
        }
        let mut seen = BTreeSet::new();
        for order in all_orders {
            for field_id in order {
                if !ids.contains(field_id) || !seen.insert(field_id.clone()) {
                    return Err(PartialResultSemanticsError::InvalidOutput(
                        "field state partitions do not reconcile".into(),
                    ));
                }
            }
        }
        if seen != ids {
            return Err(PartialResultSemanticsError::InvalidOutput(
                "every field must occur in exactly one state partition".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| PartialResultSemanticsError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(PartialResultSemanticsError::InvalidOutput(
                "partial-result digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn task_result<'a>(
    execution: &'a ComputationExecution,
    task_id: &str,
) -> Result<&'a super::execution::ComputationTaskResult, PartialResultSemanticsError> {
    execution
        .task_results
        .iter()
        .find(|result| result.task_id == task_id)
        .ok_or_else(|| {
            PartialResultSemanticsError::InvalidRequest(format!(
                "field references unknown computation task {task_id}"
            ))
        })
}

fn validate_request(
    request: &PartialResultBundleRequest,
) -> Result<(), PartialResultSemanticsError> {
    if !safe_text(&request.objective)
        || request.replay_identity.as_str().len() != 64
        || request.expected_fields.is_empty()
        || request.expected_fields.len() > MAX_FIELDS
        || request.observations.len() > MAX_FIELDS
        || request.execution.objective != request.objective
        || request.execution.replay_identity != request.replay_identity
    {
        return Err(PartialResultSemanticsError::InvalidRequest(
            "objective/replay binding, bounded expected fields, and a computation execution are required".into(),
        ));
    }
    request
        .execution
        .validate()
        .map_err(|error| PartialResultSemanticsError::InvalidRequest(error.to_string()))?;
    let mut expected_ids = BTreeSet::new();
    let mut expected_by_id = BTreeMap::new();
    for spec in &request.expected_fields {
        if !safe_text(&spec.field_id)
            || !safe_text(&spec.task_id)
            || !safe_text(&spec.output_schema)
            || !expected_ids.insert(spec.field_id.clone())
        {
            return Err(PartialResultSemanticsError::InvalidRequest(
                "expected fields must have unique bounded identities and schemas".into(),
            ));
        }
        let result = task_result(&request.execution, &spec.task_id)?;
        if result.output_schema != spec.output_schema {
            return Err(PartialResultSemanticsError::InvalidRequest(format!(
                "field {} schema does not match task {}",
                spec.field_id, spec.task_id
            )));
        }
        expected_by_id.insert(spec.field_id.clone(), spec);
    }
    let mut observation_ids = BTreeSet::new();
    for observation in &request.observations {
        let spec = expected_by_id.get(&observation.field_id).ok_or_else(|| {
            PartialResultSemanticsError::InvalidRequest(format!(
                "observation references unknown field {}",
                observation.field_id
            ))
        })?;
        if !observation_ids.insert(observation.field_id.clone()) || !safe_text(&observation.note) {
            return Err(PartialResultSemanticsError::InvalidRequest(
                "observations must be unique and carry an explicit note".into(),
            ));
        }
        let result = task_result(&request.execution, &spec.task_id)?;
        let completed = matches!(
            result.disposition,
            ComputationTaskDisposition::Completed | ComputationTaskDisposition::Cached
        );
        if observation.state == PartialResultFieldState::Measured {
            if !completed || observation.artifact.is_none() {
                return Err(PartialResultSemanticsError::InvalidRequest(
                    "measured fields require a completed/cached task and an artifact".into(),
                ));
            }
            if observation
                .artifact
                .as_ref()
                .is_some_and(|artifact| artifact.content_type != spec.output_schema)
            {
                return Err(PartialResultSemanticsError::InvalidRequest(
                    "measured artifact schema does not match the expected field schema".into(),
                ));
            }
        } else if observation.artifact.is_some() {
            return Err(PartialResultSemanticsError::InvalidRequest(
                "non-measured states cannot carry an artifact; measured null is not an artifact"
                    .into(),
            ));
        }
        if let Some(artifact) = &observation.artifact {
            artifact
                .validate()
                .map_err(|error| PartialResultSemanticsError::InvalidRequest(error.to_string()))?;
        }
        if observation.state == PartialResultFieldState::MeasuredNull
            && !request.policy.allow_measured_nulls
        {
            return Err(PartialResultSemanticsError::InvalidRequest(
                "policy disallows explicitly measured null fields".into(),
            ));
        }
    }
    Ok(())
}

fn inferred_state(
    result: &super::execution::ComputationTaskResult,
    termination: PartialResultTermination,
) -> (PartialResultFieldState, String) {
    match result.disposition {
        ComputationTaskDisposition::Completed | ComputationTaskDisposition::Cached => (
            PartialResultFieldState::Invalid,
            "completed task has no explicit typed field observation".into(),
        ),
        ComputationTaskDisposition::Negative => (
            PartialResultFieldState::Invalid,
            "negative task requires an explicit measured-null declaration".into(),
        ),
        ComputationTaskDisposition::Partial => (
            PartialResultFieldState::Censored,
            format!("field censored by {:?} task termination", termination),
        ),
        ComputationTaskDisposition::Failed => (
            PartialResultFieldState::Failed,
            "upstream computation task failed before this field was measured".into(),
        ),
        ComputationTaskDisposition::Skipped => (
            PartialResultFieldState::Unavailable,
            "upstream computation task was skipped; field was not measured".into(),
        ),
    }
}

fn operation_decisions(
    fields: &[PartialResultFieldRecord],
    policy: &PartialResultPolicy,
) -> Vec<PartialResultOperationDecision> {
    let required = fields.iter().filter(|field| field.required);
    let all_required_observed = required.clone().all(|field| {
        matches!(
            field.state,
            PartialResultFieldState::Measured | PartialResultFieldState::MeasuredNull
        )
    });
    let has_measured = fields
        .iter()
        .any(|field| field.state == PartialResultFieldState::Measured);
    let has_invalid = fields
        .iter()
        .any(|field| field.state == PartialResultFieldState::Invalid);
    let has_failure = fields.iter().any(|field| {
        matches!(
            field.state,
            PartialResultFieldState::Failed | PartialResultFieldState::Redacted
        )
    });
    let has_partial = fields.iter().any(|field| {
        matches!(
            field.state,
            PartialResultFieldState::Censored
                | PartialResultFieldState::Interrupted
                | PartialResultFieldState::Unavailable
        )
    });
    let has_censored = fields
        .iter()
        .any(|field| field.state == PartialResultFieldState::Censored);
    let partial_summary_allowed =
        policy.allow_partial_descriptive_summary && (!has_censored || policy.allow_censored_fields);
    let reason = |allowed: bool, base: &str| {
        if allowed {
            vec!["all required fields satisfy this operation's declared input contract".into()]
        } else {
            let mut reasons = vec![base.to_string()];
            if has_invalid {
                reasons.push("implicit or malformed field state is invalid".into());
            }
            if has_failure {
                reasons.push("failed or redacted fields cannot be treated as measurements".into());
            }
            if has_partial {
                reasons
                    .push("censored, interrupted, or unavailable fields remain incomplete".into());
            }
            reasons.sort();
            reasons.dedup();
            reasons
        }
    };
    let mut decisions = vec![
        PartialResultOperationDecision {
            operation: PartialResultOperation::MissingnessReview,
            allowed: true,
            reason_order: vec!["state and missingness review is always permitted".into()],
        },
        PartialResultOperationDecision {
            operation: PartialResultOperation::DescriptiveSummary,
            allowed: !has_invalid
                && !has_failure
                && (!has_partial || partial_summary_allowed),
            reason_order: reason(
                !has_invalid
                    && !has_failure
                    && (!has_partial || partial_summary_allowed),
                "descriptive summary requires explicit, non-failed field states",
            ),
        },
        PartialResultOperationDecision {
            operation: PartialResultOperation::ModelFit,
            allowed: all_required_observed && !has_invalid,
            reason_order: reason(
                all_required_observed && !has_invalid,
                "model fitting requires every required field to be measured or measured-null",
            ),
        },
        PartialResultOperationDecision {
            operation: PartialResultOperation::MechanismInference,
            allowed: all_required_observed && has_measured && !has_invalid,
            reason_order: reason(
                all_required_observed && has_measured && !has_invalid,
                "mechanism inference requires complete required coverage and at least one measured field",
            ),
        },
        PartialResultOperationDecision {
            operation: PartialResultOperation::Publication,
            allowed: policy.allow_publication && all_required_observed && !has_invalid && !has_failure,
            reason_order: reason(
                policy.allow_publication && all_required_observed && !has_invalid && !has_failure,
                "publication requires policy approval and complete required coverage",
            ),
        },
    ];
    for decision in &mut decisions {
        decision.reason_order.sort();
        decision.reason_order.dedup();
    }
    decisions
}

/// Compile explicit computation outcomes into a missingness-safe result bundle.
pub fn compile_glioma_partial_result_bundle(
    request: &PartialResultBundleRequest,
) -> Result<PartialResultBundle, PartialResultSemanticsError> {
    validate_request(request)?;
    let spec_by_id = request
        .expected_fields
        .iter()
        .map(|spec| (spec.field_id.clone(), spec))
        .collect::<BTreeMap<_, _>>();
    let observation_by_id = request
        .observations
        .iter()
        .map(|observation| (observation.field_id.clone(), observation))
        .collect::<BTreeMap<_, _>>();
    let result_by_task = request
        .execution
        .task_results
        .iter()
        .map(|result| (result.task_id.clone(), result))
        .collect::<BTreeMap<_, _>>();
    let fields = spec_by_id
        .values()
        .map(|spec| {
            if let Some(observation) = observation_by_id.get(&spec.field_id) {
                (
                    spec.field_id.clone(),
                    PartialResultFieldRecord {
                        field_id: spec.field_id.clone(),
                        task_id: spec.task_id.clone(),
                        output_schema: spec.output_schema.clone(),
                        required: spec.required,
                        state: observation.state,
                        artifact: observation.artifact.clone(),
                        reason: observation.note.clone(),
                    },
                )
            } else {
                let result = result_by_task
                    .get(&spec.task_id)
                    .expect("validated task identity");
                let (state, reason) = inferred_state(result, request.termination);
                (
                    spec.field_id.clone(),
                    PartialResultFieldRecord {
                        field_id: spec.field_id.clone(),
                        task_id: spec.task_id.clone(),
                        output_schema: spec.output_schema.clone(),
                        required: spec.required,
                        state,
                        artifact: None,
                        reason,
                    },
                )
            }
        })
        .collect::<BTreeMap<_, _>>();
    let fields = fields.values().cloned().collect::<Vec<_>>();
    let by_state = |state| {
        fields
            .iter()
            .filter(|field| field.state == state)
            .map(|field| field.field_id.clone())
            .collect::<Vec<_>>()
    };
    let measured_order = by_state(PartialResultFieldState::Measured);
    let measured_null_order = by_state(PartialResultFieldState::MeasuredNull);
    let censored_order = by_state(PartialResultFieldState::Censored);
    let interrupted_order = by_state(PartialResultFieldState::Interrupted);
    let failed_order = by_state(PartialResultFieldState::Failed);
    let unavailable_order = by_state(PartialResultFieldState::Unavailable);
    let redacted_order = by_state(PartialResultFieldState::Redacted);
    let invalid_order = by_state(PartialResultFieldState::Invalid);
    let operation_decisions = operation_decisions(&fields, &request.policy);
    let mut negative_evidence = fields
        .iter()
        .filter(|field| {
            matches!(
                field.state,
                PartialResultFieldState::Censored
                    | PartialResultFieldState::Interrupted
                    | PartialResultFieldState::Failed
                    | PartialResultFieldState::Unavailable
                    | PartialResultFieldState::Redacted
                    | PartialResultFieldState::Invalid
            )
        })
        .map(|field| format!("{}: {}", field.field_id, field.reason))
        .collect::<Vec<_>>();
    negative_evidence.sort();
    negative_evidence.dedup();
    let mut uncertainty = negative_evidence.clone();
    if !measured_null_order.is_empty() {
        uncertainty.push(
            "measured-null fields are valid observations but do not supply effect magnitude".into(),
        );
    }
    uncertainty.sort();
    uncertainty.dedup();
    let disposition = if !invalid_order.is_empty() {
        PartialResultDisposition::Blocked
    } else if !failed_order.is_empty() {
        PartialResultDisposition::Failed
    } else if !censored_order.is_empty()
        || !interrupted_order.is_empty()
        || !unavailable_order.is_empty()
        || !redacted_order.is_empty()
    {
        PartialResultDisposition::Partial
    } else if measured_order.is_empty() && !measured_null_order.is_empty() {
        PartialResultDisposition::NullOnly
    } else {
        PartialResultDisposition::Complete
    };
    let next_action = if disposition == PartialResultDisposition::Complete
        || disposition == PartialResultDisposition::NullOnly
    {
        "route only the explicitly allowed operations to interpretation; retain nulls as measured outcomes".into()
    } else {
        "resolve, reacquire, or explicitly censor incomplete fields before model fitting or publication".into()
    };
    let mut output = PartialResultBundle {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        replay_identity: request.replay_identity.clone(),
        termination: request.termination,
        fields,
        measured_order,
        measured_null_order,
        censored_order,
        interrupted_order,
        failed_order,
        unavailable_order,
        redacted_order,
        invalid_order,
        operation_decisions,
        negative_evidence,
        uncertainty,
        next_action,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-partial-result-bundle"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| PartialResultSemanticsError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::super::execution::{
        ComputationExecutionDisposition, ComputationExecutionStopReason, ComputationTaskResult,
    };
    use super::*;
    use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};

    fn hash(seed: &str) -> ContentHash {
        ContentHash::of_bytes(seed.as_bytes())
    }

    fn artifact(schema: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: "local-output".into(),
            content_hash: hash("output"),
            content_type: schema.into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn execution(disposition: ComputationTaskDisposition) -> ComputationExecution {
        let result = ComputationTaskResult {
            task_id: "quantify".into(),
            output_schema: "GliomaQuantification1@1".into(),
            disposition,
            attempt_count: if disposition == ComputationTaskDisposition::Skipped {
                0
            } else {
                1
            },
            artifact: if matches!(
                disposition,
                ComputationTaskDisposition::Completed | ComputationTaskDisposition::Cached
            ) {
                Some(artifact("GliomaQuantification1@1"))
            } else {
                None
            },
            cache_hit: disposition == ComputationTaskDisposition::Cached,
            note: "typed local result".into(),
        };
        let mut output = ComputationExecution {
            feature_id: super::super::execution::FEATURE_ID.into(),
            output_schema: super::super::execution::OUTPUT_SCHEMA.into(),
            objective: "measure invasion".into(),
            model_system: GliomaModelSystem::Organoid,
            replay_identity: hash("replay"),
            task_order: vec!["quantify".into()],
            task_results: vec![result],
            completed_order: if disposition == ComputationTaskDisposition::Completed {
                vec!["quantify".into()]
            } else {
                Vec::new()
            },
            cached_order: if disposition == ComputationTaskDisposition::Cached {
                vec!["quantify".into()]
            } else {
                Vec::new()
            },
            negative_order: if disposition == ComputationTaskDisposition::Negative {
                vec!["quantify".into()]
            } else {
                Vec::new()
            },
            partial_order: if disposition == ComputationTaskDisposition::Partial {
                vec!["quantify".into()]
            } else {
                Vec::new()
            },
            failed_order: if disposition == ComputationTaskDisposition::Failed {
                vec!["quantify".into()]
            } else {
                Vec::new()
            },
            skipped_order: if disposition == ComputationTaskDisposition::Skipped {
                vec!["quantify".into()]
            } else {
                Vec::new()
            },
            budget_used_units: 1,
            duration_used_ticks: 1,
            cache_hit_count: u32::from(disposition == ComputationTaskDisposition::Cached),
            uncertainty: Vec::new(),
            negative_evidence: Vec::new(),
            disposition: match disposition {
                ComputationTaskDisposition::Completed | ComputationTaskDisposition::Cached => {
                    ComputationExecutionDisposition::Completed
                }
                ComputationTaskDisposition::Partial => ComputationExecutionDisposition::Partial,
                ComputationTaskDisposition::Failed | ComputationTaskDisposition::Negative => {
                    ComputationExecutionDisposition::Failed
                }
                ComputationTaskDisposition::Skipped => ComputationExecutionDisposition::Blocked,
            },
            stop_reason: match disposition {
                ComputationTaskDisposition::Completed | ComputationTaskDisposition::Cached => {
                    ComputationExecutionStopReason::Completed
                }
                ComputationTaskDisposition::Partial => {
                    ComputationExecutionStopReason::BudgetExhausted
                }
                ComputationTaskDisposition::Failed | ComputationTaskDisposition::Negative => {
                    ComputationExecutionStopReason::TaskFailed
                }
                ComputationTaskDisposition::Skipped => {
                    ComputationExecutionStopReason::DependencyBlocked
                }
            },
            digest: hash("unsealed"),
        };
        output.digest = ContentHash::of_value(&super::super::execution::digest_input(&output))
            .expect("execution digest");
        output.validate().expect("valid execution");
        output
    }

    fn request(
        execution: ComputationExecution,
        observations: Vec<PartialResultFieldObservation>,
        termination: PartialResultTermination,
    ) -> PartialResultBundleRequest {
        let replay_identity = execution.replay_identity.clone();
        PartialResultBundleRequest {
            objective: "measure invasion".into(),
            replay_identity,
            execution,
            expected_fields: vec![PartialResultFieldSpec {
                field_id: "invasion_score".into(),
                task_id: "quantify".into(),
                output_schema: "GliomaQuantification1@1".into(),
                required: true,
            }],
            observations,
            termination,
            policy: PartialResultPolicy {
                allow_partial_descriptive_summary: true,
                allow_measured_nulls: true,
                allow_censored_fields: false,
                allow_publication: true,
            },
        }
    }

    #[test]
    fn measured_null_is_not_unavailable() {
        let execution = execution(ComputationTaskDisposition::Completed);
        let output = compile_glioma_partial_result_bundle(&request(
            execution,
            vec![PartialResultFieldObservation {
                field_id: "invasion_score".into(),
                state: PartialResultFieldState::MeasuredNull,
                artifact: None,
                note: "no invasion detected under the prespecified threshold".into(),
            }],
            PartialResultTermination::Completed,
        ))
        .expect("bundle");
        assert_eq!(output.disposition, PartialResultDisposition::NullOnly);
        assert_eq!(output.measured_null_order, vec!["invasion_score"]);
        assert!(output.unavailable_order.is_empty());
        assert!(output
            .operation_decisions
            .iter()
            .any(
                |decision| decision.operation == PartialResultOperation::ModelFit
                    && decision.allowed
            ));
    }

    #[test]
    fn budget_censoring_blocks_model_fit_but_allows_review() {
        let output = compile_glioma_partial_result_bundle(&request(
            execution(ComputationTaskDisposition::Partial),
            Vec::new(),
            PartialResultTermination::BudgetExhausted,
        ))
        .expect("bundle");
        assert_eq!(output.disposition, PartialResultDisposition::Partial);
        assert_eq!(output.censored_order, vec!["invasion_score"]);
        assert!(output
            .operation_decisions
            .iter()
            .any(
                |decision| decision.operation == PartialResultOperation::MissingnessReview
                    && decision.allowed
            ));
        assert!(output
            .operation_decisions
            .iter()
            .any(
                |decision| decision.operation == PartialResultOperation::ModelFit
                    && !decision.allowed
            ));
    }

    #[test]
    fn measured_artifact_is_replay_stable() {
        let execution = execution(ComputationTaskDisposition::Completed);
        let request = request(
            execution,
            vec![PartialResultFieldObservation {
                field_id: "invasion_score".into(),
                state: PartialResultFieldState::Measured,
                artifact: Some(artifact("GliomaQuantification1@1")),
                note: "local quantified output".into(),
            }],
            PartialResultTermination::Completed,
        );
        let left = compile_glioma_partial_result_bundle(&request).expect("left");
        let right = compile_glioma_partial_result_bundle(&request).expect("right");
        assert_eq!(left.digest, right.digest);
        assert_eq!(left.disposition, PartialResultDisposition::Complete);
    }

    #[test]
    fn implicit_negative_outcome_is_rejected_as_invalid_not_null() {
        let output = compile_glioma_partial_result_bundle(&request(
            execution(ComputationTaskDisposition::Negative),
            Vec::new(),
            PartialResultTermination::Failed,
        ))
        .expect("bundle");
        assert_eq!(output.disposition, PartialResultDisposition::Blocked);
        assert_eq!(output.invalid_order, vec!["invasion_score"]);
        assert!(output.measured_null_order.is_empty());
    }
}

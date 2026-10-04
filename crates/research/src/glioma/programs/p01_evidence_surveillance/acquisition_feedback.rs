//! Idempotent assimilation of site-local acquisition outcomes into P01 evidence state.
//!
//! Acquisition policy and execution are separate authorities.  This module accepts only an
//! explicit outcome envelope from a local adapter, verifies that it corresponds to a planned
//! action, and turns completed or negative outcomes into typed evidence records for the frontier
//! join. Failures, cancellations, unauthorized sites, duplicates, and protected artifacts remain
//! visible but never become scientific support.

use super::federated_acquisition_policy::FederatedAcquisitionAction;
use crate::glioma::evidence::{EvidenceRecord, EvidenceSourceKind, EvidenceState};
use crate::glioma_engine::{GliomaModality, GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P01-F15";
pub const OUTPUT_SCHEMA: &str = "GliomaEvidenceAcquisitionFeedback1@1";
pub const MAX_OUTCOMES: usize = 4_096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcquisitionOutcomeStatus {
    Completed,
    Negative,
    Failed,
    Cancelled,
    Expired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcquisitionFeedbackDecision {
    AcceptedEvidence,
    AcceptedNegative,
    RejectedUnauthorized,
    RejectedInvalidArtifact,
    RejectedDuplicate,
    RecordedFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcquisitionOutcome {
    pub outcome_id: String,
    pub action_id: String,
    pub need_id: String,
    pub site_id: String,
    pub evidence_id: String,
    pub claim: String,
    pub scope: String,
    pub modality: GliomaModality,
    pub model_system: Option<GliomaModelSystem>,
    pub source_kind: EvidenceSourceKind,
    pub artifact: Option<LocalArtifactRef>,
    pub status: AcquisitionOutcomeStatus,
    pub quality_milli: u16,
    pub reproducibility_milli: u16,
    pub relevance_milli: u16,
    pub release_epoch: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcquisitionFeedbackRequest {
    pub objective: String,
    pub planned_actions: Vec<FederatedAcquisitionAction>,
    pub outcomes: Vec<AcquisitionOutcome>,
    pub expected_epoch: u32,
    pub min_quality_milli: u16,
    pub min_reproducibility_milli: u16,
    pub require_local_raw_data: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcquisitionFeedbackRow {
    pub outcome_id: String,
    pub action_id: String,
    pub decision: AcquisitionFeedbackDecision,
    pub reason: String,
    pub evidence_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcquisitionFeedbackReport {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub accepted_evidence: Vec<EvidenceRecord>,
    pub accepted_order: Vec<String>,
    pub negative_order: Vec<String>,
    pub failure_order: Vec<String>,
    pub rejected_order: Vec<String>,
    pub duplicate_order: Vec<String>,
    pub rows: Vec<AcquisitionFeedbackRow>,
    pub next_frontier_order: Vec<String>,
    pub uncertainty: Vec<String>,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AcquisitionFeedbackError {
    #[error("acquisition feedback request is invalid: {0}")]
    InvalidRequest(String),
    #[error("acquisition feedback output is invalid: {0}")]
    InvalidOutput(String),
    #[error("acquisition feedback digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &AcquisitionFeedbackReport) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "accepted_evidence": output.accepted_evidence,
        "accepted_order": output.accepted_order,
        "negative_order": output.negative_order,
        "failure_order": output.failure_order,
        "rejected_order": output.rejected_order,
        "duplicate_order": output.duplicate_order,
        "rows": output.rows,
        "next_frontier_order": output.next_frontier_order,
        "uncertainty": output.uncertainty,
    })
}

impl AcquisitionFeedbackReport {
    pub fn validate(&self) -> Result<(), AcquisitionFeedbackError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || !canonical(&self.accepted_order)
            || !canonical(&self.negative_order)
            || !canonical(&self.failure_order)
            || !canonical(&self.rejected_order)
            || !canonical(&self.duplicate_order)
            || !canonical(&self.next_frontier_order)
            || !canonical(&self.uncertainty)
            || !canonical(
                &self
                    .rows
                    .iter()
                    .map(|row| row.outcome_id.clone())
                    .collect::<Vec<_>>(),
            )
            || self.accepted_order
                != self
                    .accepted_evidence
                    .iter()
                    .map(|record| record.evidence_id.clone())
                    .collect::<Vec<_>>()
            || self.rows.iter().any(|row| {
                row.outcome_id.trim().is_empty()
                    || row.action_id.trim().is_empty()
                    || row.reason.trim().is_empty()
            })
            || self.digest.as_str().len() != 64
        {
            return Err(AcquisitionFeedbackError::InvalidOutput(
                "identity, canonical partitions, evidence order, rows, or digest is invalid".into(),
            ));
        }
        let row_ids = self
            .rows
            .iter()
            .map(|row| row.outcome_id.clone())
            .collect::<BTreeSet<_>>();
        let accepted_rows = self
            .rows
            .iter()
            .filter(|row| {
                matches!(
                    row.decision,
                    AcquisitionFeedbackDecision::AcceptedEvidence
                        | AcquisitionFeedbackDecision::AcceptedNegative
                )
            })
            .count();
        let negative_rows = self
            .rows
            .iter()
            .filter(|row| row.decision == AcquisitionFeedbackDecision::AcceptedNegative)
            .count();
        let failure_rows = self
            .rows
            .iter()
            .filter(|row| row.decision == AcquisitionFeedbackDecision::RecordedFailure)
            .count();
        let rejected_rows = self
            .rows
            .iter()
            .filter(|row| {
                matches!(
                    row.decision,
                    AcquisitionFeedbackDecision::RejectedUnauthorized
                        | AcquisitionFeedbackDecision::RejectedInvalidArtifact
                )
            })
            .count();
        let duplicate_rows = self
            .rows
            .iter()
            .filter(|row| row.decision == AcquisitionFeedbackDecision::RejectedDuplicate)
            .count();
        if row_ids.len() != self.rows.len()
            || accepted_rows != self.accepted_order.len()
            || negative_rows != self.negative_order.len()
            || failure_rows != self.failure_order.len()
            || rejected_rows != self.rejected_order.len()
            || duplicate_rows != self.duplicate_order.len()
            || !self.next_frontier_order.iter().all(|id| {
                self.accepted_order.binary_search(id).is_ok()
                    || self.negative_order.binary_search(id).is_ok()
            })
        {
            return Err(AcquisitionFeedbackError::InvalidOutput(
                "outcome rows or next-frontier partitions are inconsistent".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| AcquisitionFeedbackError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(AcquisitionFeedbackError::Digest(
                "feedback digest does not match canonical content".into(),
            ));
        }
        Ok(())
    }
}

/// Assimilate local acquisition outcomes without granting the executor authority to invent
/// evidence. Only completed/negative outcomes with a valid local artifact enter the frontier.
pub fn assimilate_glioma_acquisition_feedback(
    request: &AcquisitionFeedbackRequest,
) -> Result<AcquisitionFeedbackReport, AcquisitionFeedbackError> {
    if request.objective.trim().is_empty()
        || request.planned_actions.is_empty()
        || request.outcomes.is_empty()
        || request.outcomes.len() > MAX_OUTCOMES
        || request.expected_epoch == 0
        || request.min_quality_milli > 1_000
        || request.min_reproducibility_milli > 1_000
    {
        return Err(AcquisitionFeedbackError::InvalidRequest(
            "objective, plans, outcomes, epoch, or quality floors are invalid".into(),
        ));
    }
    let mut planned = std::collections::BTreeMap::new();
    for action in &request.planned_actions {
        if action.action_id.trim().is_empty()
            || action.need_id.trim().is_empty()
            || action.site_id.trim().is_empty()
            || !action.local_raw_data_required
            || planned.insert(action.action_id.clone(), action).is_some()
        {
            return Err(AcquisitionFeedbackError::InvalidRequest(
                "planned actions must be unique, typed, and local-only".into(),
            ));
        }
    }
    let mut outcome_ids = BTreeSet::new();
    let mut evidence_ids = BTreeSet::new();
    let mut accepted_evidence = Vec::new();
    let mut accepted_order = BTreeSet::new();
    let mut negative_order = BTreeSet::new();
    let mut failure_order = BTreeSet::new();
    let mut rejected_order = BTreeSet::new();
    let mut duplicate_order = BTreeSet::new();
    let mut rows = Vec::new();
    let mut uncertainty = BTreeSet::new();

    for outcome in &request.outcomes {
        let duplicate = !outcome_ids.insert(outcome.outcome_id.clone());
        let Some(action) = planned.get(&outcome.action_id) else {
            rows.push(AcquisitionFeedbackRow {
                outcome_id: outcome.outcome_id.clone(),
                action_id: outcome.action_id.clone(),
                decision: AcquisitionFeedbackDecision::RejectedUnauthorized,
                reason: "outcome action is absent from the immutable plan".into(),
                evidence_id: None,
            });
            rejected_order.insert(outcome.outcome_id.clone());
            continue;
        };
        let unauthorized = outcome.site_id != action.site_id
            || outcome.need_id != action.need_id
            || outcome
                .artifact
                .as_ref()
                .map(|artifact| !artifact.local_only)
                .unwrap_or(false)
            || outcome
                .artifact
                .as_ref()
                .map(|artifact| {
                    artifact.contains_human_data || artifact.contains_direct_identifiers
                })
                .unwrap_or(false)
            || (request.require_local_raw_data
                && outcome
                    .artifact
                    .as_ref()
                    .map(|artifact| !artifact.local_only)
                    .unwrap_or(false));
        if duplicate {
            duplicate_order.insert(outcome.outcome_id.clone());
            rows.push(AcquisitionFeedbackRow {
                outcome_id: outcome.outcome_id.clone(),
                action_id: outcome.action_id.clone(),
                decision: AcquisitionFeedbackDecision::RejectedDuplicate,
                reason: "outcome id is already present in this idempotent batch".into(),
                evidence_id: None,
            });
            continue;
        }
        if unauthorized {
            rejected_order.insert(outcome.outcome_id.clone());
            uncertainty.insert(format!(
                "{}: site, need, or artifact locality did not match the plan",
                outcome.outcome_id
            ));
            rows.push(AcquisitionFeedbackRow {
                outcome_id: outcome.outcome_id.clone(),
                action_id: outcome.action_id.clone(),
                decision: AcquisitionFeedbackDecision::RejectedUnauthorized,
                reason: "site/need binding or local preclinical artifact policy failed".into(),
                evidence_id: None,
            });
            continue;
        }
        if !matches!(
            outcome.status,
            AcquisitionOutcomeStatus::Completed | AcquisitionOutcomeStatus::Negative
        ) {
            failure_order.insert(outcome.outcome_id.clone());
            uncertainty.insert(format!(
                "{}: execution status is {:?} and is not evidence",
                outcome.outcome_id, outcome.status
            ));
            rows.push(AcquisitionFeedbackRow {
                outcome_id: outcome.outcome_id.clone(),
                action_id: outcome.action_id.clone(),
                decision: AcquisitionFeedbackDecision::RecordedFailure,
                reason: "failed, cancelled, or expired acquisition remains non-evidence".into(),
                evidence_id: None,
            });
            continue;
        }
        let Some(artifact) = outcome.artifact.clone() else {
            rejected_order.insert(outcome.outcome_id.clone());
            rows.push(AcquisitionFeedbackRow {
                outcome_id: outcome.outcome_id.clone(),
                action_id: outcome.action_id.clone(),
                decision: AcquisitionFeedbackDecision::RejectedInvalidArtifact,
                reason: "completed outcome has no content-addressed local artifact".into(),
                evidence_id: None,
            });
            continue;
        };
        if artifact.content_hash.as_str().len() != 64
            || outcome.quality_milli < request.min_quality_milli
            || outcome.reproducibility_milli < request.min_reproducibility_milli
            || !evidence_ids.insert(outcome.evidence_id.clone())
        {
            rejected_order.insert(outcome.outcome_id.clone());
            rows.push(AcquisitionFeedbackRow {
                outcome_id: outcome.outcome_id.clone(),
                action_id: outcome.action_id.clone(),
                decision: AcquisitionFeedbackDecision::RejectedInvalidArtifact,
                reason: "artifact identity or evidence quality floor failed".into(),
                evidence_id: None,
            });
            continue;
        }
        let state = if outcome.status == AcquisitionOutcomeStatus::Negative {
            EvidenceState::Negative
        } else {
            EvidenceState::Supported
        };
        let record = EvidenceRecord {
            evidence_id: outcome.evidence_id.clone(),
            source_artifact: artifact,
            source_kind: outcome.source_kind,
            claim: outcome.claim.clone(),
            scope: outcome.scope.clone(),
            modality: outcome.modality,
            model_system: outcome.model_system,
            state,
            relevance_milli: outcome.relevance_milli,
            quality_milli: outcome.quality_milli,
            reproducibility_milli: outcome.reproducibility_milli,
            release_epoch: outcome.release_epoch.min(request.expected_epoch),
        };
        accepted_evidence.push(record);
        let decision = if state == EvidenceState::Negative {
            accepted_order.insert(outcome.evidence_id.clone());
            negative_order.insert(outcome.evidence_id.clone());
            AcquisitionFeedbackDecision::AcceptedNegative
        } else {
            accepted_order.insert(outcome.evidence_id.clone());
            AcquisitionFeedbackDecision::AcceptedEvidence
        };
        rows.push(AcquisitionFeedbackRow {
            outcome_id: outcome.outcome_id.clone(),
            action_id: outcome.action_id.clone(),
            decision,
            reason: "local content-addressed outcome is eligible for the next evidence frontier"
                .into(),
            evidence_id: Some(outcome.evidence_id.clone()),
        });
    }
    accepted_evidence.sort_by(|left, right| left.evidence_id.cmp(&right.evidence_id));
    rows.sort_by(|left, right| left.outcome_id.cmp(&right.outcome_id));
    let accepted_order = accepted_order.into_iter().collect::<Vec<_>>();
    let negative_order = negative_order.into_iter().collect::<Vec<_>>();
    let next_frontier_order = accepted_order
        .iter()
        .chain(negative_order.iter())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut output = AcquisitionFeedbackReport {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        accepted_evidence,
        accepted_order,
        negative_order,
        failure_order: failure_order.into_iter().collect(),
        rejected_order: rejected_order.into_iter().collect(),
        duplicate_order: duplicate_order.into_iter().collect(),
        rows,
        next_frontier_order,
        uncertainty: uncertainty.into_iter().collect(),
        digest: ContentHash::of_value(&serde_json::Value::Null)
            .map_err(|error| AcquisitionFeedbackError::Digest(error.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| AcquisitionFeedbackError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn action() -> FederatedAcquisitionAction {
        FederatedAcquisitionAction {
            action_id: "need-1::site-a".into(),
            need_id: "need-1".into(),
            site_id: "site-a".into(),
            independence_group: "north".into(),
            estimated_cost_units: 10,
            privacy_risk_milli: 10,
            score_milli: 900,
            rank: 1,
            local_raw_data_required: true,
        }
    }

    fn outcome(status: AcquisitionOutcomeStatus, evidence_id: &str) -> AcquisitionOutcome {
        AcquisitionOutcome {
            outcome_id: format!("outcome-{evidence_id}"),
            action_id: "need-1::site-a".into(),
            need_id: "need-1".into(),
            site_id: "site-a".into(),
            evidence_id: evidence_id.into(),
            claim: "egfr resistance".into(),
            scope: "organoid".into(),
            modality: GliomaModality::FunctionalPerturbation,
            model_system: Some(GliomaModelSystem::Organoid),
            source_kind: EvidenceSourceKind::Assay,
            artifact: Some(LocalArtifactRef {
                artifact_id: format!("artifact-{evidence_id}"),
                content_hash: ContentHash::of_bytes(evidence_id.as_bytes()),
                content_type: "application/json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            }),
            status,
            quality_milli: 900,
            reproducibility_milli: 900,
            relevance_milli: 900,
            release_epoch: 2,
        }
    }

    fn request(outcomes: Vec<AcquisitionOutcome>) -> AcquisitionFeedbackRequest {
        AcquisitionFeedbackRequest {
            objective: "assimilate local acquisition outcomes".into(),
            planned_actions: vec![action()],
            outcomes,
            expected_epoch: 3,
            min_quality_milli: 700,
            min_reproducibility_milli: 700,
            require_local_raw_data: true,
        }
    }

    #[test]
    fn accepts_completed_outcome_for_next_frontier() {
        let output = assimilate_glioma_acquisition_feedback(&request(vec![outcome(
            AcquisitionOutcomeStatus::Completed,
            "evidence-a",
        )]))
        .unwrap();
        assert_eq!(output.accepted_order, vec!["evidence-a"]);
        assert!(output.failure_order.is_empty());
        output.validate().unwrap();
    }

    #[test]
    fn preserves_negative_outcome_without_promoting_it_to_support() {
        let output = assimilate_glioma_acquisition_feedback(&request(vec![outcome(
            AcquisitionOutcomeStatus::Negative,
            "evidence-negative",
        )]))
        .unwrap();
        assert_eq!(output.negative_order, vec!["evidence-negative"]);
        assert_eq!(output.accepted_order, vec!["evidence-negative"]);
        assert_eq!(output.accepted_evidence[0].state, EvidenceState::Negative);
    }

    #[test]
    fn failures_and_unauthorized_sites_never_become_evidence() {
        let failed = outcome(AcquisitionOutcomeStatus::Failed, "evidence-failed");
        let mut unauthorized =
            outcome(AcquisitionOutcomeStatus::Completed, "evidence-unauthorized");
        unauthorized.site_id = "site-other".into();
        let output =
            assimilate_glioma_acquisition_feedback(&request(vec![failed.clone(), unauthorized]))
                .unwrap();
        assert!(output.accepted_evidence.is_empty());
        assert_eq!(output.failure_order, vec![failed.outcome_id]);
        assert_eq!(output.rejected_order, vec!["outcome-evidence-unauthorized"]);
    }
}

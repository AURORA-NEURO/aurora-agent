//! Multi-study typed decision-context artifact for preclinical glioma research.
//!
//! This feature aligns the portable P04 context artifacts produced by independent studies. It
//! exchanges only typed metadata and action contracts, not raw evidence. An action is promoted to
//! the shared frontier only when its definition is compatible, its support spans the configured
//! number of studies and independent groups, and its omissions/local outcomes remain explicit. The
//! matching BioPRISM blueprint source is not bundled in this checkout, so the typed outcome
//! vocabulary follows the adjacent P04 replay contract rather than claiming unavailable spec text.

use super::context_replay::{DecisionContextActionOutcome, DecisionContextActionOutcomeStatus};
use super::decision_context_artifact::{
    DecisionContextArtifact, DecisionContextArtifactAction, DecisionContextArtifactCompatibility,
};
use bioprism_foundation::PRECLINICAL_BOUNDARY;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P04-F06";
pub const OUTPUT_SCHEMA: &str = "GliomaMultiStudyDecisionContextArtifact1@2";
pub const MAX_STUDIES: usize = 256;
pub const MAX_ACTIONS: usize = 512;
pub const MAX_PARTITION_ITEMS: usize = 8_192;
pub const MAX_OUTCOME_ROWS: usize = 32_768;
pub const MAX_REQUEST_BYTES: usize = 8_000_000;
pub const MAX_REPORT_BYTES: usize = 16_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultiStudyActionDisposition {
    Qualified,
    Underpowered,
    Conflicted,
    Adverse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultiStudyContextDisposition {
    Qualified,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiStudyContextInput {
    pub study_id: String,
    pub independent_group: String,
    pub quality_milli: u16,
    pub policy_allowed: bool,
    pub artifact: DecisionContextArtifact,
    /// Explicit local observations only; omission means no outcome was supplied.
    #[serde(default)]
    pub outcome_order: Vec<DecisionContextActionOutcome>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiStudyContextRequest {
    pub objective: String,
    pub epoch: u32,
    pub minimum_studies: usize,
    pub minimum_independent_groups: usize,
    pub minimum_action_support_milli: u16,
    pub minimum_quality_milli: u16,
    pub maximum_actions: usize,
    pub compatibility: DecisionContextArtifactCompatibility,
    pub studies: Vec<MultiStudyContextInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiStudyDecisionAction {
    pub action: DecisionContextArtifactAction,
    pub study_order: Vec<String>,
    pub independent_group_order: Vec<String>,
    pub support_milli: u16,
    pub disagreement_milli: u16,
    pub disposition: MultiStudyActionDisposition,
    pub completed_study_order: Vec<String>,
    pub negative_study_order: Vec<String>,
    pub failed_study_order: Vec<String>,
    pub blocked_study_order: Vec<String>,
    pub unknown_study_order: Vec<String>,
}

/// A result reference shared from one eligible study without transferring its underlying data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiStudyActionOutcome {
    pub action_id: String,
    pub study_id: String,
    pub status: DecisionContextActionOutcomeStatus,
    pub result_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiStudyDecisionContextArtifact {
    pub feature_id: String,
    pub output_schema: String,
    pub artifact_id: String,
    pub objective: String,
    pub epoch: u32,
    pub boundary: String,
    pub study_order: Vec<String>,
    pub study_group: BTreeMap<String, String>,
    pub eligible_study_order: Vec<String>,
    pub omitted_study_order: Vec<String>,
    pub action_order: Vec<String>,
    pub frontier_order: Vec<String>,
    pub actions: Vec<MultiStudyDecisionAction>,
    /// Canonical eligible-study result ledger, including outcomes for actions omitted by capacity.
    pub outcome_order: Vec<MultiStudyActionOutcome>,
    pub omissions: BTreeMap<String, String>,
    pub negative_evidence_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub disposition: MultiStudyContextDisposition,
    pub compatibility: DecisionContextArtifactCompatibility,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MultiStudyContextError {
    #[error("multi-study context request is invalid: {0}")]
    InvalidRequest(String),
    #[error("multi-study context input is invalid: {0}")]
    InvalidInput(String),
    #[error("multi-study context artifact is invalid: {0}")]
    InvalidOutput(String),
    #[error("multi-study context digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn ranked_unique(values: &[String]) -> bool {
    let mut seen = BTreeSet::new();
    values
        .iter()
        .all(|value| !value.trim().is_empty() && seen.insert(value))
}

fn outcome_studies_for(
    outcomes: &[MultiStudyActionOutcome],
    action_id: &str,
    status: DecisionContextActionOutcomeStatus,
) -> Vec<String> {
    outcomes
        .iter()
        .filter(|outcome| outcome.action_id == action_id && outcome.status == status)
        .map(|outcome| outcome.study_id.clone())
        .collect()
}

fn has_adverse_outcome(outcomes: &[MultiStudyActionOutcome], action_id: &str) -> bool {
    outcomes.iter().any(|outcome| {
        outcome.action_id == action_id
            && outcome.status != DecisionContextActionOutcomeStatus::Completed
    })
}

fn outcome_evidence_key(outcome: &MultiStudyActionOutcome) -> String {
    format!(
        "{}:action:{}:{}:{}",
        outcome.study_id,
        outcome.action_id,
        match outcome.status {
            DecisionContextActionOutcomeStatus::Completed => "completed",
            DecisionContextActionOutcomeStatus::Negative => "negative",
            DecisionContextActionOutcomeStatus::Failed => "failed",
            DecisionContextActionOutcomeStatus::Blocked => "blocked",
            DecisionContextActionOutcomeStatus::Unknown => "unknown",
        },
        outcome.result_digest.as_str()
    )
}

fn digest_input(output: &MultiStudyDecisionContextArtifact) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "artifact_id": output.artifact_id,
        "objective": output.objective,
        "epoch": output.epoch,
        "boundary": output.boundary,
        "study_order": output.study_order,
        "study_group": output.study_group,
        "eligible_study_order": output.eligible_study_order,
        "omitted_study_order": output.omitted_study_order,
        "action_order": output.action_order,
        "frontier_order": output.frontier_order,
        "actions": output.actions,
        "outcome_order": output.outcome_order,
        "omissions": output.omissions,
        "negative_evidence_order": output.negative_evidence_order,
        "uncertainty_order": output.uncertainty_order,
        "disposition": output.disposition,
        "compatibility": output.compatibility,
    })
}

impl MultiStudyDecisionContextArtifact {
    pub fn validate(&self) -> Result<(), MultiStudyContextError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.artifact_id.trim().is_empty()
            || self.objective.trim().is_empty()
            || self.epoch == 0
            || self.boundary != PRECLINICAL_BOUNDARY
            || !canonical(&self.study_order)
            || self.study_group.keys().cloned().collect::<Vec<_>>() != self.study_order
            || self
                .study_group
                .values()
                .any(|group| group.trim().is_empty())
            || !canonical(&self.eligible_study_order)
            || !canonical(&self.omitted_study_order)
            || !canonical(&self.action_order)
            || !ranked_unique(&self.frontier_order)
            || !canonical(
                &self
                    .outcome_order
                    .iter()
                    .map(|outcome| (outcome.action_id.clone(), outcome.study_id.clone()))
                    .collect::<Vec<_>>(),
            )
            || !canonical(&self.negative_evidence_order)
            || !canonical(&self.uncertainty_order)
            || self.actions.len() != self.action_order.len()
            || self
                .actions
                .iter()
                .map(|entry| entry.action.action_id.clone())
                .collect::<Vec<_>>()
                != self.action_order
            || self.actions.len() > MAX_ACTIONS
            || self.outcome_order.len() > MAX_OUTCOME_ROWS
            || self.negative_evidence_order.len() > MAX_PARTITION_ITEMS
            || self.uncertainty_order.len() > MAX_PARTITION_ITEMS
            || self.compatibility.contract_version.trim().is_empty()
            || self.compatibility.consumer_order.is_empty()
            || !canonical(&self.compatibility.consumer_order)
            || !canonical(&self.compatibility.semantic_loss_order)
            || self.compatibility.local_raw_data_required
            || self.compatibility.clinical_decision_capable
            || self.digest.as_str().len() != 64
        {
            return Err(MultiStudyContextError::InvalidOutput(
                "identity, boundary, ordering, bounds, action partition, or digest fields are invalid".into(),
            ));
        }
        let study_set = self.study_order.iter().cloned().collect::<BTreeSet<_>>();
        let eligible_set = self
            .eligible_study_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let omitted_set = self
            .omitted_study_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        if eligible_set.intersection(&omitted_set).next().is_some()
            || eligible_set
                .union(&omitted_set)
                .cloned()
                .collect::<BTreeSet<_>>()
                != study_set
            || self.actions.iter().any(|entry| {
                entry.action.action_id.trim().is_empty()
                    || entry.action.claim_id.trim().is_empty()
                    || entry.action.cost_units == 0
                    || entry.action.effects.is_empty()
                    || !canonical(&entry.study_order)
                    || !canonical(&entry.independent_group_order)
                    || !canonical(&entry.completed_study_order)
                    || !canonical(&entry.negative_study_order)
                    || !canonical(&entry.failed_study_order)
                    || !canonical(&entry.blocked_study_order)
                    || !canonical(&entry.unknown_study_order)
                    || entry.support_milli > 1_000
                    || entry.disagreement_milli > 1_000
            })
        {
            return Err(MultiStudyContextError::InvalidOutput(
                "study partition or action summary invariants are inconsistent".into(),
            ));
        }
        let action_set = self.action_order.iter().cloned().collect::<BTreeSet<_>>();
        let outcome_rows_valid = self.outcome_order.iter().all(|outcome| {
            let action_is_retained = action_set.contains(&outcome.action_id);
            let action_is_omitted = self
                .omissions
                .get(&format!("action:{}", outcome.action_id))
                .is_some_and(|reason| reason == "action_capacity_exceeded");
            !outcome.action_id.trim().is_empty()
                && !outcome.study_id.trim().is_empty()
                && eligible_set.contains(&outcome.study_id)
                && (action_is_retained || action_is_omitted)
                && outcome.result_digest.as_str().len() == 64
        });
        let outcome_evidence_is_preserved = self.outcome_order.iter().all(|outcome| {
            let evidence_key = outcome_evidence_key(outcome);
            match outcome.status {
                DecisionContextActionOutcomeStatus::Completed => true,
                DecisionContextActionOutcomeStatus::Negative => {
                    self.negative_evidence_order.contains(&evidence_key)
                }
                DecisionContextActionOutcomeStatus::Failed
                | DecisionContextActionOutcomeStatus::Blocked
                | DecisionContextActionOutcomeStatus::Unknown => {
                    self.uncertainty_order.contains(&evidence_key)
                }
            }
        });
        let action_outcomes_match = self.actions.iter().all(|action| {
            let action_id = &action.action.action_id;
            let expected_completed = outcome_studies_for(
                &self.outcome_order,
                action_id,
                DecisionContextActionOutcomeStatus::Completed,
            );
            let expected_negative = outcome_studies_for(
                &self.outcome_order,
                action_id,
                DecisionContextActionOutcomeStatus::Negative,
            );
            let expected_failed = outcome_studies_for(
                &self.outcome_order,
                action_id,
                DecisionContextActionOutcomeStatus::Failed,
            );
            let expected_blocked = outcome_studies_for(
                &self.outcome_order,
                action_id,
                DecisionContextActionOutcomeStatus::Blocked,
            );
            let expected_unknown = outcome_studies_for(
                &self.outcome_order,
                action_id,
                DecisionContextActionOutcomeStatus::Unknown,
            );
            let outcomes_fit_lineage = self
                .outcome_order
                .iter()
                .filter(|outcome| outcome.action_id == *action_id)
                .all(|outcome| action.study_order.contains(&outcome.study_id));
            let adverse_matches_disposition = if has_adverse_outcome(&self.outcome_order, action_id)
            {
                matches!(
                    action.disposition,
                    MultiStudyActionDisposition::Adverse | MultiStudyActionDisposition::Conflicted
                )
            } else {
                action.disposition != MultiStudyActionDisposition::Adverse
            };
            action
                .study_order
                .iter()
                .all(|study_id| eligible_set.contains(study_id))
                && outcomes_fit_lineage
                && action.completed_study_order == expected_completed
                && action.negative_study_order == expected_negative
                && action.failed_study_order == expected_failed
                && action.blocked_study_order == expected_blocked
                && action.unknown_study_order == expected_unknown
                && adverse_matches_disposition
        });
        if !outcome_rows_valid || !outcome_evidence_is_preserved || !action_outcomes_match {
            return Err(MultiStudyContextError::InvalidOutput(
                "typed outcomes do not reconcile with eligible studies, action dispositions, and status partitions".into(),
            ));
        }
        if self.frontier_order.iter().any(|action_id| {
            !action_set.contains(action_id)
                || self.actions.iter().any(|entry| {
                    entry.action.action_id == *action_id
                        && entry.disposition != MultiStudyActionDisposition::Qualified
                })
        }) {
            return Err(MultiStudyContextError::InvalidOutput(
                "frontier references an unknown or unqualified action".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| MultiStudyContextError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(MultiStudyContextError::Digest(
                "multi-study context digest does not match canonical content".into(),
            ));
        }
        let report_bytes = serde_json::to_vec(self)
            .map_err(|error| MultiStudyContextError::Digest(error.to_string()))?
            .len();
        if report_bytes > MAX_REPORT_BYTES {
            return Err(MultiStudyContextError::InvalidOutput(format!(
                "report is {report_bytes} bytes, above the {MAX_REPORT_BYTES}-byte limit"
            )));
        }
        Ok(())
    }
}

fn action_signature(action: &DecisionContextArtifactAction) -> serde_json::Value {
    serde_json::json!({
        "action_id": action.action_id,
        "claim_id": action.claim_id,
        "kind": action.kind,
        "stage_kind": action.stage_kind,
        "target_modality": action.target_modality,
        "target_model_system": action.target_model_system,
        "priority_milli": action.priority_milli,
        "cost_units": action.cost_units,
        "depends_on": action.depends_on,
        "autonomy_tier": action.autonomy_tier,
        "effects": action.effects,
    })
}

/// Align compatible P04 artifacts into a deterministic multi-study action frontier.
pub fn align_glioma_multi_study_context_artifacts(
    request: &MultiStudyContextRequest,
) -> Result<MultiStudyDecisionContextArtifact, MultiStudyContextError> {
    if request.objective.trim().is_empty()
        || request.objective.len() > 4_096
        || request.epoch == 0
        || request.minimum_studies == 0
        || request.minimum_independent_groups == 0
        || request.minimum_action_support_milli > 1_000
        || request.minimum_quality_milli > 1_000
        || request.maximum_actions == 0
        || request.maximum_actions > MAX_ACTIONS
        || request.studies.is_empty()
        || request.studies.len() > MAX_STUDIES
    {
        return Err(MultiStudyContextError::InvalidRequest(
            "objective, epoch, quorum, quality, support, action, and study bounds are invalid"
                .into(),
        ));
    }
    let mut studies = request.studies.clone();
    studies.sort_by(|left, right| left.study_id.cmp(&right.study_id));
    if studies
        .windows(2)
        .any(|pair| pair[0].study_id == pair[1].study_id)
    {
        return Err(MultiStudyContextError::InvalidRequest(
            "study identifiers must be unique".into(),
        ));
    }
    if studies.iter().any(|study| {
        study.study_id.trim().is_empty()
            || study.study_id.len() > 256
            || study.independent_group.trim().is_empty()
            || study.independent_group.len() > 256
            || study.quality_milli > 1_000
            || study.artifact.objective != request.objective
            || study.artifact.study_id != study.study_id
            || study.artifact.epoch > request.epoch
    }) {
        return Err(MultiStudyContextError::InvalidInput(
            "study identity, quality, objective, epoch, and artifact bindings are invalid".into(),
        ));
    }
    let input_outcome_rows = studies.iter().fold(0usize, |total, study| {
        total.saturating_add(study.outcome_order.len())
    });
    if input_outcome_rows > MAX_OUTCOME_ROWS {
        return Err(MultiStudyContextError::InvalidRequest(format!(
            "outcome rows exceed the {MAX_OUTCOME_ROWS}-row limit"
        )));
    }
    let request_bytes = serde_json::to_vec(request)
        .map_err(|error| MultiStudyContextError::InvalidRequest(error.to_string()))?
        .len();
    if request_bytes > MAX_REQUEST_BYTES {
        return Err(MultiStudyContextError::InvalidRequest(format!(
            "request is {request_bytes} bytes, above the {MAX_REQUEST_BYTES}-byte limit"
        )));
    }
    for study in &studies {
        study
            .artifact
            .validate()
            .map_err(|error| MultiStudyContextError::InvalidInput(error.to_string()))?;
        if study.outcome_order.len() > MAX_ACTIONS {
            return Err(MultiStudyContextError::InvalidInput(format!(
                "study {} has more than {MAX_ACTIONS} local action outcomes",
                study.study_id
            )));
        }
        if !study.policy_allowed && !study.outcome_order.is_empty() {
            return Err(MultiStudyContextError::InvalidInput(format!(
                "policy-denied study {} cannot contribute local outcomes",
                study.study_id
            )));
        }
        let known_actions = study
            .artifact
            .actions
            .iter()
            .map(|action| action.action_id.as_str())
            .collect::<BTreeSet<_>>();
        let mut observed_actions = BTreeSet::new();
        if study.outcome_order.iter().any(|outcome| {
            outcome.action_id.trim().is_empty()
                || !known_actions.contains(outcome.action_id.as_str())
                || !observed_actions.insert(outcome.action_id.as_str())
                || outcome.result_digest.as_str().len() != 64
        }) {
            return Err(MultiStudyContextError::InvalidInput(format!(
                "study {} outcomes must identify distinct selected actions and carry result digests",
                study.study_id
            )));
        }
        if study.artifact.compatibility != request.compatibility {
            return Err(MultiStudyContextError::InvalidInput(
                "every source artifact must use the requested compatibility contract".into(),
            ));
        }
    }
    if request.compatibility.consumer_order != studies[0].artifact.compatibility.consumer_order
        || request.compatibility.contract_version
            != studies[0].artifact.compatibility.contract_version
    {
        return Err(MultiStudyContextError::InvalidRequest(
            "request compatibility must match the first source artifact contract".into(),
        ));
    }
    if request.compatibility.consumer_order.is_empty()
        || !canonical(&request.compatibility.consumer_order)
        || !canonical(&request.compatibility.semantic_loss_order)
        || request.compatibility.local_raw_data_required
        || request.compatibility.clinical_decision_capable
    {
        return Err(MultiStudyContextError::InvalidRequest(
            "multi-study compatibility must be ordered, loss-explicit, local-only, and preclinical"
                .into(),
        ));
    }
    let study_order = studies
        .iter()
        .map(|study| study.study_id.clone())
        .collect::<Vec<_>>();
    let study_group = studies
        .iter()
        .map(|study| (study.study_id.clone(), study.independent_group.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut omissions = BTreeMap::new();
    let mut eligible = Vec::new();
    for study in &studies {
        let reason = if !study.policy_allowed {
            Some("study_policy_denied")
        } else if study.quality_milli < request.minimum_quality_milli {
            Some("study_quality_below_threshold")
        } else {
            None
        };
        if let Some(reason) = reason {
            omissions.insert(study.study_id.clone(), reason.into());
        } else {
            eligible.push(study);
        }
    }
    let eligible_study_order = eligible
        .iter()
        .map(|study| study.study_id.clone())
        .collect::<Vec<_>>();
    let omitted_study_order = omissions.keys().cloned().collect::<Vec<_>>();
    let mut action_ids = eligible
        .iter()
        .flat_map(|study| {
            study
                .artifact
                .actions
                .iter()
                .map(|entry| entry.action_id.clone())
        })
        .collect::<BTreeSet<_>>();
    if action_ids.len() > request.maximum_actions {
        let retained = action_ids
            .iter()
            .take(request.maximum_actions)
            .cloned()
            .collect::<BTreeSet<_>>();
        for action_id in action_ids.difference(&retained) {
            omissions.insert(
                format!("action:{action_id}"),
                "action_capacity_exceeded".into(),
            );
        }
        action_ids = retained;
    }
    let action_order = action_ids.iter().cloned().collect::<Vec<_>>();
    let mut actions = Vec::with_capacity(action_order.len());
    let mut outcome_order = eligible
        .iter()
        .flat_map(|study| {
            study
                .outcome_order
                .iter()
                .map(|outcome| MultiStudyActionOutcome {
                    action_id: outcome.action_id.clone(),
                    study_id: study.study_id.clone(),
                    status: outcome.status,
                    result_digest: outcome.result_digest.clone(),
                })
        })
        .collect::<Vec<_>>();
    outcome_order.sort_by(|left, right| {
        left.action_id
            .cmp(&right.action_id)
            .then_with(|| left.study_id.cmp(&right.study_id))
    });
    let mut negative_evidence = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for study in &eligible {
        negative_evidence.extend(
            study
                .artifact
                .negative_evidence_order
                .iter()
                .map(|item| format!("{}:{item}", study.study_id)),
        );
        uncertainty.extend(
            study
                .artifact
                .uncertainty_order
                .iter()
                .map(|item| format!("{}:{item}", study.study_id)),
        );
    }
    for outcome in &outcome_order {
        match outcome.status {
            DecisionContextActionOutcomeStatus::Completed => {}
            DecisionContextActionOutcomeStatus::Negative => {
                negative_evidence.insert(outcome_evidence_key(outcome));
            }
            DecisionContextActionOutcomeStatus::Failed
            | DecisionContextActionOutcomeStatus::Blocked
            | DecisionContextActionOutcomeStatus::Unknown => {
                uncertainty.insert(outcome_evidence_key(outcome));
            }
        }
    }
    if negative_evidence.len() > MAX_PARTITION_ITEMS || uncertainty.len() > MAX_PARTITION_ITEMS {
        return Err(MultiStudyContextError::InvalidRequest(
            "aggregated negative or uncertain evidence exceeds the partition bound".into(),
        ));
    }
    for action_id in &action_order {
        let occurrences = eligible
            .iter()
            .filter_map(|study| {
                study
                    .artifact
                    .actions
                    .iter()
                    .find(|entry| entry.action_id == *action_id)
                    .map(|entry| (*study, entry))
            })
            .collect::<Vec<_>>();
        let first = occurrences.first().ok_or_else(|| {
            MultiStudyContextError::InvalidOutput("action disappeared during aggregation".into())
        })?;
        let first_signature = action_signature(first.1);
        let conflicted = occurrences
            .iter()
            .any(|(_, entry)| action_signature(entry) != first_signature);
        let study_order_for_action = occurrences
            .iter()
            .map(|(study, _)| study.study_id.clone())
            .collect::<Vec<_>>();
        let group_order = occurrences
            .iter()
            .map(|(study, _)| study.independent_group.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let support_milli =
            ((occurrences.len() as u64 * 1_000) / eligible.len().max(1) as u64).min(1_000) as u16;
        let disagreement_milli = if conflicted {
            1_000
        } else {
            1_000_u16.saturating_sub(support_milli)
        };
        let adverse_outcome = has_adverse_outcome(&outcome_order, action_id);
        let disposition = if conflicted {
            MultiStudyActionDisposition::Conflicted
        } else if adverse_outcome {
            MultiStudyActionDisposition::Adverse
        } else if occurrences.len() < request.minimum_studies
            || group_order.len() < request.minimum_independent_groups
            || support_milli < request.minimum_action_support_milli
        {
            MultiStudyActionDisposition::Underpowered
        } else {
            MultiStudyActionDisposition::Qualified
        };
        if disposition != MultiStudyActionDisposition::Qualified {
            uncertainty.insert(format!("action:{action_id}:multi-study-gate-unresolved"));
        }
        actions.push(MultiStudyDecisionAction {
            action: first.1.clone(),
            study_order: study_order_for_action,
            independent_group_order: group_order,
            support_milli,
            disagreement_milli,
            disposition,
            completed_study_order: outcome_studies_for(
                &outcome_order,
                action_id,
                DecisionContextActionOutcomeStatus::Completed,
            ),
            negative_study_order: outcome_studies_for(
                &outcome_order,
                action_id,
                DecisionContextActionOutcomeStatus::Negative,
            ),
            failed_study_order: outcome_studies_for(
                &outcome_order,
                action_id,
                DecisionContextActionOutcomeStatus::Failed,
            ),
            blocked_study_order: outcome_studies_for(
                &outcome_order,
                action_id,
                DecisionContextActionOutcomeStatus::Blocked,
            ),
            unknown_study_order: outcome_studies_for(
                &outcome_order,
                action_id,
                DecisionContextActionOutcomeStatus::Unknown,
            ),
        });
    }
    actions.sort_by(|left, right| left.action.action_id.cmp(&right.action.action_id));
    let mut frontier = actions
        .iter()
        .filter(|entry| entry.disposition == MultiStudyActionDisposition::Qualified)
        .map(|entry| {
            (
                entry.action.action_id.clone(),
                entry.support_milli,
                entry.action.priority_milli,
            )
        })
        .collect::<Vec<_>>();
    frontier.sort_by(|left, right| {
        right
            .1
            .cmp(&left.1)
            .then_with(|| right.2.cmp(&left.2))
            .then_with(|| left.0.cmp(&right.0))
    });
    let frontier_order = frontier
        .into_iter()
        .map(|entry| entry.0)
        .collect::<Vec<_>>();
    let disposition = if !frontier_order.is_empty() {
        MultiStudyContextDisposition::Qualified
    } else if !actions.is_empty() || !eligible.is_empty() {
        MultiStudyContextDisposition::Partial
    } else {
        MultiStudyContextDisposition::Unresolved
    };
    let mut negative_evidence_order = negative_evidence.into_iter().collect::<Vec<_>>();
    negative_evidence_order.sort();
    let mut uncertainty_order = uncertainty.into_iter().collect::<Vec<_>>();
    uncertainty_order.sort();
    let mut output = MultiStudyDecisionContextArtifact {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        artifact_id: format!(
            "glioma-multi-study-decision-context:epoch-{}",
            request.epoch
        ),
        objective: request.objective.clone(),
        epoch: request.epoch,
        boundary: PRECLINICAL_BOUNDARY.into(),
        study_order,
        study_group,
        eligible_study_order,
        omitted_study_order,
        action_order: actions
            .iter()
            .map(|entry| entry.action.action_id.clone())
            .collect(),
        frontier_order,
        actions,
        outcome_order,
        omissions,
        negative_evidence_order,
        uncertainty_order,
        disposition,
        compatibility: request.compatibility.clone(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-multi-study-context"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| MultiStudyContextError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::evidence::{EvidenceRecord, EvidenceSourceKind, EvidenceState};
    use crate::glioma::programs::p02_evidence_knowledge::{
        KnowledgeRequest, compile_typed_knowledge,
    };
    use crate::glioma::programs::p04_decision_context::decision_context_artifact::{
        DecisionContextArtifactConsumer, DecisionContextArtifactRequest,
        materialize_glioma_decision_context_artifact,
    };
    use crate::glioma::programs::p04_decision_context::{
        DecisionContextRequest, compile_decision_context,
    };
    use crate::glioma_engine::{GliomaModality, GliomaModelSystem, LocalArtifactRef};
    use bioprism_ids::ContentHash;

    fn hash(seed: &str) -> ContentHash {
        ContentHash::of_bytes(seed.as_bytes())
    }

    fn artifact(study_id: &str) -> DecisionContextArtifact {
        let evidence = EvidenceRecord {
            evidence_id: format!("{study_id}-evidence"),
            source_artifact: LocalArtifactRef {
                artifact_id: format!("{study_id}-artifact"),
                content_hash: hash(study_id),
                content_type: "application/vnd.aurora.glioma-evidence+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
            source_kind: EvidenceSourceKind::Dataset,
            claim: "EGFR signaling increases invasion".into(),
            scope: "preclinical glioma".into(),
            modality: GliomaModality::Proteomics,
            model_system: Some(GliomaModelSystem::Organoid),
            state: EvidenceState::Supported,
            relevance_milli: 900,
            quality_milli: 900,
            reproducibility_milli: 900,
            release_epoch: 1,
        };
        let knowledge = compile_typed_knowledge(
            &KnowledgeRequest {
                objective: "align contexts".into(),
                required_modalities: BTreeSet::from([GliomaModality::Proteomics]),
                required_model_systems: BTreeSet::from([GliomaModelSystem::Organoid]),
                min_support_milli: 700,
                min_sources_per_claim: 1,
                max_claims: 4,
            },
            &[evidence],
        )
        .unwrap();
        let context = compile_decision_context(
            &DecisionContextRequest {
                objective: "align contexts".into(),
                max_actions: 4,
                default_cost_units: 2,
            },
            &knowledge,
        )
        .unwrap();
        materialize_glioma_decision_context_artifact(&DecisionContextArtifactRequest {
            objective: "align contexts".into(),
            study_id: study_id.into(),
            epoch: 2,
            compatibility: compatibility(),
            context,
        })
        .unwrap()
    }

    fn compatibility() -> DecisionContextArtifactCompatibility {
        DecisionContextArtifactCompatibility {
            contract_version: "glioma-context-compat/1".into(),
            consumer_order: vec![
                DecisionContextArtifactConsumer::LocalAgent,
                DecisionContextArtifactConsumer::ResearcherWorkbench,
                DecisionContextArtifactConsumer::RustSdk,
                DecisionContextArtifactConsumer::PythonSdk,
                DecisionContextArtifactConsumer::TypeScriptSdk,
                DecisionContextArtifactConsumer::McpClient,
            ],
            semantic_loss_order: Vec::new(),
            local_raw_data_required: false,
            clinical_decision_capable: false,
        }
    }

    fn request(studies: Vec<MultiStudyContextInput>) -> MultiStudyContextRequest {
        MultiStudyContextRequest {
            objective: "align contexts".into(),
            epoch: 3,
            minimum_studies: 2,
            minimum_independent_groups: 2,
            minimum_action_support_milli: 700,
            minimum_quality_milli: 700,
            maximum_actions: 8,
            compatibility: compatibility(),
            studies,
        }
    }

    fn study(study_id: &str, group: &str) -> MultiStudyContextInput {
        MultiStudyContextInput {
            study_id: study_id.into(),
            independent_group: group.into(),
            quality_milli: 900,
            policy_allowed: true,
            artifact: artifact(study_id),
            outcome_order: Vec::new(),
        }
    }

    fn reseal(artifact: &mut DecisionContextArtifact) {
        let input = serde_json::json!({
            "feature_id": artifact.feature_id,
            "output_schema": artifact.output_schema,
            "artifact_id": artifact.artifact_id,
            "objective": artifact.objective,
            "study_id": artifact.study_id,
            "epoch": artifact.epoch,
            "boundary": artifact.boundary,
            "source_context_digest": artifact.source_context_digest,
            "claim_order": artifact.claim_order,
            "actions": artifact.actions,
            "action_order": artifact.action_order,
            "deferred_action_order": artifact.deferred_action_order,
            "omission_order": artifact.omission_order,
            "negative_evidence_order": artifact.negative_evidence_order,
            "uncertainty_order": artifact.uncertainty_order,
            "compatibility": artifact.compatibility,
        });
        artifact.digest = ContentHash::of_value(&input).unwrap();
    }

    #[test]
    fn compatible_studies_promote_shared_frontier_deterministically() {
        let first = align_glioma_multi_study_context_artifacts(&request(vec![
            study("study-a", "group-a"),
            study("study-b", "group-b"),
        ]))
        .unwrap();
        let second = align_glioma_multi_study_context_artifacts(&request(vec![
            study("study-b", "group-b"),
            study("study-a", "group-a"),
        ]))
        .unwrap();
        assert_eq!(first, second);
        assert_eq!(first.disposition, MultiStudyContextDisposition::Qualified);
        assert_eq!(first.frontier_order.len(), 1);
        first.validate().unwrap();
    }

    #[test]
    fn typed_local_outcomes_retain_receipts_and_hold_adverse_actions() {
        let mut completed = study("study-a", "group-a");
        let action_id = completed.artifact.actions[0].action_id.clone();
        completed.outcome_order.push(DecisionContextActionOutcome {
            action_id: action_id.clone(),
            status: DecisionContextActionOutcomeStatus::Completed,
            result_digest: hash("study-a-result"),
        });
        let mut negative = study("study-b", "group-b");
        negative.outcome_order.push(DecisionContextActionOutcome {
            action_id: action_id.clone(),
            status: DecisionContextActionOutcomeStatus::Negative,
            result_digest: hash("study-b-negative-result"),
        });

        let output =
            align_glioma_multi_study_context_artifacts(&request(vec![completed, negative]))
                .unwrap();

        assert_eq!(output.outcome_order.len(), 2);
        assert_eq!(output.actions[0].completed_study_order, vec!["study-a"]);
        assert_eq!(output.actions[0].negative_study_order, vec!["study-b"]);
        assert_eq!(
            output.actions[0].disposition,
            MultiStudyActionDisposition::Adverse
        );
        assert!(output.frontier_order.is_empty());
        assert!(output.negative_evidence_order.iter().any(|item| {
            item == &format!(
                "study-b:action:{action_id}:negative:{}",
                hash("study-b-negative-result")
            )
        }));
        output.validate().unwrap();
    }

    #[test]
    fn denied_study_is_omitted_and_shared_action_becomes_underpowered() {
        let mut denied = study("study-b", "group-b");
        denied.policy_allowed = false;
        let output = align_glioma_multi_study_context_artifacts(&request(vec![
            study("study-a", "group-a"),
            denied,
        ]))
        .unwrap();
        assert_eq!(output.omissions["study-b"], "study_policy_denied");
        assert_eq!(output.disposition, MultiStudyContextDisposition::Partial);
        assert!(output.frontier_order.is_empty());
    }

    #[test]
    fn typed_action_conflict_is_retained_but_not_promoted() {
        let mut conflicting = study("study-b", "group-b");
        conflicting.artifact.actions[0].cost_units += 1;
        reseal(&mut conflicting.artifact);
        let output = align_glioma_multi_study_context_artifacts(&request(vec![
            study("study-a", "group-a"),
            conflicting,
        ]))
        .unwrap();
        assert_eq!(
            output.actions[0].disposition,
            MultiStudyActionDisposition::Conflicted
        );
        assert_eq!(output.actions[0].disagreement_milli, 1_000);
        assert!(
            output
                .uncertainty_order
                .iter()
                .any(|item| item.contains("multi-study-gate"))
        );
    }

    #[test]
    fn nested_negative_and_unknown_partitions_are_namespaced_by_study() {
        let mut first = study("study-a", "group-a");
        first.artifact.negative_evidence_order = vec!["negative-a".into()];
        first.artifact.uncertainty_order = vec!["unknown-a".into()];
        reseal(&mut first.artifact);
        let output = align_glioma_multi_study_context_artifacts(&request(vec![
            first,
            study("study-b", "group-b"),
        ]))
        .unwrap();
        assert!(
            output
                .negative_evidence_order
                .contains(&"study-a:negative-a".into())
        );
        assert!(
            output
                .uncertainty_order
                .contains(&"study-a:unknown-a".into())
        );
    }
}

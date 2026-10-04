//! Replay aligned multi-study decision contexts across consecutive study epochs.
//!
//! The replay keeps each source artifact and derives action stability from the same explicit
//! support, independence, disagreement, and adverse-evidence gates at every epoch. Stable actions
//! remain research plans; the report never dispatches an assay or promotes a clinical conclusion.

use super::multi_study_context_artifact::{
    MAX_ACTIONS, MAX_PARTITION_ITEMS, MAX_STUDIES, MultiStudyActionDisposition,
    MultiStudyActionOutcome, MultiStudyContextDisposition, MultiStudyDecisionAction,
    MultiStudyDecisionContextArtifact,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = super::partition_checkpoint::FEATURE_ID;
pub const OUTPUT_SCHEMA: &str = "GliomaMultiStudyContextEpochReplay1@2";
pub const MIN_EPOCHS: usize = 2;
pub const MAX_EPOCHS: usize = 32;
pub const MAX_ACTION_EPOCH_ROWS: usize = MAX_ACTIONS * MAX_EPOCHS;
pub const MAX_STUDY_EPOCH_ROWS: usize = MAX_STUDIES * MAX_EPOCHS;
pub const MAX_OUTCOME_EPOCH_ROWS: usize = 65_536;
pub const MAX_REQUEST_BYTES: usize = 8_000_000;
pub const MAX_REPORT_BYTES: usize = 16_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiStudyContextEpochReplayRequest {
    pub objective: String,
    pub minimum_consecutive_qualified_epochs: usize,
    pub minimum_studies: usize,
    pub minimum_independent_groups: usize,
    pub minimum_support_milli: u16,
    pub maximum_disagreement_milli: u16,
    pub source_artifacts: Vec<MultiStudyDecisionContextArtifact>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultiStudyActionEpochDisposition {
    Qualified,
    Underpowered,
    Conflicted,
    Adverse,
    Absent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiStudyActionEpochAssessment {
    pub epoch: u32,
    pub disposition: MultiStudyActionEpochDisposition,
    pub study_count: usize,
    pub independent_group_count: usize,
    pub support_milli: u16,
    pub disagreement_milli: u16,
    pub frontier_rank: Option<usize>,
    pub completed_study_order: Vec<String>,
    pub negative_study_order: Vec<String>,
    pub failed_study_order: Vec<String>,
    pub blocked_study_order: Vec<String>,
    pub unknown_study_order: Vec<String>,
    pub outcome_order: Vec<MultiStudyActionOutcome>,
    pub reason_codes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultiStudyTemporalActionDisposition {
    Stable,
    Continuing,
    Hold,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiStudyActionTemporalAssessment {
    pub action_id: String,
    pub epoch_assessments: Vec<MultiStudyActionEpochAssessment>,
    pub qualified_epoch_order: Vec<u32>,
    pub maximum_qualified_streak: usize,
    pub trailing_qualified_streak: usize,
    pub historical_adverse_evidence: bool,
    pub stable: bool,
    pub disposition: MultiStudyTemporalActionDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiStudyContextEpochTransition {
    pub from_epoch: u32,
    pub to_epoch: u32,
    pub added_action_order: Vec<String>,
    pub retired_action_order: Vec<String>,
    pub stable_action_order: Vec<String>,
    pub newly_qualified_action_order: Vec<String>,
    pub qualification_lost_action_order: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultiStudyContextEpochReplayDisposition {
    Ready,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiStudyContextEpochReplay {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub request: MultiStudyContextEpochReplayRequest,
    pub epoch_order: Vec<u32>,
    pub source_artifact_id_order: Vec<String>,
    pub source_digest_order: Vec<ContentHash>,
    pub action_assessments: Vec<MultiStudyActionTemporalAssessment>,
    pub transition_order: Vec<String>,
    pub transitions: Vec<MultiStudyContextEpochTransition>,
    pub stable_frontier_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub disposition: MultiStudyContextEpochReplayDisposition,
    pub next_route: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MultiStudyContextEpochReplayError {
    #[error("multi-study context epoch-replay request is invalid: {0}")]
    InvalidRequest(String),
    #[error("multi-study context epoch-replay output is invalid: {0}")]
    InvalidOutput(String),
    #[error("multi-study context epoch-replay digest failed: {0}")]
    Digest(String),
}

fn digest_input(report: &MultiStudyContextEpochReplay) -> serde_json::Value {
    serde_json::json!({
        "feature_id": report.feature_id,
        "output_schema": report.output_schema,
        "objective": report.objective,
        "request": report.request,
        "epoch_order": report.epoch_order,
        "source_artifact_id_order": report.source_artifact_id_order,
        "source_digest_order": report.source_digest_order,
        "action_assessments": report.action_assessments,
        "transition_order": report.transition_order,
        "transitions": report.transitions,
        "stable_frontier_order": report.stable_frontier_order,
        "omission_order": report.omission_order,
        "negative_evidence_order": report.negative_evidence_order,
        "uncertainty_order": report.uncertainty_order,
        "disposition": report.disposition,
        "next_route": report.next_route,
    })
}

fn fail_request(message: impl Into<String>) -> MultiStudyContextEpochReplayError {
    MultiStudyContextEpochReplayError::InvalidRequest(message.into())
}

fn validate_source_semantics(
    artifact: &MultiStudyDecisionContextArtifact,
) -> Result<(), MultiStudyContextEpochReplayError> {
    let eligible = artifact
        .eligible_study_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    for action in &artifact.actions {
        if action
            .study_order
            .iter()
            .any(|study| !eligible.contains(study))
            || action
                .negative_study_order
                .iter()
                .any(|study| !action.study_order.iter().any(|source| source == study))
            || action
                .completed_study_order
                .iter()
                .any(|study| !action.study_order.iter().any(|source| source == study))
            || action
                .failed_study_order
                .iter()
                .any(|study| !action.study_order.iter().any(|source| source == study))
            || action
                .blocked_study_order
                .iter()
                .any(|study| !action.study_order.iter().any(|source| source == study))
            || action
                .unknown_study_order
                .iter()
                .any(|study| !action.study_order.iter().any(|source| source == study))
            || artifact
                .outcome_order
                .iter()
                .filter(|outcome| outcome.action_id == action.action.action_id)
                .any(|outcome| {
                    !action
                        .study_order
                        .iter()
                        .any(|study| study == &outcome.study_id)
                })
        {
            return Err(fail_request(format!(
                "action {} references a study outside its source partitions",
                action.action.action_id
            )));
        }
        let expected_groups = action
            .study_order
            .iter()
            .filter_map(|study| artifact.study_group.get(study).cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        if expected_groups != action.independent_group_order {
            return Err(fail_request(format!(
                "action {} independent-group summary does not match its study lineage",
                action.action.action_id
            )));
        }
        let has_adverse_outcome = !action.negative_study_order.is_empty()
            || !action.failed_study_order.is_empty()
            || !action.blocked_study_order.is_empty()
            || !action.unknown_study_order.is_empty();
        if (has_adverse_outcome
            && !matches!(
                action.disposition,
                MultiStudyActionDisposition::Adverse | MultiStudyActionDisposition::Conflicted
            ))
            || (!has_adverse_outcome && action.disposition == MultiStudyActionDisposition::Adverse)
        {
            return Err(fail_request(format!(
                "action {} disposition does not preserve its local outcome states",
                action.action.action_id
            )));
        }
        let expected_support = if eligible.is_empty() {
            0
        } else {
            ((action.study_order.len() as u64 * 1_000) / eligible.len() as u64).min(1_000) as u16
        };
        let expected_disagreement = if action.disposition == MultiStudyActionDisposition::Conflicted
        {
            1_000
        } else {
            1_000_u16.saturating_sub(expected_support)
        };
        if action.support_milli != expected_support
            || action.disagreement_milli != expected_disagreement
        {
            return Err(fail_request(format!(
                "action {} support or disagreement does not replay from study counts",
                action.action.action_id
            )));
        }
    }
    let mut expected_frontier = artifact
        .actions
        .iter()
        .filter(|action| action.disposition == MultiStudyActionDisposition::Qualified)
        .map(|action| {
            (
                action.action.action_id.clone(),
                action.support_milli,
                action.action.priority_milli,
            )
        })
        .collect::<Vec<_>>();
    expected_frontier.sort_by(|left, right| {
        right
            .1
            .cmp(&left.1)
            .then_with(|| right.2.cmp(&left.2))
            .then_with(|| left.0.cmp(&right.0))
    });
    let expected_frontier = expected_frontier
        .into_iter()
        .map(|row| row.0)
        .collect::<Vec<_>>();
    let expected_disposition = if !expected_frontier.is_empty() {
        MultiStudyContextDisposition::Qualified
    } else if !artifact.actions.is_empty() || !artifact.eligible_study_order.is_empty() {
        MultiStudyContextDisposition::Partial
    } else {
        MultiStudyContextDisposition::Unresolved
    };
    if artifact.frontier_order != expected_frontier || artifact.disposition != expected_disposition
    {
        return Err(fail_request(
            "source frontier rank or overall disposition does not replay from its action rows",
        ));
    }
    Ok(())
}

fn validate_request(
    request: &MultiStudyContextEpochReplayRequest,
) -> Result<(), MultiStudyContextEpochReplayError> {
    if request.objective.trim().is_empty()
        || request.objective.len() > 4_096
        || !(MIN_EPOCHS..=MAX_EPOCHS).contains(&request.source_artifacts.len())
        || !(MIN_EPOCHS..=MAX_EPOCHS).contains(&request.minimum_consecutive_qualified_epochs)
        || request.minimum_studies == 0
        || request.minimum_studies > MAX_STUDIES
        || request.minimum_independent_groups == 0
        || request.minimum_independent_groups > MAX_STUDIES
        || request.minimum_support_milli > 1_000
        || request.maximum_disagreement_milli > 1_000
    {
        return Err(fail_request(
            "objective, temporal window, study/group quorum, support, disagreement, or epoch bounds are invalid",
        ));
    }
    let request_bytes = serde_json::to_vec(request)
        .map_err(|error| fail_request(format!("request encoding failed: {error}")))?
        .len();
    if request_bytes > MAX_REQUEST_BYTES {
        return Err(fail_request(format!(
            "request is {request_bytes} bytes, above the {MAX_REQUEST_BYTES}-byte limit"
        )));
    }

    let mut artifact_ids = BTreeSet::new();
    let mut source_digests = BTreeSet::new();
    let mut previous_epoch = None;
    let mut study_groups = BTreeMap::new();
    let mut action_rows = 0usize;
    let mut study_rows = 0usize;
    let mut outcome_rows = 0usize;
    let mut compatibility = None;
    for artifact in &request.source_artifacts {
        artifact
            .validate()
            .map_err(|error| fail_request(format!("source artifact failed validation: {error}")))?;
        validate_source_semantics(artifact)?;
        if artifact.objective != request.objective {
            return Err(fail_request(
                "every source artifact must bind the exact requested objective",
            ));
        }
        if !artifact_ids.insert(artifact.artifact_id.clone())
            || !source_digests.insert(artifact.digest.clone())
        {
            return Err(fail_request(
                "source artifact identities and digests must be unique across epochs",
            ));
        }
        if previous_epoch
            .is_some_and(|previous: u32| previous.checked_add(1) != Some(artifact.epoch))
        {
            return Err(fail_request(
                "source artifacts must have unique, consecutive epochs in input order",
            ));
        }
        previous_epoch = Some(artifact.epoch);
        if compatibility.is_some_and(|value| value != &artifact.compatibility) {
            return Err(fail_request(
                "source artifacts must use one exact compatibility contract",
            ));
        }
        compatibility = Some(&artifact.compatibility);
        for (study_id, group) in &artifact.study_group {
            if study_groups
                .insert(study_id.clone(), group.clone())
                .is_some_and(|previous| previous != *group)
            {
                return Err(fail_request(format!(
                    "study {study_id} changes independent-group identity across epochs"
                )));
            }
        }
        action_rows = action_rows.saturating_add(artifact.actions.len());
        study_rows = study_rows.saturating_add(artifact.study_order.len());
        outcome_rows = outcome_rows.saturating_add(artifact.outcome_order.len());
        if action_rows > MAX_ACTION_EPOCH_ROWS
            || study_rows > MAX_STUDY_EPOCH_ROWS
            || outcome_rows > MAX_OUTCOME_EPOCH_ROWS
        {
            return Err(fail_request(
                "aggregate action, study, or outcome rows exceed the temporal replay bound",
            ));
        }
        if artifact.omissions.len() > MAX_PARTITION_ITEMS
            || artifact.negative_evidence_order.len() > MAX_PARTITION_ITEMS
            || artifact.uncertainty_order.len() > MAX_PARTITION_ITEMS
        {
            return Err(fail_request(
                "source artifact evidence partitions exceed the replay bound",
            ));
        }
    }
    Ok(())
}

fn qualifies(
    row: &MultiStudyActionEpochAssessment,
    source: &MultiStudyDecisionAction,
    request: &MultiStudyContextEpochReplayRequest,
) -> bool {
    row.disposition == MultiStudyActionEpochDisposition::Qualified
        && source.disposition == MultiStudyActionDisposition::Qualified
        && row.frontier_rank.is_some()
        && row.study_count >= request.minimum_studies
        && row.independent_group_count >= request.minimum_independent_groups
        && row.support_milli >= request.minimum_support_milli
        && row.disagreement_milli <= request.maximum_disagreement_milli
        && row.negative_study_order.is_empty()
        && row.failed_study_order.is_empty()
        && row.blocked_study_order.is_empty()
        && row.unknown_study_order.is_empty()
        && source.negative_study_order.is_empty()
        && source.failed_study_order.is_empty()
        && source.blocked_study_order.is_empty()
        && source.unknown_study_order.is_empty()
}

fn source_action_outcomes(
    artifact: &MultiStudyDecisionContextArtifact,
    action_id: &str,
) -> Vec<MultiStudyActionOutcome> {
    artifact
        .outcome_order
        .iter()
        .filter(|outcome| outcome.action_id == action_id)
        .cloned()
        .collect()
}

fn assess_epoch_action(
    epoch: u32,
    action_id: &str,
    source_artifact: &MultiStudyDecisionContextArtifact,
    source_action: Option<&MultiStudyDecisionAction>,
    request: &MultiStudyContextEpochReplayRequest,
) -> MultiStudyActionEpochAssessment {
    let Some(source) = source_action else {
        return MultiStudyActionEpochAssessment {
            epoch,
            disposition: MultiStudyActionEpochDisposition::Absent,
            study_count: 0,
            independent_group_count: 0,
            support_milli: 0,
            disagreement_milli: 1_000,
            frontier_rank: None,
            completed_study_order: Vec::new(),
            negative_study_order: Vec::new(),
            failed_study_order: Vec::new(),
            blocked_study_order: Vec::new(),
            unknown_study_order: Vec::new(),
            outcome_order: Vec::new(),
            reason_codes: vec!["action_absent_from_epoch".into()],
        };
    };

    let frontier_rank = source_artifact
        .frontier_order
        .iter()
        .position(|candidate| candidate == action_id)
        .map(|rank| rank + 1);
    let mut reasons = BTreeSet::new();
    if source.disposition == MultiStudyActionDisposition::Conflicted {
        reasons.insert("action_contract_conflicted".to_string());
    }
    if !source.negative_study_order.is_empty() {
        reasons.insert("action_has_negative_study_outcomes".to_string());
    }
    if !source.failed_study_order.is_empty() {
        reasons.insert("action_has_failed_study_outcomes".to_string());
    }
    if !source.blocked_study_order.is_empty() {
        reasons.insert("action_has_blocked_study_outcomes".to_string());
    }
    if !source.unknown_study_order.is_empty() {
        reasons.insert("action_has_unknown_study_outcomes".to_string());
    }
    if source.study_order.len() < request.minimum_studies {
        reasons.insert("minimum_study_quorum_not_met".to_string());
    }
    if source.independent_group_order.len() < request.minimum_independent_groups {
        reasons.insert("minimum_independent_group_quorum_not_met".to_string());
    }
    if source.support_milli < request.minimum_support_milli {
        reasons.insert("minimum_support_not_met".to_string());
    }
    if source.disagreement_milli > request.maximum_disagreement_milli {
        reasons.insert("maximum_disagreement_exceeded".to_string());
    }
    if frontier_rank.is_none() {
        reasons.insert("action_not_on_epoch_frontier".to_string());
    }
    let adverse = !source.negative_study_order.is_empty()
        || !source.failed_study_order.is_empty()
        || !source.blocked_study_order.is_empty()
        || !source.unknown_study_order.is_empty();
    let disposition = if adverse {
        MultiStudyActionEpochDisposition::Adverse
    } else if source.disposition == MultiStudyActionDisposition::Conflicted {
        MultiStudyActionEpochDisposition::Conflicted
    } else if !qualifies(
        &MultiStudyActionEpochAssessment {
            epoch,
            disposition: MultiStudyActionEpochDisposition::Qualified,
            study_count: source.study_order.len(),
            independent_group_count: source.independent_group_order.len(),
            support_milli: source.support_milli,
            disagreement_milli: source.disagreement_milli,
            frontier_rank,
            completed_study_order: source.completed_study_order.clone(),
            negative_study_order: source.negative_study_order.clone(),
            failed_study_order: source.failed_study_order.clone(),
            blocked_study_order: source.blocked_study_order.clone(),
            unknown_study_order: source.unknown_study_order.clone(),
            outcome_order: source_action_outcomes(source_artifact, action_id),
            reason_codes: Vec::new(),
        },
        source,
        request,
    ) {
        MultiStudyActionEpochDisposition::Underpowered
    } else {
        MultiStudyActionEpochDisposition::Qualified
    };
    MultiStudyActionEpochAssessment {
        epoch,
        disposition,
        study_count: source.study_order.len(),
        independent_group_count: source.independent_group_order.len(),
        support_milli: source.support_milli,
        disagreement_milli: source.disagreement_milli,
        frontier_rank,
        completed_study_order: source.completed_study_order.clone(),
        negative_study_order: source.negative_study_order.clone(),
        failed_study_order: source.failed_study_order.clone(),
        blocked_study_order: source.blocked_study_order.clone(),
        unknown_study_order: source.unknown_study_order.clone(),
        outcome_order: source_action_outcomes(source_artifact, action_id),
        reason_codes: reasons.into_iter().collect(),
    }
}

fn action_temporal_assessment(
    action_id: String,
    epoch_assessments: Vec<MultiStudyActionEpochAssessment>,
    request: &MultiStudyContextEpochReplayRequest,
) -> MultiStudyActionTemporalAssessment {
    let qualified_epoch_order = epoch_assessments
        .iter()
        .filter(|row| row.disposition == MultiStudyActionEpochDisposition::Qualified)
        .map(|row| row.epoch)
        .collect::<Vec<_>>();
    let historical_adverse_evidence = epoch_assessments.iter().any(|row| {
        matches!(
            row.disposition,
            MultiStudyActionEpochDisposition::Adverse
                | MultiStudyActionEpochDisposition::Conflicted
        )
    });
    let mut maximum_qualified_streak = 0usize;
    let mut current_streak = 0usize;
    for row in &epoch_assessments {
        if row.disposition == MultiStudyActionEpochDisposition::Qualified {
            current_streak += 1;
            maximum_qualified_streak = maximum_qualified_streak.max(current_streak);
        } else {
            current_streak = 0;
        }
    }
    let trailing_qualified_streak = current_streak;
    let stable = trailing_qualified_streak >= request.minimum_consecutive_qualified_epochs
        && !historical_adverse_evidence;
    let disposition = if stable {
        MultiStudyTemporalActionDisposition::Stable
    } else if historical_adverse_evidence {
        MultiStudyTemporalActionDisposition::Hold
    } else if !qualified_epoch_order.is_empty() {
        MultiStudyTemporalActionDisposition::Continuing
    } else {
        MultiStudyTemporalActionDisposition::Unavailable
    };
    MultiStudyActionTemporalAssessment {
        action_id,
        epoch_assessments,
        qualified_epoch_order,
        maximum_qualified_streak,
        trailing_qualified_streak,
        historical_adverse_evidence,
        stable,
        disposition,
    }
}

fn calculate(
    request: &MultiStudyContextEpochReplayRequest,
) -> Result<MultiStudyContextEpochReplay, MultiStudyContextEpochReplayError> {
    validate_request(request)?;

    let mut action_maps = Vec::with_capacity(request.source_artifacts.len());
    let mut all_action_ids = BTreeSet::new();
    let mut omission_order = BTreeSet::new();
    let mut negative_evidence_order = BTreeSet::new();
    let mut uncertainty_order = BTreeSet::new();
    for artifact in &request.source_artifacts {
        let action_map = artifact
            .actions
            .iter()
            .map(|entry| (entry.action.action_id.clone(), entry))
            .collect::<BTreeMap<_, _>>();
        all_action_ids.extend(action_map.keys().cloned());
        for (study_id, reason) in &artifact.omissions {
            omission_order.insert(format!("epoch-{}:{study_id}:{reason}", artifact.epoch));
        }
        for item in &artifact.negative_evidence_order {
            negative_evidence_order.insert(format!("epoch-{}:{item}", artifact.epoch));
        }
        for item in &artifact.uncertainty_order {
            uncertainty_order.insert(format!("epoch-{}:{item}", artifact.epoch));
        }
        action_maps.push(action_map);
    }

    let mut action_assessments = Vec::with_capacity(all_action_ids.len());
    for action_id in all_action_ids {
        let epoch_assessments = request
            .source_artifacts
            .iter()
            .zip(&action_maps)
            .map(|(artifact, action_map)| {
                assess_epoch_action(
                    artifact.epoch,
                    &action_id,
                    artifact,
                    action_map.get(&action_id).copied(),
                    request,
                )
            })
            .collect::<Vec<_>>();
        action_assessments.push(action_temporal_assessment(
            action_id,
            epoch_assessments,
            request,
        ));
    }

    let assessment_by_action = action_assessments
        .iter()
        .map(|assessment| (assessment.action_id.as_str(), assessment))
        .collect::<BTreeMap<_, _>>();
    let stable_frontier_order = request
        .source_artifacts
        .last()
        .expect("request validation requires source artifacts")
        .frontier_order
        .iter()
        .filter(|action_id| {
            assessment_by_action
                .get(action_id.as_str())
                .is_some_and(|assessment| assessment.stable)
        })
        .cloned()
        .collect::<Vec<_>>();

    let mut transitions = Vec::with_capacity(request.source_artifacts.len() - 1);
    for (transition_index, pair) in request.source_artifacts.windows(2).enumerate() {
        let previous = &pair[0];
        let current = &pair[1];
        let previous_ids = previous
            .action_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let current_ids = current
            .action_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let previous_qualified = action_assessments
            .iter()
            .filter(|assessment| {
                assessment.epoch_assessments[transition_index].disposition
                    == MultiStudyActionEpochDisposition::Qualified
            })
            .map(|assessment| assessment.action_id.clone())
            .collect::<BTreeSet<_>>();
        let current_qualified = action_assessments
            .iter()
            .filter(|assessment| {
                assessment.epoch_assessments[transition_index + 1].disposition
                    == MultiStudyActionEpochDisposition::Qualified
            })
            .map(|assessment| assessment.action_id.clone())
            .collect::<BTreeSet<_>>();
        let newly_qualified = current_qualified
            .difference(&previous_qualified)
            .cloned()
            .collect::<Vec<_>>();
        let qualification_lost = previous_qualified
            .difference(&current_qualified)
            .cloned()
            .collect::<Vec<_>>();
        transitions.push(MultiStudyContextEpochTransition {
            from_epoch: previous.epoch,
            to_epoch: current.epoch,
            added_action_order: current_ids.difference(&previous_ids).cloned().collect(),
            retired_action_order: previous_ids.difference(&current_ids).cloned().collect(),
            stable_action_order: previous_ids.intersection(&current_ids).cloned().collect(),
            newly_qualified_action_order: newly_qualified,
            qualification_lost_action_order: qualification_lost,
        });
    }
    let epoch_order = request
        .source_artifacts
        .iter()
        .map(|artifact| artifact.epoch)
        .collect::<Vec<_>>();
    let source_artifact_id_order = request
        .source_artifacts
        .iter()
        .map(|artifact| artifact.artifact_id.clone())
        .collect::<Vec<_>>();
    let source_digest_order = request
        .source_artifacts
        .iter()
        .map(|artifact| artifact.digest.clone())
        .collect::<Vec<_>>();
    let transition_order = transitions
        .iter()
        .map(|transition| format!("{}->{}", transition.from_epoch, transition.to_epoch))
        .collect::<Vec<_>>();
    let has_qualified_history = action_assessments
        .iter()
        .any(|assessment| !assessment.qualified_epoch_order.is_empty());
    let has_source_debt = !omission_order.is_empty()
        || !negative_evidence_order.is_empty()
        || !uncertainty_order.is_empty()
        || action_assessments
            .iter()
            .any(|assessment| assessment.historical_adverse_evidence);
    let disposition = if !stable_frontier_order.is_empty() && !has_source_debt {
        MultiStudyContextEpochReplayDisposition::Ready
    } else if !stable_frontier_order.is_empty() || has_qualified_history || has_source_debt {
        MultiStudyContextEpochReplayDisposition::Partial
    } else {
        MultiStudyContextEpochReplayDisposition::Blocked
    };
    let next_route = match disposition {
        MultiStudyContextEpochReplayDisposition::Ready => "glioma_decision_operating_cycle",
        MultiStudyContextEpochReplayDisposition::Partial => "glioma_researcher_workbench",
        MultiStudyContextEpochReplayDisposition::Blocked => "glioma_multi_study_workflow_plan",
    }
    .to_string();

    let mut report = MultiStudyContextEpochReplay {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        request: request.clone(),
        epoch_order,
        source_artifact_id_order,
        source_digest_order,
        action_assessments,
        transition_order,
        transitions,
        stable_frontier_order,
        omission_order: omission_order.into_iter().collect(),
        negative_evidence_order: negative_evidence_order.into_iter().collect(),
        uncertainty_order: uncertainty_order.into_iter().collect(),
        disposition,
        next_route,
        digest: ContentHash::of_bytes(b"unsealed-multistudy-context-epoch-replay"),
    };
    report.digest = ContentHash::of_value(&digest_input(&report))
        .map_err(|error| MultiStudyContextEpochReplayError::Digest(error.to_string()))?;
    let report_bytes = serde_json::to_vec(&report)
        .map_err(|error| MultiStudyContextEpochReplayError::Digest(error.to_string()))?
        .len();
    if report_bytes > MAX_REPORT_BYTES {
        return Err(fail_request(format!(
            "report is {report_bytes} bytes, above the {MAX_REPORT_BYTES}-byte limit"
        )));
    }
    Ok(report)
}

impl MultiStudyContextEpochReplay {
    /// Validates the retained artifacts and recomputes every temporal assessment and route.
    pub fn validate(&self) -> Result<(), MultiStudyContextEpochReplayError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.objective != self.request.objective
            || self.digest.as_str().len() != 64
        {
            return Err(MultiStudyContextEpochReplayError::InvalidOutput(
                "feature identity, schema, objective binding, or digest shape is invalid".into(),
            ));
        }
        validate_request(&self.request).map_err(|error| {
            MultiStudyContextEpochReplayError::InvalidOutput(format!(
                "retained request failed validation: {error}"
            ))
        })?;
        let expected = calculate(&self.request).map_err(|error| {
            MultiStudyContextEpochReplayError::InvalidOutput(format!(
                "retained request cannot be replayed: {error}"
            ))
        })?;
        if expected != *self {
            return Err(MultiStudyContextEpochReplayError::InvalidOutput(
                "report does not match deterministic replay of its retained artifact ledger".into(),
            ));
        }
        Ok(())
    }
}

/// Replay a multi-study decision frontier over consecutive source epochs.
pub fn replay_glioma_multi_study_context_epochs(
    request: &MultiStudyContextEpochReplayRequest,
) -> Result<MultiStudyContextEpochReplay, MultiStudyContextEpochReplayError> {
    validate_request(request)?;
    let report = calculate(request)?;
    report.validate()?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::evidence::{EvidenceRecord, EvidenceSourceKind, EvidenceState};
    use crate::glioma::programs::p02_evidence_knowledge::{
        KnowledgeRequest, compile_typed_knowledge,
    };
    use crate::glioma::programs::p04_decision_context::context_compiler::compile_decision_context;
    use crate::glioma::programs::p04_decision_context::context_replay::{
        DecisionContextActionOutcome, DecisionContextActionOutcomeStatus,
    };
    use crate::glioma::programs::p04_decision_context::{
        DecisionContextArtifactCompatibility, DecisionContextArtifactConsumer,
        DecisionContextArtifactRequest, DecisionContextRequest, MultiStudyContextInput,
        MultiStudyContextRequest, align_glioma_multi_study_context_artifacts,
        materialize_glioma_decision_context_artifact,
    };
    use crate::glioma_engine::{GliomaModality, GliomaModelSystem, LocalArtifactRef};

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
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

    fn study(study_id: &str, group: &str, claim: &str) -> MultiStudyContextInput {
        let objective = "replay multi-study decision epochs";
        let evidence = EvidenceRecord {
            evidence_id: format!("evidence-{study_id}"),
            source_artifact: LocalArtifactRef {
                artifact_id: format!("artifact-{study_id}"),
                content_hash: hash(study_id),
                content_type: "application/vnd.aurora.glioma-evidence+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
            source_kind: EvidenceSourceKind::Dataset,
            claim: claim.into(),
            scope: "preclinical glioma".into(),
            modality: GliomaModality::Genomics,
            model_system: Some(GliomaModelSystem::Organoid),
            state: EvidenceState::Supported,
            relevance_milli: 900,
            quality_milli: 900,
            reproducibility_milli: 900,
            release_epoch: 1,
        };
        let knowledge = compile_typed_knowledge(
            &KnowledgeRequest {
                objective: objective.into(),
                required_modalities: BTreeSet::from([GliomaModality::Genomics]),
                required_model_systems: BTreeSet::from([GliomaModelSystem::Organoid]),
                min_support_milli: 700,
                min_sources_per_claim: 1,
                max_claims: 8,
            },
            &[evidence],
        )
        .unwrap();
        let context = compile_decision_context(
            &DecisionContextRequest {
                objective: objective.into(),
                max_actions: 8,
                default_cost_units: 5,
            },
            &knowledge,
        )
        .unwrap();
        let artifact =
            materialize_glioma_decision_context_artifact(&DecisionContextArtifactRequest {
                objective: objective.into(),
                study_id: study_id.into(),
                epoch: 1,
                compatibility: compatibility(),
                context,
            })
            .unwrap();
        MultiStudyContextInput {
            study_id: study_id.into(),
            independent_group: group.into(),
            quality_milli: 900,
            policy_allowed: true,
            artifact,
            outcome_order: Vec::new(),
        }
    }

    fn source_artifact(epoch: u32, claim: &str) -> MultiStudyDecisionContextArtifact {
        align_glioma_multi_study_context_artifacts(&MultiStudyContextRequest {
            objective: "replay multi-study decision epochs".into(),
            epoch,
            minimum_studies: 2,
            minimum_independent_groups: 2,
            minimum_action_support_milli: 700,
            minimum_quality_milli: 700,
            maximum_actions: 8,
            compatibility: compatibility(),
            studies: vec![
                study("study-a", "group-a", claim),
                study("study-b", "group-b", claim),
            ],
        })
        .unwrap()
    }

    fn source_artifact_with_negative_outcome(
        epoch: u32,
        claim: &str,
    ) -> MultiStudyDecisionContextArtifact {
        let mut first = study("study-a", "group-a", claim);
        let action_id = first.artifact.actions[0].action_id.clone();
        let result_digest = hash("study-a-negative-result");
        first.outcome_order.push(DecisionContextActionOutcome {
            action_id,
            status: DecisionContextActionOutcomeStatus::Negative,
            result_digest,
        });
        align_glioma_multi_study_context_artifacts(&MultiStudyContextRequest {
            objective: "replay multi-study decision epochs".into(),
            epoch,
            minimum_studies: 2,
            minimum_independent_groups: 2,
            minimum_action_support_milli: 700,
            minimum_quality_milli: 700,
            maximum_actions: 8,
            compatibility: compatibility(),
            studies: vec![first, study("study-b", "group-b", claim)],
        })
        .unwrap()
    }

    fn request(
        source_artifacts: Vec<MultiStudyDecisionContextArtifact>,
    ) -> MultiStudyContextEpochReplayRequest {
        MultiStudyContextEpochReplayRequest {
            objective: "replay multi-study decision epochs".into(),
            minimum_consecutive_qualified_epochs: 2,
            minimum_studies: 2,
            minimum_independent_groups: 2,
            minimum_support_milli: 700,
            maximum_disagreement_milli: 250,
            source_artifacts,
        }
    }

    #[test]
    fn stable_action_is_qualified_only_after_consecutive_epochs() {
        let claim = "EGFR signaling increases invasion";
        let report = replay_glioma_multi_study_context_epochs(&request(vec![
            source_artifact(1, claim),
            source_artifact(2, claim),
        ]))
        .unwrap();

        assert_eq!(
            report.disposition,
            MultiStudyContextEpochReplayDisposition::Ready
        );
        assert_eq!(report.stable_frontier_order.len(), 1);
        assert!(report.action_assessments[0].stable);
        assert_eq!(report.action_assessments[0].trailing_qualified_streak, 2);
        report.validate().unwrap();
    }

    #[test]
    fn local_negative_outcome_is_retained_and_prevents_temporal_promotion() {
        let claim = "EGFR signaling increases invasion";
        let first = source_artifact(1, claim);
        let second = source_artifact_with_negative_outcome(2, claim);
        let action_id = second.actions[0].action.action_id.clone();
        let report =
            replay_glioma_multi_study_context_epochs(&request(vec![first, second])).unwrap();
        let assessment = report
            .action_assessments
            .iter()
            .find(|assessment| assessment.action_id == action_id)
            .unwrap();
        let epoch = assessment
            .epoch_assessments
            .iter()
            .find(|epoch| epoch.epoch == 2)
            .unwrap();

        assert_eq!(epoch.disposition, MultiStudyActionEpochDisposition::Adverse);
        assert_eq!(epoch.outcome_order.len(), 1);
        assert_eq!(
            epoch.outcome_order[0].status,
            DecisionContextActionOutcomeStatus::Negative
        );
        assert!(assessment.historical_adverse_evidence);
        assert!(!assessment.stable);
        assert!(report.stable_frontier_order.is_empty());
        report.validate().unwrap();
    }

    #[test]
    fn changed_frontier_resets_temporal_stability_and_preserves_transitions() {
        let report = replay_glioma_multi_study_context_epochs(&request(vec![
            source_artifact(4, "EGFR signaling increases invasion"),
            source_artifact(5, "NF1 loss increases invasion"),
        ]))
        .unwrap();

        assert!(report.stable_frontier_order.is_empty());
        assert_eq!(
            report.disposition,
            MultiStudyContextEpochReplayDisposition::Partial
        );
        assert_eq!(report.transitions[0].added_action_order.len(), 1);
        assert_eq!(report.transitions[0].retired_action_order.len(), 1);
    }

    #[test]
    fn caller_thresholds_override_a_source_frontier_qualified_under_weaker_gates() {
        let claim = "EGFR signaling increases invasion";
        let mut request = request(vec![source_artifact(1, claim), source_artifact(2, claim)]);
        request.minimum_studies = 3;
        let report = replay_glioma_multi_study_context_epochs(&request).unwrap();

        assert_eq!(
            report.disposition,
            MultiStudyContextEpochReplayDisposition::Blocked
        );
        assert!(report.stable_frontier_order.is_empty());
        assert!(
            report.action_assessments[0]
                .qualified_epoch_order
                .is_empty()
        );
        assert_eq!(
            report.action_assessments[0].epoch_assessments[0].disposition,
            MultiStudyActionEpochDisposition::Underpowered
        );
        assert!(
            report.transitions[0]
                .newly_qualified_action_order
                .is_empty()
        );
    }

    #[test]
    fn source_epochs_must_be_consecutive_and_keep_study_group_identity() {
        let first = source_artifact(1, "EGFR signaling increases invasion");
        let mut skipped = source_artifact(3, "EGFR signaling increases invasion");
        let skipped_request = request(vec![first.clone(), skipped.clone()]);
        assert!(matches!(
            replay_glioma_multi_study_context_epochs(&skipped_request),
            Err(MultiStudyContextEpochReplayError::InvalidRequest(_))
        ));

        skipped.epoch = 2;
        skipped
            .study_group
            .insert("study-a".into(), "changed-group".into());
        for action in &mut skipped.actions {
            if action.study_order.iter().any(|study| study == "study-a") {
                action.independent_group_order = vec!["changed-group".into(), "group-b".into()];
            }
        }
        let input = digest_input_for_source(&skipped);
        skipped.digest = ContentHash::of_value(&input).unwrap();
        let changed_identity = request(vec![first, skipped]);
        assert!(matches!(
            replay_glioma_multi_study_context_epochs(&changed_identity),
            Err(MultiStudyContextEpochReplayError::InvalidRequest(_))
        ));
    }

    fn digest_input_for_source(source: &MultiStudyDecisionContextArtifact) -> serde_json::Value {
        serde_json::json!({
            "feature_id": source.feature_id,
            "output_schema": source.output_schema,
            "artifact_id": source.artifact_id,
            "objective": source.objective,
            "epoch": source.epoch,
            "boundary": source.boundary,
            "study_order": source.study_order,
            "study_group": source.study_group,
            "eligible_study_order": source.eligible_study_order,
            "omitted_study_order": source.omitted_study_order,
            "action_order": source.action_order,
            "frontier_order": source.frontier_order,
            "actions": source.actions,
            "outcome_order": source.outcome_order,
            "omissions": source.omissions,
            "negative_evidence_order": source.negative_evidence_order,
            "uncertainty_order": source.uncertainty_order,
            "disposition": source.disposition,
            "compatibility": source.compatibility,
        })
    }

    #[test]
    fn digest_restamping_cannot_change_temporal_assessment() {
        let claim = "IDH mutation changes state";
        let mut report = replay_glioma_multi_study_context_epochs(&request(vec![
            source_artifact(1, claim),
            source_artifact(2, claim),
        ]))
        .unwrap();
        report.stable_frontier_order.clear();
        report.digest = ContentHash::of_value(&digest_input(&report)).unwrap();

        assert!(matches!(
            report.validate(),
            Err(MultiStudyContextEpochReplayError::InvalidOutput(_))
        ));
    }

    #[test]
    fn source_metrics_must_recompute_from_the_retained_study_lineage() {
        let first = source_artifact(1, "EGFR signaling increases invasion");
        let mut second = source_artifact(2, "EGFR signaling increases invasion");
        second.actions[0].support_milli = 999;
        second.digest = ContentHash::of_value(&digest_input_for_source(&second)).unwrap();

        assert!(matches!(
            replay_glioma_multi_study_context_epochs(&request(vec![first, second])),
            Err(MultiStudyContextEpochReplayError::InvalidRequest(_))
        ));
    }

    #[test]
    fn negative_source_history_forces_researcher_review() {
        let claim = "IDH mutation changes state";
        let mut second = source_artifact(2, claim);
        second.negative_evidence_order = vec!["study-a:negative-result".into()];
        second.digest = ContentHash::of_value(&digest_input_for_source(&second)).unwrap();
        let report = replay_glioma_multi_study_context_epochs(&request(vec![
            source_artifact(1, claim),
            second,
        ]))
        .unwrap();

        assert_eq!(
            report.disposition,
            MultiStudyContextEpochReplayDisposition::Partial
        );
        assert_eq!(report.next_route, "glioma_researcher_workbench");
        assert!(
            report
                .negative_evidence_order
                .contains(&"epoch-2:study-a:negative-result".into())
        );
    }
}

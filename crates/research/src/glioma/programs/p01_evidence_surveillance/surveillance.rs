//! Continuous evidence-surveillance delta detection for preclinical glioma programs.
//!
//! This product compares two caller-supplied, local evidence snapshots and turns meaningful
//! additions, removals, state transitions, score shifts, and scope changes into bounded review or
//! revalidation actions. It is intentionally a change detector, not a literature-fetching oracle:
//! source bytes remain local, and unknown/stale/contradictory evidence drives work instead of
//! being silently promoted.

use crate::glioma::evidence::{EvidenceRecord, EvidenceSourceKind, EvidenceState};
use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P01-F09";
pub const OUTPUT_SCHEMA: &str = "GliomaEvidenceSurveillance1@1";
pub const MAX_RECORDS: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceSurveillanceRequest {
    pub objective: String,
    pub required_modalities: BTreeSet<GliomaModality>,
    pub required_model_systems: BTreeSet<GliomaModelSystem>,
    pub min_priority_milli: u16,
    pub max_actions: usize,
    pub score_shift_threshold_milli: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceChangeKind {
    Added,
    Removed,
    StateTransition,
    ScoreShift,
    ScopeShift,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceChange {
    pub change_id: String,
    pub evidence_id: String,
    pub kind: EvidenceChangeKind,
    pub previous_state: Option<EvidenceState>,
    pub current_state: Option<EvidenceState>,
    pub previous_source_kind: Option<EvidenceSourceKind>,
    pub current_source_kind: Option<EvidenceSourceKind>,
    pub score_shift_milli: i32,
    pub priority_milli: u16,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceSurveillanceActionKind {
    ReviewNewEvidence,
    RestoreRemovedEvidence,
    InvestigateContradiction,
    RevalidateNegative,
    RefreshStale,
    ResolveUnknown,
    ReassessScope,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceSurveillanceAction {
    pub action_id: String,
    pub evidence_id: String,
    pub kind: EvidenceSurveillanceActionKind,
    pub priority_milli: u16,
    pub required_modalities: Vec<GliomaModality>,
    pub required_model_systems: Vec<GliomaModelSystem>,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceSurveillanceDisposition {
    Qualified,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceSurveillance {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub previous_record_order: Vec<String>,
    pub current_record_order: Vec<String>,
    pub change_order: Vec<String>,
    pub changes: Vec<EvidenceChange>,
    pub action_order: Vec<String>,
    pub actions: Vec<EvidenceSurveillanceAction>,
    pub unresolved_order: Vec<String>,
    pub missing_modality_order: Vec<GliomaModality>,
    pub missing_model_order: Vec<GliomaModelSystem>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: EvidenceSurveillanceDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EvidenceSurveillanceError {
    #[error("evidence surveillance request is invalid: {0}")]
    InvalidRequest(String),
    #[error("evidence surveillance record is invalid: {0}")]
    InvalidRecord(String),
    #[error("evidence surveillance output is invalid: {0}")]
    InvalidOutput(String),
    #[error("evidence surveillance digest failed: {0}")]
    Digest(String),
}

fn digest_input(output: &EvidenceSurveillance) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "previous_record_order": output.previous_record_order,
        "current_record_order": output.current_record_order,
        "change_order": output.change_order,
        "changes": output.changes,
        "action_order": output.action_order,
        "actions": output.actions,
        "unresolved_order": output.unresolved_order,
        "missing_modality_order": output.missing_modality_order,
        "missing_model_order": output.missing_model_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

impl EvidenceSurveillance {
    pub fn validate(&self) -> Result<(), EvidenceSurveillanceError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self
                .previous_record_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self
                .current_record_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self.change_order.windows(2).any(|pair| pair[0] >= pair[1])
            || self
                .unresolved_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self
                .missing_modality_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self
                .missing_model_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self
                .changes
                .windows(2)
                .any(|pair| pair[0].change_id >= pair[1].change_id)
            || self.changes.iter().any(|change| {
                change.change_id.trim().is_empty()
                    || change.evidence_id.trim().is_empty()
                    || change.priority_milli > 1_000
                    || change.reason.trim().is_empty()
            })
            || self.actions.windows(2).any(|pair| {
                pair[0].priority_milli < pair[1].priority_milli
                    || (pair[0].priority_milli == pair[1].priority_milli
                        && pair[0].action_id > pair[1].action_id)
            })
            || self.actions.iter().any(|action| {
                action.action_id.trim().is_empty()
                    || action.evidence_id.trim().is_empty()
                    || action.priority_milli > 1_000
                    || action.rationale.trim().is_empty()
                    || action
                        .required_modalities
                        .windows(2)
                        .any(|pair| pair[0] >= pair[1])
                    || action
                        .required_model_systems
                        .windows(2)
                        .any(|pair| pair[0] >= pair[1])
            })
            || self
                .negative_evidence
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self.uncertainty.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(EvidenceSurveillanceError::InvalidOutput(
                "identity, ordering, score bounds, or rationale is invalid".into(),
            ));
        }
        let previous = self
            .previous_record_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let current = self
            .current_record_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let changes = self
            .changes
            .iter()
            .map(|change| change.evidence_id.clone())
            .collect::<BTreeSet<_>>();
        let actions = self
            .actions
            .iter()
            .map(|action| action.evidence_id.clone())
            .collect::<BTreeSet<_>>();
        if previous.len() != self.previous_record_order.len()
            || current.len() != self.current_record_order.len()
            || changes
                .iter()
                .any(|id| !previous.contains(id) && !current.contains(id))
            || actions.iter().any(|id| !changes.contains(id))
            || self.change_order.iter().cloned().collect::<BTreeSet<_>>()
                != self
                    .changes
                    .iter()
                    .map(|change| change.change_id.clone())
                    .collect::<BTreeSet<_>>()
            || self.action_order.iter().cloned().collect::<BTreeSet<_>>()
                != self
                    .actions
                    .iter()
                    .map(|action| action.action_id.clone())
                    .collect::<BTreeSet<_>>()
            || self.action_order
                != self
                    .actions
                    .iter()
                    .map(|action| action.action_id.clone())
                    .collect::<Vec<_>>()
            || self.change_order
                != self
                    .changes
                    .iter()
                    .map(|change| change.change_id.clone())
                    .collect::<Vec<_>>()
        {
            return Err(EvidenceSurveillanceError::InvalidOutput(
                "record, change, and action partitions do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| EvidenceSurveillanceError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(EvidenceSurveillanceError::InvalidOutput(
                "digest is not bound to evidence surveillance".into(),
            ));
        }
        Ok(())
    }
}

fn record_score(record: &EvidenceRecord) -> i32 {
    ((record.relevance_milli as i32
        + record.quality_milli as i32
        + record.reproducibility_milli as i32)
        / 3)
    .clamp(0, 1_000)
}

fn transition_priority(
    previous: EvidenceState,
    current: EvidenceState,
) -> (u16, EvidenceSurveillanceActionKind, &'static str) {
    let kind = match current {
        EvidenceState::Contradicted => (
            1_000,
            EvidenceSurveillanceActionKind::InvestigateContradiction,
            "evidence became contradictory",
        ),
        EvidenceState::Negative => (
            900,
            EvidenceSurveillanceActionKind::RevalidateNegative,
            "evidence became negative",
        ),
        EvidenceState::Stale => (
            850,
            EvidenceSurveillanceActionKind::RefreshStale,
            "evidence became stale",
        ),
        EvidenceState::Unknown | EvidenceState::Unmeasured => (
            800,
            EvidenceSurveillanceActionKind::ResolveUnknown,
            "evidence became unresolved",
        ),
        EvidenceState::Supported => (
            650,
            EvidenceSurveillanceActionKind::ReviewNewEvidence,
            "evidence is now supported and requires review",
        ),
    };
    if matches!(
        previous,
        EvidenceState::Contradicted | EvidenceState::Negative
    ) && current == EvidenceState::Supported
    {
        (
            750,
            EvidenceSurveillanceActionKind::ReviewNewEvidence,
            "previously disconfirming evidence changed to support",
        )
    } else {
        kind
    }
}

pub fn surveil_glioma_evidence(
    request: &EvidenceSurveillanceRequest,
    previous: &[EvidenceRecord],
    current: &[EvidenceRecord],
) -> Result<EvidenceSurveillance, EvidenceSurveillanceError> {
    if request.objective.trim().is_empty()
        || request.min_priority_milli > 1_000
        || request.max_actions == 0
        || request.score_shift_threshold_milli == 0
        || previous.len() > MAX_RECORDS
        || current.len() > MAX_RECORDS
    {
        return Err(EvidenceSurveillanceError::InvalidRequest(
            "objective, action/priority/score thresholds, and bounded snapshots are required"
                .into(),
        ));
    }
    let mut previous_map = BTreeMap::<String, &EvidenceRecord>::new();
    let mut current_map = BTreeMap::<String, &EvidenceRecord>::new();
    for (map, records) in [(&mut previous_map, previous), (&mut current_map, current)] {
        for record in records {
            record
                .source_artifact
                .validate()
                .map_err(|error| EvidenceSurveillanceError::InvalidRecord(error.to_string()))?;
            if record.evidence_id.trim().is_empty()
                || record.claim.trim().is_empty()
                || record.scope.trim().is_empty()
                || record.relevance_milli > 1_000
                || record.quality_milli > 1_000
                || record.reproducibility_milli > 1_000
                || map.insert(record.evidence_id.clone(), record).is_some()
            {
                return Err(EvidenceSurveillanceError::InvalidRecord(
                    "evidence identity, claim, scope, score, or uniqueness is invalid".into(),
                ));
            }
        }
    }
    let previous_order = previous_map.keys().cloned().collect::<Vec<_>>();
    let current_order = current_map.keys().cloned().collect::<Vec<_>>();
    let all_ids = previous_map
        .keys()
        .chain(current_map.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut changes = Vec::new();
    let mut unresolved = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for evidence_id in all_ids {
        let old = previous_map.get(&evidence_id).copied();
        let new = current_map.get(&evidence_id).copied();
        let (
            kind,
            priority,
            reason,
            score_shift,
            previous_state,
            current_state,
            previous_source_kind,
            current_source_kind,
        ) = match (old, new) {
            (None, Some(record)) => {
                let priority = (650 + record_score(record) / 3).min(1_000) as u16;
                (
                    EvidenceChangeKind::Added,
                    priority,
                    "new evidence entered the current snapshot",
                    record_score(record),
                    None,
                    Some(record.state),
                    None,
                    Some(record.source_kind),
                )
            }
            (Some(record), None) => (
                EvidenceChangeKind::Removed,
                850,
                "previous evidence disappeared from the current snapshot",
                -record_score(record),
                Some(record.state),
                None,
                Some(record.source_kind),
                None,
            ),
            (Some(old), Some(new)) if old.state != new.state => {
                let (priority, _, reason) = transition_priority(old.state, new.state);
                (
                    EvidenceChangeKind::StateTransition,
                    priority,
                    reason,
                    record_score(new) - record_score(old),
                    Some(old.state),
                    Some(new.state),
                    Some(old.source_kind),
                    Some(new.source_kind),
                )
            }
            (Some(old), Some(new))
                if old.modality != new.modality
                    || old.model_system != new.model_system
                    || old.scope != new.scope =>
            {
                (
                    EvidenceChangeKind::ScopeShift,
                    700,
                    "evidence scope or modality/model binding changed",
                    record_score(new) - record_score(old),
                    Some(old.state),
                    Some(new.state),
                    Some(old.source_kind),
                    Some(new.source_kind),
                )
            }
            (Some(old), Some(new)) => {
                let shift = record_score(new) - record_score(old);
                if shift.unsigned_abs() < u32::from(request.score_shift_threshold_milli) {
                    continue;
                }
                (
                    EvidenceChangeKind::ScoreShift,
                    (300 + shift.unsigned_abs().min(700)) as u16,
                    "evidence quality/relevance/reproducibility score shifted",
                    shift,
                    Some(old.state),
                    Some(new.state),
                    Some(old.source_kind),
                    Some(new.source_kind),
                )
            }
            (None, None) => continue,
        };
        if matches!(
            current_state,
            Some(EvidenceState::Unknown | EvidenceState::Unmeasured | EvidenceState::Stale)
        ) {
            unresolved.insert(evidence_id.clone());
        }
        if matches!(
            current_state,
            Some(EvidenceState::Contradicted | EvidenceState::Negative)
        ) || matches!(kind, EvidenceChangeKind::Removed)
        {
            negative.insert(evidence_id.clone());
        }
        changes.push(EvidenceChange {
            change_id: format!("{}:{:?}", evidence_id, kind),
            evidence_id,
            kind,
            previous_state,
            current_state,
            previous_source_kind,
            current_source_kind,
            score_shift_milli: score_shift,
            priority_milli: priority,
            reason: reason.into(),
        });
    }
    changes.sort_by(|left, right| left.change_id.cmp(&right.change_id));
    let mut actions = changes
        .iter()
        .filter_map(|change| {
            if change.priority_milli < request.min_priority_milli {
                return None;
            }
            let current_record = current_map.get(&change.evidence_id).copied();
            let action_kind = match change.kind {
                EvidenceChangeKind::Added | EvidenceChangeKind::ScoreShift => {
                    EvidenceSurveillanceActionKind::ReviewNewEvidence
                }
                EvidenceChangeKind::Removed => {
                    EvidenceSurveillanceActionKind::RestoreRemovedEvidence
                }
                EvidenceChangeKind::ScopeShift => EvidenceSurveillanceActionKind::ReassessScope,
                EvidenceChangeKind::StateTransition => {
                    transition_priority(
                        change.previous_state.unwrap_or(EvidenceState::Unknown),
                        change.current_state.unwrap_or(EvidenceState::Unknown),
                    )
                    .1
                }
            };
            Some(EvidenceSurveillanceAction {
                action_id: format!("surveillance:{}:{:?}", change.evidence_id, action_kind),
                evidence_id: change.evidence_id.clone(),
                kind: action_kind,
                priority_milli: change.priority_milli,
                required_modalities: current_record
                    .map(|record| vec![record.modality])
                    .unwrap_or_else(|| request.required_modalities.iter().copied().collect()),
                required_model_systems: current_record
                    .and_then(|record| record.model_system)
                    .map(|model| vec![model])
                    .unwrap_or_else(|| request.required_model_systems.iter().copied().collect()),
                rationale: change.reason.clone(),
            })
        })
        .collect::<Vec<_>>();
    actions.sort_by(|left, right| {
        right
            .priority_milli
            .cmp(&left.priority_milli)
            .then_with(|| left.action_id.cmp(&right.action_id))
    });
    actions.truncate(request.max_actions);
    let action_order = actions
        .iter()
        .map(|action| action.action_id.clone())
        .collect::<Vec<_>>();
    let change_order = changes
        .iter()
        .map(|change| change.change_id.clone())
        .collect::<Vec<_>>();
    let selected_modalities = current_map
        .values()
        .map(|record| record.modality)
        .collect::<BTreeSet<_>>();
    let selected_models = current_map
        .values()
        .filter_map(|record| record.model_system)
        .collect::<BTreeSet<_>>();
    let missing_modality_order = request
        .required_modalities
        .difference(&selected_modalities)
        .copied()
        .collect::<Vec<_>>();
    let missing_model_order = request
        .required_model_systems
        .difference(&selected_models)
        .copied()
        .collect::<Vec<_>>();
    let mut uncertainty = BTreeSet::new();
    if previous.is_empty() {
        uncertainty.insert("no-previous-snapshot-for-delta-comparison".into());
    }
    if current.is_empty() {
        uncertainty.insert("current-snapshot-is-empty".into());
    }
    if !missing_modality_order.is_empty() {
        uncertainty.insert("required-modality-coverage-incomplete".into());
    }
    if !missing_model_order.is_empty() {
        uncertainty.insert("required-model-coverage-incomplete".into());
    }
    if !unresolved.is_empty() {
        uncertainty.insert("current-snapshot-retains-stale-or-unknown-evidence".into());
    }
    let disposition = if current.is_empty() {
        EvidenceSurveillanceDisposition::Unresolved
    } else if !uncertainty.is_empty() || !unresolved.is_empty() {
        EvidenceSurveillanceDisposition::Partial
    } else {
        EvidenceSurveillanceDisposition::Qualified
    };
    let mut output = EvidenceSurveillance {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        previous_record_order: previous_order,
        current_record_order: current_order,
        change_order,
        changes,
        action_order,
        actions,
        unresolved_order: unresolved.into_iter().collect(),
        missing_modality_order,
        missing_model_order,
        negative_evidence: negative.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_value(&serde_json::json!({}))
            .map_err(|error| EvidenceSurveillanceError::Digest(error.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| EvidenceSurveillanceError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::evidence::EvidenceSourceKind;

    fn hash(id: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"id": id})).unwrap()
    }

    fn record(id: &str, state: EvidenceState, quality: u16) -> EvidenceRecord {
        EvidenceRecord {
            evidence_id: id.into(),
            source_artifact: crate::glioma_engine::LocalArtifactRef {
                artifact_id: format!("artifact-{id}"),
                content_hash: hash(id),
                content_type: "application/vnd.aurora.glioma-evidence+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
            source_kind: EvidenceSourceKind::Dataset,
            claim: format!("claim-{id}"),
            scope: "preclinical glioma".into(),
            modality: GliomaModality::Genomics,
            model_system: Some(GliomaModelSystem::Organoid),
            state,
            relevance_milli: quality,
            quality_milli: quality,
            reproducibility_milli: quality,
            release_epoch: 1,
        }
    }

    fn request() -> EvidenceSurveillanceRequest {
        EvidenceSurveillanceRequest {
            objective: "monitor invasion evidence".into(),
            required_modalities: BTreeSet::from([GliomaModality::Genomics]),
            required_model_systems: BTreeSet::from([GliomaModelSystem::Organoid]),
            min_priority_milli: 500,
            max_actions: 8,
            score_shift_threshold_milli: 50,
        }
    }

    #[test]
    fn contradiction_transition_becomes_high_priority_action() {
        let output = surveil_glioma_evidence(
            &request(),
            &[record("e1", EvidenceState::Supported, 900)],
            &[
                record("e1", EvidenceState::Contradicted, 900),
                record("e2", EvidenceState::Supported, 850),
            ],
        )
        .unwrap();
        assert_eq!(
            output.disposition,
            EvidenceSurveillanceDisposition::Qualified
        );
        assert!(output
            .changes
            .iter()
            .any(|change| change.kind == EvidenceChangeKind::StateTransition));
        assert_eq!(
            output.actions[0].kind,
            EvidenceSurveillanceActionKind::InvestigateContradiction
        );
        output.validate().unwrap();
    }

    #[test]
    fn stale_and_removed_evidence_remain_partial_and_replay_stable() {
        let previous = vec![
            record("e1", EvidenceState::Supported, 900),
            record("e2", EvidenceState::Supported, 900),
        ];
        let current = vec![record("e1", EvidenceState::Stale, 900)];
        let first = surveil_glioma_evidence(&request(), &previous, &current).unwrap();
        let second = surveil_glioma_evidence(&request(), &previous, &current).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.disposition, EvidenceSurveillanceDisposition::Partial);
        assert!(first.negative_evidence.contains(&"e2".to_string()));
        assert!(first.unresolved_order.contains(&"e1".to_string()));
    }
}

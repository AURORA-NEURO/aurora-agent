//! Epoch-aware decision-context replay for autonomous preclinical glioma research.
//!
//! A decision context is a plan snapshot, not a timeless instruction.  This feature replays
//! successive snapshots against explicit local outcomes so stale actions cannot silently survive
//! a study update.  Added, retired, stable, completed, negative, and unresolved actions remain
//! separately observable; the replay never converts a forecast into a biological observation.

use super::context_compiler::DecisionContext;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P04-F02";
pub const OUTPUT_SCHEMA: &str = "GliomaDecisionContextReplay1@1";
pub const MAX_EPOCHS: usize = 64;
pub const MAX_OUTCOMES_PER_EPOCH: usize = 2_048;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionContextActionOutcomeStatus {
    Completed,
    Negative,
    Failed,
    Blocked,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionContextActionOutcome {
    pub action_id: String,
    pub status: DecisionContextActionOutcomeStatus,
    pub result_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionContextEpoch {
    pub epoch_id: String,
    pub context: DecisionContext,
    pub outcome_order: Vec<DecisionContextActionOutcome>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionContextReplayRequest {
    pub objective: String,
    pub epoch_order: Vec<DecisionContextEpoch>,
    pub min_promotion_delta_milli: u16,
    pub max_epochs: usize,
    pub preserve_negative_results: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionContextReplayTransition {
    pub from_epoch_id: String,
    pub to_epoch_id: String,
    pub added_action_order: Vec<String>,
    pub retired_action_order: Vec<String>,
    pub stable_action_order: Vec<String>,
    pub completed_action_order: Vec<String>,
    pub negative_action_order: Vec<String>,
    pub unresolved_action_order: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionContextReplayDisposition {
    Ready,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionContextReplay {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub epoch_order: Vec<String>,
    pub context_digest_order: Vec<ContentHash>,
    pub transition_order: Vec<String>,
    pub transitions: Vec<DecisionContextReplayTransition>,
    pub stable_action_order: Vec<String>,
    pub promoted_action_order: Vec<String>,
    pub retired_action_order: Vec<String>,
    pub unresolved_action_order: Vec<String>,
    pub negative_result_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub disposition: DecisionContextReplayDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DecisionContextReplayError {
    #[error("decision-context replay request is invalid: {0}")]
    InvalidRequest(String),
    #[error("decision-context replay epoch is invalid: {0}")]
    InvalidEpoch(String),
    #[error("decision-context replay output is invalid: {0}")]
    InvalidOutput(String),
    #[error("decision-context replay digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique_nonempty(values: &[String]) -> bool {
    values.iter().all(|value| !value.trim().is_empty())
        && values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

fn digest_input(output: &DecisionContextReplay) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "epoch_order": output.epoch_order,
        "context_digest_order": output.context_digest_order,
        "transition_order": output.transition_order,
        "transitions": output.transitions,
        "stable_action_order": output.stable_action_order,
        "promoted_action_order": output.promoted_action_order,
        "retired_action_order": output.retired_action_order,
        "unresolved_action_order": output.unresolved_action_order,
        "negative_result_order": output.negative_result_order,
        "omission_order": output.omission_order,
        "disposition": output.disposition,
    })
}

impl DecisionContextReplay {
    pub fn validate(&self) -> Result<(), DecisionContextReplayError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.epoch_order.len() < 2
            || !unique_nonempty(&self.epoch_order)
            || self.context_digest_order.len() != self.epoch_order.len()
            || self
                .context_digest_order
                .iter()
                .any(|digest| digest.as_str().len() != 64)
            || self.transition_order.len() != self.transitions.len()
            || !unique_nonempty(&self.transition_order)
            || self.transitions.iter().any(|transition| {
                transition.from_epoch_id.trim().is_empty()
                    || transition.to_epoch_id.trim().is_empty()
                    || !canonical(&transition.added_action_order)
                    || !canonical(&transition.retired_action_order)
                    || !canonical(&transition.stable_action_order)
                    || !canonical(&transition.completed_action_order)
                    || !canonical(&transition.negative_action_order)
                    || !canonical(&transition.unresolved_action_order)
            })
            || !canonical(&self.stable_action_order)
            || !canonical(&self.promoted_action_order)
            || !canonical(&self.retired_action_order)
            || !canonical(&self.unresolved_action_order)
            || !canonical(&self.negative_result_order)
            || !canonical(&self.omission_order)
            || self.digest.as_str().len() != 64
        {
            return Err(DecisionContextReplayError::InvalidOutput(
                "identity, epoch alignment, canonical partitions, or digest shape is invalid"
                    .into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| DecisionContextReplayError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(DecisionContextReplayError::Digest(
                "decision-context replay digest does not match canonical content".into(),
            ));
        }
        Ok(())
    }
}

fn ordered_difference(left: &BTreeSet<String>, right: &BTreeSet<String>) -> Vec<String> {
    left.difference(right).cloned().collect()
}

fn action_ids(context: &DecisionContext) -> BTreeSet<String> {
    context.action_order.iter().cloned().collect()
}

fn outcome_partition(
    epoch: &DecisionContextEpoch,
) -> (BTreeSet<String>, BTreeSet<String>, BTreeSet<String>) {
    let mut completed = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    for outcome in &epoch.outcome_order {
        match outcome.status {
            DecisionContextActionOutcomeStatus::Completed => {
                completed.insert(outcome.action_id.clone());
            }
            DecisionContextActionOutcomeStatus::Negative => {
                negative.insert(outcome.action_id.clone());
            }
            DecisionContextActionOutcomeStatus::Failed
            | DecisionContextActionOutcomeStatus::Blocked
            | DecisionContextActionOutcomeStatus::Unknown => {
                unresolved.insert(outcome.action_id.clone());
            }
        }
    }
    (completed, negative, unresolved)
}

fn validate_request(
    request: &DecisionContextReplayRequest,
) -> Result<(), DecisionContextReplayError> {
    if request.objective.trim().is_empty()
        || request.epoch_order.len() < 2
        || request.epoch_order.len() > MAX_EPOCHS
        || request.max_epochs == 0
        || request.max_epochs > MAX_EPOCHS
        || request.epoch_order.len() > request.max_epochs
    {
        return Err(DecisionContextReplayError::InvalidRequest(
            "objective, at least two epochs, and bounded epoch limits are required".into(),
        ));
    }
    let mut epoch_ids = BTreeSet::new();
    for epoch in &request.epoch_order {
        if epoch.epoch_id.trim().is_empty() || !epoch_ids.insert(epoch.epoch_id.clone()) {
            return Err(DecisionContextReplayError::InvalidRequest(
                "epoch ids must be non-empty and unique".into(),
            ));
        }
        if epoch.context.objective != request.objective {
            return Err(DecisionContextReplayError::InvalidEpoch(format!(
                "epoch {} objective does not match replay objective",
                epoch.epoch_id
            )));
        }
        epoch
            .context
            .validate()
            .map_err(|error| DecisionContextReplayError::InvalidEpoch(error.to_string()))?;
        if epoch.outcome_order.len() > MAX_OUTCOMES_PER_EPOCH {
            return Err(DecisionContextReplayError::InvalidEpoch(format!(
                "epoch {} exceeds the outcome bound",
                epoch.epoch_id
            )));
        }
        let action_ids = action_ids(&epoch.context);
        let mut outcome_ids = BTreeSet::new();
        for outcome in &epoch.outcome_order {
            if outcome.action_id.trim().is_empty()
                || !outcome_ids.insert(outcome.action_id.clone())
                || !action_ids.contains(&outcome.action_id)
                || outcome.result_digest.as_str().len() != 64
            {
                return Err(DecisionContextReplayError::InvalidEpoch(format!(
                    "epoch {} contains an unknown, duplicate, or undigested action outcome",
                    epoch.epoch_id
                )));
            }
        }
    }
    Ok(())
}

/// Replay decision-context snapshots and local action outcomes across study epochs.
pub fn replay_glioma_decision_context(
    request: &DecisionContextReplayRequest,
) -> Result<DecisionContextReplay, DecisionContextReplayError> {
    validate_request(request)?;

    let mut stable = action_ids(&request.epoch_order[0].context);
    let mut promoted = BTreeSet::new();
    let mut retired = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut transitions = Vec::new();
    for epoch in &request.epoch_order {
        omissions.extend(epoch.context.omission_order.iter().cloned());
        let (_, epoch_negative, epoch_unresolved) = outcome_partition(epoch);
        negative.extend(epoch_negative);
        unresolved.extend(epoch_unresolved);
    }

    for pair in request.epoch_order.windows(2) {
        let previous = &pair[0];
        let current = &pair[1];
        let previous_ids = action_ids(&previous.context);
        let current_ids = action_ids(&current.context);
        let added = ordered_difference(&current_ids, &previous_ids);
        let removed = ordered_difference(&previous_ids, &current_ids);
        let shared = previous_ids
            .intersection(&current_ids)
            .cloned()
            .collect::<BTreeSet<_>>();
        let (completed, negative_outcomes, unresolved_outcomes) = outcome_partition(current);
        for action in &added {
            promoted.insert(action.clone());
        }
        for action in &removed {
            retired.insert(action.clone());
        }
        for action in &shared {
            let previous_priority = previous
                .context
                .actions
                .iter()
                .find(|candidate| candidate.action_id == *action)
                .map(|candidate| candidate.priority_milli)
                .unwrap_or_default();
            let current_priority = current
                .context
                .actions
                .iter()
                .find(|candidate| candidate.action_id == *action)
                .map(|candidate| candidate.priority_milli)
                .unwrap_or_default();
            if current_priority.saturating_sub(previous_priority)
                >= request.min_promotion_delta_milli
            {
                promoted.insert(action.clone());
            }
        }
        transitions.push(DecisionContextReplayTransition {
            from_epoch_id: previous.epoch_id.clone(),
            to_epoch_id: current.epoch_id.clone(),
            added_action_order: added,
            retired_action_order: removed,
            stable_action_order: shared.into_iter().collect(),
            completed_action_order: completed.into_iter().collect(),
            negative_action_order: negative_outcomes.into_iter().collect(),
            unresolved_action_order: unresolved_outcomes.into_iter().collect(),
        });
    }
    for epoch in &request.epoch_order {
        stable = stable
            .intersection(&action_ids(&epoch.context))
            .cloned()
            .collect();
    }
    if !request.preserve_negative_results {
        negative.clear();
    }
    let has_unresolved_context = request.epoch_order.iter().any(|epoch| {
        matches!(
            epoch.context.disposition,
            super::context_compiler::DecisionContextDisposition::Unresolved
        )
    });
    let has_partial_context = request.epoch_order.iter().any(|epoch| {
        matches!(
            epoch.context.disposition,
            super::context_compiler::DecisionContextDisposition::Partial
        )
    });
    let disposition = if has_unresolved_context || !unresolved.is_empty() {
        DecisionContextReplayDisposition::Blocked
    } else if has_partial_context || !omissions.is_empty() {
        DecisionContextReplayDisposition::Partial
    } else {
        DecisionContextReplayDisposition::Ready
    };
    let epoch_order = request
        .epoch_order
        .iter()
        .map(|epoch| epoch.epoch_id.clone())
        .collect::<Vec<_>>();
    let context_digest_order = request
        .epoch_order
        .iter()
        .map(|epoch| epoch.context.digest.clone())
        .collect::<Vec<_>>();
    let transition_order = transitions
        .iter()
        .map(|transition| format!("{}->{}", transition.from_epoch_id, transition.to_epoch_id))
        .collect::<Vec<_>>();
    let mut output = DecisionContextReplay {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        epoch_order,
        context_digest_order,
        transition_order,
        transitions,
        stable_action_order: stable.into_iter().collect(),
        promoted_action_order: promoted.into_iter().collect(),
        retired_action_order: retired.into_iter().collect(),
        unresolved_action_order: unresolved.into_iter().collect(),
        negative_result_order: negative.into_iter().collect(),
        omission_order: omissions.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"placeholder"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| DecisionContextReplayError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::evidence::{EvidenceRecord, EvidenceSourceKind, EvidenceState};
    use crate::glioma::programs::p02_evidence_knowledge::{
        compile_typed_knowledge, KnowledgeRequest,
    };
    use crate::glioma::programs::p04_decision_context::context_compiler::{
        compile_decision_context, DecisionContextRequest,
    };
    use crate::glioma_engine::{GliomaModality, GliomaModelSystem, LocalArtifactRef};
    use std::collections::BTreeSet;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn context(statement: &str) -> DecisionContext {
        let record = EvidenceRecord {
            evidence_id: format!("evidence-{statement}"),
            source_artifact: LocalArtifactRef {
                artifact_id: format!("artifact-{statement}"),
                content_hash: hash(statement),
                content_type: "application/vnd.aurora.glioma-evidence+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
            source_kind: EvidenceSourceKind::Dataset,
            claim: statement.into(),
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
                objective: "replay glioma decision context".into(),
                required_modalities: BTreeSet::from([GliomaModality::Genomics]),
                required_model_systems: BTreeSet::from([GliomaModelSystem::Organoid]),
                min_support_milli: 700,
                min_sources_per_claim: 1,
                max_claims: 8,
            },
            &[record],
        )
        .unwrap();
        compile_decision_context(
            &DecisionContextRequest {
                objective: "replay glioma decision context".into(),
                max_actions: 8,
                default_cost_units: 5,
            },
            &knowledge,
        )
        .unwrap()
    }

    fn epoch(id: &str, context: DecisionContext) -> DecisionContextEpoch {
        DecisionContextEpoch {
            epoch_id: id.into(),
            context,
            outcome_order: Vec::new(),
        }
    }

    #[test]
    fn replay_detects_promotion_retirement_and_negative_results() {
        let first = context("EGFR signaling increases invasion");
        let second = context("NF1 loss increases invasion");
        let mut final_epoch = epoch("epoch-2", second.clone());
        final_epoch
            .outcome_order
            .push(DecisionContextActionOutcome {
                action_id: second.action_order[0].clone(),
                status: DecisionContextActionOutcomeStatus::Negative,
                result_digest: hash("negative-result"),
            });
        let output = replay_glioma_decision_context(&DecisionContextReplayRequest {
            objective: "replay glioma decision context".into(),
            epoch_order: vec![epoch("epoch-1", first), final_epoch],
            min_promotion_delta_milli: 50,
            max_epochs: 8,
            preserve_negative_results: true,
        })
        .unwrap();
        assert_eq!(output.disposition, DecisionContextReplayDisposition::Ready);
        assert_eq!(output.promoted_action_order, second.action_order);
        assert_eq!(output.negative_result_order, second.action_order);
        assert_eq!(output.retired_action_order.len(), 1);
        output.validate().unwrap();
    }

    #[test]
    fn replay_marks_unknown_outcomes_blocked_and_is_deterministic() {
        let context = context("IDH mutation changes state");
        let action_id = context.action_order[0].clone();
        let mut second = epoch("epoch-2", context.clone());
        second.outcome_order.push(DecisionContextActionOutcome {
            action_id,
            status: DecisionContextActionOutcomeStatus::Unknown,
            result_digest: hash("unknown-result"),
        });
        let request = DecisionContextReplayRequest {
            objective: "replay glioma decision context".into(),
            epoch_order: vec![epoch("epoch-1", context), second],
            min_promotion_delta_milli: 100,
            max_epochs: 8,
            preserve_negative_results: true,
        };
        let first = replay_glioma_decision_context(&request).unwrap();
        let second = replay_glioma_decision_context(&request).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.disposition, DecisionContextReplayDisposition::Blocked);
        assert_eq!(first.unresolved_action_order.len(), 1);
    }
}

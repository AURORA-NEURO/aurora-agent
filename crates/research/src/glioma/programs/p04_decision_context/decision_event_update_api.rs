//! Event-sourced updates for a live preclinical glioma decision context.
//!
//! This is the prospective counterpart to context compilation and replay.  A site can submit
//! signed, content-addressed evidence/QC/resource events against a context anchor; the engine
//! orders them deterministically, deduplicates retries, rejects stale anchors, and emits an
//! immutable context epoch after every applied event.  Actions invalidated by contradiction,
//! quality failure, resource exhaustion, or negative outcomes are removed from the executable
//! frontier and retained in explicit deferred/negative partitions.

use super::context_compiler::DecisionContext;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P04-F23";
pub const OUTPUT_SCHEMA: &str = "GliomaDecisionContextUpdate1@1";
pub const MAX_EVENTS: usize = 2_048;
pub const MAX_SUBJECTS: usize = 256;
pub const MAX_TEXT_LEN: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionEventKind {
    EvidenceAdded,
    EvidenceContradiction,
    QualityFailure,
    ResourceExhausted,
    ActionCompleted,
    ActionNegative,
    ActionBlocked,
}

impl DecisionEventKind {
    fn invalidates_actions(self) -> bool {
        matches!(
            self,
            Self::EvidenceContradiction
                | Self::QualityFailure
                | Self::ResourceExhausted
                | Self::ActionCompleted
                | Self::ActionNegative
                | Self::ActionBlocked
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionContextEvent {
    pub event_id: String,
    pub sequence: u64,
    pub observed_tick: u64,
    pub context_anchor_digest: ContentHash,
    pub kind: DecisionEventKind,
    pub subject_order: Vec<String>,
    pub payload_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub reason: String,
    pub event_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionContextUpdateEpoch {
    pub epoch_id: String,
    pub sequence: u64,
    pub event_id: String,
    pub event_kind: DecisionEventKind,
    pub prior_context_digest: ContentHash,
    pub context_digest: ContentHash,
    pub invalidated_action_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub reconsideration_required: bool,
    pub epoch_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionContextUpdateRequest {
    pub base_context: DecisionContext,
    pub event_order: Vec<DecisionContextEvent>,
    pub last_applied_sequence: u64,
    pub max_events: usize,
    pub current_tick: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionContextUpdateDisposition {
    Applied,
    Noop,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionContextUpdateResult {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub base_context_digest: ContentHash,
    pub final_context: DecisionContext,
    pub epoch_order: Vec<DecisionContextUpdateEpoch>,
    pub applied_event_order: Vec<String>,
    pub duplicate_event_order: Vec<String>,
    pub stale_event_order: Vec<String>,
    pub rejected_event_order: Vec<String>,
    pub invalidated_action_order: Vec<String>,
    pub reconsideration_order: Vec<String>,
    pub replay_cursor: u64,
    pub disposition: DecisionContextUpdateDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DecisionContextUpdateError {
    #[error("decision-context update request is invalid: {0}")]
    InvalidRequest(String),
    #[error("decision-context event is invalid: {0}")]
    InvalidEvent(String),
    #[error("decision-context update output is invalid: {0}")]
    InvalidOutput(String),
    #[error("decision-context update digest failed: {0}")]
    Digest(String),
}

fn safe_text(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= MAX_TEXT_LEN
        && !value.chars().any(|character| character.is_control())
}

fn valid_hash(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique_bounded(values: &[String], max: usize) -> bool {
    values.len() <= max
        && values.iter().all(|value| safe_text(value))
        && values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

fn event_body(event: &DecisionContextEvent) -> serde_json::Value {
    serde_json::json!({
        "event_id": event.event_id,
        "sequence": event.sequence,
        "observed_tick": event.observed_tick,
        "context_anchor_digest": event.context_anchor_digest,
        "kind": event.kind,
        "subject_order": event.subject_order,
        "payload_digest": event.payload_digest,
        "provenance_digest": event.provenance_digest,
        "reason": event.reason,
    })
}

fn context_body(context: &DecisionContext) -> serde_json::Value {
    serde_json::json!({
        "feature_id": context.feature_id,
        "output_schema": context.output_schema,
        "objective": context.objective,
        "claim_order": context.claim_order,
        "actions": context.actions,
        "action_order": context.action_order,
        "deferred_action_order": context.deferred_action_order,
        "omission_order": context.omission_order,
        "negative_evidence_order": context.negative_evidence_order,
        "uncertainty_order": context.uncertainty_order,
        "disposition": context.disposition,
    })
}

fn epoch_body(epoch: &DecisionContextUpdateEpoch) -> serde_json::Value {
    serde_json::json!({
        "epoch_id": epoch.epoch_id,
        "sequence": epoch.sequence,
        "event_id": epoch.event_id,
        "event_kind": epoch.event_kind,
        "prior_context_digest": epoch.prior_context_digest,
        "context_digest": epoch.context_digest,
        "invalidated_action_order": epoch.invalidated_action_order,
        "omission_order": epoch.omission_order,
        "negative_evidence_order": epoch.negative_evidence_order,
        "uncertainty_order": epoch.uncertainty_order,
        "reconsideration_required": epoch.reconsideration_required,
    })
}

fn result_body(result: &DecisionContextUpdateResult) -> serde_json::Value {
    serde_json::json!({
        "feature_id": result.feature_id,
        "output_schema": result.output_schema,
        "objective": result.objective,
        "base_context_digest": result.base_context_digest,
        "final_context": result.final_context,
        "epoch_order": result.epoch_order,
        "applied_event_order": result.applied_event_order,
        "duplicate_event_order": result.duplicate_event_order,
        "stale_event_order": result.stale_event_order,
        "rejected_event_order": result.rejected_event_order,
        "invalidated_action_order": result.invalidated_action_order,
        "reconsideration_order": result.reconsideration_order,
        "replay_cursor": result.replay_cursor,
        "disposition": result.disposition,
    })
}

fn seal_context(context: &mut DecisionContext) -> Result<(), DecisionContextUpdateError> {
    context.action_order = context
        .actions
        .iter()
        .map(|action| action.action_id.clone())
        .collect();
    context.action_order.sort();
    context
        .actions
        .sort_by(|left, right| left.action_id.cmp(&right.action_id));
    context.deferred_action_order.sort();
    context.deferred_action_order.dedup();
    context.omission_order.sort();
    context.omission_order.dedup();
    context.negative_evidence_order.sort();
    context.negative_evidence_order.dedup();
    context.uncertainty_order.sort();
    context.uncertainty_order.dedup();
    context.digest = ContentHash::of_value(&context_body(context))
        .map_err(|error| DecisionContextUpdateError::Digest(error.to_string()))?;
    context
        .validate()
        .map_err(|error| DecisionContextUpdateError::InvalidOutput(error.to_string()))
}

fn validate_event(
    event: &DecisionContextEvent,
    request: &DecisionContextUpdateRequest,
) -> Result<(), DecisionContextUpdateError> {
    if !safe_text(&event.event_id)
        || event.sequence == 0
        || event.observed_tick == 0
        || event.observed_tick > request.current_tick
        || event.context_anchor_digest != request.base_context.digest
        || !unique_bounded(&event.subject_order, MAX_SUBJECTS)
        || !canonical(&event.subject_order)
        || !valid_hash(&event.payload_digest)
        || !valid_hash(&event.provenance_digest)
        || !safe_text(&event.reason)
        || !valid_hash(&event.event_digest)
    {
        return Err(DecisionContextUpdateError::InvalidEvent(format!(
            "event {} has an invalid identity, anchor, ordering, evidence digests, tick, or reason",
            event.event_id
        )));
    }
    let expected = ContentHash::of_value(&event_body(event))
        .map_err(|error| DecisionContextUpdateError::Digest(error.to_string()))?;
    if expected != event.event_digest {
        return Err(DecisionContextUpdateError::InvalidEvent(format!(
            "event {} digest does not match its signed body",
            event.event_id
        )));
    }
    Ok(())
}

fn validate_request(
    request: &DecisionContextUpdateRequest,
) -> Result<(), DecisionContextUpdateError> {
    request
        .base_context
        .validate()
        .map_err(|error| DecisionContextUpdateError::InvalidRequest(error.to_string()))?;
    if request.max_events == 0
        || request.max_events > MAX_EVENTS
        || request.event_order.len() > request.max_events
        || request.current_tick == 0
        || request.last_applied_sequence > u64::MAX - MAX_EVENTS as u64
    {
        return Err(DecisionContextUpdateError::InvalidRequest(
            "bounded event count, positive current tick, and valid replay cursor are required"
                .into(),
        ));
    }
    for event in &request.event_order {
        validate_event(event, request)?;
    }
    Ok(())
}

fn apply_event(
    context: &mut DecisionContext,
    event: &DecisionContextEvent,
) -> (Vec<String>, Vec<String>) {
    let mut invalidated = BTreeSet::new();
    let active = context
        .action_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if event.kind.invalidates_actions() {
        for subject in &event.subject_order {
            if active.contains(subject) {
                invalidated.insert(subject.clone());
            }
        }
    }
    if matches!(event.kind, DecisionEventKind::ActionCompleted) {
        context
            .actions
            .retain(|action| !invalidated.contains(&action.action_id));
    } else if !invalidated.is_empty() {
        context
            .actions
            .retain(|action| !invalidated.contains(&action.action_id));
        context
            .deferred_action_order
            .extend(invalidated.iter().cloned());
    }
    match event.kind {
        DecisionEventKind::EvidenceAdded => {
            context
                .uncertainty_order
                .retain(|value| !event.subject_order.binary_search(value).is_ok());
        }
        DecisionEventKind::EvidenceContradiction => {
            context.uncertainty_order.push(event.event_id.clone());
        }
        DecisionEventKind::QualityFailure => {
            context.omission_order.push(event.event_id.clone());
        }
        DecisionEventKind::ResourceExhausted => {
            context.omission_order.push(event.event_id.clone());
        }
        DecisionEventKind::ActionNegative => {
            context.negative_evidence_order.push(event.event_id.clone());
        }
        DecisionEventKind::ActionCompleted | DecisionEventKind::ActionBlocked => {}
    }
    let invalidated = invalidated.into_iter().collect::<Vec<_>>();
    let reconsideration = if invalidated.is_empty()
        && matches!(
            event.kind,
            DecisionEventKind::EvidenceContradiction
                | DecisionEventKind::QualityFailure
                | DecisionEventKind::ResourceExhausted
                | DecisionEventKind::ActionNegative
                | DecisionEventKind::ActionBlocked
        ) {
        vec![event.event_id.clone()]
    } else {
        invalidated.clone()
    };
    (invalidated, reconsideration)
}

impl DecisionContextUpdateResult {
    pub fn validate(&self) -> Result<(), DecisionContextUpdateError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !safe_text(&self.objective)
            || !valid_hash(&self.base_context_digest)
            || self.final_context.objective != self.objective
            || self.epoch_order.len() != self.applied_event_order.len()
            || !unique_bounded(&self.applied_event_order, MAX_EVENTS)
            || !canonical(&self.duplicate_event_order)
            || !canonical(&self.stale_event_order)
            || !canonical(&self.rejected_event_order)
            || !canonical(&self.invalidated_action_order)
            || !canonical(&self.reconsideration_order)
            || !valid_hash(&self.digest)
        {
            return Err(DecisionContextUpdateError::InvalidOutput(
                "identity, epoch alignment, partitions, or digest shape is invalid".into(),
            ));
        }
        self.final_context
            .validate()
            .map_err(|error| DecisionContextUpdateError::InvalidOutput(error.to_string()))?;
        let mut sequences = BTreeSet::new();
        let mut events = BTreeSet::new();
        for epoch in &self.epoch_order {
            if epoch.epoch_id.trim().is_empty()
                || epoch.sequence == 0
                || !sequences.insert(epoch.sequence)
                || !events.insert(epoch.event_id.clone())
                || !valid_hash(&epoch.prior_context_digest)
                || !valid_hash(&epoch.context_digest)
                || !canonical(&epoch.invalidated_action_order)
                || !canonical(&epoch.omission_order)
                || !canonical(&epoch.negative_evidence_order)
                || !canonical(&epoch.uncertainty_order)
                || !valid_hash(&epoch.epoch_digest)
            {
                return Err(DecisionContextUpdateError::InvalidOutput(
                    "epoch identity, sequence, partitions, or digest shape is invalid".into(),
                ));
            }
            let expected = ContentHash::of_value(&epoch_body(epoch))
                .map_err(|error| DecisionContextUpdateError::Digest(error.to_string()))?;
            if expected != epoch.epoch_digest {
                return Err(DecisionContextUpdateError::InvalidOutput(
                    "epoch digest does not match canonical content".into(),
                ));
            }
        }
        let expected = ContentHash::of_value(&result_body(self))
            .map_err(|error| DecisionContextUpdateError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(DecisionContextUpdateError::InvalidOutput(
                "update result digest does not match canonical content".into(),
            ));
        }
        Ok(())
    }
}

/// Apply a deterministic, idempotent batch of local research events to a decision context.
pub fn update_glioma_decision_context(
    request: &DecisionContextUpdateRequest,
) -> Result<DecisionContextUpdateResult, DecisionContextUpdateError> {
    validate_request(request)?;
    let mut context = request.base_context.clone();
    let mut by_id = BTreeMap::<String, DecisionContextEvent>::new();
    let mut duplicate = BTreeSet::new();
    let mut rejected = BTreeSet::new();
    for event in &request.event_order {
        if let Some(previous) = by_id.get(&event.event_id) {
            if previous.event_digest == event.event_digest {
                duplicate.insert(event.event_id.clone());
            } else {
                rejected.insert(event.event_id.clone());
            }
            continue;
        }
        by_id.insert(event.event_id.clone(), event.clone());
    }
    let mut events = by_id.into_values().collect::<Vec<_>>();
    events.sort_by(|left, right| {
        left.sequence
            .cmp(&right.sequence)
            .then_with(|| left.event_id.cmp(&right.event_id))
    });
    let mut seen_sequences = BTreeSet::new();
    let mut stale = BTreeSet::new();
    let mut applied = Vec::new();
    let mut epochs = Vec::new();
    let mut invalidated = BTreeSet::new();
    let mut reconsideration = BTreeSet::new();
    let mut replay_cursor = request.last_applied_sequence;
    for event in events {
        if event.sequence <= request.last_applied_sequence {
            stale.insert(event.event_id);
            continue;
        }
        if !seen_sequences.insert(event.sequence) {
            rejected.insert(event.event_id);
            continue;
        }
        let prior_digest = context.digest.clone();
        let (event_invalidated, event_reconsideration) = apply_event(&mut context, &event);
        seal_context(&mut context)?;
        let mut epoch = DecisionContextUpdateEpoch {
            epoch_id: format!("{}-{}", event.event_id, event.sequence),
            sequence: event.sequence,
            event_id: event.event_id.clone(),
            event_kind: event.kind,
            prior_context_digest: prior_digest,
            context_digest: context.digest.clone(),
            invalidated_action_order: event_invalidated.clone(),
            omission_order: context.omission_order.clone(),
            negative_evidence_order: context.negative_evidence_order.clone(),
            uncertainty_order: context.uncertainty_order.clone(),
            reconsideration_required: !event_reconsideration.is_empty(),
            epoch_digest: ContentHash::of_bytes(b"unsealed-glioma-context-update-epoch"),
        };
        epoch.epoch_digest = ContentHash::of_value(&epoch_body(&epoch))
            .map_err(|error| DecisionContextUpdateError::Digest(error.to_string()))?;
        invalidated.extend(event_invalidated);
        reconsideration.extend(event_reconsideration);
        applied.push(event.event_id);
        replay_cursor = replay_cursor.max(epoch.sequence);
        epochs.push(epoch);
    }
    let disposition = if !applied.is_empty() && (!rejected.is_empty() || !stale.is_empty()) {
        DecisionContextUpdateDisposition::Partial
    } else if !applied.is_empty() {
        DecisionContextUpdateDisposition::Applied
    } else if !rejected.is_empty() {
        DecisionContextUpdateDisposition::Blocked
    } else {
        DecisionContextUpdateDisposition::Noop
    };
    let mut result = DecisionContextUpdateResult {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: context.objective.clone(),
        base_context_digest: request.base_context.digest.clone(),
        final_context: context,
        epoch_order: epochs,
        applied_event_order: applied,
        duplicate_event_order: duplicate.into_iter().collect(),
        stale_event_order: stale.into_iter().collect(),
        rejected_event_order: rejected.into_iter().collect(),
        invalidated_action_order: invalidated.into_iter().collect(),
        reconsideration_order: reconsideration.into_iter().collect(),
        replay_cursor,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-context-update"),
    };
    result.digest = ContentHash::of_value(&result_body(&result))
        .map_err(|error| DecisionContextUpdateError::Digest(error.to_string()))?;
    result.validate()?;
    Ok(result)
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

    fn context() -> DecisionContext {
        let evidence = EvidenceRecord {
            evidence_id: "evidence-egfr".into(),
            source_artifact: LocalArtifactRef {
                artifact_id: "artifact-egfr".into(),
                content_hash: hash("artifact-egfr"),
                content_type: "application/vnd.aurora.glioma-evidence+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
            source_kind: EvidenceSourceKind::Dataset,
            claim: "EGFR signaling increases invasion".into(),
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
                objective: "update glioma context".into(),
                required_modalities: BTreeSet::from([GliomaModality::Genomics]),
                required_model_systems: BTreeSet::from([GliomaModelSystem::Organoid]),
                min_support_milli: 700,
                min_sources_per_claim: 1,
                max_claims: 4,
            },
            &[evidence],
        )
        .unwrap();
        compile_decision_context(
            &DecisionContextRequest {
                objective: "update glioma context".into(),
                max_actions: 4,
                default_cost_units: 3,
            },
            &knowledge,
        )
        .unwrap()
    }

    fn event(
        context_digest: ContentHash,
        id: &str,
        sequence: u64,
        kind: DecisionEventKind,
        subjects: &[&str],
    ) -> DecisionContextEvent {
        let mut event = DecisionContextEvent {
            event_id: id.into(),
            sequence,
            observed_tick: sequence,
            context_anchor_digest: context_digest,
            kind,
            subject_order: subjects.iter().map(|value| (*value).to_owned()).collect(),
            payload_digest: hash(&format!("payload-{id}")),
            provenance_digest: hash("provenance"),
            reason: format!("event reason {id}"),
            event_digest: hash("unsealed"),
        };
        event.event_digest = ContentHash::of_value(&event_body(&event)).unwrap();
        event
    }

    fn request(
        base_context: DecisionContext,
        events: Vec<DecisionContextEvent>,
    ) -> DecisionContextUpdateRequest {
        DecisionContextUpdateRequest {
            base_context,
            event_order: events,
            last_applied_sequence: 0,
            max_events: MAX_EVENTS,
            current_tick: 20,
        }
    }

    #[test]
    fn update_orders_events_and_deduplicates_retries() {
        let base = context();
        let action = base.action_order[0].clone();
        let first = event(
            base.digest.clone(),
            "negative",
            2,
            DecisionEventKind::ActionNegative,
            &[&action],
        );
        let duplicate = first.clone();
        let second = event(
            base.digest.clone(),
            "quality",
            3,
            DecisionEventKind::QualityFailure,
            &[],
        );
        let result =
            update_glioma_decision_context(&request(base, vec![second, duplicate, first])).unwrap();
        assert_eq!(result.applied_event_order, vec!["negative", "quality"]);
        assert_eq!(result.duplicate_event_order, vec!["negative"]);
        assert_eq!(result.final_context.action_order, Vec::<String>::new());
        assert_eq!(
            result.disposition,
            DecisionContextUpdateDisposition::Applied
        );
    }

    #[test]
    fn update_marks_stale_events_without_mutating_the_context() {
        let base = context();
        let event = event(
            base.digest.clone(),
            "stale",
            2,
            DecisionEventKind::EvidenceAdded,
            &[],
        );
        let mut request = request(base.clone(), vec![event]);
        request.last_applied_sequence = 2;
        let result = update_glioma_decision_context(&request).unwrap();
        assert_eq!(result.stale_event_order, vec!["stale"]);
        assert!(result.applied_event_order.is_empty());
        assert_eq!(result.final_context.digest, base.digest);
        assert_eq!(result.disposition, DecisionContextUpdateDisposition::Noop);
    }

    #[test]
    fn update_rejects_tampered_event_digest() {
        let base = context();
        let mut event = event(
            base.digest.clone(),
            "tampered",
            1,
            DecisionEventKind::EvidenceAdded,
            &[],
        );
        event.reason.push_str(" altered");
        assert!(matches!(
            update_glioma_decision_context(&request(base, vec![event])),
            Err(DecisionContextUpdateError::InvalidEvent(_))
        ));
    }

    #[test]
    fn update_rejects_stale_context_anchor() {
        let base = context();
        let event = event(
            hash("wrong-context"),
            "wrong-anchor",
            1,
            DecisionEventKind::EvidenceAdded,
            &[],
        );
        assert!(matches!(
            update_glioma_decision_context(&request(base, vec![event])),
            Err(DecisionContextUpdateError::InvalidEvent(_))
        ));
    }

    #[test]
    fn update_preserves_negative_and_reconsideration_partitions() {
        let base = context();
        let action = base.action_order[0].clone();
        let event = event(
            base.digest.clone(),
            "blocked",
            1,
            DecisionEventKind::ActionBlocked,
            &[&action],
        );
        let result = update_glioma_decision_context(&request(base, vec![event])).unwrap();
        assert_eq!(result.invalidated_action_order, vec![action.clone()]);
        assert_eq!(result.reconsideration_order, vec![action]);
        assert!(result
            .final_context
            .deferred_action_order
            .contains(&result.reconsideration_order[0]));
        assert!(result.epoch_order[0].reconsideration_required);
    }
}

//! Replayable event batches for autonomous preclinical glioma computation.
//!
//! The stream is value-only and local by default. It canonicalizes an institution-owned event
//! log into durable cursor pages, deduplicates exact retries, exposes sequence gaps, and keeps
//! event identity when payloads are redacted. A client can therefore resume after network loss
//! without treating missing telemetry as successful computation.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P09-F23";
pub const OUTPUT_SCHEMA: &str = "GliomaComputationEventBatch1@1";
pub const MAX_EVENTS: usize = 8_192;
pub const MAX_BATCH: usize = 1_024;
pub const MAX_TEXT_LEN: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputationEventKind {
    TaskQueued,
    TaskStarted,
    ResourceSample,
    QcPassed,
    QcFailed,
    RecoveryRequested,
    TaskCompleted,
    TaskFailed,
    TaskCancelled,
    RunPaused,
    RunResumed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationStreamEvent {
    pub event_id: String,
    pub run_id: String,
    pub replay_identity: ContentHash,
    pub sequence: u64,
    pub emitted_tick: u64,
    pub task_id: Option<String>,
    pub kind: ComputationEventKind,
    pub payload_digest: ContentHash,
    pub payload_redacted: bool,
    pub local_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationEventCursor {
    pub run_id: String,
    pub replay_identity: ContentHash,
    pub next_sequence: u64,
    pub last_event_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationEventFilter {
    pub kind_order: Vec<ComputationEventKind>,
    pub task_id_order: Vec<String>,
    pub include_resource_events: bool,
    pub redact_payloads: bool,
    pub max_events: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationEventAccessScope {
    pub site_id: String,
    pub authorized: bool,
    pub local_only: bool,
    pub allow_resource_events: bool,
    pub allow_qc_events: bool,
    pub allow_recovery_events: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationEventStreamRequest {
    pub run_id: String,
    pub replay_identity: ContentHash,
    pub events: Vec<ComputationStreamEvent>,
    pub client_cursor: Option<ComputationEventCursor>,
    pub filter: ComputationEventFilter,
    pub access: ComputationEventAccessScope,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationEventGap {
    pub start_sequence: u64,
    pub end_sequence: u64,
    pub recovery_marker: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputationEventBatchDisposition {
    Complete,
    Partial,
    Gap,
    Empty,
    Unauthorized,
    ReplayMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationEventBatch {
    pub feature_id: String,
    pub output_schema: String,
    pub run_id: String,
    pub replay_identity: ContentHash,
    pub event_order: Vec<String>,
    pub events: Vec<ComputationStreamEvent>,
    pub duplicate_event_order: Vec<String>,
    pub omitted_event_order: Vec<String>,
    pub gaps: Vec<ComputationEventGap>,
    pub cursor: ComputationEventCursor,
    pub acknowledged_through_sequence: u64,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: ComputationEventBatchDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ComputationEventStreamError {
    #[error("computation event stream request is invalid: {0}")]
    InvalidRequest(String),
    #[error("computation event batch is invalid: {0}")]
    InvalidOutput(String),
    #[error("computation event stream digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique_strings(values: &[String], max: usize) -> bool {
    let mut seen = BTreeSet::new();
    values.len() <= max
        && values.iter().all(|value| {
            !value.trim().is_empty() && value.len() <= MAX_TEXT_LEN && seen.insert(value)
        })
}

fn unique_event_ids(values: &[String], max: usize) -> bool {
    let mut seen = BTreeSet::new();
    values.len() <= max
        && values.iter().all(|value| {
            !value.trim().is_empty() && value.len() <= MAX_TEXT_LEN && seen.insert(value)
        })
}

fn valid_hash(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

fn digest_input(batch: &ComputationEventBatch) -> serde_json::Value {
    serde_json::json!({
        "feature_id": batch.feature_id,
        "output_schema": batch.output_schema,
        "run_id": batch.run_id,
        "replay_identity": batch.replay_identity,
        "event_order": batch.event_order,
        "events": batch.events,
        "duplicate_event_order": batch.duplicate_event_order,
        "omitted_event_order": batch.omitted_event_order,
        "gaps": batch.gaps,
        "cursor": batch.cursor,
        "acknowledged_through_sequence": batch.acknowledged_through_sequence,
        "negative_evidence": batch.negative_evidence,
        "uncertainty": batch.uncertainty,
        "disposition": batch.disposition,
    })
}

fn event_fingerprint(
    event: &ComputationStreamEvent,
) -> Result<ContentHash, ComputationEventStreamError> {
    ContentHash::of_value(&serde_json::json!({
        "event_id": event.event_id,
        "run_id": event.run_id,
        "replay_identity": event.replay_identity,
        "sequence": event.sequence,
        "emitted_tick": event.emitted_tick,
        "task_id": event.task_id,
        "kind": event.kind,
        "payload_digest": event.payload_digest,
        "payload_redacted": event.payload_redacted,
        "local_only": event.local_only,
    }))
    .map_err(|error| ComputationEventStreamError::Digest(error.to_string()))
}

fn validate_event(
    event: &ComputationStreamEvent,
    request: &ComputationEventStreamRequest,
) -> Result<(), ComputationEventStreamError> {
    if event.event_id.trim().is_empty()
        || event.event_id.len() > MAX_TEXT_LEN
        || event.run_id != request.run_id
        || event.replay_identity != request.replay_identity
        || event.sequence == 0
        || event.emitted_tick == 0
        || event
            .task_id
            .as_ref()
            .is_some_and(|task| task.trim().is_empty())
        || event
            .task_id
            .as_ref()
            .is_some_and(|task| task.len() > MAX_TEXT_LEN)
        || !valid_hash(&event.replay_identity)
        || !valid_hash(&event.payload_digest)
        || !event.local_only
    {
        return Err(ComputationEventStreamError::InvalidRequest(
            "events require bounded identity, run/replay binding, positive sequence/tick, content digests, and local locality".into(),
        ));
    }
    Ok(())
}

fn validate_request(
    request: &ComputationEventStreamRequest,
) -> Result<(), ComputationEventStreamError> {
    if request.run_id.trim().is_empty()
        || request.run_id.len() > MAX_TEXT_LEN
        || !valid_hash(&request.replay_identity)
        || request.events.is_empty()
        || request.events.len() > MAX_EVENTS
        || !canonical(&request.filter.kind_order)
        || request
            .filter
            .kind_order
            .windows(2)
            .any(|pair| pair[0] == pair[1])
        || !unique_strings(&request.filter.task_id_order, MAX_EVENTS)
        || !canonical(&request.filter.task_id_order)
        || request.filter.max_events == 0
        || request.filter.max_events > MAX_BATCH
        || request.access.site_id.trim().is_empty()
        || request.access.site_id.len() > MAX_TEXT_LEN
    {
        return Err(ComputationEventStreamError::InvalidRequest(
            "bounded run/replay identity, event history, canonical filter, batch bound, and access scope are required".into(),
        ));
    }
    if !request.access.authorized || !request.access.local_only {
        return Err(ComputationEventStreamError::InvalidRequest(
            "event access must be explicitly authorized and local-only".into(),
        ));
    }
    if let Some(cursor) = &request.client_cursor {
        if cursor.run_id != request.run_id
            || cursor.replay_identity != request.replay_identity
            || cursor.next_sequence == 0
            || !valid_hash(&cursor.last_event_digest)
        {
            return Err(ComputationEventStreamError::InvalidRequest(
                "client cursor must bind to run/replay identity and a positive sequence".into(),
            ));
        }
    }
    for event in &request.events {
        validate_event(event, request)?;
    }
    Ok(())
}

impl ComputationEventBatch {
    pub fn validate(&self) -> Result<(), ComputationEventStreamError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.run_id.trim().is_empty()
            || !valid_hash(&self.replay_identity)
            || !unique_event_ids(&self.event_order, MAX_BATCH)
            || !canonical(&self.duplicate_event_order)
            || !canonical(&self.omitted_event_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.events.len() > MAX_BATCH
            || self
                .events
                .windows(2)
                .any(|pair| pair[0].sequence >= pair[1].sequence)
            || self.events.iter().any(|event| {
                event.run_id != self.run_id
                    || event.replay_identity != self.replay_identity
                    || !event.local_only
                    || !valid_hash(&event.payload_digest)
            })
            || self
                .gaps
                .windows(2)
                .any(|pair| pair[0].start_sequence >= pair[1].start_sequence)
            || self.gaps.iter().any(|gap| {
                gap.start_sequence == 0
                    || gap.start_sequence > gap.end_sequence
                    || gap.recovery_marker.trim().is_empty()
            })
            || self.cursor.run_id != self.run_id
            || self.cursor.replay_identity != self.replay_identity
            || self.cursor.next_sequence == 0
            || !valid_hash(&self.cursor.last_event_digest)
        {
            return Err(ComputationEventStreamError::InvalidOutput(
                "batch identity, canonical ordering, event binding, gap ranges, or cursor continuity is invalid".into(),
            ));
        }
        let expected_ids = self
            .events
            .iter()
            .map(|event| event.event_id.clone())
            .collect::<Vec<_>>();
        if expected_ids != self.event_order {
            return Err(ComputationEventStreamError::InvalidOutput(
                "event order does not match emitted events".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ComputationEventStreamError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ComputationEventStreamError::InvalidOutput(
                "event batch digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Page a local event log with deterministic replay, explicit gaps, and resilient cursors.
pub fn stream_glioma_computation_events(
    request: &ComputationEventStreamRequest,
) -> Result<ComputationEventBatch, ComputationEventStreamError> {
    validate_request(request)?;
    let start_sequence = request
        .client_cursor
        .as_ref()
        .map(|cursor| cursor.next_sequence)
        .unwrap_or(1);
    let mut by_id = BTreeMap::<String, ComputationStreamEvent>::new();
    let mut duplicate_event_order = BTreeSet::new();
    for event in &request.events {
        if let Some(previous) = by_id.get(&event.event_id) {
            if event_fingerprint(previous)? != event_fingerprint(event)? {
                return Err(ComputationEventStreamError::InvalidRequest(
                    "conflicting duplicate event identity".into(),
                ));
            }
            duplicate_event_order.insert(event.event_id.clone());
        } else {
            by_id.insert(event.event_id.clone(), event.clone());
        }
    }
    let mut ordered = by_id.into_values().collect::<Vec<_>>();
    ordered.sort_by(|left, right| {
        left.sequence
            .cmp(&right.sequence)
            .then(left.event_id.cmp(&right.event_id))
    });
    let max_sequence = ordered
        .iter()
        .map(|event| event.sequence)
        .max()
        .unwrap_or(start_sequence);
    let mut gaps = Vec::new();
    let present = ordered
        .iter()
        .filter(|event| event.sequence >= start_sequence)
        .map(|event| event.sequence)
        .collect::<BTreeSet<_>>();
    let mut gap_start = None;
    for sequence in start_sequence..=max_sequence {
        if !present.contains(&sequence) && gap_start.is_none() {
            gap_start = Some(sequence);
        }
        if present.contains(&sequence) {
            if let Some(begin) = gap_start.take() {
                gaps.push(ComputationEventGap {
                    start_sequence: begin,
                    end_sequence: sequence - 1,
                    recovery_marker: format!("recover-sequences:{begin}-{}", sequence - 1),
                });
            }
        }
    }
    if let Some(begin) = gap_start {
        gaps.push(ComputationEventGap {
            start_sequence: begin,
            end_sequence: max_sequence,
            recovery_marker: format!("recover-sequences:{begin}-{max_sequence}"),
        });
    }
    let mut omitted = BTreeSet::new();
    let mut selected = Vec::new();
    for mut event in ordered {
        if event.sequence < start_sequence {
            continue;
        }
        let kind_allowed =
            request.filter.kind_order.is_empty() || request.filter.kind_order.contains(&event.kind);
        let task_allowed = request.filter.task_id_order.is_empty()
            || event
                .task_id
                .as_ref()
                .is_some_and(|task| request.filter.task_id_order.contains(task));
        let access_allowed = match event.kind {
            ComputationEventKind::ResourceSample => {
                request.access.allow_resource_events && request.filter.include_resource_events
            }
            ComputationEventKind::QcPassed | ComputationEventKind::QcFailed => {
                request.access.allow_qc_events
            }
            ComputationEventKind::RecoveryRequested => request.access.allow_recovery_events,
            _ => true,
        };
        if !kind_allowed || !task_allowed || !access_allowed {
            omitted.insert(event.event_id.clone());
            continue;
        }
        if request.filter.redact_payloads {
            event.payload_redacted = true;
        }
        if selected.len() < request.filter.max_events {
            selected.push(event);
        } else {
            omitted.insert(event.event_id.clone());
        }
    }
    selected.sort_by(|left, right| {
        left.sequence
            .cmp(&right.sequence)
            .then(left.event_id.cmp(&right.event_id))
    });
    let last_event_digest = selected
        .last()
        .map(event_fingerprint)
        .transpose()?
        .unwrap_or_else(|| ContentHash::of_bytes(b"empty-glioma-event-cursor"));
    let next_sequence = selected
        .last()
        .map(|event| event.sequence.saturating_add(1))
        .unwrap_or(start_sequence);
    let cursor = ComputationEventCursor {
        run_id: request.run_id.clone(),
        replay_identity: request.replay_identity.clone(),
        next_sequence,
        last_event_digest,
    };
    let mut negative_evidence = Vec::new();
    let mut uncertainty = Vec::new();
    if !gaps.is_empty() {
        negative_evidence.push("event-sequence-gaps-require-recovery".into());
        uncertainty.push("missing-telemetry-cannot-be-treated-as-completed-work".into());
    }
    if !omitted.is_empty() {
        uncertainty.push("filter-or-access-scope-omitted-events-from-this-page".into());
    }
    let disposition = if !gaps.is_empty() {
        ComputationEventBatchDisposition::Gap
    } else if selected.is_empty() {
        ComputationEventBatchDisposition::Empty
    } else if !omitted.is_empty() {
        ComputationEventBatchDisposition::Partial
    } else {
        ComputationEventBatchDisposition::Complete
    };
    let mut batch = ComputationEventBatch {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        run_id: request.run_id.clone(),
        replay_identity: request.replay_identity.clone(),
        event_order: selected
            .iter()
            .map(|event| event.event_id.clone())
            .collect(),
        events: selected,
        duplicate_event_order: duplicate_event_order.into_iter().collect(),
        omitted_event_order: omitted.into_iter().collect(),
        gaps,
        cursor,
        acknowledged_through_sequence: next_sequence.saturating_sub(1),
        negative_evidence,
        uncertainty,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-computation-event-batch"),
    };
    batch.digest = ContentHash::of_value(&digest_input(&batch))
        .map_err(|error| ComputationEventStreamError::Digest(error.to_string()))?;
    batch.validate()?;
    Ok(batch)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }

    fn event(sequence: u64, kind: ComputationEventKind) -> ComputationStreamEvent {
        ComputationStreamEvent {
            event_id: format!("event-{sequence}"),
            run_id: "run-1".into(),
            replay_identity: hash("replay"),
            sequence,
            emitted_tick: sequence,
            task_id: Some("normalize".into()),
            kind,
            payload_digest: hash(&format!("payload-{sequence}")),
            payload_redacted: false,
            local_only: true,
        }
    }

    fn request(events: Vec<ComputationStreamEvent>) -> ComputationEventStreamRequest {
        ComputationEventStreamRequest {
            run_id: "run-1".into(),
            replay_identity: hash("replay"),
            events,
            client_cursor: None,
            filter: ComputationEventFilter {
                kind_order: Vec::new(),
                task_id_order: Vec::new(),
                include_resource_events: true,
                redact_payloads: false,
                max_events: 16,
            },
            access: ComputationEventAccessScope {
                site_id: "site-a".into(),
                authorized: true,
                local_only: true,
                allow_resource_events: true,
                allow_qc_events: true,
                allow_recovery_events: true,
            },
        }
    }

    #[test]
    fn ordering_and_cursor_are_permutation_stable() {
        let mut left = request(vec![
            event(2, ComputationEventKind::TaskStarted),
            event(1, ComputationEventKind::TaskQueued),
        ]);
        let first = stream_glioma_computation_events(&left).expect("left");
        left.events.reverse();
        let second = stream_glioma_computation_events(&left).expect("right");
        assert_eq!(first.digest, second.digest);
        assert_eq!(first.event_order, vec!["event-1", "event-2"]);
        assert_eq!(first.cursor.next_sequence, 3);
    }

    #[test]
    fn exact_duplicates_are_deduplicated_but_conflicts_fail_closed() {
        let mut events = vec![
            event(1, ComputationEventKind::TaskQueued),
            event(1, ComputationEventKind::TaskQueued),
        ];
        let batch = stream_glioma_computation_events(&request(events.clone())).expect("duplicate");
        assert_eq!(batch.events.len(), 1);
        assert_eq!(batch.duplicate_event_order, vec!["event-1"]);
        events[1].payload_digest = hash("conflict");
        assert!(matches!(
            stream_glioma_computation_events(&request(events)),
            Err(ComputationEventStreamError::InvalidRequest(_))
        ));
    }

    #[test]
    fn missing_sequences_are_explicit_gaps() {
        let batch = stream_glioma_computation_events(&request(vec![
            event(1, ComputationEventKind::TaskQueued),
            event(3, ComputationEventKind::TaskCompleted),
        ]))
        .expect("gap");
        assert_eq!(batch.disposition, ComputationEventBatchDisposition::Gap);
        assert_eq!(batch.gaps[0].start_sequence, 2);
        assert!(batch
            .negative_evidence
            .iter()
            .any(|item| item.contains("gaps")));
    }

    #[test]
    fn redaction_preserves_event_identity_and_access_omission_is_explicit() {
        let mut req = request(vec![
            event(1, ComputationEventKind::TaskQueued),
            event(2, ComputationEventKind::ResourceSample),
        ]);
        req.filter.redact_payloads = true;
        req.filter.include_resource_events = false;
        req.access.allow_resource_events = false;
        let batch = stream_glioma_computation_events(&req).expect("redaction");
        assert_eq!(batch.events[0].event_id, "event-1");
        assert!(batch.events[0].payload_redacted);
        assert_eq!(batch.omitted_event_order, vec!["event-2"]);
    }

    #[test]
    fn cursor_resume_does_not_replay_acknowledged_events() {
        let first = stream_glioma_computation_events(&request(vec![
            event(1, ComputationEventKind::TaskQueued),
            event(2, ComputationEventKind::TaskStarted),
            event(3, ComputationEventKind::TaskCompleted),
        ]))
        .expect("first");
        let mut resumed = request(vec![
            event(1, ComputationEventKind::TaskQueued),
            event(2, ComputationEventKind::TaskStarted),
            event(3, ComputationEventKind::TaskCompleted),
        ]);
        resumed.client_cursor = Some(first.cursor);
        let second = stream_glioma_computation_events(&resumed).expect("resume");
        assert!(second.events.is_empty());
        assert_eq!(second.disposition, ComputationEventBatchDisposition::Empty);
    }
}

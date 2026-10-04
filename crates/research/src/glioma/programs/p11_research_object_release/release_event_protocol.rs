//! Deterministic, idempotent release-lifecycle event protocol for preclinical glioma objects.
//!
//! The protocol is a replayable state machine rather than a notification helper. Every event is
//! content-bound, ordered, authority-scoped, and chained to its predecessor. Duplicate delivery
//! is idempotent; conflicting duplicates, revoked authorities, impossible transitions, and broken
//! chains remain explicit failures. It never publishes bytes or moves raw data.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P11-F23";
pub const OUTPUT_SCHEMA: &str = "GliomaResearchReleaseEventProtocol1@1";
pub const MAX_EVENTS: usize = 2_048;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseEventKind {
    Candidate,
    Review,
    Published,
    Corrected,
    Withdrawn,
    Superseded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseEvent {
    pub event_id: String,
    pub object_id: String,
    pub sequence: u64,
    pub kind: ReleaseEventKind,
    pub object_digest: ContentHash,
    pub predecessor_digest: Option<ContentHash>,
    pub authority_id: String,
    pub key_id: String,
    pub policy_scope: String,
    pub authority_active: bool,
    pub authority_revoked: bool,
    pub issued_epoch: u64,
    pub payload_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseEventProtocolRequest {
    pub object_id: String,
    pub policy_scope: String,
    pub events: Vec<ReleaseEvent>,
    pub max_events: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseLifecycleState {
    Empty,
    Candidate,
    InReview,
    Published,
    Corrected,
    Withdrawn,
    Superseded,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseEventDisposition {
    Replayed,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseEventObservation {
    pub event_id: String,
    pub sequence: u64,
    pub kind: ReleaseEventKind,
    pub accepted: bool,
    pub idempotent: bool,
    pub reason_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseEventProtocolReport {
    pub feature_id: String,
    pub output_schema: String,
    pub object_id: String,
    pub policy_scope: String,
    pub observations: Vec<ReleaseEventObservation>,
    pub accepted_event_order: Vec<String>,
    pub duplicate_event_order: Vec<String>,
    pub rejected_event_order: Vec<String>,
    pub blocking_order: Vec<String>,
    pub warning_order: Vec<String>,
    pub cursor: u64,
    pub terminal_state: ReleaseLifecycleState,
    pub current_object_digest: Option<ContentHash>,
    pub current_event_digest: Option<ContentHash>,
    pub disposition: ReleaseEventDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReleaseEventProtocolError {
    #[error("release event protocol request is invalid: {0}")]
    InvalidRequest(String),
    #[error("release event protocol output is invalid: {0}")]
    InvalidOutput(String),
    #[error("release event protocol digest failed: {0}")]
    Digest(String),
}

fn identifier(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 160
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:/-@".contains(&byte))
}

fn text(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 512 && !value.contains('\0')
}

fn digest_input(report: &ReleaseEventProtocolReport) -> serde_json::Value {
    serde_json::json!({
        "feature_id": report.feature_id,
        "output_schema": report.output_schema,
        "object_id": report.object_id,
        "policy_scope": report.policy_scope,
        "observations": report.observations,
        "accepted_event_order": report.accepted_event_order,
        "duplicate_event_order": report.duplicate_event_order,
        "rejected_event_order": report.rejected_event_order,
        "blocking_order": report.blocking_order,
        "warning_order": report.warning_order,
        "cursor": report.cursor,
        "terminal_state": report.terminal_state,
        "current_object_digest": report.current_object_digest,
        "current_event_digest": report.current_event_digest,
        "disposition": report.disposition,
    })
}

fn event_payload(event: &ReleaseEvent) -> serde_json::Value {
    serde_json::json!({
        "event_id": event.event_id,
        "object_id": event.object_id,
        "sequence": event.sequence,
        "kind": event.kind,
        "object_digest": event.object_digest,
        "predecessor_digest": event.predecessor_digest,
        "authority_id": event.authority_id,
        "key_id": event.key_id,
        "policy_scope": event.policy_scope,
        "authority_active": event.authority_active,
        "authority_revoked": event.authority_revoked,
        "issued_epoch": event.issued_epoch,
    })
}

fn event_digest(event: &ReleaseEvent) -> Result<ContentHash, ReleaseEventProtocolError> {
    ContentHash::of_value(&event_payload(event))
        .map_err(|error| ReleaseEventProtocolError::Digest(error.to_string()))
}

fn valid_event(event: &ReleaseEvent) -> bool {
    identifier(&event.event_id)
        && identifier(&event.object_id)
        && event.sequence > 0
        && event.object_digest.as_str().len() == 64
        && event
            .predecessor_digest
            .as_ref()
            .is_none_or(|digest| digest.as_str().len() == 64)
        && identifier(&event.authority_id)
        && identifier(&event.key_id)
        && identifier(&event.policy_scope)
        && event.issued_epoch > 0
        && event.payload_digest.as_str().len() == 64
}

fn validate_request(
    request: &ReleaseEventProtocolRequest,
) -> Result<(), ReleaseEventProtocolError> {
    if !identifier(&request.object_id)
        || !identifier(&request.policy_scope)
        || request.events.is_empty()
        || request.events.len() > MAX_EVENTS
        || request.max_events == 0
        || request.max_events > MAX_EVENTS
        || request.events.len() > request.max_events
        || request.events.iter().any(|event| !valid_event(event))
    {
        return Err(ReleaseEventProtocolError::InvalidRequest(
            "bounded object identity, policy scope, event count, and content-bound events are required".into(),
        ));
    }
    Ok(())
}

impl ReleaseEventProtocolReport {
    pub fn validate(&self) -> Result<(), ReleaseEventProtocolError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !identifier(&self.object_id)
            || !identifier(&self.policy_scope)
            || !self
                .observations
                .windows(2)
                .all(|pair| pair[0].sequence <= pair[1].sequence)
            || self.observations.iter().any(|observation| {
                !identifier(&observation.event_id)
                    || observation.sequence == 0
                    || !observation.reason_order.iter().all(|reason| text(reason))
            })
            || self.cursor
                != self
                    .observations
                    .iter()
                    .filter(|observation| observation.accepted)
                    .map(|observation| observation.sequence)
                    .max()
                    .unwrap_or(0)
        {
            return Err(ReleaseEventProtocolError::InvalidOutput(
                "release protocol identity, ordering, observations, or cursor is invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ReleaseEventProtocolError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ReleaseEventProtocolError::InvalidOutput(
                "release protocol report digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn transition(
    state: ReleaseLifecycleState,
    kind: ReleaseEventKind,
) -> Option<ReleaseLifecycleState> {
    match (state, kind) {
        (ReleaseLifecycleState::Empty, ReleaseEventKind::Candidate) => {
            Some(ReleaseLifecycleState::Candidate)
        }
        (ReleaseLifecycleState::Candidate, ReleaseEventKind::Review) => {
            Some(ReleaseLifecycleState::InReview)
        }
        (ReleaseLifecycleState::InReview, ReleaseEventKind::Published) => {
            Some(ReleaseLifecycleState::Published)
        }
        (ReleaseLifecycleState::Published, ReleaseEventKind::Corrected)
        | (ReleaseLifecycleState::Corrected, ReleaseEventKind::Corrected) => {
            Some(ReleaseLifecycleState::Corrected)
        }
        (ReleaseLifecycleState::Published, ReleaseEventKind::Withdrawn)
        | (ReleaseLifecycleState::Corrected, ReleaseEventKind::Withdrawn) => {
            Some(ReleaseLifecycleState::Withdrawn)
        }
        (ReleaseLifecycleState::Published, ReleaseEventKind::Superseded)
        | (ReleaseLifecycleState::Corrected, ReleaseEventKind::Superseded) => {
            Some(ReleaseLifecycleState::Superseded)
        }
        _ => None,
    }
}

/// Replay an ordered release lifecycle and return a deterministic, idempotence-aware report.
pub fn replay_glioma_release_event_protocol(
    request: &ReleaseEventProtocolRequest,
) -> Result<ReleaseEventProtocolReport, ReleaseEventProtocolError> {
    validate_request(request)?;
    let mut events = request.events.clone();
    events.sort_by(|left, right| {
        left.sequence
            .cmp(&right.sequence)
            .then_with(|| left.event_id.cmp(&right.event_id))
    });
    let mut observations = Vec::new();
    let mut accepted = Vec::new();
    let mut duplicates = Vec::new();
    let mut rejected = Vec::new();
    let mut blocking = BTreeSet::new();
    let mut warnings = BTreeSet::new();
    let mut seen = BTreeMap::<String, (ContentHash, u64)>::new();
    let mut state = ReleaseLifecycleState::Empty;
    let mut cursor = 0_u64;
    let mut issued_epoch = 0_u64;
    let mut object_digest = None;
    let mut current_event_digest = None;
    for event in events {
        let computed = event_digest(&event)?;
        if computed != event.payload_digest {
            rejected.push(event.event_id.clone());
            blocking.insert(format!("{}:payload-digest-mismatch", event.event_id));
            observations.push(ReleaseEventObservation {
                event_id: event.event_id,
                sequence: event.sequence,
                kind: event.kind,
                accepted: false,
                idempotent: false,
                reason_order: vec!["payload-digest-mismatch".into()],
            });
            continue;
        }
        if let Some((prior_digest, prior_sequence)) = seen.get(&event.event_id) {
            if prior_digest == &event.payload_digest && prior_sequence == &event.sequence {
                duplicates.push(event.event_id.clone());
                warnings.insert(format!("{}:idempotent-duplicate", event.event_id));
                observations.push(ReleaseEventObservation {
                    event_id: event.event_id,
                    sequence: event.sequence,
                    kind: event.kind,
                    accepted: true,
                    idempotent: true,
                    reason_order: vec!["idempotent-duplicate".into()],
                });
            } else {
                rejected.push(event.event_id.clone());
                blocking.insert(format!("{}:conflicting-duplicate", event.event_id));
                observations.push(ReleaseEventObservation {
                    event_id: event.event_id,
                    sequence: event.sequence,
                    kind: event.kind,
                    accepted: false,
                    idempotent: false,
                    reason_order: vec!["conflicting-duplicate".into()],
                });
            }
            continue;
        }
        seen.insert(
            event.event_id.clone(),
            (event.payload_digest.clone(), event.sequence),
        );
        let mut reasons = BTreeSet::new();
        if event.object_id != request.object_id {
            reasons.insert("object-id-mismatch".into());
        }
        if event.policy_scope != request.policy_scope {
            reasons.insert("policy-scope-mismatch".into());
        }
        if !event.authority_active || event.authority_revoked {
            reasons.insert("authority-revoked-or-inactive".into());
        }
        if event.sequence != cursor.saturating_add(1) {
            reasons.insert("sequence-gap-or-reorder".into());
        }
        if event.issued_epoch < issued_epoch {
            reasons.insert("issued-epoch-regression".into());
        }
        if let Some(previous_digest) = &current_event_digest {
            if event.predecessor_digest.as_ref() != Some(previous_digest) {
                reasons.insert("predecessor-chain-mismatch".into());
            }
        } else if event.predecessor_digest.is_some() {
            reasons.insert("unexpected-first-predecessor".into());
        }
        if transition(state, event.kind).is_none() {
            reasons.insert("invalid-lifecycle-transition".into());
        }
        if reasons.is_empty() {
            state = transition(state, event.kind).expect("validated transition");
            cursor = event.sequence;
            issued_epoch = event.issued_epoch;
            object_digest = Some(event.object_digest.clone());
            current_event_digest = Some(event.payload_digest.clone());
            accepted.push(event.event_id.clone());
            observations.push(ReleaseEventObservation {
                event_id: event.event_id,
                sequence: event.sequence,
                kind: event.kind,
                accepted: true,
                idempotent: false,
                reason_order: vec!["transition-accepted".into()],
            });
        } else {
            let reason_order = reasons.into_iter().collect::<Vec<_>>();
            for reason in &reason_order {
                blocking.insert(format!("{}:{reason}", event.event_id));
            }
            rejected.push(event.event_id.clone());
            observations.push(ReleaseEventObservation {
                event_id: event.event_id,
                sequence: event.sequence,
                kind: event.kind,
                accepted: false,
                idempotent: false,
                reason_order,
            });
        }
    }
    observations.sort_by(|left, right| {
        left.sequence
            .cmp(&right.sequence)
            .then_with(|| left.event_id.cmp(&right.event_id))
    });
    let disposition = if !blocking.is_empty() {
        ReleaseEventDisposition::Blocked
    } else if !warnings.is_empty() {
        ReleaseEventDisposition::Partial
    } else {
        ReleaseEventDisposition::Replayed
    };
    let mut report = ReleaseEventProtocolReport {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        object_id: request.object_id.clone(),
        policy_scope: request.policy_scope.clone(),
        observations,
        accepted_event_order: accepted,
        duplicate_event_order: duplicates,
        rejected_event_order: rejected,
        blocking_order: blocking.into_iter().collect(),
        warning_order: warnings.into_iter().collect(),
        cursor,
        terminal_state: if state == ReleaseLifecycleState::Empty {
            ReleaseLifecycleState::Blocked
        } else {
            state
        },
        current_object_digest: object_digest,
        current_event_digest,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-release-event-report"),
    };
    report.digest = ContentHash::of_value(&digest_input(&report))
        .map_err(|error| ReleaseEventProtocolError::Digest(error.to_string()))?;
    report.validate()?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn event(
        id: &str,
        sequence: u64,
        kind: ReleaseEventKind,
        object_digest: ContentHash,
        predecessor_digest: Option<ContentHash>,
    ) -> ReleaseEvent {
        let mut event = ReleaseEvent {
            event_id: id.into(),
            object_id: "object-1".into(),
            sequence,
            kind,
            object_digest,
            predecessor_digest,
            authority_id: "authority".into(),
            key_id: "key-1".into(),
            policy_scope: "consortium".into(),
            authority_active: true,
            authority_revoked: false,
            issued_epoch: sequence,
            payload_digest: hash("placeholder"),
        };
        event.payload_digest = event_digest(&event).unwrap();
        event
    }

    fn request(events: Vec<ReleaseEvent>) -> ReleaseEventProtocolRequest {
        ReleaseEventProtocolRequest {
            object_id: "object-1".into(),
            policy_scope: "consortium".into(),
            events,
            max_events: 32,
        }
    }

    #[test]
    fn lifecycle_replays_correction_and_preserves_chain() {
        let object = hash("object-v1");
        let candidate = event(
            "candidate",
            1,
            ReleaseEventKind::Candidate,
            object.clone(),
            None,
        );
        let review = event(
            "review",
            2,
            ReleaseEventKind::Review,
            object.clone(),
            Some(candidate.payload_digest.clone()),
        );
        let published = event(
            "published",
            3,
            ReleaseEventKind::Published,
            object.clone(),
            Some(review.payload_digest.clone()),
        );
        let corrected = event(
            "corrected",
            4,
            ReleaseEventKind::Corrected,
            hash("object-v2"),
            Some(published.payload_digest.clone()),
        );
        let report = replay_glioma_release_event_protocol(&request(vec![
            candidate, review, published, corrected,
        ]))
        .unwrap();
        assert_eq!(report.disposition, ReleaseEventDisposition::Replayed);
        assert_eq!(report.terminal_state, ReleaseLifecycleState::Corrected);
        assert_eq!(report.cursor, 4);
        report.validate().unwrap();
    }

    #[test]
    fn duplicate_delivery_is_idempotent_but_conflicting_duplicate_blocks() {
        let candidate = event(
            "candidate",
            1,
            ReleaseEventKind::Candidate,
            hash("object"),
            None,
        );
        let mut conflict = candidate.clone();
        conflict.object_digest = hash("different");
        conflict.payload_digest = event_digest(&conflict).unwrap();
        let report = replay_glioma_release_event_protocol(&request(vec![
            candidate.clone(),
            candidate,
            conflict,
        ]))
        .unwrap();
        assert_eq!(report.disposition, ReleaseEventDisposition::Blocked);
        assert!(report.duplicate_event_order.contains(&"candidate".into()));
        assert!(report
            .blocking_order
            .iter()
            .any(|reason| reason.contains("conflicting-duplicate")));
    }

    #[test]
    fn revoked_authority_cannot_emit_authoritative_transition() {
        let mut candidate = event(
            "candidate",
            1,
            ReleaseEventKind::Candidate,
            hash("object"),
            None,
        );
        candidate.authority_revoked = true;
        candidate.payload_digest = event_digest(&candidate).unwrap();
        let report = replay_glioma_release_event_protocol(&request(vec![candidate])).unwrap();
        assert_eq!(report.disposition, ReleaseEventDisposition::Blocked);
        assert!(report.rejected_event_order.contains(&"candidate".into()));
    }
}

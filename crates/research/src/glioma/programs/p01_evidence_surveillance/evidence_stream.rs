//! Prospective high-throughput evidence stream snapshots for glioma research.
//!
//! This feature is a typed event processor, not a message broker. It accepts a bounded local
//! event window, makes duplicate and late-event handling explicit, and compiles claim-level
//! support/negative/contradiction trends for the P01 surveillance and P02 prospective monitor.
//! Raw source bytes never enter the stream contract.

use crate::glioma::evidence::{EvidenceSourceKind, EvidenceState};
use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P01-F07";
pub const OUTPUT_SCHEMA: &str = "GliomaEvidenceStreamSnapshot1@1";
pub const MAX_EVENTS: usize = 16_384;
pub const MAX_CLAIMS: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceStreamEvent {
    pub event_id: String,
    pub evidence_id: String,
    pub claim_key: String,
    pub epoch: u64,
    pub modality: GliomaModality,
    pub model_system: Option<GliomaModelSystem>,
    pub source_kind: EvidenceSourceKind,
    pub state: EvidenceState,
    pub quality_milli: u16,
    pub reproducibility_milli: u16,
    pub artifact_hash: ContentHash,
    pub preclinical_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceStreamRequest {
    pub objective: String,
    pub events: Vec<EvidenceStreamEvent>,
    pub expected_next_epoch: u64,
    pub max_events: usize,
    pub max_claims: usize,
    pub max_epoch_lag: u64,
    pub min_quality_milli: u16,
    pub trend_threshold_milli: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceStreamClaimTrend {
    Rising,
    Stable,
    Declining,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceStreamClaim {
    pub claim_key: String,
    pub event_order: Vec<String>,
    pub modality_order: Vec<GliomaModality>,
    pub model_system_order: Vec<GliomaModelSystem>,
    pub source_kind_order: Vec<EvidenceSourceKind>,
    pub first_epoch: u64,
    pub latest_epoch: u64,
    pub support_milli: u16,
    pub negative_milli: u16,
    pub contradiction_milli: u16,
    pub unknown_milli: u16,
    pub coverage_milli: u16,
    pub trend_milli: i16,
    pub trend: EvidenceStreamClaimTrend,
    pub next_action: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceStreamDisposition {
    Ready,
    Partial,
    Negative,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceStreamSnapshot {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub accepted_order: Vec<String>,
    pub duplicate_order: Vec<String>,
    pub late_order: Vec<String>,
    pub rejected_order: Vec<String>,
    pub claim_order: Vec<String>,
    pub claims: Vec<EvidenceStreamClaim>,
    pub frontier_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: EvidenceStreamDisposition,
    pub next_step: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EvidenceStreamError {
    #[error("evidence stream request is invalid: {0}")]
    InvalidRequest(String),
    #[error("evidence stream event is invalid: {0}")]
    InvalidEvent(String),
    #[error("evidence stream output is invalid: {0}")]
    InvalidOutput(String),
    #[error("evidence stream digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn weight(event: &EvidenceStreamEvent) -> u16 {
    ((u32::from(event.quality_milli) * u32::from(event.reproducibility_milli)) / 1_000) as u16
}

pub(crate) fn digest_input(output: &EvidenceStreamSnapshot) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "accepted_order": output.accepted_order,
        "duplicate_order": output.duplicate_order,
        "late_order": output.late_order,
        "rejected_order": output.rejected_order,
        "claim_order": output.claim_order,
        "claims": output.claims,
        "frontier_order": output.frontier_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
        "next_step": output.next_step,
    })
}

impl EvidenceStreamSnapshot {
    pub fn validate(&self) -> Result<(), EvidenceStreamError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || !canonical(&self.accepted_order)
            || !canonical(&self.duplicate_order)
            || !canonical(&self.late_order)
            || !canonical(&self.rejected_order)
            || !canonical(&self.claim_order)
            || !canonical(&self.frontier_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.claims.iter().any(|claim| {
                claim.claim_key.trim().is_empty()
                    || !canonical(&claim.event_order)
                    || !canonical(&claim.modality_order)
                    || !canonical(&claim.model_system_order)
                    || !canonical(&claim.source_kind_order)
                    || claim.support_milli > 1_000
                    || claim.negative_milli > 1_000
                    || claim.contradiction_milli > 1_000
                    || claim.unknown_milli > 1_000
                    || claim.coverage_milli > 1_000
                    || claim.next_action.trim().is_empty()
            })
        {
            return Err(EvidenceStreamError::InvalidOutput(
                "identity, ordering, score, or claim fields are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| EvidenceStreamError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(EvidenceStreamError::InvalidOutput(
                "evidence stream digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

pub fn snapshot_glioma_evidence_stream(
    request: &EvidenceStreamRequest,
) -> Result<EvidenceStreamSnapshot, EvidenceStreamError> {
    if request.objective.trim().is_empty()
        || request.events.is_empty()
        || request.events.len() > MAX_EVENTS
        || request.max_events == 0
        || request.max_events > MAX_EVENTS
        || request.max_claims == 0
        || request.max_claims > MAX_CLAIMS
        || request.min_quality_milli > 1_000
        || request.trend_threshold_milli > 1_000
    {
        return Err(EvidenceStreamError::InvalidRequest(
            "objective, bounded events/claims, and score thresholds are required".into(),
        ));
    }
    let mut event_ids = BTreeSet::new();
    let mut evidence_fingerprints = BTreeMap::<String, (ContentHash, EvidenceState, u64)>::new();
    let mut accepted = Vec::new();
    let mut duplicate = Vec::new();
    let mut late = Vec::new();
    let mut rejected = Vec::new();
    let reference_epoch = request.expected_next_epoch.saturating_sub(1);
    for event in &request.events {
        if event.event_id.trim().is_empty()
            || event.evidence_id.trim().is_empty()
            || event.claim_key.trim().is_empty()
            || event.quality_milli > 1_000
            || event.reproducibility_milli > 1_000
            || event.artifact_hash.as_str().len() != 64
            || event.epoch >= request.expected_next_epoch
            || !event.preclinical_only
        {
            return Err(EvidenceStreamError::InvalidEvent(
                "event identity, score, content hash, and preclinical fields are invalid".into(),
            ));
        }
        if !event_ids.insert(event.event_id.clone()) {
            return Err(EvidenceStreamError::InvalidEvent(
                "event ids must be unique".into(),
            ));
        }
        if let Some((hash, state, epoch)) = evidence_fingerprints.get(&event.evidence_id) {
            if hash == &event.artifact_hash && state == &event.state && epoch == &event.epoch {
                duplicate.push(event.event_id.clone());
                continue;
            }
        }
        evidence_fingerprints.insert(
            event.evidence_id.clone(),
            (event.artifact_hash.clone(), event.state, event.epoch),
        );
        if event.quality_milli < request.min_quality_milli {
            rejected.push(event.event_id.clone());
            continue;
        }
        if reference_epoch.saturating_sub(event.epoch) > request.max_epoch_lag {
            late.push(event.event_id.clone());
        }
        accepted.push(event.clone());
    }
    if accepted.len() > request.max_events {
        accepted.sort_by(|left, right| {
            right
                .epoch
                .cmp(&left.epoch)
                .then_with(|| left.event_id.cmp(&right.event_id))
        });
        rejected.extend(
            accepted
                .iter()
                .skip(request.max_events)
                .map(|event| event.event_id.clone()),
        );
        accepted.truncate(request.max_events);
    }
    accepted.sort_by(|left, right| left.event_id.cmp(&right.event_id));
    let mut grouped = BTreeMap::<String, Vec<EvidenceStreamEvent>>::new();
    for event in &accepted {
        grouped
            .entry(event.claim_key.clone())
            .or_default()
            .push(event.clone());
    }
    let mut claims = Vec::new();
    for (claim_key, mut events) in grouped {
        events.sort_by(|left, right| {
            left.epoch
                .cmp(&right.epoch)
                .then_with(|| left.event_id.cmp(&right.event_id))
        });
        let first_epoch = events.first().map(|event| event.epoch).unwrap_or(0);
        let latest_epoch = events.last().map(|event| event.epoch).unwrap_or(0);
        let mut modality_order = events
            .iter()
            .map(|event| event.modality)
            .collect::<Vec<_>>();
        modality_order.sort();
        modality_order.dedup();
        let mut model_system_order = events
            .iter()
            .filter_map(|event| event.model_system)
            .collect::<Vec<_>>();
        model_system_order.sort();
        model_system_order.dedup();
        let mut source_kind_order = events
            .iter()
            .map(|event| event.source_kind)
            .collect::<Vec<_>>();
        source_kind_order.sort();
        source_kind_order.dedup();
        let sum_state = |state: EvidenceState| -> u32 {
            events
                .iter()
                .filter(|event| event.state == state)
                .map(|event| u32::from(weight(event)))
                .sum::<u32>()
                .min(1_000)
        };
        let support_milli = sum_state(EvidenceState::Supported) as u16;
        let negative_milli = sum_state(EvidenceState::Negative) as u16;
        let contradiction_milli = sum_state(EvidenceState::Contradicted) as u16;
        let unknown_milli = events
            .iter()
            .filter(|event| {
                matches!(
                    event.state,
                    EvidenceState::Unknown | EvidenceState::Stale | EvidenceState::Unmeasured
                )
            })
            .map(weight)
            .map(u32::from)
            .sum::<u32>()
            .min(1_000) as u16;
        let coverage_milli = ((modality_order.len().min(4) * 250)
            .max(model_system_order.len().min(4) * 250))
        .min(1_000) as u16;
        let trend_milli = if events.len() < 2 {
            0
        } else {
            let first = weight(&events[0]) as i16;
            let last = weight(events.last().expect("event exists")) as i16;
            last - first
        };
        let trend = if (contradiction_milli > 0 && negative_milli > 0)
            || unknown_milli >= support_milli.max(negative_milli)
        {
            EvidenceStreamClaimTrend::Unresolved
        } else if trend_milli >= request.trend_threshold_milli as i16 {
            EvidenceStreamClaimTrend::Rising
        } else if trend_milli <= -(request.trend_threshold_milli as i16) {
            EvidenceStreamClaimTrend::Declining
        } else {
            EvidenceStreamClaimTrend::Stable
        };
        let next_action = if contradiction_milli > support_milli {
            "route_to_claim_adjudication_and_replication"
        } else if negative_milli > support_milli {
            "preserve_negative_result_and_test_scope_boundary"
        } else if trend == EvidenceStreamClaimTrend::Rising {
            "refresh_typed_knowledge_and_check_independent_support"
        } else if trend == EvidenceStreamClaimTrend::Declining {
            "revalidate_stale_support_and_open_drift_review"
        } else if trend == EvidenceStreamClaimTrend::Unresolved {
            "acquire_quality_or_coverage_before_promotion"
        } else {
            "continue_prospective_surveillance"
        };
        let mut event_order = events
            .iter()
            .map(|event| event.event_id.clone())
            .collect::<Vec<_>>();
        event_order.sort();
        claims.push(EvidenceStreamClaim {
            claim_key,
            event_order,
            modality_order,
            model_system_order,
            source_kind_order,
            first_epoch,
            latest_epoch,
            support_milli,
            negative_milli,
            contradiction_milli,
            unknown_milli,
            coverage_milli,
            trend_milli,
            trend,
            next_action: next_action.into(),
        });
    }
    claims.sort_by(|left, right| left.claim_key.cmp(&right.claim_key));
    let mut uncertainty = Vec::new();
    if claims.len() > request.max_claims {
        uncertainty.push(format!(
            "{} claims omitted by max_claims bound",
            claims.len() - request.max_claims
        ));
        claims.truncate(request.max_claims);
    }
    if !late.is_empty() {
        uncertainty.push(format!(
            "{} late events require epoch reconciliation",
            late.len()
        ));
    }
    uncertainty.extend(
        claims
            .iter()
            .filter(|claim| claim.trend == EvidenceStreamClaimTrend::Unresolved)
            .map(|claim| format!("{}: unresolved stream state", claim.claim_key)),
    );
    let accepted_order = accepted
        .iter()
        .map(|event| event.event_id.clone())
        .collect::<Vec<_>>();
    let duplicate_order = {
        let mut values = duplicate;
        values.sort();
        values
    };
    let late_order = {
        late.sort();
        late
    };
    let rejected_order = {
        rejected.sort();
        rejected.dedup();
        rejected
    };
    let claim_order = claims
        .iter()
        .map(|claim| claim.claim_key.clone())
        .collect::<Vec<_>>();
    let frontier_order = claims
        .iter()
        .filter(|claim| {
            claim.contradiction_milli > claim.support_milli
                || claim.negative_milli > claim.support_milli
                || claim.trend == EvidenceStreamClaimTrend::Unresolved
                || claim.trend == EvidenceStreamClaimTrend::Declining
        })
        .map(|claim| claim.claim_key.clone())
        .collect::<Vec<_>>();
    let negative_evidence = claims
        .iter()
        .filter(|claim| claim.negative_milli > claim.support_milli)
        .map(|claim| claim.claim_key.clone())
        .collect::<Vec<_>>();
    uncertainty.sort();
    let disposition = if claims.is_empty() {
        EvidenceStreamDisposition::Blocked
    } else if claims
        .iter()
        .all(|claim| claim.negative_milli > claim.support_milli)
    {
        EvidenceStreamDisposition::Negative
    } else if !frontier_order.is_empty() || !late_order.is_empty() || !rejected_order.is_empty() {
        EvidenceStreamDisposition::Partial
    } else {
        EvidenceStreamDisposition::Ready
    };
    let next_step = match disposition {
        EvidenceStreamDisposition::Ready => {
            "hand the stable prospective snapshot to P02 typed-knowledge monitoring"
        }
        EvidenceStreamDisposition::Partial => {
            "reconcile late/rejected/frontier events before promoting the full snapshot"
        }
        EvidenceStreamDisposition::Negative => {
            "preserve the negative stream state and test its declared scope boundary"
        }
        EvidenceStreamDisposition::Blocked => {
            "repair event quality or claim coverage before continuing the stream"
        }
    };
    let mut output = EvidenceStreamSnapshot {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        accepted_order,
        duplicate_order,
        late_order,
        rejected_order,
        claim_order,
        claims,
        frontier_order,
        negative_evidence,
        uncertainty,
        disposition,
        next_step: next_step.into(),
        digest: ContentHash::of_value(&serde_json::Value::Null)
            .map_err(|error| EvidenceStreamError::Digest(error.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| EvidenceStreamError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(id: &str, epoch: u64, state: EvidenceState) -> EvidenceStreamEvent {
        EvidenceStreamEvent {
            event_id: id.into(),
            evidence_id: format!("evidence-{id}"),
            claim_key: "egfr-invasion".into(),
            epoch,
            modality: GliomaModality::Transcriptomics,
            model_system: Some(GliomaModelSystem::Organoid),
            source_kind: EvidenceSourceKind::Assay,
            state,
            quality_milli: 950,
            reproducibility_milli: 900,
            artifact_hash: ContentHash::of_bytes(id.as_bytes()),
            preclinical_only: true,
        }
    }

    fn request(events: Vec<EvidenceStreamEvent>) -> EvidenceStreamRequest {
        EvidenceStreamRequest {
            objective: "prospective glioma evidence".into(),
            events,
            expected_next_epoch: 6,
            max_events: 128,
            max_claims: 32,
            max_epoch_lag: 1,
            min_quality_milli: 700,
            trend_threshold_milli: 100,
        }
    }

    #[test]
    fn stream_snapshot_is_idempotent_and_preserves_late_events() {
        let first = event("one", 4, EvidenceState::Supported);
        let mut duplicate = first.clone();
        duplicate.event_id = "duplicate-event".into();
        let output = snapshot_glioma_evidence_stream(&request(vec![
            first,
            duplicate,
            event("late", 1, EvidenceState::Supported),
        ]))
        .expect("snapshot");
        assert_eq!(output.duplicate_order, vec!["duplicate-event"]);
        assert_eq!(output.late_order, vec!["late"]);
        output.validate().expect("digest validates");
    }

    #[test]
    fn negative_and_contradictory_claims_enter_the_frontier() {
        let mut negative = event("negative", 4, EvidenceState::Negative);
        negative.evidence_id = "same-claim-negative".into();
        let mut contradiction = event("contradiction", 5, EvidenceState::Contradicted);
        contradiction.evidence_id = "same-claim-contradiction".into();
        let output = snapshot_glioma_evidence_stream(&request(vec![negative, contradiction]))
            .expect("snapshot");
        assert_eq!(output.frontier_order, vec!["egfr-invasion"]);
        assert_eq!(output.claims[0].trend, EvidenceStreamClaimTrend::Unresolved);
    }

    #[test]
    fn protected_or_non_preclinical_events_fail_closed() {
        let mut event = event("human", 5, EvidenceState::Supported);
        event.preclinical_only = false;
        assert!(matches!(
            snapshot_glioma_evidence_stream(&request(vec![event])),
            Err(EvidenceStreamError::InvalidEvent(_))
        ));
    }
}

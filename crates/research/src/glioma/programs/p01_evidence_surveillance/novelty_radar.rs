//! Deterministic novelty radar for continuous preclinical glioma evidence surveillance.
//!
//! This is a product capability rather than a search hypothesis: a local institution can feed
//! normalized literature/source records into the radar and receive a reproducible ranked queue of
//! evidence that is genuinely new relative to its existing claim and domain corpus. Freshness,
//! quality, domain gaps, near-duplicate suppression, and uncertainty are scored separately so a
//! high citation count cannot disguise stale or weak evidence.

use crate::glioma_engine::LocalArtifactRef;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P01-F02";
pub const OUTPUT_SCHEMA: &str = "GliomaEvidenceNoveltyRadar1@1";
pub const MAX_RECORDS: usize = 4_096;
pub const MAX_ACTIONS: usize = 1_024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceNoveltyRecord {
    pub evidence_id: String,
    pub source_id: String,
    pub title: String,
    pub term_order: Vec<String>,
    pub domain_order: Vec<String>,
    pub claim_order: Vec<String>,
    pub publication_tick: u64,
    pub quality_milli: u16,
    pub citation_count: u32,
    pub artifact: LocalArtifactRef,
    pub preclinical_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceNoveltyRadarRequest {
    pub objective: String,
    pub snapshot_id: String,
    pub as_of_tick: u64,
    pub novelty_floor_milli: u16,
    pub quality_floor_milli: u16,
    pub freshness_window_ticks: u64,
    pub max_actions: usize,
    pub known_term_order: Vec<String>,
    pub known_domain_order: Vec<String>,
    pub known_claim_order: Vec<String>,
    pub records: Vec<EvidenceNoveltyRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceNoveltyActionDisposition {
    Acquire,
    Review,
    Deprioritized,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceNoveltyAction {
    pub action_id: String,
    pub evidence_id: String,
    pub source_id: String,
    pub rank: u32,
    pub disposition: EvidenceNoveltyActionDisposition,
    pub novelty_milli: u16,
    pub freshness_milli: u16,
    pub quality_milli: u16,
    pub citation_signal_milli: u16,
    pub domain_gap_milli: u16,
    pub known_term_overlap_milli: u16,
    pub nearest_record_overlap_milli: u16,
    pub domain_gap_order: Vec<String>,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceNoveltyRadarDisposition {
    Ready,
    Partial,
    NoNovelEvidence,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceNoveltyRadar {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub snapshot_id: String,
    pub action_order: Vec<String>,
    pub actions: Vec<EvidenceNoveltyAction>,
    pub candidate_order: Vec<String>,
    pub review_order: Vec<String>,
    pub unresolved_evidence_order: Vec<String>,
    pub domain_gap_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: EvidenceNoveltyRadarDisposition,
    pub next_step: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EvidenceNoveltyRadarError {
    #[error("evidence novelty radar request is invalid: {0}")]
    InvalidRequest(String),
    #[error("evidence novelty radar output is invalid: {0}")]
    InvalidOutput(String),
    #[error("evidence novelty radar digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn valid_token(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 96
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._:-".contains(&byte)
        })
}

fn validate_tokens(values: &[String]) -> bool {
    canonical(values) && values.iter().all(|value| valid_token(value))
}

fn overlap_milli(left: &BTreeSet<String>, right: &BTreeSet<String>) -> u16 {
    if left.is_empty() && right.is_empty() {
        return 1_000;
    }
    let intersection = left.intersection(right).count() as u64;
    let union = left.union(right).count() as u64;
    ((intersection.saturating_mul(1_000)) / union.max(1)) as u16
}

fn freshness_milli(as_of_tick: u64, publication_tick: u64, window: u64) -> u16 {
    if publication_tick > as_of_tick {
        return 0;
    }
    if window == 0 {
        return if publication_tick == as_of_tick {
            1_000
        } else {
            0
        };
    }
    let age = as_of_tick.saturating_sub(publication_tick);
    ((1_000_u64.saturating_mul(window)) / window.saturating_add(age).max(1)).min(1_000) as u16
}

fn citation_signal_milli(citation_count: u32) -> u16 {
    if citation_count == 0 {
        return 0;
    }
    let scaled = (u64::from(citation_count).ilog2() as u64 + 1).saturating_mul(125);
    scaled.min(1_000) as u16
}

fn digest_input(output: &EvidenceNoveltyRadar) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "snapshot_id": output.snapshot_id,
        "action_order": output.action_order,
        "actions": output.actions,
        "candidate_order": output.candidate_order,
        "review_order": output.review_order,
        "unresolved_evidence_order": output.unresolved_evidence_order,
        "domain_gap_order": output.domain_gap_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
        "next_step": output.next_step,
    })
}

impl EvidenceNoveltyRadar {
    pub fn validate(&self) -> Result<(), EvidenceNoveltyRadarError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.snapshot_id.trim().is_empty()
            || !canonical(&self.action_order)
            || !canonical(&self.candidate_order)
            || !canonical(&self.review_order)
            || !canonical(&self.unresolved_evidence_order)
            || !canonical(&self.domain_gap_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.actions.len() > MAX_ACTIONS
            || self.actions.iter().any(|action| {
                action.action_id != format!("novelty:{}", action.evidence_id)
                    || action
                        .domain_gap_order
                        .windows(2)
                        .any(|pair| pair[0] >= pair[1])
                    || action.novelty_milli > 1_000
                    || action.freshness_milli > 1_000
                    || action.quality_milli > 1_000
                    || action.citation_signal_milli > 1_000
                    || action.domain_gap_milli > 1_000
                    || action.known_term_overlap_milli > 1_000
                    || action.nearest_record_overlap_milli > 1_000
            })
        {
            return Err(EvidenceNoveltyRadarError::InvalidOutput(
                "radar identity, canonical ordering, score bounds, or action identity is invalid"
                    .into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| EvidenceNoveltyRadarError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(EvidenceNoveltyRadarError::InvalidOutput(
                "radar digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &EvidenceNoveltyRadarRequest,
) -> Result<(), EvidenceNoveltyRadarError> {
    if request.objective.trim().is_empty()
        || request.snapshot_id.trim().is_empty()
        || request.novelty_floor_milli > 1_000
        || request.quality_floor_milli > 1_000
        || request.freshness_window_ticks == 0
        || request.max_actions == 0
        || request.max_actions > MAX_ACTIONS
        || request.records.is_empty()
        || request.records.len() > MAX_RECORDS
        || !validate_tokens(&request.known_term_order)
        || !validate_tokens(&request.known_domain_order)
        || !validate_tokens(&request.known_claim_order)
    {
        return Err(EvidenceNoveltyRadarError::InvalidRequest(
            "objective, snapshot, bounded thresholds, records, and canonical corpora are required"
                .into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for record in &request.records {
        if record.evidence_id.trim().is_empty()
            || record.source_id.trim().is_empty()
            || record.title.trim().is_empty()
            || !ids.insert(record.evidence_id.clone())
            || !validate_tokens(&record.term_order)
            || !validate_tokens(&record.domain_order)
            || !validate_tokens(&record.claim_order)
            || record.publication_tick > request.as_of_tick
            || record.quality_milli > 1_000
            || !record.preclinical_only
        {
            return Err(EvidenceNoveltyRadarError::InvalidRequest(
                "records must be unique, canonical, preclinical-only, and bounded to the snapshot"
                    .into(),
            ));
        }
        record
            .artifact
            .validate()
            .map_err(|error| EvidenceNoveltyRadarError::InvalidRequest(error.to_string()))?;
    }
    Ok(())
}

/// Rank new preclinical glioma evidence into a deterministic acquisition/review queue.
pub fn rank_glioma_evidence_novelty(
    request: &EvidenceNoveltyRadarRequest,
) -> Result<EvidenceNoveltyRadar, EvidenceNoveltyRadarError> {
    validate_request(request)?;
    let known_terms = request
        .known_term_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let known_domains = request
        .known_domain_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut records = request.records.clone();
    records.sort_by(|left, right| left.evidence_id.cmp(&right.evidence_id));
    let term_sets = records
        .iter()
        .map(|record| {
            (
                record.evidence_id.clone(),
                record.term_order.iter().cloned().collect::<BTreeSet<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut actions = Vec::new();
    let mut unresolved = BTreeSet::new();
    let mut domain_gaps = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for record in &records {
        let terms = term_sets
            .get(&record.evidence_id)
            .expect("term set exists for every validated record");
        let known_overlap = overlap_milli(terms, &known_terms);
        let nearest_overlap = records
            .iter()
            .filter(|other| other.evidence_id != record.evidence_id)
            .filter_map(|other| term_sets.get(&other.evidence_id))
            .map(|other_terms| overlap_milli(terms, other_terms))
            .max()
            .unwrap_or_default();
        let novelty = 1_000_u16.saturating_sub(known_overlap.max(nearest_overlap));
        let freshness = freshness_milli(
            request.as_of_tick,
            record.publication_tick,
            request.freshness_window_ticks,
        );
        let citation = citation_signal_milli(record.citation_count);
        let gaps = record
            .domain_order
            .iter()
            .filter(|domain| !known_domains.contains(*domain))
            .cloned()
            .collect::<BTreeSet<_>>();
        domain_gaps.extend(gaps.iter().cloned());
        let domain_gap = ((gaps.len().min(4) as u16) * 250).min(1_000);
        let score = ((u32::from(novelty) * 450
            + u32::from(freshness) * 200
            + u32::from(record.quality_milli) * 200
            + u32::from(citation) * 50
            + u32::from(domain_gap) * 100)
            / 1_000) as u16;
        let disposition = if record.quality_milli < request.quality_floor_milli {
            unresolved.insert(record.evidence_id.clone());
            uncertainty.insert(format!("{}:below-quality-floor", record.evidence_id));
            EvidenceNoveltyActionDisposition::Review
        } else if score >= request.novelty_floor_milli {
            EvidenceNoveltyActionDisposition::Acquire
        } else {
            EvidenceNoveltyActionDisposition::Deprioritized
        };
        let rationale = match disposition {
            EvidenceNoveltyActionDisposition::Acquire => {
                "new preclinical evidence exceeds the novelty/quality acquisition gate"
            }
            EvidenceNoveltyActionDisposition::Review => {
                "potentially novel evidence requires quality review before acquisition"
            }
            EvidenceNoveltyActionDisposition::Deprioritized => {
                "evidence is stale, overlapping, or insufficiently novel for this snapshot"
            }
        };
        actions.push(EvidenceNoveltyAction {
            action_id: format!("novelty:{}", record.evidence_id),
            evidence_id: record.evidence_id.clone(),
            source_id: record.source_id.clone(),
            rank: 0,
            disposition,
            novelty_milli: novelty,
            freshness_milli: freshness,
            quality_milli: record.quality_milli,
            citation_signal_milli: citation,
            domain_gap_milli: domain_gap,
            known_term_overlap_milli: known_overlap,
            nearest_record_overlap_milli: nearest_overlap,
            domain_gap_order: gaps.into_iter().collect(),
            rationale: rationale.into(),
        });
    }
    actions.sort_by(|left, right| {
        right
            .novelty_milli
            .cmp(&left.novelty_milli)
            .then(right.quality_milli.cmp(&left.quality_milli))
            .then(left.evidence_id.cmp(&right.evidence_id))
    });
    actions.truncate(request.max_actions);
    for (index, action) in actions.iter_mut().enumerate() {
        action.rank = u32::try_from(index + 1).unwrap_or(u32::MAX);
    }
    let action_order = actions
        .iter()
        .map(|action| action.action_id.clone())
        .collect::<Vec<_>>();
    let candidate_order = actions
        .iter()
        .filter(|action| {
            matches!(
                action.disposition,
                EvidenceNoveltyActionDisposition::Acquire
            )
        })
        .map(|action| action.action_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let review_order = actions
        .iter()
        .filter(|action| matches!(action.disposition, EvidenceNoveltyActionDisposition::Review))
        .map(|action| action.action_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let disposition = if actions.is_empty() {
        EvidenceNoveltyRadarDisposition::NoNovelEvidence
    } else if !candidate_order.is_empty() {
        if !unresolved.is_empty() {
            EvidenceNoveltyRadarDisposition::Partial
        } else {
            EvidenceNoveltyRadarDisposition::Ready
        }
    } else if !review_order.is_empty() {
        EvidenceNoveltyRadarDisposition::Partial
    } else {
        EvidenceNoveltyRadarDisposition::NoNovelEvidence
    };
    let next_step = match disposition {
        EvidenceNoveltyRadarDisposition::Ready => {
            "acquire the ranked novel sources and compile their claims into P02"
        }
        EvidenceNoveltyRadarDisposition::Partial => {
            "review unresolved quality records, then acquire only the qualified novel sources"
        }
        EvidenceNoveltyRadarDisposition::NoNovelEvidence => {
            "refresh the bounded source snapshot or widen the declared domain corpus"
        }
        EvidenceNoveltyRadarDisposition::Blocked => {
            "resolve snapshot, locality, or preclinical-boundary validation failures"
        }
    };
    let mut output = EvidenceNoveltyRadar {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        snapshot_id: request.snapshot_id.clone(),
        action_order,
        actions,
        candidate_order,
        review_order,
        unresolved_evidence_order: unresolved.into_iter().collect(),
        domain_gap_order: domain_gaps.into_iter().collect(),
        negative_evidence: vec!["novelty-ranking-is-not-evidence-validity-or-causal-support".into()],
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        next_step: next_step.into(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-evidence-novelty-radar"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| EvidenceNoveltyRadarError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlap_and_freshness_are_bounded_and_deterministic() {
        let left = BTreeSet::from(["egfr".to_string(), "mapk".to_string()]);
        let right = BTreeSet::from(["egfr".to_string(), "pi3k".to_string()]);
        assert_eq!(overlap_milli(&left, &right), 333);
        assert_eq!(freshness_milli(100, 90, 20), 666);
        assert_eq!(freshness_milli(100, 100, 20), 1_000);
    }
}

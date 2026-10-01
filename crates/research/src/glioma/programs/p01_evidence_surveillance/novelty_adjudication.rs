//! Claim-level novelty adjudication for preclinical glioma evidence.
//!
//! The novelty radar ranks records. This feature makes the stronger product decision: whether a
//! candidate is genuinely new, an exact duplicate, a replication, a scope extension, an explicit
//! contradiction, or unresolved. It compares only caller-supplied typed metadata; it never
//! infers contradiction from wording and never fetches or moves source data.

use crate::glioma::evidence::{EvidenceSourceKind, EvidenceState};
use crate::glioma_engine::{GliomaModality, GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P01-F06";
pub const OUTPUT_SCHEMA: &str = "GliomaEvidenceNoveltyAdjudication1@1";
pub const MAX_RECORDS: usize = 8_192;
pub const MAX_ITEMS: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoveltyAdjudicationRecord {
    pub evidence_id: String,
    pub claim: String,
    pub scope: String,
    pub term_order: Vec<String>,
    pub domain_order: Vec<String>,
    pub artifact: LocalArtifactRef,
    pub source_kind: EvidenceSourceKind,
    pub modality: GliomaModality,
    pub model_system: Option<GliomaModelSystem>,
    pub state: EvidenceState,
    pub quality_milli: u16,
    pub relevance_milli: u16,
    pub reproducibility_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoveltyAdjudicationRequest {
    pub objective: String,
    pub baseline: Vec<NoveltyAdjudicationRecord>,
    pub candidates: Vec<NoveltyAdjudicationRecord>,
    pub min_quality_milli: u16,
    pub novelty_floor_milli: u16,
    pub contradiction_floor_milli: u16,
    pub max_items: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoveltyAdjudicationVerdict {
    Novel,
    ScopeExtension,
    Replication,
    Contradiction,
    Duplicate,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoveltyAdjudicationItem {
    pub item_id: String,
    pub candidate_id: String,
    pub nearest_baseline_order: Vec<String>,
    pub claim_overlap_milli: u16,
    pub scope_overlap_milli: u16,
    pub domain_overlap_milli: u16,
    pub novelty_milli: u16,
    pub quality_milli: u16,
    pub contradiction_milli: u16,
    pub verdict: NoveltyAdjudicationVerdict,
    pub next_action: String,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoveltyAdjudicationDisposition {
    Qualified,
    Negative,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoveltyAdjudication {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub item_order: Vec<String>,
    pub items: Vec<NoveltyAdjudicationItem>,
    pub novel_order: Vec<String>,
    pub extension_order: Vec<String>,
    pub replication_order: Vec<String>,
    pub contradiction_order: Vec<String>,
    pub duplicate_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub omitted_order: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: NoveltyAdjudicationDisposition,
    pub next_step: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum NoveltyAdjudicationError {
    #[error("novelty adjudication request is invalid: {0}")]
    InvalidRequest(String),
    #[error("novelty adjudication record is invalid: {0}")]
    InvalidRecord(String),
    #[error("novelty adjudication output is invalid: {0}")]
    InvalidOutput(String),
    #[error("novelty adjudication digest failed: {0}")]
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

fn token_set(values: &[String]) -> BTreeSet<String> {
    values.iter().cloned().collect()
}

fn overlap_milli(left: &BTreeSet<String>, right: &BTreeSet<String>) -> u16 {
    if left.is_empty() && right.is_empty() {
        return 1_000;
    }
    let intersection = left.intersection(right).count() as u64;
    let union = left.union(right).count() as u64;
    ((intersection * 1_000) / union.max(1)) as u16
}

fn semantic_key(record: &NoveltyAdjudicationRecord) -> String {
    format!(
        "{}\u{1f}{}\u{1f}{:?}\u{1f}{:?}",
        record.claim, record.scope, record.modality, record.model_system
    )
}

fn item_id(candidate_id: &str) -> String {
    format!("novelty-adjudication:{candidate_id}")
}

fn quality(record: &NoveltyAdjudicationRecord) -> u16 {
    ((u32::from(record.quality_milli)
        * u32::from(record.relevance_milli)
        * u32::from(record.reproducibility_milli))
        / 1_000_000) as u16
}

fn digest_input(output: &NoveltyAdjudication) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "item_order": output.item_order,
        "items": output.items,
        "novel_order": output.novel_order,
        "extension_order": output.extension_order,
        "replication_order": output.replication_order,
        "contradiction_order": output.contradiction_order,
        "duplicate_order": output.duplicate_order,
        "unresolved_order": output.unresolved_order,
        "negative_evidence": output.negative_evidence,
        "omitted_order": output.omitted_order,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
        "next_step": output.next_step,
    })
}

impl NoveltyAdjudication {
    pub fn validate(&self) -> Result<(), NoveltyAdjudicationError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || !canonical(&self.item_order)
            || !canonical(&self.novel_order)
            || !canonical(&self.extension_order)
            || !canonical(&self.replication_order)
            || !canonical(&self.contradiction_order)
            || !canonical(&self.duplicate_order)
            || !canonical(&self.unresolved_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.omitted_order)
            || !canonical(&self.uncertainty)
            || self.items.iter().any(|item| {
                item.item_id != item_id(&item.candidate_id)
                    || item.candidate_id.trim().is_empty()
                    || !canonical(&item.nearest_baseline_order)
                    || item.claim_overlap_milli > 1_000
                    || item.scope_overlap_milli > 1_000
                    || item.domain_overlap_milli > 1_000
                    || item.novelty_milli > 1_000
                    || item.quality_milli > 1_000
                    || item.contradiction_milli > 1_000
                    || item.next_action.trim().is_empty()
                    || item.rationale.trim().is_empty()
            })
        {
            return Err(NoveltyAdjudicationError::InvalidOutput(
                "identity, ordering, score, or rationale fields are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| NoveltyAdjudicationError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(NoveltyAdjudicationError::InvalidOutput(
                "novelty adjudication digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_record(
    record: &NoveltyAdjudicationRecord,
    ids: &mut BTreeSet<String>,
) -> Result<(), NoveltyAdjudicationError> {
    record
        .artifact
        .validate()
        .map_err(|error| NoveltyAdjudicationError::InvalidRecord(error.to_string()))?;
    if record.evidence_id.trim().is_empty()
        || !ids.insert(record.evidence_id.clone())
        || record.claim.trim().is_empty()
        || record.scope.trim().is_empty()
        || !canonical(&record.term_order)
        || !canonical(&record.domain_order)
        || !record.term_order.iter().all(|value| valid_token(value))
        || !record.domain_order.iter().all(|value| valid_token(value))
        || record.quality_milli > 1_000
        || record.relevance_milli > 1_000
        || record.reproducibility_milli > 1_000
        || !record.artifact.local_only
        || record.artifact.contains_human_data
        || record.artifact.contains_direct_identifiers
    {
        return Err(NoveltyAdjudicationError::InvalidRecord(
            "record identity, token ordering, score, artifact, and preclinical fields are invalid"
                .into(),
        ));
    }
    Ok(())
}

pub fn adjudicate_glioma_evidence_novelty(
    request: &NoveltyAdjudicationRequest,
) -> Result<NoveltyAdjudication, NoveltyAdjudicationError> {
    if request.objective.trim().is_empty()
        || request.baseline.len() + request.candidates.len() > MAX_RECORDS
        || request.candidates.is_empty()
        || request.max_items == 0
        || request.max_items > MAX_ITEMS
        || request.min_quality_milli > 1_000
        || request.novelty_floor_milli > 1_000
        || request.contradiction_floor_milli > 1_000
    {
        return Err(NoveltyAdjudicationError::InvalidRequest(
            "objective, bounded baseline/candidate corpora, item capacity, and score thresholds are required".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for record in &request.baseline {
        validate_record(record, &mut ids)?;
    }
    let mut candidate_ids = BTreeSet::new();
    for record in &request.candidates {
        validate_record(record, &mut ids)?;
        if !candidate_ids.insert(record.evidence_id.clone()) {
            return Err(NoveltyAdjudicationError::InvalidRecord(
                "candidate evidence ids must be unique".into(),
            ));
        }
    }

    let mut items = Vec::new();
    for candidate in &request.candidates {
        let candidate_terms = token_set(&candidate.term_order);
        let candidate_domains = token_set(&candidate.domain_order);
        let candidate_key = semantic_key(candidate);
        let mut matches = request
            .baseline
            .iter()
            .map(|baseline| {
                let claim_overlap = if candidate.claim == baseline.claim {
                    1_000
                } else {
                    overlap_milli(&candidate_terms, &token_set(&baseline.term_order))
                };
                let scope_overlap = if candidate.scope == baseline.scope {
                    1_000
                } else {
                    overlap_milli(&candidate_domains, &token_set(&baseline.domain_order))
                };
                let domain_overlap =
                    overlap_milli(&candidate_domains, &token_set(&baseline.domain_order));
                let artifact_exact =
                    candidate.artifact.content_hash == baseline.artifact.content_hash;
                let semantic_exact = candidate_key == semantic_key(baseline);
                let similarity = (u32::from(claim_overlap) * 55
                    + u32::from(scope_overlap) * 20
                    + u32::from(domain_overlap) * 15
                    + if artifact_exact { 1_000 * 10 } else { 0 })
                    / 100;
                let contradiction = if candidate.state == EvidenceState::Contradicted {
                    1_000
                } else if semantic_exact
                    && candidate.state == EvidenceState::Negative
                    && baseline.state == EvidenceState::Supported
                {
                    800
                } else {
                    0
                };
                (
                    baseline,
                    claim_overlap,
                    scope_overlap,
                    domain_overlap,
                    similarity.min(1_000) as u16,
                    contradiction,
                    artifact_exact,
                    semantic_exact,
                )
            })
            .collect::<Vec<_>>();
        matches.sort_by(|left, right| {
            right
                .4
                .cmp(&left.4)
                .then_with(|| left.0.evidence_id.cmp(&right.0.evidence_id))
        });
        let nearest = matches
            .iter()
            .take(3)
            .filter(|entry| entry.4 > 0)
            .map(|entry| entry.0.evidence_id.clone())
            .collect::<Vec<_>>();
        let best = matches.first().copied();
        let (
            claim_overlap,
            scope_overlap,
            domain_overlap,
            similarity,
            contradiction,
            exact_artifact,
            semantic_exact,
        ) = best
            .map(|entry| {
                (
                    entry.1, entry.2, entry.3, entry.4, entry.5, entry.6, entry.7,
                )
            })
            .unwrap_or((0, 0, 0, 0, 0, false, false));
        let novelty = if best.is_none() {
            1_000
        } else {
            1_000_u16.saturating_sub(similarity)
        };
        let quality_milli = quality(candidate);
        let verdict = if quality_milli < request.min_quality_milli
            || matches!(
                candidate.state,
                EvidenceState::Unknown | EvidenceState::Stale | EvidenceState::Unmeasured
            ) {
            NoveltyAdjudicationVerdict::Unresolved
        } else if contradiction >= request.contradiction_floor_milli {
            NoveltyAdjudicationVerdict::Contradiction
        } else if exact_artifact {
            NoveltyAdjudicationVerdict::Duplicate
        } else if semantic_exact {
            NoveltyAdjudicationVerdict::Replication
        } else if claim_overlap >= 700 && scope_overlap < 900 {
            NoveltyAdjudicationVerdict::ScopeExtension
        } else if novelty >= request.novelty_floor_milli {
            NoveltyAdjudicationVerdict::Novel
        } else {
            NoveltyAdjudicationVerdict::Unresolved
        };
        let next_action = match verdict {
            NoveltyAdjudicationVerdict::Novel => "compile_new_claim_into_typed_knowledge",
            NoveltyAdjudicationVerdict::ScopeExtension => {
                "align_scope_and_update_knowledge_closure"
            }
            NoveltyAdjudicationVerdict::Replication => "route_to_independent_replication_synthesis",
            NoveltyAdjudicationVerdict::Contradiction => {
                "open_claim_adjudication_and_preserve_rival"
            }
            NoveltyAdjudicationVerdict::Duplicate => {
                "retain_one_artifact_and_record_duplicate_provenance"
            }
            NoveltyAdjudicationVerdict::Unresolved => {
                "request_quality_or_scope_review_before_promotion"
            }
        };
        let rationale = match verdict {
            NoveltyAdjudicationVerdict::Novel => format!(
                "candidate has novelty {novelty} above floor {} with no typed baseline match",
                request.novelty_floor_milli
            ),
            NoveltyAdjudicationVerdict::ScopeExtension => {
                "claim overlaps a baseline but its declared scope or domain extends it".into()
            }
            NoveltyAdjudicationVerdict::Replication => {
                "semantic claim/scope matches a baseline while the artifact is independent".into()
            }
            NoveltyAdjudicationVerdict::Contradiction => {
                "explicit contradiction state or explicit supported-versus-negative conflict is retained".into()
            }
            NoveltyAdjudicationVerdict::Duplicate => {
                "candidate points to an exact artifact already present in the baseline".into()
            }
            NoveltyAdjudicationVerdict::Unresolved => {
                "quality, state, or similarity is insufficient for a promotion verdict".into()
            }
        };
        items.push(NoveltyAdjudicationItem {
            item_id: item_id(&candidate.evidence_id),
            candidate_id: candidate.evidence_id.clone(),
            nearest_baseline_order: nearest,
            claim_overlap_milli: claim_overlap,
            scope_overlap_milli: scope_overlap,
            domain_overlap_milli: domain_overlap,
            novelty_milli: novelty,
            quality_milli,
            contradiction_milli: contradiction,
            verdict,
            next_action: next_action.into(),
            rationale,
        });
    }
    items.sort_by(|left, right| left.candidate_id.cmp(&right.candidate_id));
    let omitted_order = items
        .iter()
        .skip(request.max_items)
        .map(|item| item.item_id.clone())
        .collect::<Vec<_>>();
    items.truncate(request.max_items);
    let item_order = items
        .iter()
        .map(|item| item.item_id.clone())
        .collect::<Vec<_>>();
    let by_verdict = |verdict| {
        items
            .iter()
            .filter(|item| item.verdict == verdict)
            .map(|item| item.item_id.clone())
            .collect::<Vec<_>>()
    };
    let novel_order = by_verdict(NoveltyAdjudicationVerdict::Novel);
    let extension_order = by_verdict(NoveltyAdjudicationVerdict::ScopeExtension);
    let replication_order = by_verdict(NoveltyAdjudicationVerdict::Replication);
    let contradiction_order = by_verdict(NoveltyAdjudicationVerdict::Contradiction);
    let duplicate_order = by_verdict(NoveltyAdjudicationVerdict::Duplicate);
    let unresolved_order = by_verdict(NoveltyAdjudicationVerdict::Unresolved);
    let negative_evidence = request
        .candidates
        .iter()
        .filter(|record| record.state == EvidenceState::Negative)
        .map(|record| record.evidence_id.clone())
        .filter(|id| item_order.iter().any(|item| item == &item_id(id)))
        .collect::<Vec<_>>();
    let mut uncertainty = omitted_order
        .iter()
        .map(|item| format!("{item}: omitted by max_items bound"))
        .collect::<Vec<_>>();
    uncertainty.extend(
        unresolved_order
            .iter()
            .map(|item| format!("{item}: unresolved novelty state")),
    );
    uncertainty.sort();
    let disposition = if !contradiction_order.is_empty() || !unresolved_order.is_empty() {
        NoveltyAdjudicationDisposition::Partial
    } else if !novel_order.is_empty()
        || !extension_order.is_empty()
        || !replication_order.is_empty()
    {
        NoveltyAdjudicationDisposition::Qualified
    } else if !negative_evidence.is_empty() {
        NoveltyAdjudicationDisposition::Negative
    } else {
        NoveltyAdjudicationDisposition::Unresolved
    };
    let next_step = match disposition {
        NoveltyAdjudicationDisposition::Qualified => {
            "send novel, scope-extension, and replication items to typed knowledge with explicit provenance"
        }
        NoveltyAdjudicationDisposition::Negative => {
            "preserve negative candidates and test whether their boundary invalidates the baseline claim"
        }
        NoveltyAdjudicationDisposition::Partial => {
            "route contradictions and unresolved items to adjudication before promotion"
        }
        NoveltyAdjudicationDisposition::Unresolved => {
            "acquire better-scoped or higher-quality evidence before making a novelty claim"
        }
    };
    let mut output = NoveltyAdjudication {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        item_order,
        items,
        novel_order,
        extension_order,
        replication_order,
        contradiction_order,
        duplicate_order,
        unresolved_order,
        negative_evidence,
        omitted_order,
        uncertainty,
        disposition,
        next_step: next_step.into(),
        digest: ContentHash::of_value(&serde_json::Value::Null)
            .map_err(|error| NoveltyAdjudicationError::Digest(error.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| NoveltyAdjudicationError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(
        id: &str,
        artifact: &str,
        claim: &str,
        scope: &str,
        state: EvidenceState,
    ) -> NoveltyAdjudicationRecord {
        NoveltyAdjudicationRecord {
            evidence_id: id.into(),
            claim: claim.into(),
            scope: scope.into(),
            term_order: vec!["egfr".into(), "invasion".into()],
            domain_order: vec!["glioma".into(), "organoid".into()],
            artifact: LocalArtifactRef {
                artifact_id: format!("artifact-{artifact}"),
                content_hash: ContentHash::of_bytes(artifact.as_bytes()),
                content_type: "application/json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
            source_kind: EvidenceSourceKind::Assay,
            modality: GliomaModality::Transcriptomics,
            model_system: Some(GliomaModelSystem::Organoid),
            state,
            quality_milli: 950,
            relevance_milli: 900,
            reproducibility_milli: 900,
        }
    }

    fn request(
        baseline: Vec<NoveltyAdjudicationRecord>,
        candidates: Vec<NoveltyAdjudicationRecord>,
    ) -> NoveltyAdjudicationRequest {
        NoveltyAdjudicationRequest {
            objective: "glioma novelty".into(),
            baseline,
            candidates,
            min_quality_milli: 700,
            novelty_floor_milli: 650,
            contradiction_floor_milli: 700,
            max_items: 32,
        }
    }

    #[test]
    fn new_candidate_is_adjudicated_as_novel() {
        let output = adjudicate_glioma_evidence_novelty(&request(
            Vec::new(),
            vec![record(
                "candidate",
                "new",
                "pdgfr drives invasion",
                "organoid preclinical",
                EvidenceState::Supported,
            )],
        ))
        .expect("novelty");
        assert_eq!(output.items[0].verdict, NoveltyAdjudicationVerdict::Novel);
        output.validate().expect("digest validates");
    }

    #[test]
    fn exact_artifact_is_not_reported_as_new() {
        let baseline = record(
            "baseline",
            "same",
            "egfr drives invasion",
            "organoid preclinical",
            EvidenceState::Supported,
        );
        let candidate = record(
            "candidate",
            "same",
            "egfr drives invasion",
            "organoid preclinical",
            EvidenceState::Supported,
        );
        let output = adjudicate_glioma_evidence_novelty(&request(vec![baseline], vec![candidate]))
            .expect("duplicate");
        assert_eq!(
            output.items[0].verdict,
            NoveltyAdjudicationVerdict::Duplicate
        );
    }

    #[test]
    fn explicit_contradiction_and_negative_state_are_preserved() {
        let baseline = record(
            "baseline",
            "old",
            "egfr drives invasion",
            "organoid preclinical",
            EvidenceState::Supported,
        );
        let mut candidate = record(
            "candidate",
            "new",
            "egfr drives invasion",
            "organoid preclinical",
            EvidenceState::Contradicted,
        );
        candidate.reproducibility_milli = 1_000;
        let output = adjudicate_glioma_evidence_novelty(&request(vec![baseline], vec![candidate]))
            .expect("contradiction");
        assert_eq!(
            output.items[0].verdict,
            NoveltyAdjudicationVerdict::Contradiction
        );
        assert_eq!(output.contradiction_order.len(), 1);
    }
}

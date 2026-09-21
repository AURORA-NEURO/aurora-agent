//! Local single-study researcher workbench for evidence exploration.
//!
//! This product is the interactive/query surface between typed local evidence and the
//! autonomous research engine. It provides deterministic term, scope, modality, model, source,
//! state, quality, reproducibility, and recency filtering; ranks records with an explainable
//! score; and makes every omission visible. It does not retrieve literature, infer causality, or
//! turn a ranked record into a scientific conclusion.

use crate::glioma::evidence::{EvidenceRecord, EvidenceSourceKind, EvidenceState};
use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P01-F17";
pub const OUTPUT_SCHEMA: &str = "GliomaResearcherEvidenceWorkbench1@1";
pub const MAX_RECORDS: usize = 16_384;
pub const MAX_RESULTS: usize = 4_096;
pub const MAX_TERMS: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceWorkbenchSort {
    Relevance,
    Recency,
    Quality,
    StatePressure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceWorkbenchDisposition {
    Ready,
    Partial,
    NoMatches,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceWorkbenchRequest {
    pub objective: String,
    pub researcher: String,
    pub claim_terms: Vec<String>,
    pub scope_terms: Vec<String>,
    pub required_modalities: BTreeSet<GliomaModality>,
    pub required_model_systems: BTreeSet<GliomaModelSystem>,
    pub required_source_kinds: BTreeSet<EvidenceSourceKind>,
    pub allowed_states: BTreeSet<EvidenceState>,
    pub minimum_quality_milli: u16,
    pub minimum_reproducibility_milli: u16,
    pub current_epoch: u32,
    pub maximum_age_epochs: u32,
    pub maximum_results: usize,
    pub include_negative: bool,
    pub include_contradicted: bool,
    pub include_uncertain: bool,
    pub sort: EvidenceWorkbenchSort,
    pub records: Vec<EvidenceRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceWorkbenchHit {
    pub evidence_id: String,
    pub rank: usize,
    pub score_milli: u16,
    pub term_match_milli: u16,
    pub metadata_match_milli: u16,
    pub quality_milli: u16,
    pub reproducibility_milli: u16,
    pub age_epochs: u32,
    pub state: EvidenceState,
    pub explanation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceWorkbenchOmission {
    pub evidence_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceWorkbenchPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub researcher: String,
    pub query_digest: ContentHash,
    pub record_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub omitted_order: Vec<String>,
    pub hits: Vec<EvidenceWorkbenchHit>,
    pub omissions: Vec<EvidenceWorkbenchOmission>,
    pub modality_counts: BTreeMap<String, usize>,
    pub model_system_counts: BTreeMap<String, usize>,
    pub source_kind_counts: BTreeMap<String, usize>,
    pub state_counts: BTreeMap<String, usize>,
    pub unmatched_claim_terms: Vec<String>,
    pub unmatched_scope_terms: Vec<String>,
    pub negative_order: Vec<String>,
    pub contradicted_order: Vec<String>,
    pub uncertain_order: Vec<String>,
    pub disposition: EvidenceWorkbenchDisposition,
    pub next_route: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EvidenceWorkbenchError {
    #[error("researcher evidence workbench request is invalid: {0}")]
    InvalidRequest(String),
    #[error("researcher evidence workbench record is invalid: {0}")]
    InvalidRecord(String),
    #[error("researcher evidence workbench output is invalid: {0}")]
    InvalidOutput(String),
    #[error("researcher evidence workbench digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique_nonempty(values: &[String]) -> bool {
    values.iter().all(|value| !value.trim().is_empty())
        && values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

fn normalized_terms(values: &[String]) -> Vec<String> {
    let mut terms = values
        .iter()
        .flat_map(|value| {
            value
                .split(|character: char| !character.is_alphanumeric())
                .filter(|term| !term.is_empty())
                .map(|term| term.to_lowercase())
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    terms.sort();
    terms
}

fn token_set(value: &str) -> BTreeSet<String> {
    value
        .split(|character: char| !character.is_alphanumeric())
        .filter(|term| !term.is_empty())
        .map(|term| term.to_lowercase())
        .collect()
}

fn digest_input(output: &EvidenceWorkbenchPlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "researcher": output.researcher,
        "query_digest": output.query_digest,
        "record_order": output.record_order,
        "selected_order": output.selected_order,
        "omitted_order": output.omitted_order,
        "hits": output.hits,
        "omissions": output.omissions,
        "modality_counts": output.modality_counts,
        "model_system_counts": output.model_system_counts,
        "source_kind_counts": output.source_kind_counts,
        "state_counts": output.state_counts,
        "unmatched_claim_terms": output.unmatched_claim_terms,
        "unmatched_scope_terms": output.unmatched_scope_terms,
        "negative_order": output.negative_order,
        "contradicted_order": output.contradicted_order,
        "uncertain_order": output.uncertain_order,
        "disposition": output.disposition,
        "next_route": output.next_route,
    })
}

fn validate_request(request: &EvidenceWorkbenchRequest) -> Result<(), EvidenceWorkbenchError> {
    if request.objective.trim().is_empty()
        || request.researcher.trim().is_empty()
        || request.current_epoch == 0
        || request.maximum_age_epochs == 0
        || request.maximum_results == 0
        || request.maximum_results > MAX_RESULTS
        || request.records.len() > MAX_RECORDS
        || request.minimum_quality_milli > 1_000
        || request.minimum_reproducibility_milli > 1_000
        || request.claim_terms.len() + request.scope_terms.len() > MAX_TERMS
    {
        return Err(EvidenceWorkbenchError::InvalidRequest(
            "researcher/objective identity, positive epoch/age/results, bounded records/terms, and score bounds are required".into(),
        ));
    }
    let claim_terms = normalized_terms(&request.claim_terms);
    let scope_terms = normalized_terms(&request.scope_terms);
    if claim_terms.is_empty()
        && scope_terms.is_empty()
        && request.required_modalities.is_empty()
        && request.required_model_systems.is_empty()
        && request.required_source_kinds.is_empty()
        && request.allowed_states.is_empty()
    {
        return Err(EvidenceWorkbenchError::InvalidRequest(
            "at least one term or structured evidence filter is required".into(),
        ));
    }
    let mut record_ids = BTreeSet::new();
    for record in &request.records {
        if record.evidence_id.trim().is_empty()
            || record.claim.trim().is_empty()
            || record.scope.trim().is_empty()
            || record.release_epoch > request.current_epoch
            || record.relevance_milli > 1_000
            || record.quality_milli > 1_000
            || record.reproducibility_milli > 1_000
            || record.source_artifact.validate().is_err()
            || !record_ids.insert(record.evidence_id.clone())
        {
            return Err(EvidenceWorkbenchError::InvalidRecord(
                "records must be uniquely identified, preclinical/local, epoch-bounded, and score-bounded".into(),
            ));
        }
    }
    Ok(())
}

fn state_pressure(state: EvidenceState) -> u16 {
    match state {
        EvidenceState::Contradicted => 1_000,
        EvidenceState::Unknown | EvidenceState::Stale | EvidenceState::Unmeasured => 850,
        EvidenceState::Negative => 700,
        EvidenceState::Supported => 250,
    }
}

fn state_allowed(request: &EvidenceWorkbenchRequest, state: EvidenceState) -> Option<String> {
    if !request.allowed_states.is_empty() && !request.allowed_states.contains(&state) {
        return Some("state-filter".into());
    }
    match state {
        EvidenceState::Negative if !request.include_negative => Some("negative-excluded".into()),
        EvidenceState::Contradicted if !request.include_contradicted => {
            Some("contradiction-excluded".into())
        }
        EvidenceState::Unknown | EvidenceState::Stale | EvidenceState::Unmeasured
            if !request.include_uncertain =>
        {
            Some("uncertainty-excluded".into())
        }
        _ => None,
    }
}

fn label<T: Serialize>(value: T) -> String {
    serde_json::to_string(&value)
        .unwrap_or_default()
        .trim_matches('"')
        .to_string()
}

/// Query and rank a local evidence set for a researcher without generating a conclusion.
pub fn query_glioma_evidence_workbench(
    request: &EvidenceWorkbenchRequest,
) -> Result<EvidenceWorkbenchPlan, EvidenceWorkbenchError> {
    validate_request(request)?;
    let claim_terms = normalized_terms(&request.claim_terms);
    let scope_terms = normalized_terms(&request.scope_terms);
    let mut record_order = request
        .records
        .iter()
        .map(|record| record.evidence_id.clone())
        .collect::<Vec<_>>();
    record_order.sort();
    let max_epoch = request.current_epoch;
    let mut selected = Vec::<EvidenceWorkbenchHit>::new();
    let mut omissions = Vec::<EvidenceWorkbenchOmission>::new();
    let mut modality_counts = BTreeMap::new();
    let mut model_system_counts = BTreeMap::new();
    let mut source_kind_counts = BTreeMap::new();
    let mut state_counts = BTreeMap::new();
    let mut negative_order = Vec::new();
    let mut contradicted_order = Vec::new();
    let mut uncertain_order = Vec::new();
    let mut matched_claim_terms = BTreeSet::new();
    let mut matched_scope_terms = BTreeSet::new();
    let mut candidates = Vec::<(EvidenceRecord, EvidenceWorkbenchHit)>::new();

    for record in &request.records {
        *modality_counts
            .entry(label(record.modality))
            .or_insert(0_usize) += 1;
        if let Some(model_system) = record.model_system {
            *model_system_counts.entry(label(model_system)).or_insert(0) += 1;
        }
        *source_kind_counts
            .entry(label(record.source_kind))
            .or_insert(0_usize) += 1;
        *state_counts.entry(label(record.state)).or_insert(0_usize) += 1;
        match record.state {
            EvidenceState::Negative => negative_order.push(record.evidence_id.clone()),
            EvidenceState::Contradicted => contradicted_order.push(record.evidence_id.clone()),
            EvidenceState::Unknown | EvidenceState::Stale | EvidenceState::Unmeasured => {
                uncertain_order.push(record.evidence_id.clone())
            }
            EvidenceState::Supported => {}
        }
        if let Some(reason) = state_allowed(request, record.state) {
            omissions.push(EvidenceWorkbenchOmission {
                evidence_id: record.evidence_id.clone(),
                reason,
            });
            continue;
        }
        if !request.required_modalities.is_empty()
            && !request.required_modalities.contains(&record.modality)
        {
            omissions.push(EvidenceWorkbenchOmission {
                evidence_id: record.evidence_id.clone(),
                reason: "modality-filter".into(),
            });
            continue;
        }
        if !request.required_model_systems.is_empty()
            && !record
                .model_system
                .is_some_and(|model| request.required_model_systems.contains(&model))
        {
            omissions.push(EvidenceWorkbenchOmission {
                evidence_id: record.evidence_id.clone(),
                reason: "model-system-filter".into(),
            });
            continue;
        }
        if !request.required_source_kinds.is_empty()
            && !request.required_source_kinds.contains(&record.source_kind)
        {
            omissions.push(EvidenceWorkbenchOmission {
                evidence_id: record.evidence_id.clone(),
                reason: "source-kind-filter".into(),
            });
            continue;
        }
        if record.quality_milli < request.minimum_quality_milli {
            omissions.push(EvidenceWorkbenchOmission {
                evidence_id: record.evidence_id.clone(),
                reason: "quality-floor".into(),
            });
            continue;
        }
        if record.reproducibility_milli < request.minimum_reproducibility_milli {
            omissions.push(EvidenceWorkbenchOmission {
                evidence_id: record.evidence_id.clone(),
                reason: "reproducibility-floor".into(),
            });
            continue;
        }
        let age_epochs = max_epoch.saturating_sub(record.release_epoch);
        if age_epochs > request.maximum_age_epochs {
            omissions.push(EvidenceWorkbenchOmission {
                evidence_id: record.evidence_id.clone(),
                reason: "maximum-age-filter".into(),
            });
            continue;
        }
        let claim_tokens = token_set(&record.claim);
        let scope_tokens = token_set(&record.scope);
        let matched_claim = claim_terms
            .iter()
            .filter(|term| claim_tokens.contains(*term))
            .count();
        let matched_scope = scope_terms
            .iter()
            .filter(|term| scope_tokens.contains(*term))
            .count();
        if !claim_terms.is_empty() && matched_claim == 0 {
            omissions.push(EvidenceWorkbenchOmission {
                evidence_id: record.evidence_id.clone(),
                reason: "claim-term-mismatch".into(),
            });
            continue;
        }
        if !scope_terms.is_empty() && matched_scope == 0 {
            omissions.push(EvidenceWorkbenchOmission {
                evidence_id: record.evidence_id.clone(),
                reason: "scope-term-mismatch".into(),
            });
            continue;
        }
        matched_claim_terms.extend(
            claim_terms
                .iter()
                .filter(|term| claim_tokens.contains(*term))
                .cloned(),
        );
        matched_scope_terms.extend(
            scope_terms
                .iter()
                .filter(|term| scope_tokens.contains(*term))
                .cloned(),
        );
        let claim_score = if claim_terms.is_empty() {
            1_000
        } else {
            (matched_claim * 1_000 / claim_terms.len()) as u16
        };
        let scope_score = if scope_terms.is_empty() {
            1_000
        } else {
            (matched_scope * 1_000 / scope_terms.len()) as u16
        };
        let term_match =
            ((u32::from(claim_score) * 700 + u32::from(scope_score) * 300) / 1_000) as u16;
        let metadata_match = if request.required_modalities.is_empty()
            && request.required_model_systems.is_empty()
            && request.required_source_kinds.is_empty()
        {
            800
        } else {
            1_000
        };
        let recency = if max_epoch == 0 {
            0
        } else {
            (u64::from(record.release_epoch.min(max_epoch)) * 1_000 / u64::from(max_epoch)) as u16
        };
        let score = (u32::from(term_match) * 450
            + u32::from(metadata_match) * 150
            + u32::from(record.quality_milli) * 170
            + u32::from(record.reproducibility_milli) * 130
            + u32::from(recency) * 50
            + u32::from(state_pressure(record.state)) * 50)
            / 1_000;
        let explanation = format!(
            "term-match={term_match};metadata-match={metadata_match};quality={};reproducibility={};recency={recency};state-pressure={}",
            record.quality_milli,
            record.reproducibility_milli,
            state_pressure(record.state)
        );
        candidates.push((
            record.clone(),
            EvidenceWorkbenchHit {
                evidence_id: record.evidence_id.clone(),
                rank: 0,
                score_milli: score.min(1_000) as u16,
                term_match_milli: term_match,
                metadata_match_milli: metadata_match,
                quality_milli: record.quality_milli,
                reproducibility_milli: record.reproducibility_milli,
                age_epochs,
                state: record.state,
                explanation,
            },
        ));
    }

    candidates.sort_by(|left, right| {
        let left_hit = &left.1;
        let right_hit = &right.1;
        let ordering = match request.sort {
            EvidenceWorkbenchSort::Relevance => right_hit.score_milli.cmp(&left_hit.score_milli),
            EvidenceWorkbenchSort::Recency => left_hit.age_epochs.cmp(&right_hit.age_epochs),
            EvidenceWorkbenchSort::Quality => right_hit.quality_milli.cmp(&left_hit.quality_milli),
            EvidenceWorkbenchSort::StatePressure => {
                state_pressure(right_hit.state).cmp(&state_pressure(left_hit.state))
            }
        };
        ordering
            .then_with(|| right_hit.score_milli.cmp(&left_hit.score_milli))
            .then_with(|| left_hit.evidence_id.cmp(&right_hit.evidence_id))
    });
    let selected_candidates = candidates.into_iter().take(request.maximum_results);
    for (rank, (_, mut hit)) in selected_candidates.enumerate() {
        hit.rank = rank + 1;
        selected.push(hit);
    }
    let selected_ids = selected
        .iter()
        .map(|hit| hit.evidence_id.clone())
        .collect::<BTreeSet<_>>();
    for record in &request.records {
        if !selected_ids.contains(&record.evidence_id)
            && !omissions
                .iter()
                .any(|omission| omission.evidence_id == record.evidence_id)
        {
            omissions.push(EvidenceWorkbenchOmission {
                evidence_id: record.evidence_id.clone(),
                reason: "maximum-results-truncation".into(),
            });
        }
    }
    omissions.sort_by(|left, right| left.evidence_id.cmp(&right.evidence_id));
    omissions.dedup_by(|left, right| left.evidence_id == right.evidence_id);
    selected.sort_by_key(|hit| hit.rank);
    let selected_order = selected
        .iter()
        .map(|hit| hit.evidence_id.clone())
        .collect::<Vec<_>>();
    let omitted_order = omissions
        .iter()
        .map(|omission| omission.evidence_id.clone())
        .collect::<Vec<_>>();
    let mut unmatched_claim_terms = claim_terms
        .into_iter()
        .filter(|term| !matched_claim_terms.contains(term))
        .collect::<Vec<_>>();
    let mut unmatched_scope_terms = scope_terms
        .into_iter()
        .filter(|term| !matched_scope_terms.contains(term))
        .collect::<Vec<_>>();
    unmatched_claim_terms.sort();
    unmatched_scope_terms.sort();
    negative_order.sort();
    negative_order.dedup();
    contradicted_order.sort();
    contradicted_order.dedup();
    uncertain_order.sort();
    uncertain_order.dedup();
    let disposition = if selected.is_empty() {
        if omissions.is_empty() {
            EvidenceWorkbenchDisposition::Blocked
        } else {
            EvidenceWorkbenchDisposition::NoMatches
        }
    } else if omissions.is_empty() {
        EvidenceWorkbenchDisposition::Ready
    } else {
        EvidenceWorkbenchDisposition::Partial
    };
    let next_route = if selected.is_empty() {
        "glioma_evidence_acquisition_plan"
    } else {
        "glioma_evidence_prospective_triage"
    };
    let query_digest = ContentHash::of_value(&serde_json::json!({
        "objective": request.objective,
        "researcher": request.researcher,
        "claim_terms": normalized_terms(&request.claim_terms),
        "scope_terms": normalized_terms(&request.scope_terms),
        "required_modalities": request.required_modalities,
        "required_model_systems": request.required_model_systems,
        "required_source_kinds": request.required_source_kinds,
        "allowed_states": request.allowed_states,
        "minimum_quality_milli": request.minimum_quality_milli,
        "minimum_reproducibility_milli": request.minimum_reproducibility_milli,
        "current_epoch": request.current_epoch,
        "maximum_age_epochs": request.maximum_age_epochs,
        "maximum_results": request.maximum_results,
        "include_negative": request.include_negative,
        "include_contradicted": request.include_contradicted,
        "include_uncertain": request.include_uncertain,
        "sort": request.sort,
        "record_order": record_order,
    }))
    .map_err(|error| EvidenceWorkbenchError::Digest(error.to_string()))?;
    let mut output = EvidenceWorkbenchPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        researcher: request.researcher.clone(),
        query_digest,
        record_order,
        selected_order,
        omitted_order,
        hits: selected,
        omissions,
        modality_counts,
        model_system_counts,
        source_kind_counts,
        state_counts,
        unmatched_claim_terms,
        unmatched_scope_terms,
        negative_order,
        contradicted_order,
        uncertain_order,
        disposition,
        next_route: next_route.into(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-researcher-workbench"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| EvidenceWorkbenchError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

impl EvidenceWorkbenchPlan {
    pub fn validate(&self) -> Result<(), EvidenceWorkbenchError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.researcher.trim().is_empty()
            || self.query_digest.as_str().len() != 64
            || !canonical(&self.record_order)
            || !unique_nonempty(&self.record_order)
            || !unique_nonempty(&self.selected_order)
            || !canonical(&self.omitted_order)
            || !unique_nonempty(&self.omitted_order)
            || self.hits.len() != self.selected_order.len()
            || self.hits.iter().enumerate().any(|(index, hit)| {
                hit.rank != index + 1
                    || hit.evidence_id.trim().is_empty()
                    || hit.score_milli > 1_000
                    || hit.term_match_milli > 1_000
                    || hit.metadata_match_milli > 1_000
                    || hit.quality_milli > 1_000
                    || hit.reproducibility_milli > 1_000
                    || hit.explanation.trim().is_empty()
            })
            || self
                .hits
                .iter()
                .map(|hit| hit.evidence_id.clone())
                .collect::<Vec<_>>()
                != self.selected_order
            || !canonical(&self.modality_counts.keys().cloned().collect::<Vec<_>>())
            || !canonical(&self.model_system_counts.keys().cloned().collect::<Vec<_>>())
            || !canonical(&self.source_kind_counts.keys().cloned().collect::<Vec<_>>())
            || !canonical(&self.state_counts.keys().cloned().collect::<Vec<_>>())
            || !canonical(&self.unmatched_claim_terms)
            || !canonical(&self.unmatched_scope_terms)
            || !canonical(&self.negative_order)
            || !canonical(&self.contradicted_order)
            || !canonical(&self.uncertain_order)
            || self.next_route.trim().is_empty()
        {
            return Err(EvidenceWorkbenchError::InvalidOutput(
                "identity, ordering, result ranks, score bounds, facets, or digest shape is invalid".into(),
            ));
        }
        let record_ids = self.record_order.iter().cloned().collect::<BTreeSet<_>>();
        let selected_ids = self.selected_order.iter().cloned().collect::<BTreeSet<_>>();
        let omitted_ids = self.omitted_order.iter().cloned().collect::<BTreeSet<_>>();
        let partition = selected_ids
            .union(&omitted_ids)
            .cloned()
            .collect::<BTreeSet<_>>();
        if partition != record_ids
            || selected_ids.len() + omitted_ids.len() != partition.len()
            || self
                .omissions
                .iter()
                .map(|omission| omission.evidence_id.clone())
                .collect::<BTreeSet<_>>()
                != omitted_ids
            || self.omissions.len() != omitted_ids.len()
            || self
                .omissions
                .iter()
                .map(|omission| omission.evidence_id.clone())
                .collect::<Vec<_>>()
                != self.omitted_order
            || self.omissions.iter().any(|omission| {
                omission.evidence_id.trim().is_empty() || omission.reason.trim().is_empty()
            })
            || self.modality_counts.values().any(|count| *count == 0)
            || self.model_system_counts.values().any(|count| *count == 0)
            || self.source_kind_counts.values().any(|count| *count == 0)
            || self.state_counts.values().any(|count| *count == 0)
        {
            return Err(EvidenceWorkbenchError::InvalidOutput(
                "record/result/omission partition or facet counts do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| EvidenceWorkbenchError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(EvidenceWorkbenchError::Digest(
                "researcher evidence workbench digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(id: &str) -> crate::glioma_engine::LocalArtifactRef {
        crate::glioma_engine::LocalArtifactRef {
            artifact_id: id.into(),
            content_hash: ContentHash::of_bytes(id.as_bytes()),
            content_type: "application/vnd.aurora.glioma-evidence+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn record(
        id: &str,
        claim: &str,
        state: EvidenceState,
        quality_milli: u16,
        release_epoch: u32,
    ) -> EvidenceRecord {
        EvidenceRecord {
            evidence_id: id.into(),
            source_artifact: artifact(&format!("artifact-{id}")),
            source_kind: EvidenceSourceKind::Literature,
            claim: claim.into(),
            scope: "glioma invasion organoid".into(),
            modality: GliomaModality::Imaging,
            model_system: Some(GliomaModelSystem::Organoid),
            state,
            relevance_milli: 900,
            quality_milli,
            reproducibility_milli: 850,
            release_epoch,
        }
    }

    fn request() -> EvidenceWorkbenchRequest {
        EvidenceWorkbenchRequest {
            objective: "inspect glioma invasion evidence".into(),
            researcher: "researcher-a".into(),
            claim_terms: vec!["invasion".into()],
            scope_terms: vec!["organoid".into()],
            required_modalities: [GliomaModality::Imaging].into_iter().collect(),
            required_model_systems: [GliomaModelSystem::Organoid].into_iter().collect(),
            required_source_kinds: BTreeSet::new(),
            allowed_states: BTreeSet::new(),
            minimum_quality_milli: 500,
            minimum_reproducibility_milli: 500,
            current_epoch: 5,
            maximum_age_epochs: 10,
            maximum_results: 3,
            include_negative: true,
            include_contradicted: true,
            include_uncertain: true,
            sort: EvidenceWorkbenchSort::Relevance,
            records: vec![
                record(
                    "evidence-a",
                    "EGFR drives invasion",
                    EvidenceState::Supported,
                    900,
                    5,
                ),
                record(
                    "evidence-b",
                    "EGFR fails invasion",
                    EvidenceState::Contradicted,
                    700,
                    4,
                ),
                record(
                    "evidence-c",
                    "unrelated metabolism",
                    EvidenceState::Negative,
                    900,
                    5,
                ),
            ],
        }
    }

    #[test]
    fn ranks_local_matches_and_retains_contradiction() {
        let plan = query_glioma_evidence_workbench(&request()).unwrap();
        assert_eq!(plan.selected_order, vec!["evidence-a", "evidence-b"]);
        assert!(plan.contradicted_order.contains(&"evidence-b".to_string()));
        assert_eq!(plan.disposition, EvidenceWorkbenchDisposition::Partial);
        assert!(plan.hits[0].explanation.contains("term-match"));
        plan.validate().unwrap();
    }

    #[test]
    fn filters_are_explicit_and_negative_evidence_is_not_silent() {
        let mut request = request();
        request.include_contradicted = false;
        request.include_negative = false;
        request.minimum_quality_milli = 800;
        let plan = query_glioma_evidence_workbench(&request).unwrap();
        assert!(plan.selected_order.contains(&"evidence-a".to_string()));
        assert!(plan
            .omissions
            .iter()
            .any(|omission| omission.evidence_id == "evidence-b"
                && omission.reason == "contradiction-excluded"));
        assert!(plan
            .omissions
            .iter()
            .any(|omission| omission.evidence_id == "evidence-c"
                && omission.reason == "negative-excluded"));
        plan.validate().unwrap();
    }

    #[test]
    fn identical_queries_replay_to_identical_digest() {
        let first = query_glioma_evidence_workbench(&request()).unwrap();
        let second = query_glioma_evidence_workbench(&request()).unwrap();
        assert_eq!(first.digest, second.digest);
        assert_eq!(first.query_digest, second.query_digest);
    }

    #[test]
    fn empty_local_surface_is_an_explicit_block_not_a_false_success() {
        let mut request = request();
        request.records.clear();
        let plan = query_glioma_evidence_workbench(&request).unwrap();
        assert_eq!(plan.disposition, EvidenceWorkbenchDisposition::Blocked);
        assert!(plan.selected_order.is_empty());
        assert!(plan.omitted_order.is_empty());
        assert_eq!(plan.next_route, "glioma_evidence_acquisition_plan");
        plan.validate().unwrap();
    }
}

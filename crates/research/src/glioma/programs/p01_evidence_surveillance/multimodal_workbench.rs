//! Multimodal, multi-study researcher workbench for preclinical glioma evidence.
//!
//! This product surface turns a local evidence collection into comparable researcher panels. It
//! keeps study identity, claim/scope grouping, modality and model coverage, disagreement, and
//! negative results explicit while choosing a bounded deterministic representative set for a
//! scientist to inspect. It does not pool raw measurements, infer causality, retrieve sources,
//! or make a clinical decision. Source payloads remain behind local content-addressed artifacts.

use crate::glioma::evidence::{EvidenceRecord, EvidenceSourceKind, EvidenceState};
use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P01-F18";
pub const OUTPUT_SCHEMA: &str = "GliomaMultimodalResearchWorkbench1@1";
pub const MAX_RECORDS: usize = 16_384;
pub const MAX_PANELS: usize = 1_024;
pub const MAX_TERMS: usize = 128;
pub const MAX_STUDIES: usize = 4_096;
pub const MODALITY_VARIANT_COUNT: usize = 15;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultimodalWorkbenchSort {
    Relevance,
    Coverage,
    Disagreement,
    Freshness,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultimodalWorkbenchDisposition {
    Ready,
    Partial,
    NoPanels,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalWorkbenchRecord {
    pub study_id: String,
    pub sample_lineage: String,
    pub observation_id: String,
    pub claim_key: String,
    pub scope_key: String,
    pub evidence: EvidenceRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalWorkbenchRequest {
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
    pub minimum_studies: usize,
    pub minimum_modalities: usize,
    pub maximum_panels: usize,
    pub maximum_records_per_panel: usize,
    pub include_negative: bool,
    pub include_contradicted: bool,
    pub include_uncertain: bool,
    pub sort: MultimodalWorkbenchSort,
    pub records: Vec<MultimodalWorkbenchRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalWorkbenchStudySummary {
    pub study_id: String,
    pub evidence_order: Vec<String>,
    pub modality_order: Vec<GliomaModality>,
    pub model_system_order: Vec<GliomaModelSystem>,
    pub state_counts: BTreeMap<String, usize>,
    pub mean_quality_milli: u16,
    pub mean_reproducibility_milli: u16,
    pub latest_epoch: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalWorkbenchPanel {
    pub panel_id: String,
    pub rank: usize,
    pub claim_key: String,
    pub scope_key: String,
    pub evidence_order: Vec<String>,
    pub study_order: Vec<String>,
    pub modality_order: Vec<GliomaModality>,
    pub model_system_order: Vec<GliomaModelSystem>,
    pub state_counts: BTreeMap<String, usize>,
    pub study_coverage_milli: u16,
    pub modality_coverage_milli: u16,
    pub model_coverage_milli: u16,
    pub disagreement_milli: u16,
    pub quality_milli: u16,
    pub reproducibility_milli: u16,
    pub freshness_milli: u16,
    pub score_milli: u16,
    pub explanation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalWorkbenchOmission {
    pub evidence_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalWorkbenchPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub researcher: String,
    pub query_digest: ContentHash,
    pub record_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub omitted_order: Vec<String>,
    pub panel_order: Vec<String>,
    pub panels: Vec<MultimodalWorkbenchPanel>,
    pub omissions: Vec<MultimodalWorkbenchOmission>,
    pub study_order: Vec<String>,
    pub study_summaries: Vec<MultimodalWorkbenchStudySummary>,
    pub modality_order: Vec<GliomaModality>,
    pub model_system_order: Vec<GliomaModelSystem>,
    pub coverage_matrix: BTreeMap<String, usize>,
    pub state_counts: BTreeMap<String, usize>,
    pub unmatched_claim_terms: Vec<String>,
    pub unmatched_scope_terms: Vec<String>,
    pub negative_order: Vec<String>,
    pub contradicted_order: Vec<String>,
    pub uncertain_order: Vec<String>,
    pub disposition: MultimodalWorkbenchDisposition,
    pub next_route: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MultimodalWorkbenchError {
    #[error("multimodal researcher workbench request is invalid: {0}")]
    InvalidRequest(String),
    #[error("multimodal researcher workbench record is invalid: {0}")]
    InvalidRecord(String),
    #[error("multimodal researcher workbench output is invalid: {0}")]
    InvalidOutput(String),
    #[error("multimodal researcher workbench digest failed: {0}")]
    Digest(String),
}

#[derive(Debug, Clone)]
struct Candidate {
    record: MultimodalWorkbenchRecord,
    score_milli: u16,
    term_match_milli: u16,
    metadata_match_milli: u16,
    age_epochs: u32,
}

#[derive(Debug, Clone)]
struct Group {
    claim_key: String,
    scope_key: String,
    candidates: Vec<Candidate>,
}

fn add_representative(
    candidate: &Candidate,
    request: &MultimodalWorkbenchRequest,
    selected: &mut Vec<Candidate>,
    selected_ids: &mut BTreeSet<String>,
    covered_studies: &mut BTreeSet<String>,
    covered_modalities: &mut BTreeSet<GliomaModality>,
    covered_models: &mut BTreeSet<GliomaModelSystem>,
) {
    if selected.len() < request.maximum_records_per_panel
        && selected_ids.insert(candidate.record.evidence.evidence_id.clone())
    {
        covered_studies.insert(candidate.record.study_id.clone());
        covered_modalities.insert(candidate.record.evidence.modality);
        if let Some(model) = candidate.record.evidence.model_system {
            covered_models.insert(model);
        }
        selected.push(candidate.clone());
    }
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique_nonempty(values: &[String]) -> bool {
    values.iter().all(|value| !value.trim().is_empty())
        && values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

fn normalized_terms(values: &[String]) -> Vec<String> {
    values
        .iter()
        .flat_map(|value| {
            value
                .split(|character: char| !character.is_alphanumeric())
                .filter(|term| !term.is_empty())
                .map(|term| term.to_lowercase())
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn token_set(value: &str) -> BTreeSet<String> {
    value
        .split(|character: char| !character.is_alphanumeric())
        .filter(|term| !term.is_empty())
        .map(|term| term.to_lowercase())
        .collect()
}

fn label<T: Serialize>(value: T) -> String {
    serde_json::to_string(&value)
        .unwrap_or_default()
        .trim_matches('"')
        .to_string()
}

fn state_pressure(state: EvidenceState) -> u16 {
    match state {
        EvidenceState::Contradicted => 1_000,
        EvidenceState::Unknown | EvidenceState::Stale | EvidenceState::Unmeasured => 850,
        EvidenceState::Negative => 700,
        EvidenceState::Supported => 250,
    }
}

fn is_uncertain(state: EvidenceState) -> bool {
    matches!(
        state,
        EvidenceState::Unknown | EvidenceState::Stale | EvidenceState::Unmeasured
    )
}

fn state_allowed(request: &MultimodalWorkbenchRequest, state: EvidenceState) -> Option<String> {
    if !request.allowed_states.is_empty() && !request.allowed_states.contains(&state) {
        return Some("state-filter".into());
    }
    match state {
        EvidenceState::Negative if !request.include_negative => Some("negative-excluded".into()),
        EvidenceState::Contradicted if !request.include_contradicted => {
            Some("contradiction-excluded".into())
        }
        state if is_uncertain(state) && !request.include_uncertain => {
            Some("uncertainty-excluded".into())
        }
        _ => None,
    }
}

fn panel_id(claim_key: &str, scope_key: &str) -> String {
    format!("{claim_key}::{scope_key}")
}

fn digest_input(output: &MultimodalWorkbenchPlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "researcher": output.researcher,
        "query_digest": output.query_digest,
        "record_order": output.record_order,
        "selected_order": output.selected_order,
        "omitted_order": output.omitted_order,
        "panel_order": output.panel_order,
        "panels": output.panels,
        "omissions": output.omissions,
        "study_order": output.study_order,
        "study_summaries": output.study_summaries,
        "modality_order": output.modality_order,
        "model_system_order": output.model_system_order,
        "coverage_matrix": output.coverage_matrix,
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

fn validate_request(request: &MultimodalWorkbenchRequest) -> Result<(), MultimodalWorkbenchError> {
    if request.objective.trim().is_empty()
        || request.researcher.trim().is_empty()
        || request.current_epoch == 0
        || request.maximum_age_epochs == 0
        || request.minimum_studies == 0
        || request.minimum_studies > MAX_STUDIES
        || request.minimum_modalities == 0
        || request.minimum_modalities > MODALITY_VARIANT_COUNT
        || request.maximum_panels == 0
        || request.maximum_panels > MAX_PANELS
        || request.maximum_records_per_panel == 0
        || request.maximum_records_per_panel > MAX_RECORDS
        || request.maximum_records_per_panel
            < request
                .minimum_studies
                .max(request.minimum_modalities)
                .max(request.required_modalities.len())
                .max(request.required_model_systems.len())
        || request.records.len() > MAX_RECORDS
        || request.minimum_quality_milli > 1_000
        || request.minimum_reproducibility_milli > 1_000
        || request.claim_terms.len() + request.scope_terms.len() > MAX_TERMS
    {
        return Err(MultimodalWorkbenchError::InvalidRequest(
            "identity, positive coverage/result bounds, bounded records/terms, and score bounds are required".into(),
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
        return Err(MultimodalWorkbenchError::InvalidRequest(
            "at least one term or structured evidence filter is required".into(),
        ));
    }
    let mut evidence_ids = BTreeSet::new();
    let mut observation_ids = BTreeSet::new();
    let mut study_ids = BTreeSet::new();
    for record in &request.records {
        if record.study_id.trim().is_empty()
            || record.sample_lineage.trim().is_empty()
            || record.observation_id.trim().is_empty()
            || record.claim_key.trim().is_empty()
            || record.scope_key.trim().is_empty()
            || record.study_id.len() > 256
            || record.evidence.evidence_id.trim().is_empty()
            || record.evidence.claim.trim().is_empty()
            || record.evidence.scope.trim().is_empty()
            || record.evidence.release_epoch > request.current_epoch
            || record.evidence.relevance_milli > 1_000
            || record.evidence.quality_milli > 1_000
            || record.evidence.reproducibility_milli > 1_000
            || record.evidence.source_artifact.validate().is_err()
            || !evidence_ids.insert(record.evidence.evidence_id.clone())
            || !observation_ids.insert(record.observation_id.clone())
            || !study_ids.insert(record.study_id.clone())
        {
            return Err(MultimodalWorkbenchError::InvalidRecord(
                "records must have unique local evidence/observation identities, explicit study and claim grouping, preclinical artifacts, epoch bounds, and bounded scores".into(),
            ));
        }
    }
    if study_ids.len() > MAX_STUDIES {
        return Err(MultimodalWorkbenchError::InvalidRecord(
            "the local evidence surface exceeds the bounded study cardinality".into(),
        ));
    }
    Ok(())
}

fn candidate_score(
    record: &MultimodalWorkbenchRecord,
    request: &MultimodalWorkbenchRequest,
    term_match_milli: u16,
    metadata_match_milli: u16,
) -> u16 {
    let recency = if request.current_epoch == 0 {
        0
    } else {
        (u64::from(record.evidence.release_epoch.min(request.current_epoch)) * 1_000
            / u64::from(request.current_epoch)) as u16
    };
    let state_bonus = state_pressure(record.evidence.state);
    ((u32::from(term_match_milli) * 350
        + u32::from(metadata_match_milli) * 150
        + u32::from(record.evidence.quality_milli) * 170
        + u32::from(record.evidence.reproducibility_milli) * 130
        + u32::from(recency) * 100
        + u32::from(state_bonus) * 100)
        / 1_000)
        .min(1_000) as u16
}

fn select_representatives(
    candidates: &[Candidate],
    request: &MultimodalWorkbenchRequest,
) -> Vec<Candidate> {
    let mut ranked = candidates.to_vec();
    ranked.sort_by(|left, right| {
        right.score_milli.cmp(&left.score_milli).then_with(|| {
            left.record
                .evidence
                .evidence_id
                .cmp(&right.record.evidence.evidence_id)
        })
    });
    let mut selected = Vec::new();
    let mut selected_ids = BTreeSet::new();
    let mut covered_studies = BTreeSet::new();
    let mut covered_modalities = BTreeSet::new();
    let mut covered_models = BTreeSet::new();

    let modality_target = request
        .minimum_modalities
        .max(request.required_modalities.len());
    loop {
        let missing_required_modality = request
            .required_modalities
            .iter()
            .any(|modality| !covered_modalities.contains(modality));
        let missing_required_model = request
            .required_model_systems
            .iter()
            .any(|model| !covered_models.contains(model));
        if selected.len() >= request.maximum_records_per_panel
            || (covered_studies.len() >= request.minimum_studies
                && covered_modalities.len() >= modality_target
                && !missing_required_model
                && !missing_required_modality)
        {
            break;
        }
        let best = ranked
            .iter()
            .filter(|candidate| !selected_ids.contains(&candidate.record.evidence.evidence_id))
            .map(|candidate| {
                let mut gain = 0_u8;
                if !covered_studies.contains(&candidate.record.study_id) {
                    gain += 1;
                }
                if !covered_modalities.contains(&candidate.record.evidence.modality)
                    && (covered_modalities.len() < modality_target
                        || request
                            .required_modalities
                            .contains(&candidate.record.evidence.modality))
                {
                    gain += 1;
                }
                if candidate.record.evidence.model_system.is_some_and(|model| {
                    request.required_model_systems.contains(&model)
                        && !covered_models.contains(&model)
                }) {
                    gain += 1;
                }
                (gain, candidate)
            })
            .max_by(|(left_gain, left), (right_gain, right)| {
                left_gain.cmp(right_gain).then_with(|| {
                    right.score_milli.cmp(&left.score_milli).then_with(|| {
                        right
                            .record
                            .evidence
                            .evidence_id
                            .cmp(&left.record.evidence.evidence_id)
                    })
                })
            })
            .filter(|(gain, _)| *gain > 0)
            .map(|(_, candidate)| candidate);
        let Some(candidate) = best else {
            break;
        };
        add_representative(
            candidate,
            request,
            &mut selected,
            &mut selected_ids,
            &mut covered_studies,
            &mut covered_modalities,
            &mut covered_models,
        );
    }
    for candidate in &ranked {
        add_representative(
            candidate,
            request,
            &mut selected,
            &mut selected_ids,
            &mut covered_studies,
            &mut covered_modalities,
            &mut covered_models,
        );
    }
    selected
}

fn mean(values: impl Iterator<Item = u16>) -> u16 {
    let values = values.collect::<Vec<_>>();
    if values.is_empty() {
        0
    } else {
        (values.iter().map(|value| u32::from(*value)).sum::<u32>() / values.len() as u32) as u16
    }
}

fn representatives_cover_targets(
    candidates: &[Candidate],
    request: &MultimodalWorkbenchRequest,
) -> bool {
    let studies = candidates
        .iter()
        .map(|candidate| candidate.record.study_id.clone())
        .collect::<BTreeSet<_>>();
    let modalities = candidates
        .iter()
        .map(|candidate| candidate.record.evidence.modality)
        .collect::<BTreeSet<_>>();
    let models = candidates
        .iter()
        .filter_map(|candidate| candidate.record.evidence.model_system)
        .collect::<BTreeSet<_>>();
    studies.len() >= request.minimum_studies
        && modalities.len() >= request.minimum_modalities
        && request
            .required_modalities
            .iter()
            .all(|modality| modalities.contains(modality))
        && request
            .required_model_systems
            .iter()
            .all(|model| models.contains(model))
}

fn build_panel(
    claim_key: String,
    scope_key: String,
    candidates: Vec<Candidate>,
    request: &MultimodalWorkbenchRequest,
    rank: usize,
) -> MultimodalWorkbenchPanel {
    let representatives = select_representatives(&candidates, request);
    let study_order = representatives
        .iter()
        .map(|candidate| candidate.record.study_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let modality_order = representatives
        .iter()
        .map(|candidate| candidate.record.evidence.modality)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let model_system_order = representatives
        .iter()
        .filter_map(|candidate| candidate.record.evidence.model_system)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut state_counts = BTreeMap::new();
    let mut support = 0_usize;
    let mut negative = 0_usize;
    let mut contradiction = 0_usize;
    for candidate in &representatives {
        *state_counts
            .entry(label(candidate.record.evidence.state))
            .or_insert(0) += 1;
        match candidate.record.evidence.state {
            EvidenceState::Supported => support += 1,
            EvidenceState::Negative => negative += 1,
            EvidenceState::Contradicted => contradiction += 1,
            _ => {}
        }
    }
    let total = representatives.len().max(1) as u32;
    let disagreement_milli =
        (((negative + contradiction) as u32 * 1_000) / total).min(1_000) as u16;
    let study_coverage_milli =
        ((study_order.len() as u32 * 1_000) / request.minimum_studies as u32).min(1_000) as u16;
    let modality_target = request
        .minimum_modalities
        .max(request.required_modalities.len());
    let modality_coverage_milli =
        ((modality_order.len() as u32 * 1_000) / modality_target as u32).min(1_000) as u16;
    let model_target = request.required_model_systems.len().max(1);
    let model_coverage_milli = if request.required_model_systems.is_empty() {
        if model_system_order.is_empty() {
            0
        } else {
            1_000
        }
    } else {
        ((model_system_order.len() as u32 * 1_000) / model_target as u32).min(1_000) as u16
    };
    let quality_milli = mean(
        representatives
            .iter()
            .map(|candidate| candidate.record.evidence.quality_milli),
    );
    let reproducibility_milli = mean(
        representatives
            .iter()
            .map(|candidate| candidate.record.evidence.reproducibility_milli),
    );
    let freshness_milli = mean(representatives.iter().map(|candidate| {
        if request.maximum_age_epochs == 0 {
            0
        } else {
            (request
                .maximum_age_epochs
                .saturating_sub(candidate.age_epochs)
                .min(request.maximum_age_epochs)
                * 1_000
                / request.maximum_age_epochs) as u16
        }
    }));
    let relevance_milli = mean(
        representatives
            .iter()
            .map(|candidate| candidate.score_milli),
    );
    let term_match_milli = mean(
        representatives
            .iter()
            .map(|candidate| candidate.term_match_milli),
    );
    let metadata_match_milli = mean(
        representatives
            .iter()
            .map(|candidate| candidate.metadata_match_milli),
    );
    let score_milli = ((u32::from(relevance_milli) * 350
        + u32::from(study_coverage_milli) * 200
        + u32::from(modality_coverage_milli) * 150
        + u32::from(quality_milli) * 120
        + u32::from(reproducibility_milli) * 100
        + u32::from(freshness_milli) * 50
        + u32::from(disagreement_milli) * 30)
        / 1_000)
        .min(1_000) as u16;
    let explanation = format!(
        "studies={};modalities={};models={};term-match={term_match_milli};metadata-match={metadata_match_milli};support={support};negative={negative};contradicted={contradiction};study-coverage={study_coverage_milli};modality-coverage={modality_coverage_milli};disagreement={disagreement_milli}",
        study_order.len(),
        modality_order.len(),
        model_system_order.len()
    );
    MultimodalWorkbenchPanel {
        panel_id: panel_id(&claim_key, &scope_key),
        rank,
        claim_key,
        scope_key,
        evidence_order: representatives
            .iter()
            .map(|candidate| candidate.record.evidence.evidence_id.clone())
            .collect(),
        study_order,
        modality_order,
        model_system_order,
        state_counts,
        study_coverage_milli,
        modality_coverage_milli,
        model_coverage_milli,
        disagreement_milli,
        quality_milli,
        reproducibility_milli,
        freshness_milli,
        score_milli,
        explanation,
    }
}

/// Build a deterministic multimodal, multi-study evidence panel for researcher inspection.
pub fn query_glioma_multimodal_researcher_workbench(
    request: &MultimodalWorkbenchRequest,
) -> Result<MultimodalWorkbenchPlan, MultimodalWorkbenchError> {
    validate_request(request)?;
    let claim_terms = normalized_terms(&request.claim_terms);
    let scope_terms = normalized_terms(&request.scope_terms);
    let mut record_order = request
        .records
        .iter()
        .map(|record| record.evidence.evidence_id.clone())
        .collect::<Vec<_>>();
    record_order.sort();
    let mut study_order = request
        .records
        .iter()
        .map(|record| record.study_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    study_order.sort();
    let mut modality_order = BTreeSet::new();
    let mut model_system_order = BTreeSet::new();
    let mut coverage_matrix = BTreeMap::new();
    let mut state_counts = BTreeMap::new();
    let mut study_records = BTreeMap::<String, Vec<&MultimodalWorkbenchRecord>>::new();
    let mut negative_order = Vec::new();
    let mut contradicted_order = Vec::new();
    let mut uncertain_order = Vec::new();
    for record in &request.records {
        modality_order.insert(record.evidence.modality);
        if let Some(model) = record.evidence.model_system {
            model_system_order.insert(model);
        }
        *coverage_matrix
            .entry(format!(
                "{}::{}",
                record.study_id,
                label(record.evidence.modality)
            ))
            .or_insert(0) += 1;
        *state_counts
            .entry(label(record.evidence.state))
            .or_insert(0) += 1;
        study_records
            .entry(record.study_id.clone())
            .or_default()
            .push(record);
        match record.evidence.state {
            EvidenceState::Negative => negative_order.push(record.evidence.evidence_id.clone()),
            EvidenceState::Contradicted => {
                contradicted_order.push(record.evidence.evidence_id.clone())
            }
            state if is_uncertain(state) => {
                uncertain_order.push(record.evidence.evidence_id.clone())
            }
            EvidenceState::Supported => {}
            EvidenceState::Unknown | EvidenceState::Stale | EvidenceState::Unmeasured => {}
        }
    }
    let study_summaries = study_records
        .into_iter()
        .map(|(study_id, records)| {
            let evidence_order = records
                .iter()
                .map(|record| record.evidence.evidence_id.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            let modality_order = records
                .iter()
                .map(|record| record.evidence.modality)
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            let model_system_order = records
                .iter()
                .filter_map(|record| record.evidence.model_system)
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            let mut study_state_counts = BTreeMap::new();
            for record in &records {
                *study_state_counts
                    .entry(label(record.evidence.state))
                    .or_insert(0) += 1;
            }
            MultimodalWorkbenchStudySummary {
                study_id,
                evidence_order,
                modality_order,
                model_system_order,
                state_counts: study_state_counts,
                mean_quality_milli: mean(
                    records.iter().map(|record| record.evidence.quality_milli),
                ),
                mean_reproducibility_milli: mean(
                    records
                        .iter()
                        .map(|record| record.evidence.reproducibility_milli),
                ),
                latest_epoch: records
                    .iter()
                    .map(|record| record.evidence.release_epoch)
                    .max()
                    .unwrap_or_default(),
            }
        })
        .collect::<Vec<_>>();

    let mut candidates = Vec::new();
    let mut omissions = Vec::new();
    let mut matched_claim_terms = BTreeSet::new();
    let mut matched_scope_terms = BTreeSet::new();
    for record in &request.records {
        if let Some(reason) = state_allowed(request, record.evidence.state) {
            omissions.push(MultimodalWorkbenchOmission {
                evidence_id: record.evidence.evidence_id.clone(),
                reason,
            });
            continue;
        }
        if !request.required_modalities.is_empty()
            && !request
                .required_modalities
                .contains(&record.evidence.modality)
        {
            omissions.push(MultimodalWorkbenchOmission {
                evidence_id: record.evidence.evidence_id.clone(),
                reason: "modality-filter".into(),
            });
            continue;
        }
        if !request.required_model_systems.is_empty()
            && !record
                .evidence
                .model_system
                .is_some_and(|model| request.required_model_systems.contains(&model))
        {
            omissions.push(MultimodalWorkbenchOmission {
                evidence_id: record.evidence.evidence_id.clone(),
                reason: "model-system-filter".into(),
            });
            continue;
        }
        if !request.required_source_kinds.is_empty()
            && !request
                .required_source_kinds
                .contains(&record.evidence.source_kind)
        {
            omissions.push(MultimodalWorkbenchOmission {
                evidence_id: record.evidence.evidence_id.clone(),
                reason: "source-kind-filter".into(),
            });
            continue;
        }
        if record.evidence.quality_milli < request.minimum_quality_milli {
            omissions.push(MultimodalWorkbenchOmission {
                evidence_id: record.evidence.evidence_id.clone(),
                reason: "quality-floor".into(),
            });
            continue;
        }
        if record.evidence.reproducibility_milli < request.minimum_reproducibility_milli {
            omissions.push(MultimodalWorkbenchOmission {
                evidence_id: record.evidence.evidence_id.clone(),
                reason: "reproducibility-floor".into(),
            });
            continue;
        }
        let age_epochs = request
            .current_epoch
            .saturating_sub(record.evidence.release_epoch);
        if age_epochs > request.maximum_age_epochs {
            omissions.push(MultimodalWorkbenchOmission {
                evidence_id: record.evidence.evidence_id.clone(),
                reason: "maximum-age-filter".into(),
            });
            continue;
        }
        let claim_tokens = token_set(&record.evidence.claim);
        let scope_tokens = token_set(&record.evidence.scope);
        let matched_claim = claim_terms
            .iter()
            .filter(|term| claim_tokens.contains(*term))
            .count();
        let matched_scope = scope_terms
            .iter()
            .filter(|term| scope_tokens.contains(*term))
            .count();
        if !claim_terms.is_empty() && matched_claim == 0 {
            omissions.push(MultimodalWorkbenchOmission {
                evidence_id: record.evidence.evidence_id.clone(),
                reason: "claim-term-mismatch".into(),
            });
            continue;
        }
        if !scope_terms.is_empty() && matched_scope == 0 {
            omissions.push(MultimodalWorkbenchOmission {
                evidence_id: record.evidence.evidence_id.clone(),
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
        let term_match_milli =
            ((u32::from(claim_score) * 700 + u32::from(scope_score) * 300) / 1_000) as u16;
        let metadata_match_milli = if request.required_modalities.is_empty()
            && request.required_model_systems.is_empty()
            && request.required_source_kinds.is_empty()
        {
            800
        } else {
            1_000
        };
        let score_milli = candidate_score(record, request, term_match_milli, metadata_match_milli);
        candidates.push(Candidate {
            record: record.clone(),
            score_milli,
            term_match_milli,
            metadata_match_milli,
            age_epochs,
        });
    }

    let mut grouped = BTreeMap::<(String, String), Vec<Candidate>>::new();
    for candidate in candidates {
        grouped
            .entry((
                candidate.record.claim_key.clone(),
                candidate.record.scope_key.clone(),
            ))
            .or_default()
            .push(candidate);
    }
    let mut groups = Vec::new();
    for ((claim_key, scope_key), candidates) in grouped {
        let studies = candidates
            .iter()
            .map(|candidate| candidate.record.study_id.clone())
            .collect::<BTreeSet<_>>();
        let modalities = candidates
            .iter()
            .map(|candidate| candidate.record.evidence.modality)
            .collect::<BTreeSet<_>>();
        let has_required_modalities = request
            .required_modalities
            .iter()
            .all(|modality| modalities.contains(modality));
        let has_required_models = request.required_model_systems.iter().all(|model| {
            candidates
                .iter()
                .any(|candidate| candidate.record.evidence.model_system == Some(*model))
        });
        if studies.len() < request.minimum_studies
            || modalities.len() < request.minimum_modalities
            || !has_required_modalities
            || !has_required_models
        {
            for candidate in candidates {
                omissions.push(MultimodalWorkbenchOmission {
                    evidence_id: candidate.record.evidence.evidence_id,
                    reason: "insufficient-cross-study-coverage".into(),
                });
            }
            continue;
        }
        groups.push(Group {
            claim_key,
            scope_key,
            candidates,
        });
    }

    let mut panels = groups
        .into_iter()
        .filter_map(|group| {
            let representatives = select_representatives(&group.candidates, request);
            if !representatives_cover_targets(&representatives, request) {
                for candidate in group.candidates {
                    omissions.push(MultimodalWorkbenchOmission {
                        evidence_id: candidate.record.evidence.evidence_id,
                        reason: "representative-capacity".into(),
                    });
                }
                None
            } else {
                Some(build_panel(
                    group.claim_key,
                    group.scope_key,
                    group.candidates,
                    request,
                    0,
                ))
            }
        })
        .collect::<Vec<_>>();
    panels.sort_by(|left, right| {
        let ordering = match request.sort {
            MultimodalWorkbenchSort::Relevance => right.score_milli.cmp(&left.score_milli),
            MultimodalWorkbenchSort::Coverage => right
                .study_coverage_milli
                .saturating_add(right.modality_coverage_milli)
                .cmp(
                    &left
                        .study_coverage_milli
                        .saturating_add(left.modality_coverage_milli),
                ),
            MultimodalWorkbenchSort::Disagreement => {
                right.disagreement_milli.cmp(&left.disagreement_milli)
            }
            MultimodalWorkbenchSort::Freshness => right.freshness_milli.cmp(&left.freshness_milli),
        };
        ordering
            .then_with(|| right.score_milli.cmp(&left.score_milli))
            .then_with(|| left.panel_id.cmp(&right.panel_id))
    });
    let mut selected_panel_ids = BTreeSet::new();
    panels.truncate(request.maximum_panels);
    for (rank, panel) in panels.iter_mut().enumerate() {
        panel.rank = rank + 1;
        selected_panel_ids.insert(panel.panel_id.clone());
    }
    let selected_order = panels
        .iter()
        .flat_map(|panel| panel.evidence_order.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let selected_ids = selected_order.iter().cloned().collect::<BTreeSet<_>>();
    for record in &request.records {
        if selected_ids.contains(&record.evidence.evidence_id)
            || omissions
                .iter()
                .any(|omission| omission.evidence_id == record.evidence.evidence_id)
        {
            continue;
        }
        omissions.push(MultimodalWorkbenchOmission {
            evidence_id: record.evidence.evidence_id.clone(),
            reason: if selected_panel_ids.is_empty() {
                "no-panel-selected".into()
            } else if selected_panel_ids.contains(&panel_id(&record.claim_key, &record.scope_key)) {
                "maximum-records-per-panel".into()
            } else {
                "maximum-panels-truncation".into()
            },
        });
    }
    omissions.sort_by(|left, right| left.evidence_id.cmp(&right.evidence_id));
    omissions.dedup_by(|left, right| left.evidence_id == right.evidence_id);
    let omitted_order = omissions
        .iter()
        .map(|omission| omission.evidence_id.clone())
        .collect::<Vec<_>>();
    let panel_order = panels
        .iter()
        .map(|panel| panel.panel_id.clone())
        .collect::<Vec<_>>();
    let unmatched_claim_terms = claim_terms
        .into_iter()
        .filter(|term| !matched_claim_terms.contains(term))
        .collect::<Vec<_>>();
    let unmatched_scope_terms = scope_terms
        .into_iter()
        .filter(|term| !matched_scope_terms.contains(term))
        .collect::<Vec<_>>();
    let mut negative_order = negative_order;
    let mut contradicted_order = contradicted_order;
    let mut uncertain_order = uncertain_order;
    negative_order.sort();
    negative_order.dedup();
    contradicted_order.sort();
    contradicted_order.dedup();
    uncertain_order.sort();
    uncertain_order.dedup();
    let disposition = if request.records.is_empty() {
        MultimodalWorkbenchDisposition::Blocked
    } else if panels.is_empty() {
        MultimodalWorkbenchDisposition::NoPanels
    } else if omissions.is_empty() {
        MultimodalWorkbenchDisposition::Ready
    } else {
        MultimodalWorkbenchDisposition::Partial
    };
    let next_route = if panels.is_empty() {
        "glioma_multimodal_evidence_gap_router"
    } else {
        "glioma_multimodal_knowledge_protocol_gateway"
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
        "minimum_studies": request.minimum_studies,
        "minimum_modalities": request.minimum_modalities,
        "maximum_panels": request.maximum_panels,
        "maximum_records_per_panel": request.maximum_records_per_panel,
        "include_negative": request.include_negative,
        "include_contradicted": request.include_contradicted,
        "include_uncertain": request.include_uncertain,
        "sort": request.sort,
        "record_order": record_order,
    }))
    .map_err(|error| MultimodalWorkbenchError::Digest(error.to_string()))?;
    let mut output = MultimodalWorkbenchPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        researcher: request.researcher.clone(),
        query_digest,
        record_order,
        selected_order,
        omitted_order,
        panel_order,
        panels,
        omissions,
        study_order,
        study_summaries,
        modality_order: modality_order.into_iter().collect(),
        model_system_order: model_system_order.into_iter().collect(),
        coverage_matrix,
        state_counts,
        unmatched_claim_terms,
        unmatched_scope_terms,
        negative_order,
        contradicted_order,
        uncertain_order,
        disposition,
        next_route: next_route.into(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-multimodal-workbench"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| MultimodalWorkbenchError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

impl MultimodalWorkbenchPlan {
    pub fn validate(&self) -> Result<(), MultimodalWorkbenchError> {
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
            || !unique_nonempty(&self.panel_order)
            || self.panel_order
                != self
                    .panels
                    .iter()
                    .map(|panel| panel.panel_id.clone())
                    .collect::<Vec<_>>()
            || self.panels.iter().enumerate().any(|(index, panel)| {
                panel.rank != index + 1
                    || panel.panel_id.trim().is_empty()
                    || panel.claim_key.trim().is_empty()
                    || panel.scope_key.trim().is_empty()
                    || panel.panel_id != panel_id(&panel.claim_key, &panel.scope_key)
                    || panel.evidence_order.is_empty()
                    || !unique_nonempty(&panel.evidence_order)
                    || panel.explanation.trim().is_empty()
                    || panel.score_milli > 1_000
                    || panel.study_coverage_milli > 1_000
                    || panel.modality_coverage_milli > 1_000
                    || panel.model_coverage_milli > 1_000
                    || panel.disagreement_milli > 1_000
                    || panel.quality_milli > 1_000
                    || panel.reproducibility_milli > 1_000
                    || panel.freshness_milli > 1_000
                    || !canonical(&panel.study_order)
                    || !canonical(&panel.modality_order)
                    || !canonical(&panel.model_system_order)
                    || !canonical(&panel.state_counts.keys().cloned().collect::<Vec<_>>())
            })
            || !canonical(&self.study_order)
            || !unique_nonempty(&self.study_order)
            || self
                .study_summaries
                .iter()
                .map(|summary| summary.study_id.clone())
                .collect::<Vec<_>>()
                != self.study_order
            || self.study_summaries.iter().any(|summary| {
                summary.study_id.trim().is_empty()
                    || !canonical(&summary.evidence_order)
                    || !unique_nonempty(&summary.evidence_order)
                    || !canonical(&summary.modality_order)
                    || !canonical(&summary.model_system_order)
                    || !canonical(&summary.state_counts.keys().cloned().collect::<Vec<_>>())
            })
            || !canonical(&self.modality_order)
            || !canonical(&self.model_system_order)
            || !canonical(&self.coverage_matrix.keys().cloned().collect::<Vec<_>>())
            || !canonical(&self.state_counts.keys().cloned().collect::<Vec<_>>())
            || !canonical(&self.unmatched_claim_terms)
            || !canonical(&self.unmatched_scope_terms)
            || !canonical(&self.negative_order)
            || !canonical(&self.contradicted_order)
            || !canonical(&self.uncertain_order)
            || self.next_route.trim().is_empty()
        {
            return Err(MultimodalWorkbenchError::InvalidOutput(
                "identity, panel ranks, result partition, coverage ordering, or score bounds are invalid".into(),
            ));
        }
        let record_ids = self.record_order.iter().cloned().collect::<BTreeSet<_>>();
        let selected_ids = self.selected_order.iter().cloned().collect::<BTreeSet<_>>();
        let omitted_ids = self.omitted_order.iter().cloned().collect::<BTreeSet<_>>();
        let panel_evidence_ids = self
            .panels
            .iter()
            .flat_map(|panel| panel.evidence_order.iter().cloned())
            .collect::<Vec<_>>();
        let panel_evidence_set = panel_evidence_ids.iter().cloned().collect::<BTreeSet<_>>();
        let partition = selected_ids
            .union(&omitted_ids)
            .cloned()
            .collect::<BTreeSet<_>>();
        if partition != record_ids
            || selected_ids.len() + omitted_ids.len() != partition.len()
            || panel_evidence_set != selected_ids
            || panel_evidence_ids.len() != panel_evidence_set.len()
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
            || self.coverage_matrix.values().any(|count| *count == 0)
            || self.state_counts.values().any(|count| *count == 0)
        {
            return Err(MultimodalWorkbenchError::InvalidOutput(
                "record/panel/omission partition or coverage counts do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| MultimodalWorkbenchError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(MultimodalWorkbenchError::Digest(
                "multimodal researcher workbench digest is not content-addressed".into(),
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
        evidence_id: &str,
        observation_id: &str,
        study_id: &str,
        modality: GliomaModality,
        state: EvidenceState,
        claim: &str,
        release_epoch: u32,
    ) -> MultimodalWorkbenchRecord {
        MultimodalWorkbenchRecord {
            study_id: study_id.into(),
            sample_lineage: format!("lineage-{study_id}"),
            observation_id: observation_id.into(),
            claim_key: "egfr-invasion".into(),
            scope_key: "organoid".into(),
            evidence: EvidenceRecord {
                evidence_id: evidence_id.into(),
                source_artifact: artifact(&format!("artifact-{evidence_id}")),
                source_kind: EvidenceSourceKind::Assay,
                claim: claim.into(),
                scope: "glioma invasion organoid".into(),
                modality,
                model_system: Some(GliomaModelSystem::Organoid),
                state,
                relevance_milli: 900,
                quality_milli: 850,
                reproducibility_milli: 800,
                release_epoch,
            },
        }
    }

    fn request() -> MultimodalWorkbenchRequest {
        MultimodalWorkbenchRequest {
            objective: "compare EGFR invasion evidence across glioma organoid studies".into(),
            researcher: "researcher-a".into(),
            claim_terms: vec!["invasion".into()],
            scope_terms: vec!["organoid".into()],
            required_modalities: BTreeSet::new(),
            required_model_systems: [GliomaModelSystem::Organoid].into_iter().collect(),
            required_source_kinds: BTreeSet::new(),
            allowed_states: BTreeSet::new(),
            minimum_quality_milli: 500,
            minimum_reproducibility_milli: 500,
            current_epoch: 5,
            maximum_age_epochs: 10,
            minimum_studies: 2,
            minimum_modalities: 2,
            maximum_panels: 4,
            maximum_records_per_panel: 6,
            include_negative: true,
            include_contradicted: true,
            include_uncertain: true,
            sort: MultimodalWorkbenchSort::Relevance,
            records: vec![
                record(
                    "evidence-a",
                    "observation-a",
                    "study-a",
                    GliomaModality::Imaging,
                    EvidenceState::Supported,
                    "EGFR drives invasion",
                    5,
                ),
                record(
                    "evidence-b",
                    "observation-b",
                    "study-b",
                    GliomaModality::Transcriptomics,
                    EvidenceState::Contradicted,
                    "EGFR fails invasion",
                    4,
                ),
                record(
                    "evidence-c",
                    "observation-c",
                    "study-c",
                    GliomaModality::Proteomics,
                    EvidenceState::Negative,
                    "EGFR does not alter invasion",
                    5,
                ),
            ],
        }
    }

    #[test]
    fn builds_cross_study_panel_with_modality_coverage_and_disagreement() {
        let plan = query_glioma_multimodal_researcher_workbench(&request()).unwrap();
        assert_eq!(plan.panels.len(), 1);
        assert_eq!(
            plan.panels[0].study_order,
            vec!["study-a", "study-b", "study-c"]
        );
        assert_eq!(plan.panels[0].modality_order.len(), 3);
        assert!(plan.panels[0].disagreement_milli > 0);
        assert!(plan.contradicted_order.contains(&"evidence-b".to_string()));
        assert_eq!(plan.disposition, MultimodalWorkbenchDisposition::Ready);
        plan.validate().unwrap();
    }

    #[test]
    fn incomplete_panel_coverage_is_an_explicit_gap() {
        let mut request = request();
        request.minimum_studies = 3;
        request.records.pop();
        let plan = query_glioma_multimodal_researcher_workbench(&request).unwrap();
        assert!(plan.panels.is_empty());
        assert_eq!(plan.disposition, MultimodalWorkbenchDisposition::NoPanels);
        assert!(plan
            .omissions
            .iter()
            .all(|omission| omission.reason == "insufficient-cross-study-coverage"));
        assert_eq!(plan.next_route, "glioma_multimodal_evidence_gap_router");
        plan.validate().unwrap();
    }

    #[test]
    fn filters_do_not_hide_negative_or_contradictory_records() {
        let mut request = request();
        request.include_negative = false;
        request.include_contradicted = false;
        let plan = query_glioma_multimodal_researcher_workbench(&request).unwrap();
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
    fn identical_multimodal_queries_replay_to_identical_digest() {
        let first = query_glioma_multimodal_researcher_workbench(&request()).unwrap();
        let second = query_glioma_multimodal_researcher_workbench(&request()).unwrap();
        assert_eq!(first.digest, second.digest);
        assert_eq!(first.query_digest, second.query_digest);
    }

    #[test]
    fn empty_local_surface_is_blocked_and_routed_to_gap_analysis() {
        let mut request = request();
        request.records.clear();
        let plan = query_glioma_multimodal_researcher_workbench(&request).unwrap();
        assert_eq!(plan.disposition, MultimodalWorkbenchDisposition::Blocked);
        assert_eq!(plan.next_route, "glioma_multimodal_evidence_gap_router");
        plan.validate().unwrap();
    }
}

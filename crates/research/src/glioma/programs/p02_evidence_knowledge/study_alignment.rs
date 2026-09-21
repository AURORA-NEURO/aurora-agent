//! Explicit multi-study typed-knowledge alignment for preclinical glioma research.
//!
//! This feature is a data primitive for the autonomous engine: it creates a deterministic table
//! of caller-declared equivalent claims across studies, computes support/confidence pooling and
//! leave-one-study-out influence, and makes modality/model coverage and disagreement visible.
//! Equivalence is never guessed from text. Each row is bound by a study-owned claim id and an
//! explicit canonical key, so semantic loss and negative findings remain auditable before any
//! downstream mechanism, experiment, or computation workflow is selected.

use super::knowledge_graph::{KnowledgeClaim, KnowledgeClaimDisposition, TypedKnowledge};
use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P02-F06";
pub const OUTPUT_SCHEMA: &str = "GliomaMultiStudyKnowledge1@1";
pub const MAX_STUDIES: usize = 4_096;
pub const MAX_BINDINGS_PER_STUDY: usize = 65_536;
pub const MAX_CLAIMS: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiStudyKnowledgeRequest {
    pub objective: String,
    pub snapshots: Vec<StudyKnowledgeSnapshot>,
    pub required_modalities: BTreeSet<GliomaModality>,
    pub required_model_systems: BTreeSet<GliomaModelSystem>,
    pub min_studies: usize,
    pub min_equivalence_milli: u16,
    pub min_agreement_milli: u16,
    pub max_claims: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StudyKnowledgeSnapshot {
    pub study_id: String,
    pub site_id: String,
    pub release_epoch: u32,
    pub knowledge: TypedKnowledge,
    pub bindings: Vec<StudyClaimBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StudyClaimBinding {
    pub canonical_claim_key: String,
    pub claim_id: String,
    pub equivalence_milli: u16,
    pub semantic_loss_milli: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultiStudyClaimDisposition {
    Aligned,
    Heterogeneous,
    Negative,
    Unresolved,
    Insufficient,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StudyClaimObservation {
    pub study_id: String,
    pub site_id: String,
    pub claim_id: String,
    pub equivalence_milli: u16,
    pub semantic_loss_milli: u16,
    pub support_milli: u16,
    pub contradiction_milli: u16,
    pub confidence_milli: u16,
    pub modality_order: Vec<GliomaModality>,
    pub model_system_order: Vec<GliomaModelSystem>,
    pub disposition: KnowledgeClaimDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiStudyKnowledgeRow {
    pub canonical_claim_key: String,
    pub statement_order: Vec<String>,
    pub scope_order: Vec<String>,
    pub observations: Vec<StudyClaimObservation>,
    pub study_count: usize,
    pub supported_count: usize,
    pub negative_count: usize,
    pub contested_count: usize,
    pub unresolved_count: usize,
    pub pooled_support_milli: u16,
    pub pooled_confidence_milli: u16,
    pub agreement_milli: u16,
    pub leave_one_out_influence_milli: u16,
    pub modality_order: Vec<GliomaModality>,
    pub model_system_order: Vec<GliomaModelSystem>,
    pub missing_modality_order: Vec<GliomaModality>,
    pub missing_model_system_order: Vec<GliomaModelSystem>,
    pub omission_order: Vec<String>,
    pub disposition: MultiStudyClaimDisposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultiStudyKnowledgeDisposition {
    Qualified,
    Partial,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiStudyKnowledge {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub study_order: Vec<String>,
    pub claim_order: Vec<String>,
    pub aligned_claim_order: Vec<String>,
    pub heterogeneous_claim_order: Vec<String>,
    pub negative_claim_order: Vec<String>,
    pub unresolved_claim_order: Vec<String>,
    pub insufficient_claim_order: Vec<String>,
    pub rows: Vec<MultiStudyKnowledgeRow>,
    pub unbound_claim_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: MultiStudyKnowledgeDisposition,
    pub next_step: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MultiStudyKnowledgeError {
    #[error("multi-study knowledge request is invalid: {0}")]
    InvalidRequest(String),
    #[error("multi-study knowledge binding is invalid: {0}")]
    InvalidBinding(String),
    #[error("multi-study knowledge output is invalid: {0}")]
    InvalidOutput(String),
    #[error("multi-study knowledge digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &MultiStudyKnowledge) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "study_order": output.study_order,
        "claim_order": output.claim_order,
        "aligned_claim_order": output.aligned_claim_order,
        "heterogeneous_claim_order": output.heterogeneous_claim_order,
        "negative_claim_order": output.negative_claim_order,
        "unresolved_claim_order": output.unresolved_claim_order,
        "insufficient_claim_order": output.insufficient_claim_order,
        "rows": output.rows,
        "unbound_claim_order": output.unbound_claim_order,
        "omission_order": output.omission_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
        "next_step": output.next_step,
    })
}

impl MultiStudyKnowledge {
    pub fn validate(&self) -> Result<(), MultiStudyKnowledgeError> {
        let row_keys = self
            .rows
            .iter()
            .map(|row| row.canonical_claim_key.clone())
            .collect::<Vec<_>>();
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || !canonical(&self.study_order)
            || !canonical(&self.claim_order)
            || !canonical(&self.aligned_claim_order)
            || !canonical(&self.heterogeneous_claim_order)
            || !canonical(&self.negative_claim_order)
            || !canonical(&self.unresolved_claim_order)
            || !canonical(&self.insufficient_claim_order)
            || !canonical(&row_keys)
            || row_keys != self.claim_order
            || !canonical(&self.unbound_claim_order)
            || !canonical(&self.omission_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.rows.iter().any(|row| {
                row.canonical_claim_key.trim().is_empty()
                    || row.study_count != row.observations.len()
                    || !canonical(&row.statement_order)
                    || !canonical(&row.scope_order)
                    || !canonical(&row.modality_order)
                    || !canonical(&row.model_system_order)
                    || !canonical(&row.missing_modality_order)
                    || !canonical(&row.missing_model_system_order)
                    || !canonical(&row.omission_order)
                    || row.observations.iter().any(|observation| {
                        observation.study_id.trim().is_empty()
                            || observation.site_id.trim().is_empty()
                            || observation.claim_id.trim().is_empty()
                            || observation.equivalence_milli > 1_000
                            || observation.semantic_loss_milli > 1_000
                            || observation.support_milli > 1_000
                            || observation.contradiction_milli > 1_000
                            || observation.confidence_milli > 1_000
                            || !canonical(&observation.modality_order)
                            || !canonical(&observation.model_system_order)
                    })
            })
        {
            return Err(MultiStudyKnowledgeError::InvalidOutput(
                "identity, ordering, row cardinality, or score bounds are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| MultiStudyKnowledgeError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(MultiStudyKnowledgeError::InvalidOutput(
                "multi-study knowledge digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn claim_by_id<'a>(knowledge: &'a TypedKnowledge, claim_id: &str) -> Option<&'a KnowledgeClaim> {
    knowledge
        .claims
        .iter()
        .find(|claim| claim.claim_id == claim_id)
}

fn validate_request(request: &MultiStudyKnowledgeRequest) -> Result<(), MultiStudyKnowledgeError> {
    if request.objective.trim().is_empty()
        || request.snapshots.is_empty()
        || request.snapshots.len() > MAX_STUDIES
        || request.min_studies == 0
        || request.min_studies > request.snapshots.len()
        || request.min_equivalence_milli > 1_000
        || request.min_agreement_milli > 1_000
        || request.max_claims == 0
        || request.max_claims > MAX_CLAIMS
    {
        return Err(MultiStudyKnowledgeError::InvalidRequest(
            "objective, study floor, study capacity, equivalence/agreement floors, or claim bound is invalid".into(),
        ));
    }
    let mut study_ids = BTreeSet::new();
    for snapshot in &request.snapshots {
        if snapshot.study_id.trim().is_empty()
            || snapshot.site_id.trim().is_empty()
            || !study_ids.insert(snapshot.study_id.clone())
            || snapshot.bindings.len() > MAX_BINDINGS_PER_STUDY
            || snapshot.knowledge.objective != request.objective
        {
            return Err(MultiStudyKnowledgeError::InvalidRequest(
                "study identity, uniqueness, binding capacity, or objective binding is invalid"
                    .into(),
            ));
        }
        snapshot
            .knowledge
            .validate()
            .map_err(|error| MultiStudyKnowledgeError::InvalidRequest(error.to_string()))?;
        if snapshot.knowledge.claims.len() > request.max_claims {
            return Err(MultiStudyKnowledgeError::InvalidRequest(
                "study knowledge exceeds the requested claim capacity".into(),
            ));
        }
        let binding_keys = snapshot
            .bindings
            .iter()
            .map(|binding| {
                (
                    binding.canonical_claim_key.clone(),
                    binding.claim_id.clone(),
                )
            })
            .collect::<Vec<_>>();
        if !canonical(&binding_keys) {
            return Err(MultiStudyKnowledgeError::InvalidBinding(
                "bindings must be sorted and unique by canonical key and claim id".into(),
            ));
        }
        for binding in &snapshot.bindings {
            if binding.canonical_claim_key.trim().is_empty()
                || binding.claim_id.trim().is_empty()
                || binding.equivalence_milli > 1_000
                || binding.semantic_loss_milli > 1_000
                || claim_by_id(&snapshot.knowledge, &binding.claim_id).is_none()
            {
                return Err(MultiStudyKnowledgeError::InvalidBinding(
                    "binding key, claim reference, equivalence, semantic-loss, or claim membership is invalid".into(),
                ));
            }
        }
    }
    Ok(())
}

fn weighted_mean(values: impl Iterator<Item = (u16, u16)>) -> u16 {
    let mut numerator = 0_u64;
    let mut denominator = 0_u64;
    for (value, weight) in values {
        numerator += u64::from(value) * u64::from(weight.max(1));
        denominator += u64::from(weight.max(1));
    }
    if denominator == 0 {
        0
    } else {
        (numerator / denominator).min(1_000) as u16
    }
}

fn absolute_deviation(values: &[u16], mean: u16) -> u16 {
    if values.is_empty() {
        return 1_000;
    }
    (values
        .iter()
        .map(|value| u32::from((*value).abs_diff(mean)))
        .sum::<u32>()
        / values.len() as u32) as u16
}

fn leave_one_out_influence(values: &[u16], weights: &[u16], pooled: u16) -> u16 {
    if values.len() <= 1 {
        return 0;
    }
    values
        .iter()
        .enumerate()
        .map(|(index, _)| {
            let remaining = values
                .iter()
                .enumerate()
                .filter_map(|(other, value)| (other != index).then_some((*value, weights[other])));
            weighted_mean(remaining).abs_diff(pooled)
        })
        .max()
        .unwrap_or(0)
}

fn row_disposition(
    observations: &[StudyClaimObservation],
    min_studies: usize,
    min_equivalence_milli: u16,
    min_agreement_milli: u16,
    agreement_milli: u16,
    missing_modalities: &[GliomaModality],
    missing_models: &[GliomaModelSystem],
) -> MultiStudyClaimDisposition {
    let supported = observations
        .iter()
        .filter(|observation| {
            matches!(
                observation.disposition,
                KnowledgeClaimDisposition::Supported
            )
        })
        .count();
    let negative = observations
        .iter()
        .filter(|observation| {
            matches!(observation.disposition, KnowledgeClaimDisposition::Negative)
        })
        .count();
    let unresolved = observations
        .iter()
        .filter(|observation| {
            matches!(
                observation.disposition,
                KnowledgeClaimDisposition::Unresolved
            )
        })
        .count();
    let contested = observations
        .iter()
        .filter(|observation| {
            matches!(
                observation.disposition,
                KnowledgeClaimDisposition::Contested
            )
        })
        .count();
    if observations.len() < min_studies
        || observations
            .iter()
            .any(|observation| observation.equivalence_milli < min_equivalence_milli)
        || !missing_modalities.is_empty()
        || !missing_models.is_empty()
    {
        return MultiStudyClaimDisposition::Insufficient;
    }
    if unresolved > 0 {
        MultiStudyClaimDisposition::Unresolved
    } else if negative == observations.len() {
        MultiStudyClaimDisposition::Negative
    } else if contested > 0
        || negative > 0
        || supported != observations.len()
        || agreement_milli < min_agreement_milli
    {
        MultiStudyClaimDisposition::Heterogeneous
    } else {
        MultiStudyClaimDisposition::Aligned
    }
}

pub fn compile_multi_study_knowledge(
    request: &MultiStudyKnowledgeRequest,
) -> Result<MultiStudyKnowledge, MultiStudyKnowledgeError> {
    validate_request(request)?;
    let mut grouped =
        BTreeMap::<String, Vec<(String, String, u32, StudyClaimBinding, KnowledgeClaim)>>::new();
    let mut study_order = request
        .snapshots
        .iter()
        .map(|snapshot| snapshot.study_id.clone())
        .collect::<Vec<_>>();
    study_order.sort();
    let mut unbound_claim_order = Vec::new();
    for snapshot in &request.snapshots {
        let bound = snapshot
            .bindings
            .iter()
            .map(|binding| binding.claim_id.clone())
            .collect::<BTreeSet<_>>();
        for claim in &snapshot.knowledge.claims {
            if !bound.contains(&claim.claim_id) {
                unbound_claim_order.push(format!("{}::{}", snapshot.study_id, claim.claim_id));
            }
        }
        for binding in &snapshot.bindings {
            let claim = claim_by_id(&snapshot.knowledge, &binding.claim_id)
                .expect("validated binding claim exists")
                .clone();
            grouped
                .entry(binding.canonical_claim_key.clone())
                .or_default()
                .push((
                    snapshot.study_id.clone(),
                    snapshot.site_id.clone(),
                    snapshot.release_epoch,
                    binding.clone(),
                    claim,
                ));
        }
    }
    unbound_claim_order.sort();
    let mut claim_order = Vec::new();
    let mut aligned_claim_order = Vec::new();
    let mut heterogeneous_claim_order = Vec::new();
    let mut negative_claim_order = Vec::new();
    let mut unresolved_claim_order = Vec::new();
    let mut insufficient_claim_order = Vec::new();
    let mut omission_order = Vec::new();
    let mut negative_evidence = Vec::new();
    let mut uncertainty = Vec::new();
    if !unbound_claim_order.is_empty() {
        uncertainty
            .push("one or more typed claims are unbound from a canonical multi-study key".into());
        omission_order.extend(
            unbound_claim_order
                .iter()
                .map(|claim| format!("{claim}: canonical multi-study binding is missing")),
        );
    }
    let mut rows = Vec::new();
    for (canonical_claim_key, mut entries) in grouped {
        entries.sort_by(|left, right| left.0.cmp(&right.0));
        let observations = entries
            .iter()
            .map(
                |(study_id, site_id, _, binding, claim)| StudyClaimObservation {
                    study_id: study_id.clone(),
                    site_id: site_id.clone(),
                    claim_id: claim.claim_id.clone(),
                    equivalence_milli: binding.equivalence_milli,
                    semantic_loss_milli: binding.semantic_loss_milli,
                    support_milli: claim.support_milli,
                    contradiction_milli: claim.contradiction_milli,
                    confidence_milli: claim.confidence_milli,
                    modality_order: claim.modality_order.clone(),
                    model_system_order: claim.model_system_order.clone(),
                    disposition: claim.disposition,
                },
            )
            .collect::<Vec<_>>();
        let statement_order = entries
            .iter()
            .map(|(_, _, _, _, claim)| claim.statement.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let scope_order = entries
            .iter()
            .map(|(_, _, _, _, claim)| claim.scope.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let modality_order = entries
            .iter()
            .flat_map(|(_, _, _, _, claim)| claim.modality_order.iter().copied())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let model_system_order = entries
            .iter()
            .flat_map(|(_, _, _, _, claim)| claim.model_system_order.iter().copied())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let observed_modalities = modality_order.iter().copied().collect::<BTreeSet<_>>();
        let observed_models = model_system_order.iter().copied().collect::<BTreeSet<_>>();
        let missing_modality_order = request
            .required_modalities
            .difference(&observed_modalities)
            .copied()
            .collect::<Vec<_>>();
        let missing_model_system_order = request
            .required_model_systems
            .difference(&observed_models)
            .copied()
            .collect::<Vec<_>>();
        let weights = observations
            .iter()
            .map(|observation| observation.equivalence_milli)
            .collect::<Vec<_>>();
        let support_values = observations
            .iter()
            .map(|observation| observation.support_milli)
            .collect::<Vec<_>>();
        let pooled_support_milli = weighted_mean(
            observations
                .iter()
                .map(|observation| (observation.support_milli, observation.equivalence_milli)),
        );
        let pooled_confidence_milli = weighted_mean(
            observations
                .iter()
                .map(|observation| (observation.confidence_milli, observation.equivalence_milli)),
        );
        let agreement_milli =
            1_000_u16.saturating_sub(absolute_deviation(&support_values, pooled_support_milli));
        let leave_one_out_influence_milli =
            leave_one_out_influence(&support_values, &weights, pooled_support_milli);
        let mut omission_row = Vec::new();
        if observations.len() < request.min_studies {
            omission_row.push(format!("{canonical_claim_key}: study floor is unmet"));
        }
        if observations
            .iter()
            .any(|observation| observation.equivalence_milli < request.min_equivalence_milli)
        {
            omission_row.push(format!("{canonical_claim_key}: equivalence floor is unmet"));
        }
        if agreement_milli < request.min_agreement_milli {
            omission_row.push(format!("{canonical_claim_key}: agreement floor is unmet"));
        }
        omission_row.extend(
            missing_modality_order
                .iter()
                .map(|modality| format!("{canonical_claim_key}: missing modality {modality:?}")),
        );
        omission_row.extend(
            missing_model_system_order
                .iter()
                .map(|model| format!("{canonical_claim_key}: missing model {model:?}")),
        );
        omission_row.sort();
        omission_row.dedup();
        let disposition = row_disposition(
            &observations,
            request.min_studies,
            request.min_equivalence_milli,
            request.min_agreement_milli,
            agreement_milli,
            &missing_modality_order,
            &missing_model_system_order,
        );
        let supported_count = observations
            .iter()
            .filter(|observation| {
                matches!(
                    observation.disposition,
                    KnowledgeClaimDisposition::Supported
                )
            })
            .count();
        let negative_count = observations
            .iter()
            .filter(|observation| {
                matches!(observation.disposition, KnowledgeClaimDisposition::Negative)
            })
            .count();
        let contested_count = observations
            .iter()
            .filter(|observation| {
                matches!(
                    observation.disposition,
                    KnowledgeClaimDisposition::Contested
                )
            })
            .count();
        let unresolved_count = observations
            .iter()
            .filter(|observation| {
                matches!(
                    observation.disposition,
                    KnowledgeClaimDisposition::Unresolved
                )
            })
            .count();
        match disposition {
            MultiStudyClaimDisposition::Aligned => {
                aligned_claim_order.push(canonical_claim_key.clone())
            }
            MultiStudyClaimDisposition::Heterogeneous => {
                heterogeneous_claim_order.push(canonical_claim_key.clone())
            }
            MultiStudyClaimDisposition::Negative => {
                negative_claim_order.push(canonical_claim_key.clone())
            }
            MultiStudyClaimDisposition::Unresolved => {
                unresolved_claim_order.push(canonical_claim_key.clone())
            }
            MultiStudyClaimDisposition::Insufficient => {
                insufficient_claim_order.push(canonical_claim_key.clone())
            }
        }
        if negative_count > 0 {
            negative_evidence.push(format!(
                "{canonical_claim_key}: one or more studies report negative evidence"
            ));
        }
        if observations
            .iter()
            .any(|observation| observation.semantic_loss_milli > 0)
        {
            uncertainty.push(format!(
                "{canonical_claim_key}: semantic-loss declarations are non-zero"
            ));
        }
        if !omission_row.is_empty() {
            uncertainty.push(format!(
                "{canonical_claim_key}: multi-study closure is incomplete"
            ));
        }
        omission_order.extend(omission_row.iter().cloned());
        claim_order.push(canonical_claim_key.clone());
        rows.push(MultiStudyKnowledgeRow {
            canonical_claim_key,
            statement_order,
            scope_order,
            observations,
            study_count: entries.len(),
            supported_count,
            negative_count,
            contested_count,
            unresolved_count,
            pooled_support_milli,
            pooled_confidence_milli,
            agreement_milli,
            leave_one_out_influence_milli,
            modality_order,
            model_system_order,
            missing_modality_order,
            missing_model_system_order,
            omission_order: omission_row,
            disposition,
        });
    }
    omission_order.sort();
    omission_order.dedup();
    negative_evidence.sort();
    negative_evidence.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    let disposition = if rows.is_empty() {
        MultiStudyKnowledgeDisposition::Blocked
    } else if unbound_claim_order.is_empty()
        && !aligned_claim_order.is_empty()
        && heterogeneous_claim_order.is_empty()
        && negative_claim_order.is_empty()
        && unresolved_claim_order.is_empty()
        && insufficient_claim_order.is_empty()
    {
        MultiStudyKnowledgeDisposition::Qualified
    } else if !unresolved_claim_order.is_empty() {
        MultiStudyKnowledgeDisposition::Unresolved
    } else {
        MultiStudyKnowledgeDisposition::Partial
    };
    let next_step = match disposition {
        MultiStudyKnowledgeDisposition::Qualified => {
            "release aligned rows to contradiction-aware consistency and bounded mechanism planning"
        }
        MultiStudyKnowledgeDisposition::Partial => {
            "bind omitted claims and route heterogeneous, negative, and insufficient rows to adjudication or targeted acquisition"
        }
        MultiStudyKnowledgeDisposition::Unresolved => {
            "resolve unresolved study claims before autonomous downstream execution"
        }
        MultiStudyKnowledgeDisposition::Blocked => {
            "provide at least one explicitly bound multi-study claim row"
        }
    };
    let mut output = MultiStudyKnowledge {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        study_order,
        claim_order,
        aligned_claim_order,
        heterogeneous_claim_order,
        negative_claim_order,
        unresolved_claim_order,
        insufficient_claim_order,
        rows,
        unbound_claim_order,
        omission_order,
        negative_evidence,
        uncertainty,
        disposition,
        next_step: next_step.into(),
        digest: ContentHash::of_value(&serde_json::Value::Null)
            .map_err(|error| MultiStudyKnowledgeError::Digest(error.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| MultiStudyKnowledgeError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::evidence::{EvidenceRecord, EvidenceSourceKind, EvidenceState};
    use crate::glioma_engine::LocalArtifactRef;

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: id.into(),
            content_hash: ContentHash::of_value(&serde_json::json!({"id": id})).unwrap(),
            content_type: "study-evidence".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn record(id: &str, state: EvidenceState, study: &str) -> EvidenceRecord {
        EvidenceRecord {
            evidence_id: format!("{study}-{id}"),
            source_artifact: artifact(&format!("{study}-artifact-{id}")),
            source_kind: EvidenceSourceKind::Assay,
            claim: "egfr activation increases invasion".into(),
            scope: "organoid".into(),
            modality: GliomaModality::Transcriptomics,
            model_system: Some(GliomaModelSystem::Organoid),
            state,
            relevance_milli: 900,
            quality_milli: 900,
            reproducibility_milli: 900,
            release_epoch: 1,
        }
    }

    fn snapshot(study: &str, state: EvidenceState) -> StudyKnowledgeSnapshot {
        let knowledge = super::super::knowledge_graph::compile_typed_knowledge(
            &super::super::knowledge_graph::KnowledgeRequest {
                objective: "multi-study alignment".into(),
                required_modalities: BTreeSet::from([GliomaModality::Transcriptomics]),
                required_model_systems: BTreeSet::from([GliomaModelSystem::Organoid]),
                min_support_milli: 500,
                min_sources_per_claim: 1,
                max_claims: 8,
            },
            &[record("evidence", state, study)],
        )
        .unwrap();
        let claim_id = knowledge.claims[0].claim_id.clone();
        StudyKnowledgeSnapshot {
            study_id: study.into(),
            site_id: format!("site-{study}"),
            release_epoch: 1,
            knowledge,
            bindings: vec![StudyClaimBinding {
                canonical_claim_key: "egfr-invasion-organoid".into(),
                claim_id,
                equivalence_milli: 950,
                semantic_loss_milli: 0,
            }],
        }
    }

    fn request(snapshots: Vec<StudyKnowledgeSnapshot>) -> MultiStudyKnowledgeRequest {
        MultiStudyKnowledgeRequest {
            objective: "multi-study alignment".into(),
            snapshots,
            required_modalities: BTreeSet::from([GliomaModality::Transcriptomics]),
            required_model_systems: BTreeSet::from([GliomaModelSystem::Organoid]),
            min_studies: 2,
            min_equivalence_milli: 800,
            min_agreement_milli: 800,
            max_claims: 8,
        }
    }

    #[test]
    fn aligns_explicitly_bound_supported_studies() {
        let output = compile_multi_study_knowledge(&request(vec![
            snapshot("study-a", EvidenceState::Supported),
            snapshot("study-b", EvidenceState::Supported),
        ]))
        .expect("alignment");
        assert_eq!(
            output.disposition,
            MultiStudyKnowledgeDisposition::Qualified
        );
        assert_eq!(output.aligned_claim_order, vec!["egfr-invasion-organoid"]);
        assert_eq!(output.rows[0].leave_one_out_influence_milli, 0);
        output.validate().expect("digest validates");
    }

    #[test]
    fn preserves_negative_study_without_promoting_consensus() {
        let output = compile_multi_study_knowledge(&request(vec![
            snapshot("study-a", EvidenceState::Supported),
            snapshot("study-b", EvidenceState::Negative),
        ]))
        .expect("alignment");
        assert_eq!(output.disposition, MultiStudyKnowledgeDisposition::Partial);
        assert!(output
            .heterogeneous_claim_order
            .contains(&"egfr-invasion-organoid".into()));
        assert!(!output.negative_evidence.is_empty());
    }

    #[test]
    fn records_unbound_claims_as_omissions() {
        let mut one = snapshot("study-a", EvidenceState::Supported);
        one.bindings.clear();
        let output = compile_multi_study_knowledge(&request(vec![
            one,
            snapshot("study-b", EvidenceState::Supported),
        ]))
        .expect("alignment");
        assert!(output.rows.len() == 1);
        assert!(!output.unbound_claim_order.is_empty());
        assert!(!output.uncertainty.is_empty());
    }
}

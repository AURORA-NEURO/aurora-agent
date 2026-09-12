//! Evidence surveillance and qualification for preclinical glioma programs.
//!
//! This program does not fetch the internet.  A local provider supplies source metadata and
//! content-addressed artifact references; this module makes the selection and its uncertainty
//! reproducible.  Unknown, stale, contradicted, and negative records are never silently promoted
//! to positive support.

use super::super::glioma_engine::{GliomaModality, GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P01-F01";
pub const OUTPUT_SCHEMA: &str = "GliomaEvidenceQualification1@1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceSourceKind {
    Literature,
    Dataset,
    Assay,
    Model,
    Computation,
    Replication,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceState {
    Supported,
    Negative,
    Unknown,
    Contradicted,
    Stale,
    Unmeasured,
}

impl EvidenceState {
    pub const fn is_positive_selection(self) -> bool {
        matches!(self, Self::Supported | Self::Negative)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceRecord {
    pub evidence_id: String,
    pub source_artifact: LocalArtifactRef,
    pub source_kind: EvidenceSourceKind,
    pub claim: String,
    pub scope: String,
    pub modality: GliomaModality,
    pub model_system: Option<GliomaModelSystem>,
    pub state: EvidenceState,
    pub relevance_milli: u16,
    pub quality_milli: u16,
    pub reproducibility_milli: u16,
    pub release_epoch: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceRequest {
    pub objective: String,
    pub required_modalities: BTreeSet<GliomaModality>,
    pub required_model_systems: BTreeSet<GliomaModelSystem>,
    pub max_records: usize,
    pub min_quality_milli: u16,
    pub min_reproducibility_milli: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceDisposition {
    Qualified,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceQualification {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub selected_records: Vec<EvidenceRecord>,
    pub selected_order: Vec<String>,
    pub rejected_order: Vec<String>,
    pub negative_order: Vec<String>,
    pub contradictory_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub missing_modality_order: Vec<GliomaModality>,
    pub missing_model_order: Vec<GliomaModelSystem>,
    pub disposition: EvidenceDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EvidenceError {
    #[error("evidence request is invalid: {0}")]
    InvalidRequest(String),
    #[error("evidence record is invalid: {0}")]
    InvalidRecord(String),
    #[error("evidence qualification is invalid: {0}")]
    InvalidOutput(String),
    #[error("evidence digest failed: {0}")]
    Digest(String),
}

fn score(record: &EvidenceRecord, max_epoch: u32, request: &EvidenceRequest) -> u64 {
    let scope_match = if request.required_modalities.contains(&record.modality) {
        1_000
    } else if request.required_modalities.is_empty() {
        800
    } else {
        350
    };
    let model_match = match record.model_system {
        Some(model) if request.required_model_systems.contains(&model) => 1_000,
        Some(_) if request.required_model_systems.is_empty() => 800,
        Some(_) => 350,
        None if request.required_model_systems.is_empty() => 700,
        None => 250,
    };
    let recency = if max_epoch == 0 {
        500
    } else {
        ((record.release_epoch.min(max_epoch) as u64) * 1_000 / max_epoch as u64) as u16
    } as u64;
    35 * record.quality_milli as u64
        + 30 * record.relevance_milli as u64
        + 20 * record.reproducibility_milli as u64
        + 10 * scope_match
        + 3 * model_match
        + 2 * recency
}

fn digest_input(output: &EvidenceQualification) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "selected_records": output.selected_records,
        "selected_order": output.selected_order,
        "rejected_order": output.rejected_order,
        "negative_order": output.negative_order,
        "contradictory_order": output.contradictory_order,
        "unknown_order": output.unknown_order,
        "missing_modality_order": output.missing_modality_order,
        "missing_model_order": output.missing_model_order,
        "disposition": output.disposition,
    })
}

impl EvidenceQualification {
    pub fn validate(&self) -> Result<(), EvidenceError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.selected_order.len() != self.selected_records.len()
            || self.selected_order
                != self
                    .selected_records
                    .iter()
                    .map(|r| r.evidence_id.clone())
                    .collect::<Vec<_>>()
            || self.selected_order.windows(2).any(|pair| pair[0] > pair[1])
            || self.rejected_order.windows(2).any(|pair| pair[0] > pair[1])
            || self.negative_order.windows(2).any(|pair| pair[0] > pair[1])
            || self
                .contradictory_order
                .windows(2)
                .any(|pair| pair[0] > pair[1])
            || self.unknown_order.windows(2).any(|pair| pair[0] > pair[1])
            || self
                .missing_modality_order
                .windows(2)
                .any(|pair| pair[0] > pair[1])
            || self
                .missing_model_order
                .windows(2)
                .any(|pair| pair[0] > pair[1])
        {
            return Err(EvidenceError::InvalidOutput(
                "identity, selection, or canonical ordering is invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|e| EvidenceError::Digest(e.to_string()))?;
        if expected != self.digest {
            return Err(EvidenceError::InvalidOutput(
                "digest is not bound to the qualification".into(),
            ));
        }
        Ok(())
    }
}

pub fn qualify_evidence(
    request: &EvidenceRequest,
    records: &[EvidenceRecord],
) -> Result<EvidenceQualification, EvidenceError> {
    if request.objective.trim().is_empty()
        || request.max_records == 0
        || request.min_quality_milli > 1_000
        || request.min_reproducibility_milli > 1_000
    {
        return Err(EvidenceError::InvalidRequest(
            "objective, record bound, or score thresholds are invalid".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    let mut max_epoch = 0;
    for record in records {
        record
            .source_artifact
            .validate()
            .map_err(|e| EvidenceError::InvalidRecord(e.to_string()))?;
        if record.evidence_id.trim().is_empty()
            || record.claim.trim().is_empty()
            || record.scope.trim().is_empty()
            || record.relevance_milli > 1_000
            || record.quality_milli > 1_000
            || record.reproducibility_milli > 1_000
            || !ids.insert(record.evidence_id.clone())
        {
            return Err(EvidenceError::InvalidRecord(
                "ids, claim scope, scores, or uniqueness are invalid".into(),
            ));
        }
        max_epoch = max_epoch.max(record.release_epoch);
    }

    let mut ranked = records
        .iter()
        .filter(|record| {
            record.state.is_positive_selection()
                && record.quality_milli >= request.min_quality_milli
                && record.reproducibility_milli >= request.min_reproducibility_milli
        })
        .map(|record| (score(record, max_epoch, request), record))
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| left.1.evidence_id.cmp(&right.1.evidence_id))
    });
    let selected_records = ranked
        .into_iter()
        .take(request.max_records)
        .map(|(_, record)| record.clone())
        .collect::<Vec<_>>();
    let selected_order = selected_records
        .iter()
        .map(|record| record.evidence_id.clone())
        .collect::<Vec<_>>();
    let selected_ids = selected_order.iter().cloned().collect::<BTreeSet<_>>();
    let rejected_order = records
        .iter()
        .filter(|record| !selected_ids.contains(&record.evidence_id))
        .map(|record| record.evidence_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let negative_order = records
        .iter()
        .filter(|record| record.state == EvidenceState::Negative)
        .map(|record| record.evidence_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let contradictory_order = records
        .iter()
        .filter(|record| record.state == EvidenceState::Contradicted)
        .map(|record| record.evidence_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let unknown_order = records
        .iter()
        .filter(|record| {
            matches!(
                record.state,
                EvidenceState::Unknown | EvidenceState::Unmeasured | EvidenceState::Stale
            )
        })
        .map(|record| record.evidence_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let selected_modalities = selected_records
        .iter()
        .map(|record| record.modality)
        .collect::<BTreeSet<_>>();
    let selected_models = selected_records
        .iter()
        .filter_map(|record| record.model_system)
        .collect::<BTreeSet<_>>();
    let missing_modality_order = request
        .required_modalities
        .difference(&selected_modalities)
        .copied()
        .collect::<Vec<_>>();
    let missing_model_order = request
        .required_model_systems
        .difference(&selected_models)
        .copied()
        .collect::<Vec<_>>();
    let disposition = if selected_records.is_empty()
        || !missing_modality_order.is_empty()
        || !missing_model_order.is_empty()
    {
        EvidenceDisposition::Unresolved
    } else if !contradictory_order.is_empty() || !unknown_order.is_empty() {
        EvidenceDisposition::Partial
    } else {
        EvidenceDisposition::Qualified
    };
    let mut output = EvidenceQualification {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        selected_records,
        selected_order,
        rejected_order,
        negative_order,
        contradictory_order,
        unknown_order,
        missing_modality_order,
        missing_model_order,
        disposition,
        digest: ContentHash::of_value(&serde_json::json!({}))
            .map_err(|e| EvidenceError::Digest(e.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|e| EvidenceError::Digest(e.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma_engine::GliomaModality;
    use bioprism_ids::ContentHash;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn record(id: &str, state: EvidenceState, modality: GliomaModality) -> EvidenceRecord {
        EvidenceRecord {
            evidence_id: id.into(),
            source_artifact: LocalArtifactRef {
                artifact_id: format!("artifact-{id}"),
                content_hash: hash(id),
                content_type: "application/vnd.aurora.glioma-evidence+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
            source_kind: EvidenceSourceKind::Dataset,
            claim: format!("claim-{id}"),
            scope: "preclinical glioma".into(),
            modality,
            model_system: Some(GliomaModelSystem::Organoid),
            state,
            relevance_milli: 900,
            quality_milli: 900,
            reproducibility_milli: 850,
            release_epoch: 2,
        }
    }

    #[test]
    fn qualification_preserves_negative_and_unknown_without_promoting_unknown() {
        let request = EvidenceRequest {
            objective: "test mechanism".into(),
            required_modalities: BTreeSet::from([GliomaModality::Genomics]),
            required_model_systems: BTreeSet::from([GliomaModelSystem::Organoid]),
            max_records: 4,
            min_quality_milli: 500,
            min_reproducibility_milli: 500,
        };
        let output = qualify_evidence(
            &request,
            &[
                record(
                    "supported",
                    EvidenceState::Supported,
                    GliomaModality::Genomics,
                ),
                record(
                    "negative",
                    EvidenceState::Negative,
                    GliomaModality::Genomics,
                ),
                record("unknown", EvidenceState::Unknown, GliomaModality::Genomics),
            ],
        )
        .unwrap();
        assert_eq!(output.selected_order, vec!["negative", "supported"]);
        assert_eq!(output.unknown_order, vec!["unknown"]);
        assert_eq!(output.disposition, EvidenceDisposition::Partial);
        output.validate().unwrap();
    }

    #[test]
    fn missing_required_modality_is_unresolved_not_a_pass() {
        let request = EvidenceRequest {
            objective: "test".into(),
            required_modalities: BTreeSet::from([GliomaModality::Spatial]),
            required_model_systems: BTreeSet::new(),
            max_records: 2,
            min_quality_milli: 0,
            min_reproducibility_milli: 0,
        };
        let output = qualify_evidence(
            &request,
            &[record(
                "x",
                EvidenceState::Supported,
                GliomaModality::Genomics,
            )],
        )
        .unwrap();
        assert_eq!(output.disposition, EvidenceDisposition::Unresolved);
        assert_eq!(output.missing_modality_order, vec![GliomaModality::Spatial]);
    }
}

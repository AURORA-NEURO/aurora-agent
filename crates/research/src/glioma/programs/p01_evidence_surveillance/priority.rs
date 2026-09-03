//! Continuous evidence-priority scheduling for preclinical glioma research.
//!
//! Snapshot surveillance detects what changed.  This module handles the complementary question:
//! given the current local evidence set, which records should drive the next research cycle?  It
//! combines deterministic recency decay with state pressure, quality, relevance, reproducibility,
//! and missing coverage.  The result is a queue of concrete review/revalidation actions, not a
//! conclusion about a mechanism and not a literature-fetching service.

use super::super::super::evidence::{EvidenceRecord, EvidenceState};
use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P01-F10";
pub const OUTPUT_SCHEMA: &str = "GliomaEvidencePriority1@1";
pub const MAX_RECORDS: usize = 16_384;
const SCORE_SCALE: u64 = 1_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidencePriorityWeights {
    pub recency_milli: u16,
    pub state_pressure_milli: u16,
    pub quality_milli: u16,
    pub relevance_milli: u16,
    pub reproducibility_milli: u16,
    pub coverage_debt_milli: u16,
}

impl Default for EvidencePriorityWeights {
    fn default() -> Self {
        Self {
            recency_milli: 100,
            state_pressure_milli: 300,
            quality_milli: 100,
            relevance_milli: 150,
            reproducibility_milli: 150,
            coverage_debt_milli: 200,
        }
    }
}

impl EvidencePriorityWeights {
    fn validate(self) -> Result<(), EvidencePriorityError> {
        let total = u32::from(self.recency_milli)
            + u32::from(self.state_pressure_milli)
            + u32::from(self.quality_milli)
            + u32::from(self.relevance_milli)
            + u32::from(self.reproducibility_milli)
            + u32::from(self.coverage_debt_milli);
        if total != SCORE_SCALE as u32 {
            return Err(EvidencePriorityError::InvalidRequest(
                "evidence-priority weights must sum to 1,000 milli-units".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidencePriorityRequest {
    pub objective: String,
    pub current_epoch: u64,
    pub recency_half_life_epochs: u64,
    pub required_modalities: BTreeSet<GliomaModality>,
    pub required_model_systems: BTreeSet<GliomaModelSystem>,
    pub max_actions: usize,
    pub min_priority_milli: u16,
    pub weights: EvidencePriorityWeights,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidencePriorityActionKind {
    RefreshStale,
    ResolveContradiction,
    MeasureUnknown,
    RevalidateNegative,
    CloseCoverage,
    ReplicateSupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidencePriorityAction {
    pub action_id: String,
    pub evidence_id: String,
    pub kind: EvidencePriorityActionKind,
    pub priority_milli: u16,
    pub age_epochs: u64,
    pub recency_milli: u16,
    pub state_pressure_milli: u16,
    pub quality_milli: u16,
    pub relevance_milli: u16,
    pub reproducibility_milli: u16,
    pub coverage_debt_milli: u16,
    pub missing_modality_order: Vec<GliomaModality>,
    pub missing_model_system_order: Vec<GliomaModelSystem>,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidencePriorityDisposition {
    Qualified,
    Partial,
    NoRecords,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidencePriorityPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub current_epoch: u64,
    pub record_order: Vec<String>,
    pub action_order: Vec<String>,
    pub actions: Vec<EvidencePriorityAction>,
    pub selected_order: Vec<String>,
    pub deferred_order: Vec<String>,
    pub missing_modality_order: Vec<GliomaModality>,
    pub missing_model_system_order: Vec<GliomaModelSystem>,
    pub negative_evidence_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub disposition: EvidencePriorityDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EvidencePriorityError {
    #[error("evidence-priority request is invalid: {0}")]
    InvalidRequest(String),
    #[error("evidence-priority record is invalid: {0}")]
    InvalidRecord(String),
    #[error("evidence-priority output is invalid: {0}")]
    InvalidOutput(String),
    #[error("evidence-priority digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &EvidencePriorityPlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "current_epoch": output.current_epoch,
        "record_order": output.record_order,
        "action_order": output.action_order,
        "actions": output.actions,
        "selected_order": output.selected_order,
        "deferred_order": output.deferred_order,
        "missing_modality_order": output.missing_modality_order,
        "missing_model_system_order": output.missing_model_system_order,
        "negative_evidence_order": output.negative_evidence_order,
        "uncertainty_order": output.uncertainty_order,
        "disposition": output.disposition,
    })
}

impl EvidencePriorityPlan {
    pub fn validate(&self) -> Result<(), EvidencePriorityError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || !canonical(&self.record_order)
            || !canonical(&self.selected_order)
            || !canonical(&self.deferred_order)
            || !canonical(&self.missing_modality_order)
            || !canonical(&self.missing_model_system_order)
            || !canonical(&self.negative_evidence_order)
            || !canonical(&self.uncertainty_order)
            || self.actions.len() != self.action_order.len()
            || self.actions.iter().any(|action| {
                action.action_id.trim().is_empty()
                    || action.evidence_id.trim().is_empty()
                    || action.priority_milli > SCORE_SCALE as u16
                    || action.recency_milli > SCORE_SCALE as u16
                    || action.state_pressure_milli > SCORE_SCALE as u16
                    || action.quality_milli > SCORE_SCALE as u16
                    || action.relevance_milli > SCORE_SCALE as u16
                    || action.reproducibility_milli > SCORE_SCALE as u16
                    || action.coverage_debt_milli > SCORE_SCALE as u16
                    || !canonical(&action.missing_modality_order)
                    || !canonical(&action.missing_model_system_order)
                    || action.rationale.trim().is_empty()
            })
            || self.actions.windows(2).any(|pair| {
                pair[0].priority_milli < pair[1].priority_milli
                    || (pair[0].priority_milli == pair[1].priority_milli
                        && pair[0].action_id > pair[1].action_id)
            })
        {
            return Err(EvidencePriorityError::InvalidOutput(
                "identity, ordering, action count, score bounds, or rationale is invalid".into(),
            ));
        }
        let action_ids = self
            .actions
            .iter()
            .map(|action| action.action_id.clone())
            .collect::<BTreeSet<_>>();
        let expected_action_order = self
            .actions
            .iter()
            .map(|action| action.action_id.clone())
            .collect::<Vec<_>>();
        let selected = self.selected_order.iter().collect::<BTreeSet<_>>();
        let deferred = self.deferred_order.iter().collect::<BTreeSet<_>>();
        if action_ids.len() != self.actions.len()
            || self.action_order != expected_action_order
            || self.action_order.iter().cloned().collect::<BTreeSet<_>>() != action_ids
            || selected.len() != self.selected_order.len()
            || deferred.len() != self.deferred_order.len()
            || selected.len() + deferred.len() != action_ids.len()
            || selected.intersection(&deferred).next().is_some()
            || self
                .selected_order
                .iter()
                .chain(self.deferred_order.iter())
                .any(|id| !action_ids.contains(id))
        {
            return Err(EvidencePriorityError::InvalidOutput(
                "action partitions do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| EvidencePriorityError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(EvidencePriorityError::InvalidOutput(
                "evidence-priority digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(request: &EvidencePriorityRequest) -> Result<(), EvidencePriorityError> {
    if request.objective.trim().is_empty()
        || request.recency_half_life_epochs == 0
        || request.max_actions == 0
        || request.max_actions > MAX_RECORDS
        || u64::from(request.min_priority_milli) > SCORE_SCALE
    {
        return Err(EvidencePriorityError::InvalidRequest(
            "objective, positive recency half-life/action bound, and bounded minimum priority are required"
                .into(),
        ));
    }
    request.weights.validate()
}

fn state_pressure(state: EvidenceState) -> u16 {
    match state {
        EvidenceState::Stale => 1_000,
        EvidenceState::Unknown | EvidenceState::Unmeasured => 900,
        EvidenceState::Contradicted => 850,
        EvidenceState::Negative => 750,
        EvidenceState::Supported => 250,
    }
}

fn action_kind(state: EvidenceState, coverage_debt: u16) -> EvidencePriorityActionKind {
    if coverage_debt > 0 {
        EvidencePriorityActionKind::CloseCoverage
    } else {
        match state {
            EvidenceState::Stale => EvidencePriorityActionKind::RefreshStale,
            EvidenceState::Unknown | EvidenceState::Unmeasured => {
                EvidencePriorityActionKind::MeasureUnknown
            }
            EvidenceState::Contradicted => EvidencePriorityActionKind::ResolveContradiction,
            EvidenceState::Negative => EvidencePriorityActionKind::RevalidateNegative,
            EvidenceState::Supported => EvidencePriorityActionKind::ReplicateSupported,
        }
    }
}

fn action_rationale(
    kind: EvidencePriorityActionKind,
    record: &EvidenceRecord,
    missing_count: usize,
) -> String {
    match kind {
        EvidencePriorityActionKind::RefreshStale => {
            "refresh stale evidence before using it to drive a mechanism or assay decision".into()
        }
        EvidencePriorityActionKind::ResolveContradiction => {
            "run an independent preclinical check to reconcile contradictory evidence".into()
        }
        EvidencePriorityActionKind::MeasureUnknown => {
            "measure the unknown or unmeasured endpoint before promoting this record".into()
        }
        EvidencePriorityActionKind::RevalidateNegative => {
            "revalidate the negative result while retaining it as first-class evidence".into()
        }
        EvidencePriorityActionKind::CloseCoverage => format!(
            "close {missing_count} missing modality/model coverage item(s) for this {} record",
            record.evidence_id
        ),
        EvidencePriorityActionKind::ReplicateSupported => {
            "replicate the supported result across an independent preclinical batch".into()
        }
    }
}

/// Build a deterministic action queue for the current local evidence snapshot.
pub fn prioritize_glioma_evidence(
    request: &EvidencePriorityRequest,
    records: &[EvidenceRecord],
) -> Result<EvidencePriorityPlan, EvidencePriorityError> {
    validate_request(request)?;
    if records.is_empty() {
        let mut output = EvidencePriorityPlan {
            feature_id: FEATURE_ID.into(),
            output_schema: OUTPUT_SCHEMA.into(),
            objective: request.objective.clone(),
            current_epoch: request.current_epoch,
            record_order: Vec::new(),
            action_order: Vec::new(),
            actions: Vec::new(),
            selected_order: Vec::new(),
            deferred_order: Vec::new(),
            missing_modality_order: request.required_modalities.iter().copied().collect(),
            missing_model_system_order: request.required_model_systems.iter().copied().collect(),
            negative_evidence_order: Vec::new(),
            uncertainty_order: vec!["no-local-evidence-records".into()],
            disposition: EvidencePriorityDisposition::NoRecords,
            digest: ContentHash::of_bytes(b"unsealed-glioma-evidence-priority"),
        };
        output.digest = ContentHash::of_value(&digest_input(&output))
            .map_err(|error| EvidencePriorityError::Digest(error.to_string()))?;
        output.validate()?;
        return Ok(output);
    }
    if records.len() > MAX_RECORDS {
        return Err(EvidencePriorityError::InvalidRecord(
            "evidence record count exceeds the deterministic bound".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for record in records {
        record
            .source_artifact
            .validate()
            .map_err(|error| EvidencePriorityError::InvalidRecord(error.to_string()))?;
        if record.evidence_id.trim().is_empty()
            || record.claim.trim().is_empty()
            || record.scope.trim().is_empty()
            || record.relevance_milli > SCORE_SCALE as u16
            || record.quality_milli > SCORE_SCALE as u16
            || record.reproducibility_milli > SCORE_SCALE as u16
            || !ids.insert(record.evidence_id.clone())
        {
            return Err(EvidencePriorityError::InvalidRecord(
                "evidence identity, claim scope, scores, or uniqueness is invalid".into(),
            ));
        }
    }
    let record_order = ids.iter().cloned().collect::<Vec<_>>();
    let observed_modalities = records
        .iter()
        .map(|record| record.modality)
        .collect::<BTreeSet<_>>();
    let observed_model_systems = records
        .iter()
        .filter_map(|record| record.model_system)
        .collect::<BTreeSet<_>>();
    let snapshot_missing_modalities = request
        .required_modalities
        .difference(&observed_modalities)
        .copied()
        .collect::<Vec<_>>();
    let snapshot_missing_model_systems = request
        .required_model_systems
        .difference(&observed_model_systems)
        .copied()
        .collect::<Vec<_>>();
    let mut actions = Vec::with_capacity(records.len());
    let mut missing_modalities = BTreeSet::new();
    let mut missing_models = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for record in records {
        let age_epochs = request
            .current_epoch
            .saturating_sub(u64::from(record.release_epoch));
        let recency = SCORE_SCALE
            .saturating_mul(request.recency_half_life_epochs)
            .checked_div(request.recency_half_life_epochs.saturating_add(age_epochs))
            .unwrap_or(0)
            .min(SCORE_SCALE) as u16;
        let missing_modality_order = snapshot_missing_modalities.clone();
        let missing_model_system_order = snapshot_missing_model_systems.clone();
        missing_modalities.extend(missing_modality_order.iter().copied());
        missing_models.extend(missing_model_system_order.iter().copied());
        let missing_count = missing_modality_order.len() + missing_model_system_order.len();
        let coverage_debt = ((missing_count.min(4) * 250) as u16).min(SCORE_SCALE as u16);
        let pressure = state_pressure(record.state);
        let weighted = u64::from(recency)
            .saturating_mul(u64::from(request.weights.recency_milli))
            .saturating_add(
                u64::from(pressure).saturating_mul(u64::from(request.weights.state_pressure_milli)),
            )
            .saturating_add(
                u64::from(record.quality_milli)
                    .saturating_mul(u64::from(request.weights.quality_milli)),
            )
            .saturating_add(
                u64::from(record.relevance_milli)
                    .saturating_mul(u64::from(request.weights.relevance_milli)),
            )
            .saturating_add(
                u64::from(record.reproducibility_milli)
                    .saturating_mul(u64::from(request.weights.reproducibility_milli)),
            )
            .saturating_add(
                u64::from(coverage_debt)
                    .saturating_mul(u64::from(request.weights.coverage_debt_milli)),
            );
        let priority = weighted.saturating_div(SCORE_SCALE).min(SCORE_SCALE) as u16;
        let kind = action_kind(record.state, coverage_debt);
        if record.state == EvidenceState::Negative {
            negative.insert(record.evidence_id.clone());
        }
        if matches!(
            record.state,
            EvidenceState::Unknown
                | EvidenceState::Unmeasured
                | EvidenceState::Stale
                | EvidenceState::Contradicted
        ) {
            uncertainty.insert(record.evidence_id.clone());
        }
        actions.push(EvidencePriorityAction {
            action_id: format!("evidence-priority:{}", record.evidence_id),
            evidence_id: record.evidence_id.clone(),
            kind,
            priority_milli: priority,
            age_epochs,
            recency_milli: recency,
            state_pressure_milli: pressure,
            quality_milli: record.quality_milli,
            relevance_milli: record.relevance_milli,
            reproducibility_milli: record.reproducibility_milli,
            coverage_debt_milli: coverage_debt,
            missing_modality_order,
            missing_model_system_order,
            rationale: action_rationale(kind, record, missing_count),
        });
    }
    actions.sort_by(|left, right| {
        right
            .priority_milli
            .cmp(&left.priority_milli)
            .then_with(|| left.action_id.cmp(&right.action_id))
    });
    let mut selected_order = actions
        .iter()
        .filter(|action| action.priority_milli >= request.min_priority_milli)
        .take(request.max_actions)
        .map(|action| action.action_id.clone())
        .collect::<Vec<_>>();
    selected_order.sort();
    let selected_set = selected_order.iter().collect::<BTreeSet<_>>();
    let deferred_order = actions
        .iter()
        .filter(|action| !selected_set.contains(&action.action_id))
        .map(|action| action.action_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let action_order = actions
        .iter()
        .map(|action| action.action_id.clone())
        .collect::<Vec<_>>();
    let disposition = if selected_order.is_empty() {
        EvidencePriorityDisposition::Unresolved
    } else if deferred_order.is_empty() && uncertainty.is_empty() {
        EvidencePriorityDisposition::Qualified
    } else {
        EvidencePriorityDisposition::Partial
    };
    let mut output = EvidencePriorityPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        current_epoch: request.current_epoch,
        record_order,
        action_order,
        actions,
        selected_order,
        deferred_order,
        missing_modality_order: missing_modalities.into_iter().collect(),
        missing_model_system_order: missing_models.into_iter().collect(),
        negative_evidence_order: negative.into_iter().collect(),
        uncertainty_order: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-evidence-priority"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| EvidencePriorityError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::evidence::EvidenceSourceKind;
    use crate::glioma_engine::LocalArtifactRef;

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: format!("artifact:{id}"),
            content_hash: ContentHash::of_bytes(id.as_bytes()),
            content_type: "application/vnd.aurora.glioma-evidence+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn request() -> EvidencePriorityRequest {
        EvidencePriorityRequest {
            objective: "prioritize glioma invasion evidence".into(),
            current_epoch: 10,
            recency_half_life_epochs: 4,
            required_modalities: BTreeSet::new(),
            required_model_systems: BTreeSet::new(),
            max_actions: 2,
            min_priority_milli: 0,
            weights: Default::default(),
        }
    }

    fn record(id: &str, state: EvidenceState, epoch: u64) -> EvidenceRecord {
        EvidenceRecord {
            evidence_id: id.into(),
            source_artifact: artifact(id),
            source_kind: EvidenceSourceKind::Dataset,
            claim: format!("claim {id}"),
            scope: "preclinical glioma".into(),
            modality: GliomaModality::Genomics,
            model_system: Some(GliomaModelSystem::Organoid),
            state,
            relevance_milli: 900,
            quality_milli: 900,
            reproducibility_milli: 900,
            release_epoch: epoch as u32,
        }
    }

    #[test]
    fn stale_and_negative_records_receive_explicit_actions() {
        let output = prioritize_glioma_evidence(
            &request(),
            &[
                record("stale", EvidenceState::Stale, 0),
                record("negative", EvidenceState::Negative, 10),
            ],
        )
        .unwrap();
        assert_eq!(output.selected_order.len(), 2);
        assert!(output
            .actions
            .iter()
            .any(|action| { action.kind == EvidencePriorityActionKind::RefreshStale }));
        assert!(output.negative_evidence_order.contains(&"negative".into()));
        output.validate().unwrap();
    }

    #[test]
    fn missing_coverage_is_visible_and_raises_priority() {
        let mut request = request();
        request.required_modalities = BTreeSet::from([GliomaModality::Spatial]);
        request.required_model_systems = BTreeSet::from([GliomaModelSystem::MouseModel]);
        let output = prioritize_glioma_evidence(
            &request,
            &[record("coverage-gap", EvidenceState::Supported, 10)],
        )
        .unwrap();
        assert_eq!(
            output.actions[0].kind,
            EvidencePriorityActionKind::CloseCoverage
        );
        assert_eq!(output.actions[0].coverage_debt_milli, 500);
        assert_eq!(output.missing_modality_order, vec![GliomaModality::Spatial]);
    }

    #[test]
    fn coverage_is_assessed_across_the_snapshot_not_per_record() {
        let mut request = request();
        request.required_modalities =
            BTreeSet::from([GliomaModality::Genomics, GliomaModality::Spatial]);
        request.required_model_systems =
            BTreeSet::from([GliomaModelSystem::Organoid, GliomaModelSystem::MouseModel]);
        let mut spatial = record("spatial", EvidenceState::Supported, 10);
        spatial.modality = GliomaModality::Spatial;
        spatial.model_system = Some(GliomaModelSystem::MouseModel);
        let output = prioritize_glioma_evidence(
            &request,
            &[record("genomics", EvidenceState::Supported, 10), spatial],
        )
        .unwrap();
        assert!(output.missing_modality_order.is_empty());
        assert!(output.missing_model_system_order.is_empty());
        assert!(output
            .actions
            .iter()
            .all(|action| action.kind == EvidencePriorityActionKind::ReplicateSupported));
        output.validate().unwrap();
    }

    #[test]
    fn empty_snapshot_is_a_valid_unresolved_plan() {
        let output = prioritize_glioma_evidence(&request(), &[]).unwrap();
        assert_eq!(output.disposition, EvidencePriorityDisposition::NoRecords);
        assert!(output
            .uncertainty_order
            .contains(&"no-local-evidence-records".into()));
        output.validate().unwrap();
    }
}

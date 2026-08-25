//! Multi-study quality envelope and comparability contract.
//!
//! Atlas feature: `AFA-adapter-P07-F06`.
//!
//! This is the consortium-facing quality-control product around individual QC receipts. It
//! compares typed imaging/omics study envelopes without reading raw bytes, refuses incomparable
//! protocol or instrument profiles, preserves missing modality coverage, and emits a deterministic
//! quality verdict that downstream execution can replay.

use crate::{QualityControlReceipt, QualityDisposition};
use bioprism_foundation::{
    LossSeverity, ProvenanceLink, SemanticLoss, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P07-F06";
pub const CONTRACT_VERSION: &str = "multi-study-quality-envelope/1.0";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StudyQualityRecord {
    pub study_id: String,
    pub modality: String,
    pub comparability_key: String,
    pub quality_receipt: QualityControlReceipt,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QualityEnvelopeRequest {
    pub envelope_id: String,
    pub reference_schema: String,
    pub comparability_profile: String,
    pub studies: Vec<StudyQualityRecord>,
    pub required_modalities: Vec<String>,
    pub minimum_studies_per_modality: u32,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityEnvelopeDecision {
    Qualified,
    Partial,
    Blocked,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StudyQualityVerdict {
    pub study_id: String,
    pub modality: String,
    pub quality_disposition: QualityDisposition,
    pub comparable: bool,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityEnvelopeReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub envelope_id: String,
    pub reference_schema: String,
    pub comparability_profile: String,
    pub decision: QualityEnvelopeDecision,
    pub study_order: Vec<String>,
    pub modality_coverage: BTreeMap<String, u32>,
    pub verdicts: Vec<StudyQualityVerdict>,
    pub omitted_modalities: Vec<String>,
    pub comparability_conflicts: Vec<String>,
    pub semantic_loss: Vec<SemanticLoss>,
    pub reasons: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

impl QualityEnvelopeReceipt {
    pub fn validate(&self) -> Result<(), QualityEnvelopeError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
        {
            return Err(QualityEnvelopeError::Contract(
                "quality envelope contract identity mismatch".into(),
            ));
        }
        if self.envelope_id.trim().is_empty()
            || self.reference_schema.trim().is_empty()
            || self.comparability_profile.trim().is_empty()
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
        {
            return Err(QualityEnvelopeError::InvalidRequest(
                "envelope identity, profile, boundary, and locality are required".into(),
            ));
        }
        if self.study_order.is_empty()
            || self.study_order.len() != self.verdicts.len()
            || self.reasons.is_empty()
        {
            return Err(QualityEnvelopeError::InvalidRequest(
                "study order, verdicts, and reasons are required".into(),
            ));
        }
        if self.study_order.windows(2).any(|pair| pair[0] >= pair[1])
            || self.study_order.iter().collect::<BTreeSet<_>>().len() != self.study_order.len()
        {
            return Err(QualityEnvelopeError::InvalidRequest(
                "study order must be unique and canonical".into(),
            ));
        }
        if self
            .verdicts
            .iter()
            .zip(&self.study_order)
            .any(|(verdict, study_id)| &verdict.study_id != study_id)
        {
            return Err(QualityEnvelopeError::InvalidRequest(
                "verdict order is not linked to study order".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| QualityEnvelopeError::Contract(error.to_string()))?;
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, QualityEnvelopeError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| QualityEnvelopeError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| QualityEnvelopeError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum QualityEnvelopeError {
    #[error("invalid multi-study quality-envelope request: {0}")]
    InvalidRequest(String),
    #[error("quality-envelope contract rejected: {0}")]
    Contract(String),
    #[error("raw quality data must remain local")]
    Localization,
    #[error("duplicate study id {0}")]
    DuplicateStudy(String),
    #[error("quality envelope serialization failed: {0}")]
    Serialization(String),
}

pub fn evaluate_quality_envelope(
    request: &QualityEnvelopeRequest,
) -> Result<QualityEnvelopeReceipt, QualityEnvelopeError> {
    validate_request(request)?;
    let mut studies = request.studies.clone();
    studies.sort_by(|left, right| left.study_id.cmp(&right.study_id));
    let mut modality_coverage = BTreeMap::<String, u32>::new();
    let mut modality_profiles = BTreeMap::<String, BTreeSet<String>>::new();
    let mut verdicts = Vec::with_capacity(studies.len());
    let mut reasons = Vec::new();
    let mut semantic_loss = Vec::new();
    for study in &studies {
        *modality_coverage.entry(study.modality.clone()).or_default() += 1;
        modality_profiles
            .entry(study.modality.clone())
            .or_default()
            .insert(study.comparability_key.clone());
        let quality_disposition = study.quality_receipt.summary.disposition;
        let mut study_reasons = study.quality_receipt.summary.reasons.clone();
        let comparable = quality_disposition != QualityDisposition::Blocked;
        if !comparable {
            study_reasons.push("study quality receipt is blocked".into());
        }
        verdicts.push(StudyQualityVerdict {
            study_id: study.study_id.clone(),
            modality: study.modality.clone(),
            quality_disposition,
            comparable,
            reasons: study_reasons,
        });
    }
    let mut comparability_conflicts = Vec::new();
    for (modality, profiles) in &modality_profiles {
        if profiles.len() > 1 {
            comparability_conflicts.push(format!(
                "modality {} has incompatible comparability profiles: {}",
                modality,
                profiles.iter().cloned().collect::<Vec<_>>().join(", ")
            ));
        }
    }
    let omitted_modalities = request
        .required_modalities
        .iter()
        .filter(|modality| {
            modality_coverage
                .get(*modality)
                .copied()
                .unwrap_or_default()
                < request.minimum_studies_per_modality
        })
        .cloned()
        .collect::<Vec<_>>();
    if !omitted_modalities.is_empty() {
        reasons.push(format!(
            "required modality coverage is incomplete: {}",
            omitted_modalities.join(", ")
        ));
    }
    if !comparability_conflicts.is_empty() {
        reasons.extend(comparability_conflicts.iter().cloned());
        semantic_loss.push(SemanticLoss {
            field: "comparability".into(),
            reason: "studies within a modality use incompatible protocol or instrument profiles"
                .into(),
            severity: LossSeverity::DecisionRelevant,
        });
    }
    let blocked_studies = verdicts
        .iter()
        .filter(|verdict| verdict.quality_disposition == QualityDisposition::Blocked)
        .count();
    let unknown_studies = verdicts
        .iter()
        .filter(|verdict| verdict.quality_disposition == QualityDisposition::Unknown)
        .count();
    if !request.protected_closure {
        reasons.push("protected quality closure is incomplete".into());
        semantic_loss.push(SemanticLoss {
            field: "protected_closure".into(),
            reason: "unmeasured or protected quality dimensions cannot be certified".into(),
            severity: LossSeverity::DecisionRelevant,
        });
    }
    let decision =
        if !request.protected_closure || blocked_studies > 0 || !comparability_conflicts.is_empty()
        {
            QualityEnvelopeDecision::Blocked
        } else if !omitted_modalities.is_empty() {
            QualityEnvelopeDecision::Partial
        } else if unknown_studies > 0 {
            QualityEnvelopeDecision::Unknown
        } else {
            reasons.push(
                "all required studies, modality coverage, comparability, and quality gates passed"
                    .into(),
            );
            QualityEnvelopeDecision::Qualified
        };
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "envelope_id": request.envelope_id,
        "reference_schema": request.reference_schema,
        "comparability_profile": request.comparability_profile,
        "decision": decision,
        "study_order": studies.iter().map(|study| study.study_id.clone()).collect::<Vec<_>>(),
        "modality_coverage": modality_coverage,
        "omitted_modalities": omitted_modalities,
        "comparability_conflicts": comparability_conflicts,
        "semantic_loss": semantic_loss,
        "reasons": reasons,
        "raw_data_local": true,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let provenance = studies
        .iter()
        .map(|study| ProvenanceLink {
            source_id: study.study_id.clone(),
            relation: "quality-envelope-from-local-receipt".into(),
            digest: study.quality_receipt.artifact.content_hash.clone(),
        })
        .collect();
    let artifact = TypedResearchArtifact::from_payload(
        format!("quality-envelope:{}", request.envelope_id),
        "application/vnd.aurora.multi-study-quality-envelope+json",
        &payload,
        semantic_loss.clone(),
        provenance,
    )
    .map_err(|error| QualityEnvelopeError::Contract(error.to_string()))?;
    let receipt = QualityEnvelopeReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        envelope_id: request.envelope_id.clone(),
        reference_schema: request.reference_schema.clone(),
        comparability_profile: request.comparability_profile.clone(),
        decision,
        study_order: studies.iter().map(|study| study.study_id.clone()).collect(),
        modality_coverage,
        verdicts,
        omitted_modalities,
        comparability_conflicts,
        semantic_loss,
        reasons,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &QualityEnvelopeRequest) -> Result<(), QualityEnvelopeError> {
    if request.envelope_id.trim().is_empty()
        || request.reference_schema.trim().is_empty()
        || request.comparability_profile.trim().is_empty()
        || request.studies.is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || request.minimum_studies_per_modality == 0
    {
        return if !request.raw_data_local || request.boundary != PRECLINICAL_BOUNDARY {
            Err(QualityEnvelopeError::Localization)
        } else {
            Err(QualityEnvelopeError::InvalidRequest(
                "envelope identity, studies, profile, positive minimum, and boundary are required"
                    .into(),
            ))
        };
    }
    let mut ids = BTreeSet::new();
    for study in &request.studies {
        if study.study_id.trim().is_empty()
            || study.modality.trim().is_empty()
            || study.comparability_key.trim().is_empty()
            || study.quality_receipt.dataset_id != study.study_id
            || study.quality_receipt.modality != study.modality
            || !study.quality_receipt.raw_data_local
        {
            return Err(QualityEnvelopeError::InvalidRequest(
                "study identity, comparability key, linked QC receipt, and locality are required"
                    .into(),
            ));
        }
        if !ids.insert(study.study_id.clone()) {
            return Err(QualityEnvelopeError::DuplicateStudy(study.study_id.clone()));
        }
    }
    if request
        .required_modalities
        .iter()
        .any(|modality| modality.trim().is_empty())
    {
        return Err(QualityEnvelopeError::InvalidRequest(
            "required modality names cannot be empty".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MetricDirection, QualityControlRequest, QualityMetric};

    fn quality(study: &str, modality: &str) -> StudyQualityRecord {
        let receipt = crate::evaluate_quality_control(&QualityControlRequest {
            dataset_id: study.into(),
            modality: modality.into(),
            source_digest: ContentHash::of_bytes(study.as_bytes()),
            metrics: vec![QualityMetric {
                metric_id: "signal_to_noise".into(),
                value: Some(4.0),
                threshold: 3.0,
                direction: MetricDirection::AtLeast,
                required: true,
            }],
            conformance_verified: true,
            raw_data_local: true,
        })
        .unwrap();
        StudyQualityRecord {
            study_id: study.into(),
            modality: modality.into(),
            comparability_key: "protocol-v2|instrument-v3".into(),
            quality_receipt: receipt,
        }
    }

    fn request() -> QualityEnvelopeRequest {
        QualityEnvelopeRequest {
            envelope_id: "envelope:organoid".into(),
            reference_schema: "aurora-qc/1".into(),
            comparability_profile: "protocol-v2|instrument-v3".into(),
            studies: vec![
                quality("study:rna", "transcriptomics"),
                quality("study:image", "imaging"),
            ],
            required_modalities: vec!["transcriptomics".into(), "imaging".into()],
            minimum_studies_per_modality: 1,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn quality_envelope_is_sorted_and_replayable() {
        let mut reversed = request();
        reversed.studies.reverse();
        let left = evaluate_quality_envelope(&request()).unwrap();
        let right = evaluate_quality_envelope(&reversed).unwrap();
        assert_eq!(left.digest().unwrap(), right.digest().unwrap());
        assert_eq!(left.decision, QualityEnvelopeDecision::Qualified);
        assert_eq!(left.study_order, vec!["study:image", "study:rna"]);
        assert_eq!(left.modality_coverage["imaging"], 1);
    }

    #[test]
    fn incompatible_profiles_block_cross_study_qualification() {
        let mut request = request();
        request.studies[1].comparability_key = "protocol-v9|instrument-v9".into();
        let receipt = evaluate_quality_envelope(&request).unwrap();
        assert_eq!(receipt.decision, QualityEnvelopeDecision::Blocked);
        assert!(!receipt.comparability_conflicts.is_empty());
    }

    #[test]
    fn incomplete_protected_closure_is_not_a_pass() {
        let mut request = request();
        request.protected_closure = false;
        let receipt = evaluate_quality_envelope(&request).unwrap();
        assert_eq!(receipt.decision, QualityEnvelopeDecision::Blocked);
        assert!(receipt
            .semantic_loss
            .iter()
            .any(|loss| loss.field == "protected_closure"));
    }

    #[test]
    fn duplicate_study_is_rejected() {
        let mut request = request();
        request.studies[1].study_id = request.studies[0].study_id.clone();
        assert!(matches!(
            evaluate_quality_envelope(&request).unwrap_err(),
            QualityEnvelopeError::DuplicateStudy(_)
        ));
    }
}

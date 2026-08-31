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
const MAX_TEXT_BYTES: usize = 512;
const MAX_STUDIES: usize = 8192;
const MAX_ITEMS: usize = 16384;

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QualityEnvelopeReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub input: QualityEnvelopeRequest,
    pub input_digest: ContentHash,
    pub envelope_id: String,
    pub reference_schema: String,
    pub comparability_profile: String,
    pub required_modalities: Vec<String>,
    pub minimum_studies_per_modality: u32,
    pub protected_closure: bool,
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
        if self.boundary != PRECLINICAL_BOUNDARY || !self.raw_data_local {
            return Err(QualityEnvelopeError::InvalidRequest(
                "envelope identity, profile, boundary, and locality are required".into(),
            ));
        }
        if self.minimum_studies_per_modality == 0 {
            return Err(QualityEnvelopeError::InvalidRequest(
                "minimum studies per modality must be positive".into(),
            ));
        }
        for (field, value) in [
            ("envelope_id", self.envelope_id.as_str()),
            ("reference_schema", self.reference_schema.as_str()),
            ("comparability_profile", self.comparability_profile.as_str()),
            ("boundary", self.boundary.as_str()),
        ] {
            validate_text(field, value)?;
        }
        if self.study_order.is_empty()
            || self.study_order.len() > MAX_STUDIES
            || self.study_order.len() != self.verdicts.len()
            || self.reasons.is_empty()
        {
            return Err(QualityEnvelopeError::InvalidRequest(
                "study order, verdicts, and reasons are required".into(),
            ));
        }
        validate_sorted_strings("study_order", &self.study_order)?;
        validate_sorted_strings("required_modalities", &self.required_modalities)?;
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
        let mut expected_coverage = BTreeMap::new();
        for verdict in &self.verdicts {
            validate_text("verdict.study_id", &verdict.study_id)?;
            validate_text("verdict.modality", &verdict.modality)?;
            validate_sorted_strings("verdict.reasons", &verdict.reasons)?;
            if verdict.comparable && verdict.quality_disposition != QualityDisposition::Blocked {
                *expected_coverage
                    .entry(verdict.modality.clone())
                    .or_insert(0_u32) += 1;
            }
        }
        if self.modality_coverage != expected_coverage {
            return Err(QualityEnvelopeError::InvalidRequest(
                "modality coverage is not derived from comparable verdicts".into(),
            ));
        }
        let expected_omitted = self
            .required_modalities
            .iter()
            .filter(|modality| {
                self.modality_coverage
                    .get(*modality)
                    .copied()
                    .unwrap_or_default()
                    < self.minimum_studies_per_modality
            })
            .cloned()
            .collect::<Vec<_>>();
        if self.omitted_modalities != expected_omitted {
            return Err(QualityEnvelopeError::InvalidRequest(
                "omitted modalities are not derived from required coverage".into(),
            ));
        }
        validate_sorted_strings("omitted_modalities", &self.omitted_modalities)?;
        validate_sorted_strings("comparability_conflicts", &self.comparability_conflicts)?;
        validate_sorted_strings("reasons", &self.reasons)?;
        for loss in &self.semantic_loss {
            validate_text("semantic_loss.field", &loss.field)?;
            validate_text("semantic_loss.reason", &loss.reason)?;
        }
        if self.semantic_loss.windows(2).any(|pair| {
            (
                pair[0].field.as_str(),
                pair[0].reason.as_str(),
                pair[0].severity,
            ) >= (
                pair[1].field.as_str(),
                pair[1].reason.as_str(),
                pair[1].severity,
            )
        }) {
            return Err(QualityEnvelopeError::InvalidRequest(
                "semantic loss ordering is not canonical".into(),
            ));
        }
        if self.semantic_loss
            != canonical_semantic_loss(
                !self.comparability_conflicts.is_empty(),
                self.protected_closure,
            )
        {
            return Err(QualityEnvelopeError::InvalidRequest(
                "semantic loss is not derived from quality-envelope gates".into(),
            ));
        }
        let blocked_studies = self
            .verdicts
            .iter()
            .filter(|verdict| verdict.quality_disposition == QualityDisposition::Blocked)
            .count();
        let unknown_studies = self
            .verdicts
            .iter()
            .filter(|verdict| verdict.quality_disposition == QualityDisposition::Unknown)
            .count();
        let expected_decision = quality_envelope_decision(
            self.protected_closure,
            blocked_studies,
            &self.omitted_modalities,
            &self.comparability_conflicts,
            unknown_studies,
        );
        if self.decision != expected_decision {
            return Err(QualityEnvelopeError::InvalidRequest(
                "quality-envelope decision is not derived from its gate state".into(),
            ));
        }
        if self.artifact.artifact_id != format!("quality-envelope:{}", self.envelope_id)
            || self.artifact.content_type
                != "application/vnd.aurora.multi-study-quality-envelope+json"
            || self.artifact.semantic_loss != self.semantic_loss
            || self.artifact.provenance.len() != self.study_order.len()
            || self
                .artifact
                .provenance
                .iter()
                .zip(&self.study_order)
                .any(|(link, study_id)| {
                    link.source_id != *study_id
                        || link.relation != "quality-envelope-from-local-receipt"
                        || link.digest == ContentHash::of_bytes(b"")
                })
        {
            return Err(QualityEnvelopeError::Contract(
                "quality envelope artifact is not bound to ordered study provenance".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| QualityEnvelopeError::Contract(error.to_string()))?;
        self.artifact
            .verify_payload(&quality_envelope_payload(self))
            .map_err(|error| QualityEnvelopeError::Contract(error.to_string()))?;
        validate_request(&self.input)?;
        if self.input_digest != quality_envelope_input_digest(&self.input)? {
            return Err(QualityEnvelopeError::Contract(
                "quality envelope retained input digest does not match the request".into(),
            ));
        }
        let expected = build_quality_envelope(&self.input)?;
        if self != &expected {
            return Err(QualityEnvelopeError::Contract(
                "quality envelope receipt is not derived from its retained request".into(),
            ));
        }
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

fn validate_text(field: &str, value: &str) -> Result<(), QualityEnvelopeError> {
    if value.is_empty() || value.trim() != value {
        return Err(QualityEnvelopeError::InvalidRequest(format!(
            "{field} must be non-empty and trimmed"
        )));
    }
    if value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(QualityEnvelopeError::InvalidRequest(format!(
            "{field} is outside its bounded text contract"
        )));
    }
    Ok(())
}

fn quality_envelope_input_digest(
    request: &QualityEnvelopeRequest,
) -> Result<ContentHash, QualityEnvelopeError> {
    let value = serde_json::to_value(&canonical_quality_envelope_request(request))
        .map_err(|error| QualityEnvelopeError::Serialization(error.to_string()))?;
    ContentHash::of_value(&value)
        .map_err(|error| QualityEnvelopeError::Serialization(error.to_string()))
}

fn canonical_quality_envelope_request(request: &QualityEnvelopeRequest) -> QualityEnvelopeRequest {
    let mut canonical = request.clone();
    canonical.required_modalities.sort();
    canonical
        .studies
        .sort_by(|left, right| left.study_id.cmp(&right.study_id));
    canonical
}

fn validate_sorted_strings(field: &str, values: &[String]) -> Result<(), QualityEnvelopeError> {
    validate_unique_strings(field, values)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(QualityEnvelopeError::InvalidRequest(format!(
            "{field} ordering is not canonical"
        )));
    }
    Ok(())
}

fn validate_unique_strings(field: &str, values: &[String]) -> Result<(), QualityEnvelopeError> {
    if values.len() > MAX_ITEMS {
        return Err(QualityEnvelopeError::InvalidRequest(format!(
            "{field} exceeds its item bound"
        )));
    }
    let mut unique = BTreeSet::new();
    for value in values {
        validate_text(field, value)?;
        if !unique.insert(value) {
            return Err(QualityEnvelopeError::InvalidRequest(format!(
                "{field} contains duplicate values"
            )));
        }
    }
    Ok(())
}

fn canonical_semantic_loss(
    has_comparability_conflicts: bool,
    protected_closure: bool,
) -> Vec<SemanticLoss> {
    let mut losses = Vec::new();
    if has_comparability_conflicts {
        losses.push(SemanticLoss {
            field: "comparability".into(),
            reason: "studies within a modality use incompatible protocol or instrument profiles"
                .into(),
            severity: LossSeverity::DecisionRelevant,
        });
    }
    if !protected_closure {
        losses.push(SemanticLoss {
            field: "protected_closure".into(),
            reason: "unmeasured or protected quality dimensions cannot be certified".into(),
            severity: LossSeverity::DecisionRelevant,
        });
    }
    losses.sort_by(|left, right| {
        (left.field.as_str(), left.reason.as_str(), left.severity).cmp(&(
            right.field.as_str(),
            right.reason.as_str(),
            right.severity,
        ))
    });
    losses
}

fn quality_envelope_decision(
    protected_closure: bool,
    blocked_studies: usize,
    omitted_modalities: &[String],
    comparability_conflicts: &[String],
    unknown_studies: usize,
) -> QualityEnvelopeDecision {
    if !protected_closure || blocked_studies > 0 || !comparability_conflicts.is_empty() {
        QualityEnvelopeDecision::Blocked
    } else if !omitted_modalities.is_empty() {
        QualityEnvelopeDecision::Partial
    } else if unknown_studies > 0 {
        QualityEnvelopeDecision::Unknown
    } else {
        QualityEnvelopeDecision::Qualified
    }
}

fn quality_envelope_payload(receipt: &QualityEnvelopeReceipt) -> serde_json::Value {
    quality_envelope_payload_from_parts(
        &receipt.schema_version,
        &receipt.contract_version,
        &receipt.feature_id,
        &receipt.envelope_id,
        &receipt.reference_schema,
        &receipt.comparability_profile,
        &receipt.required_modalities,
        receipt.minimum_studies_per_modality,
        receipt.protected_closure,
        receipt.decision,
        &receipt.study_order,
        &receipt.modality_coverage,
        &receipt.verdicts,
        &receipt.omitted_modalities,
        &receipt.comparability_conflicts,
        &receipt.semantic_loss,
        &receipt.reasons,
        &receipt.artifact.provenance,
        receipt.raw_data_local,
        &receipt.boundary,
    )
}

#[allow(clippy::too_many_arguments)]
fn quality_envelope_payload_from_parts(
    schema_version: &str,
    contract_version: &str,
    feature_id: &str,
    envelope_id: &str,
    reference_schema: &str,
    comparability_profile: &str,
    required_modalities: &[String],
    minimum_studies_per_modality: u32,
    protected_closure: bool,
    decision: QualityEnvelopeDecision,
    study_order: &[String],
    modality_coverage: &BTreeMap<String, u32>,
    verdicts: &[StudyQualityVerdict],
    omitted_modalities: &[String],
    comparability_conflicts: &[String],
    semantic_loss: &[SemanticLoss],
    reasons: &[String],
    provenance: &[ProvenanceLink],
    raw_data_local: bool,
    boundary: &str,
) -> serde_json::Value {
    json!({
        "schema_version": schema_version,
        "contract_version": contract_version,
        "feature_id": feature_id,
        "envelope_id": envelope_id,
        "reference_schema": reference_schema,
        "comparability_profile": comparability_profile,
        "required_modalities": required_modalities,
        "minimum_studies_per_modality": minimum_studies_per_modality,
        "protected_closure": protected_closure,
        "decision": decision,
        "study_order": study_order,
        "modality_coverage": modality_coverage,
        "verdicts": verdicts,
        "omitted_modalities": omitted_modalities,
        "comparability_conflicts": comparability_conflicts,
        "semantic_loss": semantic_loss,
        "reasons": reasons,
        "provenance": provenance,
        "raw_data_local": raw_data_local,
        "boundary": boundary,
    })
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
    let receipt = build_quality_envelope(request)?;
    receipt.validate()?;
    Ok(receipt)
}

fn build_quality_envelope(
    request: &QualityEnvelopeRequest,
) -> Result<QualityEnvelopeReceipt, QualityEnvelopeError> {
    validate_request(request)?;
    let mut studies = request.studies.clone();
    studies.sort_by(|left, right| left.study_id.cmp(&right.study_id));
    let mut required_modalities = request.required_modalities.clone();
    required_modalities.sort();
    let mut modality_coverage = BTreeMap::<String, u32>::new();
    let mut comparability_profiles = BTreeSet::new();
    let mut verdicts = Vec::with_capacity(studies.len());
    let mut reasons = Vec::new();
    for study in &studies {
        comparability_profiles.insert(study.comparability_key.clone());
        let quality_disposition = study.quality_receipt.summary.disposition;
        let mut study_reasons = study.quality_receipt.summary.reasons.clone();
        let profile_matches = study.comparability_key == request.comparability_profile;
        let comparable = quality_disposition != QualityDisposition::Blocked && profile_matches;
        if quality_disposition == QualityDisposition::Blocked {
            study_reasons.push("study quality receipt is blocked".into());
        }
        if !profile_matches {
            study_reasons.push("study comparability key differs from the requested profile".into());
        }
        if comparable {
            *modality_coverage.entry(study.modality.clone()).or_default() += 1;
        }
        study_reasons.sort();
        study_reasons.dedup();
        verdicts.push(StudyQualityVerdict {
            study_id: study.study_id.clone(),
            modality: study.modality.clone(),
            quality_disposition,
            comparable,
            reasons: study_reasons,
        });
    }
    let mut comparability_conflicts = Vec::new();
    if comparability_profiles.len() > 1
        || comparability_profiles
            .iter()
            .any(|profile| profile != &request.comparability_profile)
    {
        comparability_conflicts.push(format!(
            "studies have incompatible comparability profiles: {}",
            comparability_profiles
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    let mut omitted_modalities = required_modalities
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
    omitted_modalities.sort();
    if !omitted_modalities.is_empty() {
        reasons.push(format!(
            "required modality coverage is incomplete: {}",
            omitted_modalities.join(", ")
        ));
    }
    if !comparability_conflicts.is_empty() {
        reasons.extend(comparability_conflicts.iter().cloned());
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
    }
    let decision = quality_envelope_decision(
        request.protected_closure,
        blocked_studies,
        &omitted_modalities,
        &comparability_conflicts,
        unknown_studies,
    );
    if decision == QualityEnvelopeDecision::Qualified {
        reasons.push(
            "all required studies, modality coverage, comparability, and quality gates passed"
                .into(),
        );
    }
    reasons.sort();
    reasons.dedup();
    let semantic_loss = canonical_semantic_loss(
        !comparability_conflicts.is_empty(),
        request.protected_closure,
    );
    let study_order = studies
        .iter()
        .map(|study| study.study_id.clone())
        .collect::<Vec<_>>();
    let provenance = studies
        .iter()
        .map(|study| ProvenanceLink {
            source_id: study.study_id.clone(),
            relation: "quality-envelope-from-local-receipt".into(),
            digest: study.quality_receipt.artifact.content_hash.clone(),
        })
        .collect::<Vec<_>>();
    let payload = quality_envelope_payload_from_parts(
        RESEARCH_CONTRACT_SCHEMA_VERSION,
        CONTRACT_VERSION,
        FEATURE_ID,
        &request.envelope_id,
        &request.reference_schema,
        &request.comparability_profile,
        &required_modalities,
        request.minimum_studies_per_modality,
        request.protected_closure,
        decision,
        &study_order,
        &modality_coverage,
        &verdicts,
        &omitted_modalities,
        &comparability_conflicts,
        &semantic_loss,
        &reasons,
        &provenance,
        true,
        PRECLINICAL_BOUNDARY,
    );
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
        input: canonical_quality_envelope_request(request),
        input_digest: quality_envelope_input_digest(request)?,
        envelope_id: request.envelope_id.clone(),
        reference_schema: request.reference_schema.clone(),
        comparability_profile: request.comparability_profile.clone(),
        required_modalities,
        minimum_studies_per_modality: request.minimum_studies_per_modality,
        protected_closure: request.protected_closure,
        decision,
        study_order,
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
    Ok(receipt)
}

fn validate_request(request: &QualityEnvelopeRequest) -> Result<(), QualityEnvelopeError> {
    if request.envelope_id.trim().is_empty()
        || request.reference_schema.trim().is_empty()
        || request.comparability_profile.trim().is_empty()
        || request.studies.is_empty()
        || request.studies.len() > MAX_STUDIES
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
    validate_text("envelope_id", &request.envelope_id)?;
    validate_text("reference_schema", &request.reference_schema)?;
    validate_text("comparability_profile", &request.comparability_profile)?;
    validate_text("boundary", &request.boundary)?;
    let mut ids = BTreeSet::new();
    for study in &request.studies {
        validate_text("study_id", &study.study_id)?;
        if !ids.insert(study.study_id.clone()) {
            return Err(QualityEnvelopeError::DuplicateStudy(study.study_id.clone()));
        }
        validate_text("modality", &study.modality)?;
        validate_text("comparability_key", &study.comparability_key)?;
        if study.quality_receipt.validate().is_err()
            || study.quality_receipt.dataset_id != study.study_id
            || study.quality_receipt.modality != study.modality
            || !study.quality_receipt.raw_data_local
        {
            return Err(QualityEnvelopeError::InvalidRequest(
                "study identity, comparability key, linked QC receipt, and locality are required"
                    .into(),
            ));
        }
    }
    validate_unique_strings("required_modalities", &request.required_modalities)?;
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

    #[test]
    fn profile_mismatch_cannot_qualify_the_envelope() {
        let mut value = request();
        for study in &mut value.studies {
            study.comparability_key = "protocol-v9|instrument-v9".into();
        }
        let receipt = evaluate_quality_envelope(&value).unwrap();
        assert_eq!(receipt.decision, QualityEnvelopeDecision::Blocked);
        assert!(receipt.modality_coverage.is_empty());
    }

    #[test]
    fn coverage_tampering_is_rejected() {
        let mut receipt = evaluate_quality_envelope(&request()).unwrap();
        receipt.modality_coverage.insert("imaging".into(), 99);
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn duplicate_required_modality_is_rejected() {
        let mut value = request();
        value.required_modalities.push("imaging".into());
        assert!(evaluate_quality_envelope(&value).is_err());
    }

    #[test]
    fn decision_state_tampering_is_rejected() {
        let mut receipt = evaluate_quality_envelope(&request()).unwrap();
        receipt.decision = QualityEnvelopeDecision::Partial;
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn verdict_payload_tampering_is_rejected() {
        let mut receipt = evaluate_quality_envelope(&request()).unwrap();
        receipt.verdicts[0].reasons.push("forged reason".into());
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn source_provenance_tampering_is_rejected() {
        let mut receipt = evaluate_quality_envelope(&request()).unwrap();
        receipt.artifact.provenance[0].digest = ContentHash::of_bytes(b"tampered");
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn retained_request_tampering_is_rejected() {
        let mut receipt = evaluate_quality_envelope(&request()).unwrap();
        receipt.input.comparability_profile = "profile:tampered".into();
        assert!(receipt.validate().is_err());
    }
}

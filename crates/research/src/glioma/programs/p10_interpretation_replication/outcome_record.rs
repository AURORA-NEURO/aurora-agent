//! Digest-bound typed registered-outcome evidence record for one local study.
//!
//! The detailed P10-F05 source blueprint is not configured in this checkout. This
//! repository-defined primitive validates one study's outcome identity, availability, reviewed
//! result fields, and local source artifacts. It performs no cross-study comparison or effect
//! interpretation.

use crate::glioma_engine::LocalArtifactRef;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P10-F05";
pub const OUTPUT_SCHEMA: &str = "GliomaRegisteredOutcomeRecord1@1";
pub const MAX_TEXT_BYTES: usize = 128;
pub const MAX_EFFECT_ABS_MILLI: u64 = 1_000_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeAvailability {
    Estimate,
    Null,
    Missing,
    Incomplete,
    Ambiguous,
    NotDue,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutcomeSensitivityStudy {
    pub study_id: String,
    pub independence_group: String,
    pub registry_report_digest: ContentHash,
    pub registry_artifact: LocalArtifactRef,
    pub result_report_digest: Option<ContentHash>,
    pub result_artifact: Option<LocalArtifactRef>,
    pub outcome_id: String,
    pub estimand_id: String,
    pub effect_unit: String,
    pub availability: OutcomeAvailability,
    pub effect_milli: Option<i64>,
    pub uncertainty_milli: Option<u64>,
    pub quality_milli: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegisteredOutcomeRecord {
    pub feature_id: String,
    pub output_schema: String,
    pub study: OutcomeSensitivityStudy,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum OutcomeRecordError {
    #[error("registered-outcome record is invalid: {0}")]
    InvalidRecord(String),
    #[error("registered-outcome record output is invalid: {0}")]
    InvalidOutput(String),
    #[error("registered-outcome record digest failed: {0}")]
    Digest(String),
}

fn valid_hash(hash: &ContentHash) -> bool {
    hash.as_str().len() == 64
}

fn artifact_is_local_and_deidentified(artifact: &LocalArtifactRef) -> bool {
    artifact.validate().is_ok()
        && artifact.local_only
        && !artifact.contains_human_data
        && !artifact.contains_direct_identifiers
}

pub(super) fn validate_record_content(
    study: &OutcomeSensitivityStudy,
) -> Result<(), OutcomeRecordError> {
    let result_shape_valid = match study.availability {
        OutcomeAvailability::Estimate | OutcomeAvailability::Null => {
            study.result_report_digest.is_some()
                && study.result_artifact.is_some()
                && study.effect_milli.is_some()
                && study.uncertainty_milli.is_some()
                && study.quality_milli.is_some()
        }
        OutcomeAvailability::Incomplete | OutcomeAvailability::Ambiguous => {
            study.result_report_digest.is_some()
                && study.result_artifact.is_some()
                && study.effect_milli.is_none()
                && study.uncertainty_milli.is_none()
                && study.quality_milli.is_none()
        }
        OutcomeAvailability::Missing | OutcomeAvailability::Unresolved => {
            study.effect_milli.is_none()
                && study.uncertainty_milli.is_none()
                && study.quality_milli.is_none()
                && study.result_report_digest.is_some() == study.result_artifact.is_some()
        }
        OutcomeAvailability::NotDue => {
            study.result_report_digest.is_none()
                && study.result_artifact.is_none()
                && study.effect_milli.is_none()
                && study.uncertainty_milli.is_none()
                && study.quality_milli.is_none()
        }
    };
    let result_artifact_valid = match (&study.result_report_digest, &study.result_artifact) {
        (Some(digest), Some(artifact)) => {
            valid_hash(digest) && artifact_is_local_and_deidentified(artifact)
        }
        (None, None) => true,
        _ => false,
    };

    if study.study_id.trim().is_empty()
        || study.study_id.len() > MAX_TEXT_BYTES
        || study.independence_group.trim().is_empty()
        || study.independence_group.len() > MAX_TEXT_BYTES
        || !valid_hash(&study.registry_report_digest)
        || !artifact_is_local_and_deidentified(&study.registry_artifact)
        || study.outcome_id.trim().is_empty()
        || study.outcome_id.len() > MAX_TEXT_BYTES
        || study.estimand_id.trim().is_empty()
        || study.estimand_id.len() > MAX_TEXT_BYTES
        || study.effect_unit.trim().is_empty()
        || study.effect_unit.len() > MAX_TEXT_BYTES
        || !result_shape_valid
        || !result_artifact_valid
        || study
            .effect_milli
            .is_some_and(|effect| effect.unsigned_abs() > MAX_EFFECT_ABS_MILLI)
        || study
            .uncertainty_milli
            .is_some_and(|uncertainty| uncertainty > MAX_EFFECT_ABS_MILLI)
        || study.quality_milli.is_some_and(|quality| quality > 1_000)
    {
        return Err(OutcomeRecordError::InvalidRecord(
            "one outcome record must bind a valid study, exact outcome/estimand/unit, status-specific result fields, and local de-identified source artifacts".into(),
        ));
    }
    Ok(())
}

#[derive(Serialize)]
struct RecordDigest<'a> {
    feature_id: &'a str,
    output_schema: &'a str,
    study: &'a OutcomeSensitivityStudy,
}

fn record_digest(study: &OutcomeSensitivityStudy) -> Result<ContentHash, OutcomeRecordError> {
    ContentHash::of_serializable(&RecordDigest {
        feature_id: FEATURE_ID,
        output_schema: OUTPUT_SCHEMA,
        study,
    })
    .map_err(|error| OutcomeRecordError::Digest(error.to_string()))
}

fn compile(study: &OutcomeSensitivityStudy) -> Result<RegisteredOutcomeRecord, OutcomeRecordError> {
    let mut record = RegisteredOutcomeRecord {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        study: study.clone(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-registered-outcome-record"),
    };
    record.digest = record_digest(study)?;
    Ok(record)
}

impl RegisteredOutcomeRecord {
    pub fn validate(&self) -> Result<(), OutcomeRecordError> {
        validate_record_content(&self.study)?;
        if self.feature_id != FEATURE_ID || self.output_schema != OUTPUT_SCHEMA {
            return Err(OutcomeRecordError::InvalidOutput(
                "feature identity or schema is invalid".into(),
            ));
        }
        if self != &compile(&self.study)? {
            return Err(OutcomeRecordError::InvalidOutput(
                "record data or digest does not replay from the bound single-study evidence".into(),
            ));
        }
        Ok(())
    }
}

/// Validate and seal one study-level registered-outcome evidence record.
pub fn build_glioma_registered_outcome_record(
    study: &OutcomeSensitivityStudy,
) -> Result<RegisteredOutcomeRecord, OutcomeRecordError> {
    validate_record_content(study)?;
    let record = compile(study)?;
    record.validate()?;
    Ok(record)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_bytes(label.as_bytes())
    }

    fn artifact(label: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: label.into(),
            content_hash: hash(label),
            content_type: "application/vnd.aurora.local-study-outcome+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn study(availability: OutcomeAvailability) -> OutcomeSensitivityStudy {
        let reports_result = matches!(
            availability,
            OutcomeAvailability::Estimate
                | OutcomeAvailability::Null
                | OutcomeAvailability::Incomplete
                | OutcomeAvailability::Ambiguous
        );
        let observed = matches!(
            availability,
            OutcomeAvailability::Estimate | OutcomeAvailability::Null
        );
        OutcomeSensitivityStudy {
            study_id: "study-a".into(),
            independence_group: "group-a".into(),
            registry_report_digest: hash("registry-a"),
            registry_artifact: artifact("registry-artifact-a"),
            result_report_digest: reports_result.then(|| hash("result-a")),
            result_artifact: reports_result.then(|| artifact("result-artifact-a")),
            outcome_id: "primary-viability".into(),
            estimand_id: "viability-change-at-24h".into(),
            effect_unit: "normalized-signal-milli".into(),
            availability,
            effect_milli: observed.then_some(200),
            uncertainty_milli: observed.then_some(20),
            quality_milli: observed.then_some(950),
        }
    }

    #[test]
    fn one_record_validates_status_specific_values_and_replays() {
        for availability in [
            OutcomeAvailability::Estimate,
            OutcomeAvailability::Null,
            OutcomeAvailability::Missing,
            OutcomeAvailability::Incomplete,
            OutcomeAvailability::Ambiguous,
            OutcomeAvailability::NotDue,
            OutcomeAvailability::Unresolved,
        ] {
            let study = study(availability);
            let record = build_glioma_registered_outcome_record(&study).unwrap();
            assert_eq!(record.feature_id, FEATURE_ID);
            record.validate().unwrap();
        }
    }

    #[test]
    fn identifiers_and_result_fields_cannot_be_tampered_after_sealing() {
        let mut record =
            build_glioma_registered_outcome_record(&study(OutcomeAvailability::Estimate)).unwrap();
        record.study.effect_milli = Some(-250);
        assert!(matches!(
            record.validate(),
            Err(OutcomeRecordError::InvalidOutput(_))
        ));
        let mut invalid = study(OutcomeAvailability::Missing);
        invalid.effect_milli = Some(42);
        assert!(matches!(
            build_glioma_registered_outcome_record(&invalid),
            Err(OutcomeRecordError::InvalidRecord(_))
        ));
    }

    #[test]
    fn source_artifacts_with_identifiers_fail_closed() {
        let mut invalid = study(OutcomeAvailability::Estimate);
        invalid.registry_artifact.contains_human_data = true;
        assert!(matches!(
            build_glioma_registered_outcome_record(&invalid),
            Err(OutcomeRecordError::InvalidRecord(_))
        ));
    }
}

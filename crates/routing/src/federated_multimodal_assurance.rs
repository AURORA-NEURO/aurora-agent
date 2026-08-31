//! Federated continual multimodal-ingestion assurance.
//!
//! Atlas feature: `AFA-routing-P06-F28`.
//!
//! This routing-owned gate composes the adapter harmonizer with federation and policy checks.
//! Institutions exchange manifests and digests only; raw imaging and omics bytes remain local.

use bioprism_adapter::{
    harmonize_multimodal, HarmonizationDecision, MultimodalHarmonizationRequest,
};
use bioprism_foundation::{
    PolicyDecision, TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-routing-P06-F28";
pub const CONTRACT_VERSION: &str = "federated-multimodal-assurance/1.0";
const ASSURANCE_CONTENT_TYPE: &str = "application/vnd.aurora.federated-multimodal-assurance+json";
const MAX_INSTITUTIONS: usize = 4096;
const MAX_TEXT_BYTES: usize = 512;
const MAX_LIST_ENTRIES: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedMultimodalAssuranceRequest {
    pub request_id: String,
    pub federation_id: String,
    pub institution_ids: Vec<String>,
    pub benchmark_id: String,
    pub harmonization: MultimodalHarmonizationRequest,
    pub policy_decision: PolicyDecision,
    pub protected_closure_satisfied: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedAssuranceDisposition {
    Passed,
    Blocked,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedMultimodalAssuranceReceipt {
    pub schema_version: String,
    pub feature_id: String,
    pub contract_version: String,
    pub request_id: String,
    pub federation_id: String,
    pub benchmark_id: String,
    pub institution_ids: Vec<String>,
    pub disposition: FederatedAssuranceDisposition,
    pub harmonized_digest: ContentHash,
    pub checks: Vec<String>,
    pub omissions: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

impl FederatedMultimodalAssuranceReceipt {
    pub fn validate(&self) -> Result<(), FederatedAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.feature_id != FEATURE_ID
            || self.contract_version != CONTRACT_VERSION
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.institution_ids.len() < 2
            || self.institution_ids.len() > MAX_INSTITUTIONS
            || self.checks.is_empty()
            || self.checks.len() > MAX_LIST_ENTRIES
            || self.omissions.len() > MAX_LIST_ENTRIES
        {
            return Err(FederatedAssuranceError::InvalidField(
                "identity, consortium, locality, or checks are incomplete".into(),
            ));
        }
        validate_text(&self.request_id, "request_id")?;
        validate_text(&self.federation_id, "federation_id")?;
        validate_text(&self.benchmark_id, "benchmark_id")?;
        validate_institution_ids(&self.institution_ids)?;
        let mut canonical_institution_ids = self.institution_ids.clone();
        canonical_institution_ids.sort();
        if canonical_institution_ids != self.institution_ids {
            return Err(FederatedAssuranceError::InvalidField(
                "institution_ids must use canonical sorted order".into(),
            ));
        }
        validate_text_list(&self.checks, "checks")?;
        validate_text_list(&self.omissions, "omissions")?;
        match self.disposition {
            FederatedAssuranceDisposition::Passed if !self.omissions.is_empty() => {
                return Err(FederatedAssuranceError::InvalidField(
                    "passed assurance cannot contain omissions".into(),
                ));
            }
            FederatedAssuranceDisposition::Blocked if !self.omissions.is_empty() => {
                return Err(FederatedAssuranceError::InvalidField(
                    "blocked assurance cannot contain harmonization omissions".into(),
                ));
            }
            FederatedAssuranceDisposition::Unknown if self.omissions.is_empty() => {
                return Err(FederatedAssuranceError::InvalidField(
                    "unknown assurance requires an omission".into(),
                ));
            }
            _ => {}
        }
        let expected_artifact_id = format!("federated-multimodal-assurance:{}", self.request_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != ASSURANCE_CONTENT_TYPE
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(FederatedAssuranceError::InvalidField(
                "assurance artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| FederatedAssuranceError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
            .map_err(|error| FederatedAssuranceError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, FederatedAssuranceError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| FederatedAssuranceError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| FederatedAssuranceError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum FederatedAssuranceError {
    #[error("invalid federated multimodal assurance field: {0}")]
    InvalidField(String),
    #[error("federated multimodal assurance artifact error: {0}")]
    Artifact(String),
    #[error("federated multimodal assurance harmonization failed: {0}")]
    Harmonization(String),
    #[error("federated multimodal assurance serialization error: {0}")]
    Serialization(String),
}

pub fn assure_federated_multimodal(
    request: &FederatedMultimodalAssuranceRequest,
) -> Result<FederatedMultimodalAssuranceReceipt, FederatedAssuranceError> {
    validate_request(request)?;
    let mut institution_ids = request.institution_ids.clone();
    institution_ids.sort();
    let harmonized = harmonize_multimodal(&request.harmonization)
        .map_err(|error| FederatedAssuranceError::Harmonization(error.to_string()))?;
    let harmonized_digest = harmonized
        .digest()
        .map_err(|error| FederatedAssuranceError::Harmonization(error.to_string()))?;
    let mut checks = vec![
        "federation has at least two distinct institutions".to_string(),
        "raw multimodal data remains institution-local".to_string(),
        "harmonized object digest is deterministic".to_string(),
    ];
    let mut omissions = Vec::new();
    let disposition = if request.policy_decision != PolicyDecision::Allow
        || !request.protected_closure_satisfied
    {
        checks.push("policy or protected closure prevented federation admission".into());
        FederatedAssuranceDisposition::Blocked
    } else if matches!(harmonized.decision, HarmonizationDecision::Partial)
        || !harmonized.omitted_modalities.is_empty()
        || !harmonized.semantic_loss.is_empty()
    {
        omissions.extend(
            harmonized
                .omitted_modalities
                .iter()
                .map(|modality| format!("required modality omitted or unresolved: {modality}")),
        );
        if !harmonized.semantic_loss.is_empty() {
            omissions.push("modality semantic loss remains bounded and must be reported".into());
        }
        checks.push("partial harmonization remains unknown rather than comparable".into());
        FederatedAssuranceDisposition::Unknown
    } else {
        checks.push("all federated harmonization and policy gates passed".into());
        FederatedAssuranceDisposition::Passed
    };
    omissions.sort();
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "feature_id": FEATURE_ID,
        "contract_version": CONTRACT_VERSION,
        "request_id": request.request_id,
        "federation_id": request.federation_id,
        "benchmark_id": request.benchmark_id,
        "institution_ids": institution_ids,
        "disposition": disposition,
        "harmonized_digest": harmonized_digest,
        "checks": checks,
        "omissions": omissions,
        "raw_data_local": true,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!("federated-multimodal-assurance:{}", request.request_id),
        ASSURANCE_CONTENT_TYPE,
        &payload,
        Vec::new(),
        vec![],
    )
    .map_err(|error| FederatedAssuranceError::Artifact(error.to_string()))?;
    let receipt = FederatedMultimodalAssuranceReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        contract_version: CONTRACT_VERSION.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        benchmark_id: request.benchmark_id.clone(),
        institution_ids,
        disposition,
        harmonized_digest,
        checks,
        omissions,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(
    request: &FederatedMultimodalAssuranceRequest,
) -> Result<(), FederatedAssuranceError> {
    if request.boundary != PRECLINICAL_BOUNDARY || !request.harmonization.raw_data_local {
        return Err(FederatedAssuranceError::InvalidField(
            "federation identity, institutions, locality, and boundary are required".into(),
        ));
    }
    validate_text(&request.request_id, "request_id")?;
    validate_text(&request.federation_id, "federation_id")?;
    validate_text(&request.benchmark_id, "benchmark_id")?;
    if request.institution_ids.len() < 2 || request.institution_ids.len() > MAX_INSTITUTIONS {
        return Err(FederatedAssuranceError::InvalidField(
            "institution_ids must contain between two and 4096 institutions".into(),
        ));
    }
    validate_institution_ids(&request.institution_ids)?;
    Ok(())
}

fn receipt_payload(receipt: &FederatedMultimodalAssuranceReceipt) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "feature_id": receipt.feature_id,
        "contract_version": receipt.contract_version,
        "request_id": receipt.request_id,
        "federation_id": receipt.federation_id,
        "benchmark_id": receipt.benchmark_id,
        "institution_ids": receipt.institution_ids,
        "disposition": receipt.disposition,
        "harmonized_digest": receipt.harmonized_digest,
        "checks": receipt.checks,
        "omissions": receipt.omissions,
        "raw_data_local": receipt.raw_data_local,
        "boundary": receipt.boundary,
    })
}

fn validate_text(value: &str, field: &str) -> Result<(), FederatedAssuranceError> {
    if value.trim().is_empty()
        || value != value.trim()
        || value.len() > MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(FederatedAssuranceError::InvalidField(format!(
            "{field} must be bounded, non-empty text without surrounding whitespace or control characters"
        )));
    }
    Ok(())
}

fn validate_institution_ids(values: &[String]) -> Result<(), FederatedAssuranceError> {
    let mut seen = BTreeSet::new();
    for value in values {
        validate_text(value, "institution_ids")?;
        if !seen.insert(value.to_ascii_lowercase()) {
            return Err(FederatedAssuranceError::InvalidField(
                "institution_ids must not contain duplicate or case-colliding values".into(),
            ));
        }
    }
    Ok(())
}

fn validate_text_list(values: &[String], field: &str) -> Result<(), FederatedAssuranceError> {
    let mut seen = BTreeSet::new();
    for value in values {
        validate_text(value, field)?;
        if !seen.insert(value.to_ascii_lowercase()) {
            return Err(FederatedAssuranceError::InvalidField(format!(
                "{field} must not contain duplicate or case-colliding values"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bioprism_adapter::ModalityManifest;
    use std::collections::BTreeMap;

    fn request(required_modalities: Vec<String>) -> FederatedMultimodalAssuranceRequest {
        FederatedMultimodalAssuranceRequest {
            request_id: "request:federated".into(),
            federation_id: "federation:preclinical".into(),
            institution_ids: vec!["site:a".into(), "site:b".into()],
            benchmark_id: "benchmark:multimodal".into(),
            harmonization: MultimodalHarmonizationRequest {
                study_id: "study:organoid".into(),
                reference_schema: "ome-ngff:0.5".into(),
                modalities: vec![ModalityManifest {
                    modality_id: "image:a".into(),
                    modality_type: "imaging".into(),
                    schema_version: "ome-ngff:0.5".into(),
                    source_digest: ContentHash::of_bytes(b"image"),
                    units: BTreeMap::new(),
                    feature_names: vec!["nucleus".into()],
                    coordinate_system: Some("micron".into()),
                    qc_digest: Some(ContentHash::of_bytes(b"qc")),
                }],
                required_modalities,
                raw_data_local: true,
            },
            policy_decision: PolicyDecision::Allow,
            protected_closure_satisfied: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn missing_required_modality_is_unknown_not_passed() {
        let receipt =
            assure_federated_multimodal(&request(vec!["imaging".into(), "rna".into()])).unwrap();
        assert_eq!(receipt.disposition, FederatedAssuranceDisposition::Unknown);
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }

    #[test]
    fn denied_policy_blocks_federation() {
        let mut input = request(vec!["imaging".into()]);
        input.policy_decision = PolicyDecision::Deny;
        let receipt = assure_federated_multimodal(&input).unwrap();
        assert_eq!(receipt.disposition, FederatedAssuranceDisposition::Blocked);
    }

    #[test]
    fn assurance_canonicalizes_institutions_and_binds_artifact_payload() {
        let mut input = request(vec!["imaging".into()]);
        input.institution_ids = vec!["site:b".into(), "site:a".into()];
        let receipt = assure_federated_multimodal(&input).unwrap();
        assert_eq!(receipt.institution_ids, vec!["site:a", "site:b"]);
        receipt.validate().unwrap();

        let mut reordered = receipt.clone();
        reordered.institution_ids.reverse();
        assert!(reordered.validate().is_err());

        let mut payload_drift = receipt;
        payload_drift.checks.push("unbound check".into());
        assert!(payload_drift.validate().is_err());
    }

    #[test]
    fn assurance_rejects_case_collisions_padding_and_disposition_omission_drift() {
        let mut input = request(vec!["imaging".into()]);
        input.institution_ids = vec!["site:a".into(), "SITE:A".into()];
        assert!(assure_federated_multimodal(&input).is_err());

        let mut receipt =
            assure_federated_multimodal(&request(vec!["imaging".into(), "rna".into()])).unwrap();
        receipt.request_id = " request:federated".into();
        assert!(receipt.validate().is_err());

        let mut receipt =
            assure_federated_multimodal(&request(vec!["imaging".into(), "rna".into()])).unwrap();
        receipt.disposition = FederatedAssuranceDisposition::Passed;
        assert!(receipt.validate().is_err());
    }
}

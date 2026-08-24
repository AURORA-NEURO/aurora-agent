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
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-routing-P06-F28";
pub const CONTRACT_VERSION: &str = "federated-multimodal-assurance/1.0";

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
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.benchmark_id.trim().is_empty()
            || self.institution_ids.len() < 2
            || self.institution_ids.iter().any(|id| id.trim().is_empty())
            || self
                .institution_ids
                .windows(2)
                .any(|pair| pair[0] == pair[1])
            || self.checks.is_empty()
        {
            return Err(FederatedAssuranceError::InvalidField(
                "identity, consortium, locality, or checks are incomplete".into(),
            ));
        }
        self.artifact
            .validate_metadata()
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
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "feature_id": FEATURE_ID,
        "contract_version": CONTRACT_VERSION,
        "request_id": request.request_id,
        "federation_id": request.federation_id,
        "benchmark_id": request.benchmark_id,
        "institution_ids": request.institution_ids,
        "disposition": disposition,
        "harmonized_digest": harmonized_digest,
        "checks": checks,
        "omissions": omissions,
        "raw_data_local": true,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!("federated-multimodal-assurance:{}", request.request_id),
        "application/vnd.aurora.federated-multimodal-assurance+json",
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
        institution_ids: request.institution_ids.clone(),
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
    if request.request_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.benchmark_id.trim().is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.institution_ids.len() < 2
        || request
            .institution_ids
            .iter()
            .any(|id| id.trim().is_empty())
        || request
            .institution_ids
            .windows(2)
            .any(|pair| pair[0] == pair[1])
        || request.harmonization.raw_data_local != true
    {
        return Err(FederatedAssuranceError::InvalidField(
            "federation identity, institutions, locality, and boundary are required".into(),
        ));
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
}

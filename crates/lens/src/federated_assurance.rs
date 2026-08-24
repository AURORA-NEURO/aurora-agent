//! Federated continual knowledge-representation assurance for lens reports.
//!
//! Atlas feature: `AFA-lens-P04-F28`.
//!
//! This gate exchanges only content-addressed lens-report receipts and omission manifests. It
//! prevents a consortium from treating an institution that never ran a required lens as though
//! it supplied negative evidence.

use bioprism_foundation::{
    PolicyDecision, TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-lens-P04-F28";
pub const CONTRACT_VERSION: &str = "federated-lens-assurance/1.0";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederatedLensAssuranceRequest {
    pub request_id: String,
    pub federation_id: String,
    pub institution_ids: Vec<String>,
    pub required_lens_ids: Vec<String>,
    pub report_digests: Vec<ContentHash>,
    pub absent_lens_ids: Vec<String>,
    pub policy_decision: PolicyDecision,
    pub protected_closure_satisfied: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedLensDisposition {
    Passed,
    Blocked,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederatedLensAssuranceReceipt {
    pub schema_version: String,
    pub feature_id: String,
    pub contract_version: String,
    pub request_id: String,
    pub federation_id: String,
    pub institution_ids: Vec<String>,
    pub required_lens_ids: Vec<String>,
    pub report_digests: Vec<ContentHash>,
    pub absent_lens_ids: Vec<String>,
    pub disposition: FederatedLensDisposition,
    pub checks: Vec<String>,
    pub omissions: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub boundary: String,
}

impl FederatedLensAssuranceReceipt {
    pub fn validate(&self) -> Result<(), FederatedLensError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.feature_id != FEATURE_ID
            || self.contract_version != CONTRACT_VERSION
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.institution_ids.len() < 2
            || self
                .institution_ids
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self.required_lens_ids.is_empty()
            || self.checks.is_empty()
        {
            return Err(FederatedLensError::InvalidField(
                "lens federation identity, ordering, required set, or checks are incomplete".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| FederatedLensError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, FederatedLensError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| FederatedLensError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| FederatedLensError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum FederatedLensError {
    #[error("invalid federated lens assurance field: {0}")]
    InvalidField(String),
    #[error("federated lens assurance artifact error: {0}")]
    Artifact(String),
    #[error("federated lens assurance serialization error: {0}")]
    Serialization(String),
}

pub fn assure_federated_lens(
    request: &FederatedLensAssuranceRequest,
) -> Result<FederatedLensAssuranceReceipt, FederatedLensError> {
    validate_request(request)?;
    let mut required_lens_ids = request.required_lens_ids.clone();
    required_lens_ids.sort();
    required_lens_ids.dedup();
    let mut absent_lens_ids = request.absent_lens_ids.clone();
    absent_lens_ids.sort();
    absent_lens_ids.dedup();
    let report_digests = request.report_digests.clone();
    let mut checks = vec![
        "institution set is canonical and distinct".to_string(),
        "lens reports are exchanged by content digest".to_string(),
    ];
    let mut omissions = Vec::new();
    let disposition = if request.policy_decision != PolicyDecision::Allow
        || !request.protected_closure_satisfied
    {
        checks.push("policy or protected closure prevented lens federation".into());
        FederatedLensDisposition::Blocked
    } else if report_digests.is_empty() || !absent_lens_ids.is_empty() {
        omissions.extend(
            absent_lens_ids
                .iter()
                .map(|lens| format!("required lens not run: {lens}")),
        );
        if report_digests.is_empty() {
            omissions.push("no federated lens report digests were supplied".into());
        }
        checks.push("missing lens evidence remains unknown rather than negative".into());
        FederatedLensDisposition::Unknown
    } else {
        checks.push("required lens report receipts passed federation admission".into());
        FederatedLensDisposition::Passed
    };
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "feature_id": FEATURE_ID,
        "contract_version": CONTRACT_VERSION,
        "request_id": request.request_id,
        "federation_id": request.federation_id,
        "institution_ids": request.institution_ids,
        "required_lens_ids": required_lens_ids,
        "report_digests": report_digests,
        "absent_lens_ids": absent_lens_ids,
        "disposition": disposition,
        "checks": checks,
        "omissions": omissions,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!("federated-lens-assurance:{}", request.request_id),
        "application/vnd.aurora.federated-lens-assurance+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| FederatedLensError::Artifact(error.to_string()))?;
    let receipt = FederatedLensAssuranceReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        contract_version: CONTRACT_VERSION.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        institution_ids: {
            let mut ids = request.institution_ids.clone();
            ids.sort();
            ids
        },
        required_lens_ids,
        report_digests,
        absent_lens_ids,
        disposition,
        checks,
        omissions,
        artifact,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &FederatedLensAssuranceRequest) -> Result<(), FederatedLensError> {
    if request.request_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.institution_ids.len() < 2
        || request
            .institution_ids
            .iter()
            .any(|id| id.trim().is_empty())
        || request.required_lens_ids.is_empty()
    {
        return Err(FederatedLensError::InvalidField(
            "federated lens identity, institutions, required lenses, and boundary are required"
                .into(),
        ));
    }
    if request
        .institution_ids
        .windows(2)
        .any(|pair| pair[0] == pair[1])
    {
        return Err(FederatedLensError::InvalidField(
            "institution ids must be unique".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> FederatedLensAssuranceRequest {
        FederatedLensAssuranceRequest {
            request_id: "request:lens".into(),
            federation_id: "federation:lens".into(),
            institution_ids: vec!["site:b".into(), "site:a".into()],
            required_lens_ids: vec!["42.13.qc".into()],
            report_digests: vec![],
            absent_lens_ids: vec!["42.13.qc".into()],
            policy_decision: PolicyDecision::Allow,
            protected_closure_satisfied: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn missing_lens_is_unknown_not_negative() {
        let receipt = assure_federated_lens(&request()).unwrap();
        assert_eq!(receipt.disposition, FederatedLensDisposition::Unknown);
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }

    #[test]
    fn denied_lens_federation_blocks() {
        let mut input = request();
        input.report_digests = vec![ContentHash::of_bytes(b"report")];
        input.absent_lens_ids.clear();
        input.policy_decision = PolicyDecision::Deny;
        let receipt = assure_federated_lens(&input).unwrap();
        assert_eq!(receipt.disposition, FederatedLensDisposition::Blocked);
    }
}

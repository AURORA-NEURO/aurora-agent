//! Prospective high-throughput research-object release assurance.
//!
//! Atlas feature: `AFA-weavelang-P16-F27`.

use bioprism_foundation::{
    PolicyDecision, TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-weavelang-P16-F27";
pub const CONTRACT_VERSION: &str = "weavelang-release-assurance/1.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeaveLangReleaseAssuranceRequest {
    pub request_id: String,
    pub run_id: String,
    pub release_id: String,
    pub artifact_digest: Option<ContentHash>,
    pub evidence_receipt_ids: Vec<String>,
    pub provenance_links: Vec<String>,
    pub policy_decision: PolicyDecision,
    pub protected_closure_satisfied: bool,
    pub authority_reference: Option<String>,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseAssuranceDisposition {
    Passed,
    Blocked,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeaveLangReleaseAssuranceReceipt {
    pub schema_version: String,
    pub feature_id: String,
    pub contract_version: String,
    pub request_id: String,
    pub run_id: String,
    pub release_id: String,
    pub disposition: ReleaseAssuranceDisposition,
    pub artifact_digest: Option<ContentHash>,
    pub checks: Vec<String>,
    pub omissions: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub boundary: String,
}

impl WeaveLangReleaseAssuranceReceipt {
    pub fn validate(&self) -> Result<(), ReleaseAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.feature_id != FEATURE_ID
            || self.contract_version != CONTRACT_VERSION
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.request_id.trim().is_empty()
            || self.run_id.trim().is_empty()
            || self.release_id.trim().is_empty()
            || self.checks.is_empty()
        {
            return Err(ReleaseAssuranceError::InvalidField(
                "release identity, boundary, or checks are incomplete".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ReleaseAssuranceError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, ReleaseAssuranceError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ReleaseAssuranceError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ReleaseAssuranceError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum ReleaseAssuranceError {
    #[error("invalid release assurance field: {0}")]
    InvalidField(String),
    #[error("release assurance artifact error: {0}")]
    Artifact(String),
    #[error("release assurance serialization error: {0}")]
    Serialization(String),
}

pub fn assure_weavelang_release(
    request: &WeaveLangReleaseAssuranceRequest,
) -> Result<WeaveLangReleaseAssuranceReceipt, ReleaseAssuranceError> {
    validate_request(request)?;
    let mut checks = vec![
        "release and run identities are canonicalized".to_string(),
        "raw preclinical data remains local to the originating institution".to_string(),
    ];
    let mut omissions = Vec::new();
    let disposition = if request.policy_decision != PolicyDecision::Allow
        || !request.protected_closure_satisfied
        || !request.raw_data_local
    {
        checks.push("policy, locality, or protected closure blocked release admission".into());
        ReleaseAssuranceDisposition::Blocked
    } else if request.artifact_digest.is_none()
        || request.evidence_receipt_ids.is_empty()
        || request.provenance_links.is_empty()
        || request.authority_reference.is_none()
    {
        if request.artifact_digest.is_none() {
            omissions.push("content-addressed release artifact digest is absent".into());
        }
        if request.evidence_receipt_ids.is_empty() {
            omissions.push("evidence receipts are absent".into());
        }
        if request.provenance_links.is_empty() {
            omissions.push("provenance links are absent".into());
        }
        if request.authority_reference.is_none() {
            omissions.push("release authority reference is absent".into());
        }
        checks.push("incomplete release closure remains unknown rather than published".into());
        ReleaseAssuranceDisposition::Unknown
    } else {
        checks.push("artifact, evidence, provenance, locality, and authority gates passed".into());
        ReleaseAssuranceDisposition::Passed
    };
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "feature_id": FEATURE_ID,
        "contract_version": CONTRACT_VERSION,
        "request_id": request.request_id,
        "run_id": request.run_id,
        "release_id": request.release_id,
        "disposition": disposition,
        "artifact_digest": request.artifact_digest,
        "checks": checks,
        "omissions": omissions,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!("weavelang-release-assurance:{}", request.release_id),
        "application/vnd.aurora.weavelang-release-assurance+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ReleaseAssuranceError::Artifact(error.to_string()))?;
    let receipt = WeaveLangReleaseAssuranceReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        contract_version: CONTRACT_VERSION.into(),
        request_id: request.request_id.clone(),
        run_id: request.run_id.clone(),
        release_id: request.release_id.clone(),
        disposition,
        artifact_digest: request.artifact_digest.clone(),
        checks,
        omissions,
        artifact,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(
    request: &WeaveLangReleaseAssuranceRequest,
) -> Result<(), ReleaseAssuranceError> {
    if request.request_id.trim().is_empty()
        || request.run_id.trim().is_empty()
        || request.release_id.trim().is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(ReleaseAssuranceError::InvalidField(
            "release identity and boundary are required".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn incomplete_release_is_unknown_not_published() {
        let receipt = assure_weavelang_release(&WeaveLangReleaseAssuranceRequest {
            request_id: "request:release".into(),
            run_id: "run:high-throughput".into(),
            release_id: "release:2026".into(),
            artifact_digest: None,
            evidence_receipt_ids: vec![],
            provenance_links: vec![],
            policy_decision: PolicyDecision::Allow,
            protected_closure_satisfied: true,
            authority_reference: None,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        })
        .unwrap();
        assert_eq!(receipt.disposition, ReleaseAssuranceDisposition::Unknown);
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
}

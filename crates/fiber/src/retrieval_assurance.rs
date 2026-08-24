//! Federated continual retrieval-and-synthesis assurance.
//!
//! Atlas feature: `AFA-fiber-P02-F28`.

use bioprism_foundation::{
    PolicyDecision, TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-fiber-P02-F28";
pub const CONTRACT_VERSION: &str = "federated-retrieval-assurance/1.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedRetrievalAssuranceRequest {
    pub request_id: String,
    pub federation_id: String,
    pub query_id: String,
    pub requested_source_ids: Vec<String>,
    pub returned_source_ids: Vec<String>,
    pub evidence_receipt_digest: Option<ContentHash>,
    pub policy_decision: PolicyDecision,
    pub protected_closure_satisfied: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetrievalAssuranceDisposition {
    Passed,
    Blocked,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedRetrievalAssuranceReceipt {
    pub schema_version: String,
    pub feature_id: String,
    pub contract_version: String,
    pub request_id: String,
    pub federation_id: String,
    pub query_id: String,
    pub returned_source_ids: Vec<String>,
    pub disposition: RetrievalAssuranceDisposition,
    pub evidence_receipt_digest: Option<ContentHash>,
    pub checks: Vec<String>,
    pub omissions: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub boundary: String,
}

impl FederatedRetrievalAssuranceReceipt {
    pub fn validate(&self) -> Result<(), RetrievalAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.feature_id != FEATURE_ID
            || self.contract_version != CONTRACT_VERSION
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.query_id.trim().is_empty()
            || self.checks.is_empty()
        {
            return Err(RetrievalAssuranceError::InvalidField(
                "retrieval identity, boundary, or checks are incomplete".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| RetrievalAssuranceError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, RetrievalAssuranceError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| RetrievalAssuranceError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| RetrievalAssuranceError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum RetrievalAssuranceError {
    #[error("invalid federated retrieval assurance field: {0}")]
    InvalidField(String),
    #[error("federated retrieval assurance artifact error: {0}")]
    Artifact(String),
    #[error("federated retrieval assurance serialization error: {0}")]
    Serialization(String),
}

pub fn assure_federated_retrieval(
    request: &FederatedRetrievalAssuranceRequest,
) -> Result<FederatedRetrievalAssuranceReceipt, RetrievalAssuranceError> {
    validate_request(request)?;
    let mut returned_source_ids = request.returned_source_ids.clone();
    returned_source_ids.sort();
    returned_source_ids.dedup();
    let mut checks = vec![
        "returned source identities are canonicalized".to_string(),
        "raw source content remains institution-local".to_string(),
    ];
    let mut omissions = Vec::new();
    let missing = request
        .requested_source_ids
        .iter()
        .filter(|source| !returned_source_ids.contains(source))
        .cloned()
        .collect::<Vec<_>>();
    let disposition = if request.policy_decision != PolicyDecision::Allow
        || !request.protected_closure_satisfied
    {
        checks.push("policy or protected closure prevented retrieval admission".into());
        RetrievalAssuranceDisposition::Blocked
    } else if request.evidence_receipt_digest.is_none() || !missing.is_empty() {
        omissions.extend(
            missing
                .into_iter()
                .map(|source| format!("requested source unavailable: {source}")),
        );
        if request.evidence_receipt_digest.is_none() {
            omissions.push("evidence derivation receipt is absent".into());
        }
        checks.push("missing retrieval evidence remains unknown rather than synthesized".into());
        RetrievalAssuranceDisposition::Unknown
    } else {
        checks.push("requested sources and evidence derivation receipt passed".into());
        RetrievalAssuranceDisposition::Passed
    };
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "feature_id": FEATURE_ID,
        "contract_version": CONTRACT_VERSION,
        "request_id": request.request_id,
        "federation_id": request.federation_id,
        "query_id": request.query_id,
        "returned_source_ids": returned_source_ids,
        "disposition": disposition,
        "evidence_receipt_digest": request.evidence_receipt_digest,
        "checks": checks,
        "omissions": omissions,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!("federated-retrieval-assurance:{}", request.request_id),
        "application/vnd.aurora.federated-retrieval-assurance+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| RetrievalAssuranceError::Artifact(error.to_string()))?;
    let receipt = FederatedRetrievalAssuranceReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        contract_version: CONTRACT_VERSION.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        query_id: request.query_id.clone(),
        returned_source_ids,
        disposition,
        evidence_receipt_digest: request.evidence_receipt_digest.clone(),
        checks,
        omissions,
        artifact,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(
    request: &FederatedRetrievalAssuranceRequest,
) -> Result<(), RetrievalAssuranceError> {
    if request.request_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.query_id.trim().is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.requested_source_ids.is_empty()
    {
        return Err(RetrievalAssuranceError::InvalidField(
            "retrieval identity, requested sources, and boundary are required".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> FederatedRetrievalAssuranceRequest {
        FederatedRetrievalAssuranceRequest {
            request_id: "request:retrieval".into(),
            federation_id: "federation:evidence".into(),
            query_id: "query:mechanism".into(),
            requested_source_ids: vec!["source:a".into(), "source:b".into()],
            returned_source_ids: vec!["source:a".into()],
            evidence_receipt_digest: None,
            policy_decision: PolicyDecision::Allow,
            protected_closure_satisfied: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn missing_source_is_unknown_not_synthesized() {
        let receipt = assure_federated_retrieval(&request()).unwrap();
        assert_eq!(receipt.disposition, RetrievalAssuranceDisposition::Unknown);
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
}

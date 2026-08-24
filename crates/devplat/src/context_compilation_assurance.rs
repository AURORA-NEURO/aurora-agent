//! Federated continual decision-context compilation assurance.
//!
//! Atlas feature: `AFA-devplat-P03-F28`.
//!
//! The harness verifies a caller-supplied context compilation receipt without executing a
//! compiler or moving institution-local records. Missing context, missing derivation, policy
//! denial, and incomplete protected closure are typed outcomes; none can be upgraded to a
//! certified decision section by an agent or connector.

use bioprism_foundation::{
    PolicyDecision, TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-devplat-P03-F28";
pub const CONTRACT_VERSION: &str = "federated-context-compilation-assurance/1.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextCompilationAssuranceRequest {
    pub request_id: String,
    pub federation_id: String,
    pub query_id: String,
    pub required_context_ids: Vec<String>,
    pub resolved_context_ids: Vec<String>,
    pub evidence_receipt_digest: Option<ContentHash>,
    pub policy_decision: PolicyDecision,
    pub protected_closure_satisfied: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextAssuranceDisposition {
    Passed,
    Blocked,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextCompilationAssuranceReceipt {
    pub schema_version: String,
    pub feature_id: String,
    pub contract_version: String,
    pub request_id: String,
    pub federation_id: String,
    pub query_id: String,
    pub resolved_context_ids: Vec<String>,
    pub disposition: ContextAssuranceDisposition,
    pub evidence_receipt_digest: Option<ContentHash>,
    pub checks: Vec<String>,
    pub omissions: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub boundary: String,
}

impl ContextCompilationAssuranceReceipt {
    pub fn validate(&self) -> Result<(), ContextAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.feature_id != FEATURE_ID
            || self.contract_version != CONTRACT_VERSION
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.query_id.trim().is_empty()
            || self.checks.is_empty()
        {
            return Err(ContextAssuranceError::InvalidField(
                "context assurance identity, boundary, or checks are incomplete".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ContextAssuranceError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, ContextAssuranceError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ContextAssuranceError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ContextAssuranceError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum ContextAssuranceError {
    #[error("invalid context assurance field: {0}")]
    InvalidField(String),
    #[error("context assurance artifact error: {0}")]
    Artifact(String),
    #[error("context assurance serialization error: {0}")]
    Serialization(String),
}

pub fn assure_context_compilation(
    request: &ContextCompilationAssuranceRequest,
) -> Result<ContextCompilationAssuranceReceipt, ContextAssuranceError> {
    validate_request(request)?;
    let mut resolved_context_ids = request.resolved_context_ids.clone();
    resolved_context_ids.sort();
    resolved_context_ids.dedup();
    let missing = request
        .required_context_ids
        .iter()
        .filter(|context| !resolved_context_ids.contains(context))
        .cloned()
        .collect::<Vec<_>>();
    let mut checks = vec![
        "context identities are canonicalized before certification".to_string(),
        "raw institution-local records remain outside the federation envelope".to_string(),
    ];
    let mut omissions = Vec::new();
    let disposition = if request.policy_decision != PolicyDecision::Allow
        || !request.protected_closure_satisfied
    {
        checks.push("policy or protected closure prevented context certification".into());
        ContextAssuranceDisposition::Blocked
    } else if request.evidence_receipt_digest.is_none() || !missing.is_empty() {
        omissions.extend(
            missing
                .iter()
                .map(|context| format!("required context unavailable: {context}")),
        );
        if request.evidence_receipt_digest.is_none() {
            omissions.push("context derivation receipt is absent".into());
        }
        checks.push("incomplete context remains unknown rather than certified".into());
        ContextAssuranceDisposition::Unknown
    } else {
        checks.push("required context and derivation receipt passed".into());
        ContextAssuranceDisposition::Passed
    };
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "feature_id": FEATURE_ID,
        "contract_version": CONTRACT_VERSION,
        "request_id": request.request_id,
        "federation_id": request.federation_id,
        "query_id": request.query_id,
        "resolved_context_ids": resolved_context_ids,
        "disposition": disposition,
        "evidence_receipt_digest": request.evidence_receipt_digest,
        "checks": checks,
        "omissions": omissions,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!("context-compilation-assurance:{}", request.request_id),
        "application/vnd.aurora.context-compilation-assurance+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ContextAssuranceError::Artifact(error.to_string()))?;
    let receipt = ContextCompilationAssuranceReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        contract_version: CONTRACT_VERSION.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        query_id: request.query_id.clone(),
        resolved_context_ids,
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
    request: &ContextCompilationAssuranceRequest,
) -> Result<(), ContextAssuranceError> {
    if request.request_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.query_id.trim().is_empty()
        || request.required_context_ids.is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(ContextAssuranceError::InvalidField(
            "context assurance identity, required context, and boundary are required".into(),
        ));
    }
    let mut seen = std::collections::BTreeSet::new();
    for context in &request.required_context_ids {
        if context.trim().is_empty() || !seen.insert(context.clone()) {
            return Err(ContextAssuranceError::InvalidField(
                "required context identities must be non-empty and unique".into(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_context_is_unknown_not_certified() {
        let receipt = assure_context_compilation(&ContextCompilationAssuranceRequest {
            request_id: "request:context".into(),
            federation_id: "federation:context".into(),
            query_id: "query:mechanism".into(),
            required_context_ids: vec!["context:a".into(), "context:b".into()],
            resolved_context_ids: vec!["context:a".into()],
            evidence_receipt_digest: None,
            policy_decision: PolicyDecision::Allow,
            protected_closure_satisfied: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        })
        .unwrap();
        assert_eq!(receipt.disposition, ContextAssuranceDisposition::Unknown);
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
}

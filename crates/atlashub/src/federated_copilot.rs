//! Federated continual retrieval and synthesis assurance.
//!
//! Atlas feature: `AFA-atlashub-P02-F12`.
//!
//! This is a product boundary for an evidence copilot, not a claim generator. Institutions send
//! source metadata and signed digests; raw experimental or source bytes remain local. The receipt
//! makes stale sources, missing baselines, policy blocks, and unresolved evidence explicit so a
//! continual synthesis cannot silently turn an incomplete refresh into a conclusion.

use bioprism_foundation::{
    PolicyDecision, TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-atlashub-P02-F12";
pub const CONTRACT_VERSION: &str = "federated-continual-retrieval-copilot/1.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrievalSourceUpdate {
    pub source_id: String,
    pub version: String,
    pub digest: String,
    pub evidence_state: String,
    pub stale: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContinualRetrievalRequest {
    pub request_id: String,
    pub federation_id: String,
    pub query_id: String,
    pub source_updates: Vec<RetrievalSourceUpdate>,
    pub prior_synthesis_digest: Option<ContentHash>,
    pub policy_decision: PolicyDecision,
    pub protected_closure_satisfied: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContinualSynthesisDisposition {
    Passed,
    Blocked,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContinualRetrievalReceipt {
    pub schema_version: String,
    pub feature_id: String,
    pub contract_version: String,
    pub request_id: String,
    pub federation_id: String,
    pub query_id: String,
    pub selected_source_ids: Vec<String>,
    pub stale_source_ids: Vec<String>,
    pub disposition: ContinualSynthesisDisposition,
    pub prior_synthesis_digest: Option<ContentHash>,
    pub checks: Vec<String>,
    pub omissions: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub boundary: String,
}

impl FederatedContinualRetrievalReceipt {
    pub fn validate(&self) -> Result<(), ContinualSynthesisError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.feature_id != FEATURE_ID
            || self.contract_version != CONTRACT_VERSION
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.query_id.trim().is_empty()
            || self.checks.is_empty()
        {
            return Err(ContinualSynthesisError::InvalidField(
                "continual retrieval identity, boundary, or checks are incomplete".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ContinualSynthesisError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, ContinualSynthesisError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ContinualSynthesisError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ContinualSynthesisError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum ContinualSynthesisError {
    #[error("invalid continual retrieval field: {0}")]
    InvalidField(String),
    #[error("continual retrieval artifact error: {0}")]
    Artifact(String),
    #[error("continual retrieval serialization error: {0}")]
    Serialization(String),
}

pub fn synthesize_federated_continuum(
    request: &FederatedContinualRetrievalRequest,
) -> Result<FederatedContinualRetrievalReceipt, ContinualSynthesisError> {
    validate_request(request)?;
    let mut updates = request.source_updates.clone();
    updates.sort_by(|left, right| left.source_id.cmp(&right.source_id));
    let selected_source_ids = updates
        .iter()
        .map(|item| item.source_id.clone())
        .collect::<Vec<_>>();
    let stale_source_ids = updates
        .iter()
        .filter(|item| item.stale)
        .map(|item| item.source_id.clone())
        .collect::<Vec<_>>();
    let mut checks = vec![
        "source metadata is canonicalized by source identity".to_string(),
        "raw source and experimental content remain institution-local".to_string(),
        "continual synthesis is anchored to an immutable prior digest".to_string(),
    ];
    let mut omissions = Vec::new();
    let disposition = if request.policy_decision != PolicyDecision::Allow
        || !request.protected_closure_satisfied
    {
        checks.push("policy or protected closure prevented synthesis admission".into());
        ContinualSynthesisDisposition::Blocked
    } else if request.prior_synthesis_digest.is_none() || !stale_source_ids.is_empty() {
        if request.prior_synthesis_digest.is_none() {
            omissions.push("prior synthesis digest is absent; change detection is unknown".into());
        }
        omissions.extend(
            stale_source_ids
                .iter()
                .map(|source| format!("source update is stale or unresolved: {source}")),
        );
        checks.push("stale or unanchored evidence remains unknown rather than synthesized".into());
        ContinualSynthesisDisposition::Unknown
    } else {
        checks.push("source updates and prior synthesis anchor passed".into());
        ContinualSynthesisDisposition::Passed
    };
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "feature_id": FEATURE_ID,
        "contract_version": CONTRACT_VERSION,
        "request_id": request.request_id,
        "federation_id": request.federation_id,
        "query_id": request.query_id,
        "selected_source_ids": selected_source_ids,
        "stale_source_ids": stale_source_ids,
        "disposition": disposition,
        "prior_synthesis_digest": request.prior_synthesis_digest,
        "checks": checks,
        "omissions": omissions,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!("federated-continual-retrieval: {}", request.request_id),
        "application/vnd.aurora.federated-continual-retrieval+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ContinualSynthesisError::Artifact(error.to_string()))?;
    let receipt = FederatedContinualRetrievalReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        contract_version: CONTRACT_VERSION.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        query_id: request.query_id.clone(),
        selected_source_ids,
        stale_source_ids,
        disposition,
        prior_synthesis_digest: request.prior_synthesis_digest.clone(),
        checks,
        omissions,
        artifact,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(
    request: &FederatedContinualRetrievalRequest,
) -> Result<(), ContinualSynthesisError> {
    if request.request_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.query_id.trim().is_empty()
        || request.source_updates.is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(ContinualSynthesisError::InvalidField(
            "continual retrieval identity, source updates, and boundary are required".into(),
        ));
    }
    let mut seen = std::collections::BTreeSet::new();
    for update in &request.source_updates {
        if update.source_id.trim().is_empty()
            || update.version.trim().is_empty()
            || update.evidence_state.trim().is_empty()
            || !seen.insert(update.source_id.clone())
            || update.digest.len() != 64
            || !update
                .digest
                .chars()
                .all(|character| character.is_ascii_hexdigit())
        {
            return Err(ContinualSynthesisError::InvalidField(
                "source updates require unique identities, versions, evidence states, and sha256 digests".into(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> FederatedContinualRetrievalRequest {
        FederatedContinualRetrievalRequest {
            request_id: "request:continuum".into(),
            federation_id: "federation:evidence".into(),
            query_id: "query:mechanism".into(),
            source_updates: vec![RetrievalSourceUpdate {
                source_id: "source:a".into(),
                version: "2026-08".into(),
                digest: "a".repeat(64),
                evidence_state: "supported".into(),
                stale: true,
            }],
            prior_synthesis_digest: None,
            policy_decision: PolicyDecision::Allow,
            protected_closure_satisfied: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn unanchored_continuum_is_unknown_not_a_conclusion() {
        let receipt = synthesize_federated_continuum(&request()).unwrap();
        assert_eq!(receipt.disposition, ContinualSynthesisDisposition::Unknown);
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
}
